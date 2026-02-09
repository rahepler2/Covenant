//! HTTP server that maps Covenant contracts to API endpoints
//!
//! `covenant serve` starts a web server where each contract becomes an endpoint:
//!   - Contract name -> route path
//!   - Contract parameters -> JSON request body / query params
//!   - Preconditions -> automatic input validation
//!   - Postconditions -> automatic output validation
//!   - Return value -> JSON response
//!
//! Routes are derived from contract names:
//!   contract list_users()          -> GET  /api/list_users
//!   contract get_user(id: Int)     -> GET  /api/get_user?id=1
//!   contract create_user(...)      -> POST /api/create_user
//!   contract update_user(...)      -> POST /api/update_user
//!   contract delete_user(id: Int)  -> POST /api/delete_user
//!
//! Convention: contracts starting with "get_" or "list_" are GET routes.
//! All others are POST routes (they likely mutate state).

use std::collections::HashMap;
use std::io::{Read, Write, BufRead, BufReader};
use std::net::TcpListener;
use std::path::{Path, PathBuf};

use crate::ast::*;
use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::runtime::{Interpreter, Value};

/// Serve configuration
pub struct ServeConfig {
    pub port: u16,
    pub host: String,
    pub static_dir: Option<PathBuf>,
    pub api_prefix: String,
}

impl Default for ServeConfig {
    fn default() -> Self {
        ServeConfig {
            port: 8080,
            host: "127.0.0.1".to_string(),
            static_dir: None,
            api_prefix: "/api".to_string(),
        }
    }
}

/// Route metadata derived from a contract
struct Route {
    contract_name: String,
    method: String,   // GET or POST
    path: String,     // /api/contract_name
    param_names: Vec<String>,
}

/// Start the HTTP server
pub fn start_server(
    files: &[PathBuf],
    config: &ServeConfig,
) -> Result<(), String> {
    // Parse all source files
    let mut all_programs: Vec<Program> = Vec::new();

    for file in files {
        let source = std::fs::read_to_string(file)
            .map_err(|e| format!("Cannot read {}: {}", file.display(), e))?;
        let filename = file.to_string_lossy().to_string();

        let tokens = Lexer::new(&source, &filename).tokenize()
            .map_err(|e| format!("Lexer error in {}: {}", filename, e))?;

        let program = Parser::new(tokens, &filename).parse()
            .map_err(|e| format!("Parse error in {}: {}", filename, e))?;

        all_programs.push(program);
    }

    // Build routes from contracts
    let mut routes: Vec<Route> = Vec::new();
    let mut interpreter = Interpreter::new();

    for program in &all_programs {
        interpreter.register_contracts(program);

        for contract in &program.contracts {
            // Skip internal contracts (main, init, setup, etc.)
            if contract.name == "main" || contract.name == "init" || contract.name == "setup" {
                continue;
            }

            let method = if contract.name.starts_with("get_")
                || contract.name.starts_with("list_")
                || contract.name.starts_with("show_")
                || contract.name.starts_with("find_")
                || contract.name.starts_with("search_")
                || contract.name.starts_with("count_")
                || (contract.params.is_empty() && contract.effects.is_none())
            {
                "GET".to_string()
            } else {
                "POST".to_string()
            };

            let path = format!("{}/{}", config.api_prefix, contract.name);
            let param_names: Vec<String> = contract.params.iter()
                .map(|p| p.name.clone())
                .collect();

            routes.push(Route {
                contract_name: contract.name.clone(),
                method,
                path,
                param_names,
            });
        }
    }

    // Run init/setup contracts if they exist
    for program in &all_programs {
        for contract in &program.contracts {
            if contract.name == "init" || contract.name == "setup" {
                let _ = interpreter.run_contract(&contract.name, HashMap::new());
            }
        }
    }

    // Print routes
    println!("Covenant API Server");
    println!("===================");
    println!("Listening on http://{}:{}", config.host, config.port);
    println!();
    println!("Routes:");
    for route in &routes {
        let params_str = if route.param_names.is_empty() {
            String::new()
        } else {
            format!("  params: {}", route.param_names.join(", "))
        };
        println!("  {} {}{}",
            route.method,
            route.path,
            params_str,
        );
    }
    if let Some(ref dir) = config.static_dir {
        println!("  Static files: {} -> /", dir.display());
    }
    println!();

    // Start TCP listener
    let addr = format!("{}:{}", config.host, config.port);
    let listener = TcpListener::bind(&addr)
        .map_err(|e| format!("Cannot bind to {}: {}", addr, e))?;

    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                let result = handle_request(
                    &mut stream,
                    &routes,
                    &mut interpreter,
                    &config,
                );
                if let Err(e) = result {
                    eprintln!("Request error: {}", e);
                }
            }
            Err(e) => {
                eprintln!("Connection error: {}", e);
            }
        }
    }

    Ok(())
}

