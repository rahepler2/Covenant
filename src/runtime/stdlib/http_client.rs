//! HTTP client — requests-style library
//!
//! Methods: get, post, put, patch, delete, head
//! Features: headers, auth, timeout, JSON body auto-serialization
//! Returns HttpResponse objects with .json(), .text() methods

use super::super::{Value, RuntimeError};
use std::collections::HashMap;
use std::process::Command;

pub fn call(
    method: &str,
    args: Vec<Value>,
    kwargs: HashMap<String, Value>,
) -> Result<Value, RuntimeError> {
    match method {
        "get" => request("GET", &args, &kwargs),
        "post" => request("POST", &args, &kwargs),
        "put" => request("PUT", &args, &kwargs),
        "patch" => request("PATCH", &args, &kwargs),
        "delete" => request("DELETE", &args, &kwargs),
        "head" => request("HEAD", &args, &kwargs),
        _ => Err(RuntimeError {
            message: format!("http.{}() not found", method),
        }),
    }
}

fn request(
    http_method: &str,
    args: &[Value],
    kwargs: &HashMap<String, Value>,
) -> Result<Value, RuntimeError> {
    let url = match args.first() {
        Some(Value::Str(s)) => s.clone(),
        _ => return Err(RuntimeError {
            message: format!("http.{}() requires a URL string", http_method.to_lowercase()),
        }),
    };

    let mut cmd = Command::new("curl");
    cmd.arg("-s").arg("-S").arg("-L")
        .arg("-w").arg("\n%{http_code}")
        .arg("-X").arg(http_method);

    // Timeout
    if let Some(Value::Int(t)) = kwargs.get("timeout") {
        cmd.arg("--max-time").arg(t.to_string());
    } else if let Some(Value::Float(t)) = kwargs.get("timeout") {
        cmd.arg("--max-time").arg((*t as i64).to_string());
    }

    // Headers from kwargs
    if let Some(Value::Object(_, headers)) = kwargs.get("headers") {
        for (k, v) in headers {
            cmd.arg("-H").arg(format!("{}: {}", k, v));
        }
    }

    // Auth: bearer token
    if let Some(Value::Str(token)) = kwargs.get("auth") {
        cmd.arg("-H").arg(format!("Authorization: Bearer {}", token));
    }

    // Body: JSON auto-serialization for objects, raw string otherwise
    let body = kwargs.get("body").or_else(|| kwargs.get("json")).or_else(|| args.get(1));
    if let Some(b) = body {
        match b {
            Value::Str(s) => {
                cmd.arg("-d").arg(s);
                // Set content-type if not already set
                if kwargs.get("headers").is_none() {
                    cmd.arg("-H").arg("Content-Type: application/json");
                }
            }
            Value::Object(_, _fields) => {
                let json_val = value_to_json(b);
                let json_str = serde_json::to_string(&json_val).unwrap_or_default();
                cmd.arg("-H").arg("Content-Type: application/json");
                cmd.arg("-d").arg(json_str);
            }
            _ => {
                cmd.arg("-d").arg(format!("{}", b));
            }
        }
    }

    cmd.arg(&url);

    let output = cmd.output().map_err(|e| RuntimeError {
        message: format!("HTTP request failed: {}", e),
    })?;

    let full_output = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() && !stderr.is_empty() {
        return Err(RuntimeError {
            message: format!("HTTP request failed: {}", stderr),
        });
    }

    let (body_str, status_code) = match full_output.rfind('\n') {
        Some(pos) => {
            let body_str = full_output[..pos].to_string();
            let code = full_output[pos + 1..].trim().parse::<i64>().unwrap_or(0);
            (body_str, code)
        }
        None => (full_output, 0),
    };

    let mut fields = HashMap::new();
    fields.insert("body".to_string(), Value::Str(body_str));
    fields.insert("status".to_string(), Value::Int(status_code));
    fields.insert("ok".to_string(), Value::Bool(status_code >= 200 && status_code < 300));

    Ok(Value::Object("HttpResponse".to_string(), fields))
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
