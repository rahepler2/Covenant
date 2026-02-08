//! OpenAI API client
//!
//! Methods: chat, complete, embed, image
//! Requires: OPENAI_API_KEY env var
//! Supports any OpenAI-compatible endpoint via base_url kwarg

use super::super::{Value, RuntimeError};
use std::collections::HashMap;
use std::process::Command;

const DEFAULT_MODEL: &str = "gpt-4o";
const DEFAULT_EMBED_MODEL: &str = "text-embedding-3-small";
const API_URL: &str = "https://api.openai.com/v1";

pub fn call(
    method: &str,
    args: Vec<Value>,
    kwargs: HashMap<String, Value>,
) -> Result<Value, RuntimeError> {
    match method {
        "chat" | "complete" | "ask" => chat(&args, &kwargs),
        "embed" | "embedding" => embed(&args, &kwargs),
        "image" => image(&args, &kwargs),
        "models" => Ok(Value::List(vec![
            Value::Str("gpt-4o".to_string()),
            Value::Str("gpt-4o-mini".to_string()),
            Value::Str("gpt-4-turbo".to_string()),
            Value::Str("o1".to_string()),
            Value::Str("o1-mini".to_string()),
        ])),
        _ => Err(RuntimeError {
            message: format!("openai.{}() not found", method),
        }),
    }
}

fn get_api_key() -> Result<String, RuntimeError> {
    std::env::var("OPENAI_API_KEY").map_err(|_| RuntimeError {
        message: "OPENAI_API_KEY not set".to_string(),
    })
}

fn get_base_url(kwargs: &HashMap<String, Value>) -> String {
    match kwargs.get("base_url") {
        Some(Value::Str(s)) => s.trim_end_matches('/').to_string(),
        _ => API_URL.to_string(),
    }
}