/// Parse and handle a single HTTP request
fn handle_request(
    stream: &mut std::net::TcpStream,
    routes: &[Route],
    interpreter: &mut Interpreter,
    config: &ServeConfig,
) -> Result<(), String> {
    let mut reader = BufReader::new(stream.try_clone().map_err(|e| e.to_string())?);

    // Read request line
    let mut request_line = String::new();
    reader.read_line(&mut request_line).map_err(|e| e.to_string())?;
    let request_line = request_line.trim().to_string();

    if request_line.is_empty() {
        return Ok(());
    }

    let parts: Vec<&str> = request_line.split_whitespace().collect();
    if parts.len() < 2 {
        return send_response(stream, 400, "Bad Request");
    }

    let method = parts[0];
    let full_path = parts[1];

    // Parse path and query string
    let (path, query_string) = match full_path.find('?') {
        Some(pos) => (&full_path[..pos], Some(&full_path[pos + 1..])),
        None => (full_path, None),
    };

    // Read headers
    let mut headers: HashMap<String, String> = HashMap::new();
    let mut content_length: usize = 0;
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).map_err(|e| e.to_string())?;
        let line = line.trim().to_string();
        if line.is_empty() {
            break;
        }
        if let Some(pos) = line.find(':') {
            let key = line[..pos].trim().to_lowercase();
            let value = line[pos + 1..].trim().to_string();
            if key == "content-length" {
                content_length = value.parse().unwrap_or(0);
            }
            headers.insert(key, value);
        }
    }

    // Read body if present
    let body = if content_length > 0 {
        // Cap at 1MB
        let read_len = content_length.min(1024 * 1024);
        let mut body_buf = vec![0u8; read_len];
        reader.read_exact(&mut body_buf).map_err(|e| e.to_string())?;
        Some(String::from_utf8_lossy(&body_buf).to_string())
    } else {
        None
    };

    // Log request
    eprintln!("{} {}", method, full_path);

    // CORS preflight
    if method == "OPTIONS" {
        return send_cors_response(stream);
    }

    // Try to match a route
    for route in routes {
        if path == route.path {
            // Build contract arguments from query string and/or body
            let mut args: HashMap<String, Value> = HashMap::new();

            // Parse query string params
            if let Some(qs) = query_string {
                for pair in qs.split('&') {
                    if let Some(pos) = pair.find('=') {
                        let key = &pair[..pos];
                        let value = &pair[pos + 1..];
                        let decoded = url_decode(value);
                        args.insert(key.to_string(), parse_value_smart(&decoded));
                    }
                }
            }

            // Parse JSON body
            if let Some(ref body_str) = body {
                if !body_str.is_empty() {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(body_str) {
                        if let serde_json::Value::Object(obj) = json {
                            for (key, val) in obj {
                                args.insert(key, json_to_covenant_value(&val));
                            }
                        }
                    }
                }
            }

            // Execute the contract
            match interpreter.run_contract(&route.contract_name, args) {
                Ok(result) => {
                    let json = value_to_json(&result);
                    let response_body = format!(
                        "{{\"ok\":true,\"data\":{}}}",
                        json
                    );
                    return send_json_response(stream, 200, &response_body);
                }
                Err(e) => {
                    let err_msg = e.message.replace('\\', "\\\\").replace('"', "\\\"");
                    let response_body = format!(
                        "{{\"ok\":false,\"error\":\"{}\"}}",
                        err_msg
                    );
                    // Precondition failures -> 400, other errors -> 500
                    let status = if e.message.contains("Precondition") {
                        400
                    } else {
                        500
                    };
                    return send_json_response(stream, status, &response_body);
                }
            }
        }
    }

    // Try static files
    if let Some(ref static_dir) = config.static_dir {
        let file_path = if path == "/" {
            static_dir.join("index.html")
        } else {
            static_dir.join(path.trim_start_matches('/'))
        };

        // Prevent directory traversal
        if let Ok(canonical) = file_path.canonicalize() {
            if let Ok(static_canonical) = static_dir.canonicalize() {
                if canonical.starts_with(&static_canonical) {
                    if canonical.is_file() {
                        return serve_static_file(stream, &canonical);
                    }
                }
            }
        }
    }

    // 404
    send_json_response(stream, 404, "{\"ok\":false,\"error\":\"Not found\"}")
}

