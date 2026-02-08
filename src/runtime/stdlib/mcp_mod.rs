//! Model Context Protocol (MCP) client
//!
//! Methods: connect, call_tool, list_tools, get_resource, list_resources
//! Connects to MCP servers via stdio or HTTP
//! Covenant contracts map naturally to MCP tools

use super::super::{Value, RuntimeError};
use std::collections::HashMap;
use std::process::{Command, Stdio};
use std::io::Write;

pub fn call(
    method: &str,
    args: Vec<Value>,
    kwargs: HashMap<String, Value>,
) -> Result<Value, RuntimeError> {
    match method {
        "connect" => connect(&args, &kwargs),
        "call_tool" | "tool" => call_tool(&args, &kwargs),
        "list_tools" | "tools" => list_tools(&args, &kwargs),
        "get_resource" | "resource" => get_resource(&args, &kwargs),
        "list_resources" | "resources" => list_resources(&args, &kwargs),
        "prompt" => call_prompt(&args, &kwargs),
        _ => Err(RuntimeError {
            message: format!("mcp.{}() not found", method),
        }),
    }
}

/// Connect to an MCP server — returns a connection object
fn connect(args: &[Value], kwargs: &HashMap<String, Value>) -> Result<Value, RuntimeError> {
    let server = match args.first() {
        Some(Value::Str(s)) => s.clone(),
        _ => return Err(RuntimeError {
            message: "mcp.connect() requires a server command or URL".to_string(),
        }),
    };

    let transport = match kwargs.get("transport") {
        Some(Value::Str(s)) => s.clone(),
        _ => {
            if server.starts_with("http") {
                "http".to_string()
            } else {
                "stdio".to_string()
            }
        }
    };

    let mut fields = HashMap::new();
    fields.insert("server".to_string(), Value::Str(server));
    fields.insert("transport".to_string(), Value::Str(transport));
    fields.insert("connected".to_string(), Value::Bool(true));

    Ok(Value::Object("McpConnection".to_string(), fields))
}

/// Call a tool on an MCP server via JSON-RPC
fn call_tool(args: &[Value], kwargs: &HashMap<String, Value>) -> Result<Value, RuntimeError> {
    // args[0] = server/command or connection, args[1] = tool name
    let (server, tool_name) = match (args.first(), args.get(1)) {
        (Some(Value::Str(s)), Some(Value::Str(t))) => (s.clone(), t.clone()),
        (Some(Value::Object(_, fields)), Some(Value::Str(t))) => {
            let s = fields.get("server").map(|v| format!("{}", v)).unwrap_or_default();
            (s, t.clone())
        }
        _ => return Err(RuntimeError {
            message: "mcp.call_tool() requires (server, tool_name)".to_string(),
        }),
    };

    // Build tool arguments from kwargs
    let mut tool_args = serde_json::Map::new();
    for (k, v) in kwargs {
        if k != "transport" {
            tool_args.insert(k.clone(), value_to_json(v));
        }
    }

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": tool_name,
            "arguments": tool_args
        }
    });

    let result = send_jsonrpc(&server, &request)?;

    // Extract content from MCP response
    if let Some(content) = result.get("content") {
        if let Some(arr) = content.as_array() {
            if let Some(first) = arr.first() {
                if let Some(text) = first.get("text").and_then(|t| t.as_str()) {
                    return Ok(Value::Str(text.to_string()));
                }
            }
        }
    }

    Ok(json_to_value(&result))
}

/// List available tools from an MCP server
fn list_tools(args: &[Value], _kwargs: &HashMap<String, Value>) -> Result<Value, RuntimeError> {
    let server = match args.first() {
        Some(Value::Str(s)) => s.clone(),
        Some(Value::Object(_, fields)) => {
            fields.get("server").map(|v| format!("{}", v)).unwrap_or_default()
        }
        _ => return Err(RuntimeError {
            message: "mcp.list_tools() requires a server".to_string(),
        }),
    };

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/list",
        "params": {}
    });

    let result = send_jsonrpc(&server, &request)?;

    let mut tools = Vec::new();
    if let Some(tool_list) = result.get("tools").and_then(|t| t.as_array()) {
        for tool in tool_list {
            let mut fields = HashMap::new();
            if let Some(name) = tool.get("name").and_then(|n| n.as_str()) {
                fields.insert("name".to_string(), Value::Str(name.to_string()));
            }
            if let Some(desc) = tool.get("description").and_then(|d| d.as_str()) {
                fields.insert("description".to_string(), Value::Str(desc.to_string()));
            }
            tools.push(Value::Object("McpTool".to_string(), fields));
        }
    }

    Ok(Value::List(tools))
}

/// Get a resource from an MCP server
fn get_resource(args: &[Value], _kwargs: &HashMap<String, Value>) -> Result<Value, RuntimeError> {
    let (server, uri) = match (args.first(), args.get(1)) {
        (Some(Value::Str(s)), Some(Value::Str(u))) => (s.clone(), u.clone()),
        (Some(Value::Object(_, fields)), Some(Value::Str(u))) => {
            let s = fields.get("server").map(|v| format!("{}", v)).unwrap_or_default();
            (s, u.clone())
        }
        _ => return Err(RuntimeError {
            message: "mcp.get_resource() requires (server, uri)".to_string(),
        }),
    };

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "resources/read",
        "params": {"uri": uri}
    });

    let result = send_jsonrpc(&server, &request)?;
    Ok(json_to_value(&result))
}

