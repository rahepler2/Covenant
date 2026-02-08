//! HTTP client via curl subprocess

use super::super::{Value, RuntimeError};
use std::collections::HashMap;
use std::process::Command;

pub fn call(
    method: &str,
    args: Vec<Value>,
    kwargs: HashMap<String, Value>,
) -> Result<Value, RuntimeError> {
    match method {
        "get" => http_request("GET", &args, &kwargs),
        "post" => http_request("POST", &args, &kwargs),
        "put" => http_request("PUT", &args, &kwargs),
        "delete" => http_request("DELETE", &args, &kwargs),
        _ => Err(RuntimeError {
            message: format!("web.{}() not found", method),
        }),
    }
}

fn http_request(
    http_method: &str,
    args: &[Value],
    kwargs: &HashMap<String, Value>,
) -> Result<Value, RuntimeError> {
    let url = match args.first() {
        Some(Value::Str(s)) => s.clone(),
        _ => {
            return Err(RuntimeError {
                message: format!(
                    "web.{}() requires a URL string",
                    http_method.to_lowercase()
                ),
            })
        }
    };

    let mut cmd = Command::new("curl");
    cmd.arg("-s") // silent
        .arg("-S") // show errors
        .arg("-L") // follow redirects
        .arg("-w")
        .arg("\n%{http_code}") // append status code
        .arg("-X")
        .arg(http_method);

    // Headers from kwargs
    if let Some(Value::Object(_, headers)) = kwargs.get("headers") {
        for (k, v) in headers {
            cmd.arg("-H").arg(format!("{}: {}", k, v));
        }
    }

    // Body (for POST/PUT)
    if let Some(body) = kwargs.get("body").or_else(|| args.get(1)) {
        match body {
            Value::Str(s) => {
                cmd.arg("-d").arg(s);
            }
            Value::Object(_, _) => {
                cmd.arg("-H").arg("Content-Type: application/json");
                cmd.arg("-d").arg(format!("{}", body));
            }
            _ => {
                cmd.arg("-d").arg(format!("{}", body));
            }
        }
    }

    cmd.arg(&url);

    let output = cmd.output().map_err(|e| RuntimeError {
        message: format!("Failed to execute curl: {}", e),
    })?;

    let full_output = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() && !stderr.is_empty() {
        return Err(RuntimeError {
            message: format!("HTTP request failed: {}", stderr),
        });
    }

    // Parse output: body + "\n" + status_code
    let (body, status_code) = match full_output.rfind('\n') {
        Some(pos) => {
            let body = full_output[..pos].to_string();
            let code_str = full_output[pos + 1..].trim();
            let code = code_str.parse::<i64>().unwrap_or(0);
            (body, code)
        }
        None => (full_output, 0),
    };

    let mut fields = HashMap::new();
    fields.insert("body".to_string(), Value::Str(body));
    fields.insert("status".to_string(), Value::Int(status_code));
    fields.insert(
        "ok".to_string(),
        Value::Bool(status_code >= 200 && status_code < 300),
    );

    Ok(Value::Object("HttpResponse".to_string(), fields))
}

pub fn call_response_method(
    fields: &HashMap<String, Value>,
    method: &str,
    _args: Vec<Value>,
    _kwargs: HashMap<String, Value>,
) -> Result<Value, RuntimeError> {
    match method {
        "json" => {
            if let Some(Value::Str(body)) = fields.get("body") {
                super::json_mod::parse_json_string(body)
            } else {
                Err(RuntimeError {
                    message: "Response has no body".to_string(),
                })
            }
        }
        "text" => Ok(fields
            .get("body")
            .cloned()
            .unwrap_or(Value::Str(String::new()))),
        _ => Err(RuntimeError {
            message: format!("HttpResponse.{}() not found", method),
        }),
    }
}