fn send_response(stream: &mut std::net::TcpStream, status: u16, body: &str) -> Result<(), String> {
    let status_text = match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        500 => "Internal Server Error",
        _ => "Unknown",
    };

    let response = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        status, status_text, body.len(), body
    );
    stream.write_all(response.as_bytes()).map_err(|e| e.to_string())?;
    stream.flush().map_err(|e| e.to_string())
}

fn send_json_response(stream: &mut std::net::TcpStream, status: u16, body: &str) -> Result<(), String> {
    let status_text = match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        500 => "Internal Server Error",
        _ => "Unknown",
    };

    let response = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, POST, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type\r\nConnection: close\r\n\r\n{}",
        status, status_text, body.len(), body
    );
    stream.write_all(response.as_bytes()).map_err(|e| e.to_string())?;
    stream.flush().map_err(|e| e.to_string())
}

fn send_cors_response(stream: &mut std::net::TcpStream) -> Result<(), String> {
    let response = "HTTP/1.1 204 No Content\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, POST, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type\r\nConnection: close\r\n\r\n";
    stream.write_all(response.as_bytes()).map_err(|e| e.to_string())?;
    stream.flush().map_err(|e| e.to_string())
}

fn serve_static_file(stream: &mut std::net::TcpStream, path: &Path) -> Result<(), String> {
    let content = std::fs::read(path).map_err(|e| e.to_string())?;

    let content_type = match path.extension().and_then(|e| e.to_str()) {
        Some("html") | Some("htm") => "text/html; charset=utf-8",
        Some("css") => "text/css",
        Some("js") => "application/javascript",
        Some("json") => "application/json",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("svg") => "image/svg+xml",
        Some("ico") => "image/x-icon",
        Some("woff2") => "font/woff2",
        Some("woff") => "font/woff",
        Some("ttf") => "font/ttf",
        _ => "application/octet-stream",
    };

    let header = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n",
        content_type, content.len()
    );

    stream.write_all(header.as_bytes()).map_err(|e| e.to_string())?;
    stream.write_all(&content).map_err(|e| e.to_string())?;
    stream.flush().map_err(|e| e.to_string())
}

fn url_decode(s: &str) -> String {
    let mut result = String::new();
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '%' {
            let hex: String = chars.by_ref().take(2).collect();
            if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                result.push(byte as char);
            }
        } else if c == '+' {
            result.push(' ');
        } else {
            result.push(c);
        }
    }
    result
}

fn parse_value_smart(s: &str) -> Value {
    if let Ok(n) = s.parse::<i64>() {
        return Value::Int(n);
    }
    if let Ok(f) = s.parse::<f64>() {
        return Value::Float(f);
    }
    match s {
        "true" => Value::Bool(true),
        "false" => Value::Bool(false),
        "null" => Value::Null,
        _ => Value::Str(s.to_string()),
    }
}

fn json_to_covenant_value(json: &serde_json::Value) -> Value {
    match json {
        serde_json::Value::Null => Value::Null,
        serde_json::Value::Bool(b) => Value::Bool(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::Int(i)
            } else if let Some(f) = n.as_f64() {
                Value::Float(f)
            } else {
                Value::Null
            }
        }
        serde_json::Value::String(s) => Value::Str(s.clone()),
        serde_json::Value::Array(arr) => {
            Value::List(arr.iter().map(json_to_covenant_value).collect())
        }
        serde_json::Value::Object(obj) => {
            let fields: HashMap<String, Value> = obj.iter()
                .map(|(k, v)| (k.clone(), json_to_covenant_value(v)))
                .collect();
            Value::Object("Object".to_string(), fields)
        }
    }
}

fn value_to_json(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Int(n) => n.to_string(),
        Value::Float(f) => {
            if f.is_finite() {
                format!("{}", f)
            } else {
                "null".to_string()
            }
        }
        Value::Str(s) => {
            // JSON-escape the string
            let escaped = s
                .replace('\\', "\\\\")
                .replace('"', "\\\"")
                .replace('\n', "\\n")
                .replace('\r', "\\r")
                .replace('\t', "\\t");
            format!("\"{}\"", escaped)
        }
        Value::List(items) => {
            let parts: Vec<String> = items.iter().map(value_to_json).collect();
            format!("[{}]", parts.join(","))
        }
        Value::Object(_, fields) => {
            let parts: Vec<String> = fields.iter()
                .map(|(k, v)| format!("\"{}\":{}", k, value_to_json(v)))
                .collect();
            format!("{{{}}}", parts.join(","))
        }
    }
}