/// List resources from an MCP server
fn list_resources(args: &[Value], _kwargs: &HashMap<String, Value>) -> Result<Value, RuntimeError> {
    let server = match args.first() {
        Some(Value::Str(s)) => s.clone(),
        Some(Value::Object(_, fields)) => {
            fields.get("server").map(|v| format!("{}", v)).unwrap_or_default()
        }
        _ => return Err(RuntimeError {
            message: "mcp.list_resources() requires a server".to_string(),
        }),
    };

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "resources/list",
        "params": {}
    });

    let result = send_jsonrpc(&server, &request)?;
    Ok(json_to_value(&result))
}

/// Call a prompt template from an MCP server
fn call_prompt(args: &[Value], kwargs: &HashMap<String, Value>) -> Result<Value, RuntimeError> {
    let (server, prompt_name) = match (args.first(), args.get(1)) {
        (Some(Value::Str(s)), Some(Value::Str(p))) => (s.clone(), p.clone()),
        (Some(Value::Object(_, fields)), Some(Value::Str(p))) => {
            let s = fields.get("server").map(|v| format!("{}", v)).unwrap_or_default();
            (s, p.clone())
        }
        _ => return Err(RuntimeError {
            message: "mcp.prompt() requires (server, prompt_name)".to_string(),
        }),
    };

    let mut prompt_args = serde_json::Map::new();
    for (k, v) in kwargs {
        prompt_args.insert(k.clone(), value_to_json(v));
    }

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "prompts/get",
        "params": {
            "name": prompt_name,
            "arguments": prompt_args
        }
    });

    let result = send_jsonrpc(&server, &request)?;
    Ok(json_to_value(&result))
}

// ── Internal helpers ────────────────────────────────────────────────

fn send_jsonrpc(server: &str, request: &serde_json::Value) -> Result<serde_json::Value, RuntimeError> {
    let request_str = serde_json::to_string(request).unwrap();

    if server.starts_with("http") {
        // HTTP transport
        let output = Command::new("curl")
            .arg("-s").arg("-S")
            .arg("-X").arg("POST")
            .arg("-H").arg("Content-Type: application/json")
            .arg("-d").arg(&request_str)
            .arg(server)
            .output()
            .map_err(|e| RuntimeError {
                message: format!("MCP HTTP request failed: {}", e),
            })?;

        let response_text = String::from_utf8_lossy(&output.stdout).to_string();
        let json_val: serde_json::Value = serde_json::from_str(&response_text)
            .map_err(|e| RuntimeError {
                message: format!("Invalid MCP response: {}", e),
            })?;

        if let Some(err) = json_val.get("error") {
            return Err(RuntimeError {
                message: format!("MCP error: {}", err),
            });
        }

        Ok(json_val.get("result").cloned().unwrap_or(serde_json::Value::Null))
    } else {
        // Stdio transport — spawn the server command
        let parts: Vec<&str> = server.split_whitespace().collect();
        if parts.is_empty() {
            return Err(RuntimeError {
                message: "Empty server command".to_string(),
            });
        }

        // Send initialize first, then our request
        let init_request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 0,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {"name": "covenant", "version": "0.1.0"}
            }
        });

        let input = format!("{}\n{}\n", init_request, request_str);

        let mut child = Command::new(parts[0])
            .args(&parts[1..])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| RuntimeError {
                message: format!("Failed to start MCP server '{}': {}", server, e),
            })?;

        if let Some(ref mut stdin) = child.stdin {
            let _ = stdin.write_all(input.as_bytes());
        }

        let output = child.wait_with_output().map_err(|e| RuntimeError {
            message: format!("MCP server error: {}", e),
        })?;

        let response_text = String::from_utf8_lossy(&output.stdout).to_string();

        // Find the last JSON-RPC response (skip the init response)
        let mut last_result = serde_json::Value::Null;
        for line in response_text.lines() {
            if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(line) {
                if let Some(result) = json_val.get("result") {
                    last_result = result.clone();
                }
                if let Some(err) = json_val.get("error") {
                    return Err(RuntimeError {
                        message: format!("MCP error: {}", err),
                    });
                }
            }
        }

        Ok(last_result)
    }
}

fn value_to_json(v: &Value) -> serde_json::Value {
    match v {
        Value::Null => serde_json::Value::Null,
        Value::Bool(b) => serde_json::Value::Bool(*b),
        Value::Int(n) => serde_json::Value::Number(serde_json::Number::from(*n)),
        Value::Float(n) => serde_json::Number::from_f64(*n)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        Value::Str(s) => serde_json::Value::String(s.clone()),
        Value::List(items) => serde_json::Value::Array(items.iter().map(value_to_json).collect()),
        Value::Object(_, fields) => {
            let obj: serde_json::Map<String, serde_json::Value> = fields
                .iter()
                .filter(|(k, _)| !k.starts_with('_'))
                .map(|(k, v)| (k.clone(), value_to_json(v)))
                .collect();
            serde_json::Value::Object(obj)
        }
    }
}

fn json_to_value(v: &serde_json::Value) -> Value {
    match v {
        serde_json::Value::Null => Value::Null,
        serde_json::Value::Bool(b) => Value::Bool(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() { Value::Int(i) }
            else if let Some(f) = n.as_f64() { Value::Float(f) }
            else { Value::Null }
        }
        serde_json::Value::String(s) => Value::Str(s.clone()),
        serde_json::Value::Array(arr) => Value::List(arr.iter().map(json_to_value).collect()),
        serde_json::Value::Object(obj) => {
            let fields: HashMap<String, Value> = obj.iter()
                .map(|(k, v)| (k.clone(), json_to_value(v)))
                .collect();
            Value::Object("JsonObject".to_string(), fields)
        }
    }
}