fn chat(args: &[Value], kwargs: &HashMap<String, Value>) -> Result<Value, RuntimeError> {
    let prompt = match args.first() {
        Some(Value::Str(s)) => s.clone(),
        _ => return Err(RuntimeError {
            message: "openai.chat() requires a prompt string".to_string(),
        }),
    };

    let api_key = get_api_key()?;
    let base_url = get_base_url(kwargs);

    let model = match kwargs.get("model") {
        Some(Value::Str(s)) => s.clone(),
        _ => DEFAULT_MODEL.to_string(),
    };

    let max_tokens = match kwargs.get("max_tokens") {
        Some(Value::Int(n)) => Some(*n),
        _ => None,
    };

    let temperature = kwargs.get("temperature").and_then(|v| match v {
        Value::Float(f) => Some(*f),
        Value::Int(i) => Some(*i as f64),
        _ => None,
    });

    let system = kwargs.get("system").and_then(|v| {
        if let Value::Str(s) = v { Some(s.clone()) } else { None }
    });

    let mut messages = Vec::new();

    // System message
    if let Some(sys) = system {
        messages.push(serde_json::json!({"role": "system", "content": sys}));
    }

    // History or single prompt
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

    if let Some(mt) = max_tokens {
        body["max_tokens"] = serde_json::json!(mt);
    }
    if let Some(temp) = temperature {
        body["temperature"] = serde_json::json!(temp);
    }

    let body_str = serde_json::to_string(&body).unwrap();
    let url = format!("{}/chat/completions", base_url);

    let output = Command::new("curl")
        .arg("-s").arg("-S")
        .arg("-X").arg("POST")
        .arg("-H").arg("Content-Type: application/json")
        .arg("-H").arg(format!("Authorization: Bearer {}", api_key))
        .arg("-d").arg(&body_str)
        .arg(&url)
        .output()
        .map_err(|e| RuntimeError {
            message: format!("Failed to call OpenAI API: {}", e),
        })?;

    let response_text = String::from_utf8_lossy(&output.stdout).to_string();
    let json_val: serde_json::Value = serde_json::from_str(&response_text)
        .map_err(|e| RuntimeError {
            message: format!("Invalid API response: {}", e),
        })?;

    if let Some(err) = json_val.get("error") {
        return Err(RuntimeError {
            message: format!("OpenAI API error: {}", err),
        });
    }

    let text = json_val["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("")
        .to_string();

    Ok(Value::Str(text))
}

fn embed(args: &[Value], kwargs: &HashMap<String, Value>) -> Result<Value, RuntimeError> {
    let input = match args.first() {
        Some(Value::Str(s)) => serde_json::json!(s.clone()),
        Some(Value::List(items)) => {
            let strs: Vec<String> = items.iter().map(|v| format!("{}", v)).collect();
            serde_json::json!(strs)
        }
        _ => return Err(RuntimeError {
            message: "openai.embed() requires a string or list of strings".to_string(),
        }),
    };

    let api_key = get_api_key()?;
    let base_url = get_base_url(kwargs);

    let model = match kwargs.get("model") {
        Some(Value::Str(s)) => s.clone(),
        _ => DEFAULT_EMBED_MODEL.to_string(),
    };

    let body = serde_json::json!({
        "model": model,
        "input": input,
    });

    let body_str = serde_json::to_string(&body).unwrap();
    let url = format!("{}/embeddings", base_url);

    let output = Command::new("curl")
        .arg("-s").arg("-S")
        .arg("-X").arg("POST")
        .arg("-H").arg("Content-Type: application/json")
        .arg("-H").arg(format!("Authorization: Bearer {}", api_key))
        .arg("-d").arg(&body_str)
        .arg(&url)
        .output()
        .map_err(|e| RuntimeError {
            message: format!("Failed to call embeddings API: {}", e),
        })?;

    let response_text = String::from_utf8_lossy(&output.stdout).to_string();
    let json_val: serde_json::Value = serde_json::from_str(&response_text)
        .map_err(|e| RuntimeError {
            message: format!("Invalid API response: {}", e),
        })?;

    if let Some(err) = json_val.get("error") {
        return Err(RuntimeError {
            message: format!("OpenAI API error: {}", err),
        });
    }

    // Return list of float vectors
    let mut results = Vec::new();
    if let Some(data) = json_val["data"].as_array() {
        for item in data {
            if let Some(embedding) = item["embedding"].as_array() {
                let vec: Vec<Value> = embedding.iter()
                    .filter_map(|v| v.as_f64().map(Value::Float))
                    .collect();
                results.push(Value::List(vec));
            }
        }
    }

    if results.len() == 1 {
        Ok(results.into_iter().next().unwrap())
    } else {
        Ok(Value::List(results))
    }
}

fn image(args: &[Value], kwargs: &HashMap<String, Value>) -> Result<Value, RuntimeError> {
    let prompt = match args.first() {
        Some(Value::Str(s)) => s.clone(),
        _ => return Err(RuntimeError {
            message: "openai.image() requires a prompt string".to_string(),
        }),
    };

    let api_key = get_api_key()?;
    let base_url = get_base_url(kwargs);

    let size = match kwargs.get("size") {
        Some(Value::Str(s)) => s.clone(),
        _ => "1024x1024".to_string(),
    };

    let body = serde_json::json!({
        "model": "dall-e-3",
        "prompt": prompt,
        "n": 1,
        "size": size,
    });

    let body_str = serde_json::to_string(&body).unwrap();
    let url = format!("{}/images/generations", base_url);

    let output = Command::new("curl")
        .arg("-s").arg("-S")
        .arg("-X").arg("POST")
        .arg("-H").arg("Content-Type: application/json")
        .arg("-H").arg(format!("Authorization: Bearer {}", api_key))
        .arg("-d").arg(&body_str)
        .arg(&url)
        .output()
        .map_err(|e| RuntimeError {
            message: format!("Failed to call images API: {}", e),
        })?;

    let response_text = String::from_utf8_lossy(&output.stdout).to_string();
    let json_val: serde_json::Value = serde_json::from_str(&response_text)
        .map_err(|e| RuntimeError {
            message: format!("Invalid API response: {}", e),
        })?;

    let image_url = json_val["data"][0]["url"]
        .as_str()
        .unwrap_or("")
        .to_string();

    Ok(Value::Str(image_url))
}
