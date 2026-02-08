//! Anthropic Claude API client
//!
//! Methods: chat, complete, embed
//! Requires: ANTHROPIC_API_KEY env var
//! Returns response strings or structured objects

use super::super::{Value, RuntimeError};
use std::collections::HashMap;
use std::process::Command;

const DEFAULT_MODEL: &str = "claude-sonnet-4-20250514";
const API_URL: &str = "https://api.anthropic.com/v1/messages";

pub fn call(
    method: &str,
    args: Vec<Value>,
    kwargs: HashMap<String, Value>,
) -> Result<Value, RuntimeError> {
    match method {
        "chat" | "complete" | "ask" => chat(&args, &kwargs),
        "models" => Ok(Value::List(vec![
            Value::Str("claude-opus-4-20250514".to_string()),
            Value::Str("claude-sonnet-4-20250514".to_string()),
            Value::Str("claude-haiku-4-5-20251001".to_string()),
        ])),
        _ => Err(RuntimeError {
            message: format!("anthropic.{}() not found", method),
        }),
    }
}

fn chat(args: &[Value], kwargs: &HashMap<String, Value>) -> Result<Value, RuntimeError> {
    let prompt = match args.first() {
        Some(Value::Str(s)) => s.clone(),
        _ => return Err(RuntimeError {
            message: "anthropic.chat() requires a prompt string".to_string(),
        }),
    };

    let api_key = std::env::var("ANTHROPIC_API_KEY").map_err(|_| RuntimeError {
        message: "ANTHROPIC_API_KEY not set".to_string(),
    })?;

    let model = match kwargs.get("model") {
        Some(Value::Str(s)) => s.clone(),
        _ => DEFAULT_MODEL.to_string(),
    };

    let max_tokens = match kwargs.get("max_tokens") {
        Some(Value::Int(n)) => *n,
        _ => 1024,
    };

    let system = kwargs.get("system").and_then(|v| {
        if let Value::Str(s) = v { Some(s.clone()) } else { None }
    });

    let temperature = kwargs.get("temperature").and_then(|v| match v {
        Value::Float(f) => Some(*f),
        Value::Int(i) => Some(*i as f64),
        _ => None,
    });

    // Build messages array
    let mut messages = vec![serde_json::json!({"role": "user", "content": prompt})];

    // If there's a conversation history in kwargs
    if let Some(Value::List(history)) = kwargs.get("messages") {
        messages.clear();
        for msg in history {
            if let Value::Object(_, fields) = msg {
                let role = fields.get("role").map(|v| format!("{}", v)).unwrap_or_default();
                let content = fields.get("content").map(|v| format!("{}", v)).unwrap_or_default();
                messages.push(serde_json::json!({"role": role, "content": content}));
            }
        }
    }

    let mut body = serde_json::json!({
        "model": model,
        "max_tokens": max_tokens,
        "messages": messages,
    });

    if let Some(sys) = system {
        body["system"] = serde_json::Value::String(sys);
    }
    if let Some(temp) = temperature {
        body["temperature"] = serde_json::json!(temp);
    }

    let body_str = serde_json::to_string(&body).unwrap();

    let output = Command::new("curl")
        .arg("-s").arg("-S")
        .arg("-X").arg("POST")
        .arg("-H").arg("Content-Type: application/json")
        .arg("-H").arg(format!("x-api-key: {}", api_key))
        .arg("-H").arg("anthropic-version: 2023-06-01")
        .arg("-d").arg(&body_str)
        .arg(API_URL)
        .output()
        .map_err(|e| RuntimeError {
            message: format!("Failed to call Anthropic API: {}", e),
        })?;

    let response_text = String::from_utf8_lossy(&output.stdout).to_string();
    let json_val: serde_json::Value = serde_json::from_str(&response_text)
        .map_err(|e| RuntimeError {
            message: format!("Invalid API response: {}", e),
        })?;

    // Check for API errors
    if let Some(err) = json_val.get("error") {
        return Err(RuntimeError {
            message: format!("Anthropic API error: {}", err),
        });
    }

    // Extract text content
    let text = json_val["content"][0]["text"]
        .as_str()
        .unwrap_or("")
        .to_string();

    // Build rich response object
    let mut fields = HashMap::new();
    fields.insert("text".to_string(), Value::Str(text.clone()));
    fields.insert("model".to_string(), Value::Str(
        json_val["model"].as_str().unwrap_or(&model).to_string()
    ));
    fields.insert("stop_reason".to_string(), Value::Str(
        json_val["stop_reason"].as_str().unwrap_or("").to_string()
    ));

    // Usage info
    if let Some(usage) = json_val.get("usage") {
        let mut usage_fields = HashMap::new();
        usage_fields.insert("input_tokens".to_string(),
            Value::Int(usage["input_tokens"].as_i64().unwrap_or(0)));
        usage_fields.insert("output_tokens".to_string(),
            Value::Int(usage["output_tokens"].as_i64().unwrap_or(0)));
        fields.insert("usage".to_string(), Value::Object("Usage".to_string(), usage_fields));
    }

    // For simple use: return just the text as a string
    // Users who need structured data can use anthropic.chat() and access .text, .usage, etc.
    Ok(Value::Str(text))
}
