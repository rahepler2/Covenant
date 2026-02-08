//! xAI Grok API client
//!
//! Methods: chat, complete
//! Requires: XAI_API_KEY env var
//! Uses OpenAI-compatible API format

use super::super::{Value, RuntimeError};
use std::collections::HashMap;
use std::process::Command;

const DEFAULT_MODEL: &str = "grok-2-latest";
const API_URL: &str = "https://api.x.ai/v1";

pub fn call(
    method: &str,
    args: Vec<Value>,
    kwargs: HashMap<String, Value>,
) -> Result<Value, RuntimeError> {
    match method {
        "chat" | "complete" | "ask" => chat(&args, &kwargs),
        "models" => Ok(Value::List(vec![
            Value::Str("grok-2-latest".to_string()),
            Value::Str("grok-2-mini".to_string()),
        ])),
        _ => Err(RuntimeError {
            message: format!("grok.{}() not found", method),
        }),
    }
}

fn chat(args: &[Value], kwargs: &HashMap<String, Value>) -> Result<Value, RuntimeError> {
    let prompt = match args.first() {
        Some(Value::Str(s)) => s.clone(),
        _ => return Err(RuntimeError {
            message: "grok.chat() requires a prompt string".to_string(),
        }),
    };

    let api_key = std::env::var("XAI_API_KEY").map_err(|_| RuntimeError {
        message: "XAI_API_KEY not set".to_string(),
    })?;

    let model = match kwargs.get("model") {
        Some(Value::Str(s)) => s.clone(),
        _ => DEFAULT_MODEL.to_string(),
    };

    let system = kwargs.get("system").and_then(|v| {
        if let Value::Str(s) = v { Some(s.clone()) } else { None }
    });

    let mut messages = Vec::new();

    if let Some(sys) = system {
        messages.push(serde_json::json!({"role": "system", "content": sys}));
    }

    if let Some(Value::List(history)) = kwargs.get("messages") {
        for msg in history {
            if let Value::Object(_, fields) = msg {
                let role = fields.get("role").map(|v| format!("{}", v)).unwrap_or_default();
                let content = fields.get("content").map(|v| format!("{}", v)).unwrap_or_default();
                messages.push(serde_json::json!({"role": role, "content": content}));
            }
        }
    } else {
        messages.push(serde_json::json!({"role": "user", "content": prompt}));
    }

    let mut body = serde_json::json!({
        "model": model,
        "messages": messages,
    });

    if let Some(Value::Int(n)) = kwargs.get("max_tokens") {
        body["max_tokens"] = serde_json::json!(n);
    }
    if let Some(Value::Float(t)) = kwargs.get("temperature") {
        body["temperature"] = serde_json::json!(t);
    }

    let body_str = serde_json::to_string(&body).unwrap();
    let url = format!("{}/chat/completions", API_URL);

    let output = Command::new("curl")
        .arg("-s").arg("-S")
        .arg("-X").arg("POST")
        .arg("-H").arg("Content-Type: application/json")
        .arg("-H").arg(format!("Authorization: Bearer {}", api_key))
        .arg("-d").arg(&body_str)
        .arg(&url)
        .output()
        .map_err(|e| RuntimeError {
            message: format!("Failed to call Grok API: {}", e),
        })?;

    let response_text = String::from_utf8_lossy(&output.stdout).to_string();
    let json_val: serde_json::Value = serde_json::from_str(&response_text)
        .map_err(|e| RuntimeError {
            message: format!("Invalid API response: {}", e),
        })?;

    if let Some(err) = json_val.get("error") {
        return Err(RuntimeError {
            message: format!("Grok API error: {}", err),
        });
    }

    let text = json_val["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("")
        .to_string();

    Ok(Value::Str(text))
}
