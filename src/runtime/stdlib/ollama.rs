//! Ollama local LLM client
//!
//! Methods: chat, generate, embed, list, pull
//! Connects to localhost:11434 by default

use super::super::{Value, RuntimeError};
use std::collections::HashMap;
use std::process::Command;

const DEFAULT_URL: &str = "http://localhost:11434";
const DEFAULT_MODEL: &str = "llama3.2";

pub fn call(
    method: &str,
    args: Vec<Value>,
    kwargs: HashMap<String, Value>,
) -> Result<Value, RuntimeError> {
    match method {
        "chat" | "ask" => chat(&args, &kwargs),
        "generate" | "complete" => generate(&args, &kwargs),
        "embed" | "embedding" => embed(&args, &kwargs),
        "list" | "models" => list_models(&kwargs),
        "pull" => pull_model(&args, &kwargs),
        _ => Err(RuntimeError {
            message: format!("ollama.{}() not found", method),
        }),
    }
}

fn get_base_url(kwargs: &HashMap<String, Value>) -> String {
    match kwargs.get("url").or_else(|| kwargs.get("base_url")) {
        Some(Value::Str(s)) => s.trim_end_matches('/').to_string(),
        _ => DEFAULT_URL.to_string(),
    }
}

fn get_model(kwargs: &HashMap<String, Value>) -> String {
    match kwargs.get("model") {
        Some(Value::Str(s)) => s.clone(),
        _ => DEFAULT_MODEL.to_string(),
    }
}

fn chat(args: &[Value], kwargs: &HashMap<String, Value>) -> Result<Value, RuntimeError> {
    let prompt = match args.first() {
        Some(Value::Str(s)) => s.clone(),
        _ => return Err(RuntimeError {
            message: "ollama.chat() requires a prompt string".to_string(),
        }),
    };

    let base_url = get_base_url(kwargs);
    let model = get_model(kwargs);

    let system = kwargs.get("system").and_then(|v| {
        if let Value::Str(s) = v { Some(s.clone()) } else { None }
    });

    let mut messages = Vec::new();

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

    let body = serde_json::json!({
        "model": model,
        "messages": messages,
        "stream": false,
    });

    let body_str = serde_json::to_string(&body).unwrap();
    let url = format!("{}/api/chat", base_url);

    let output = Command::new("curl")
        .arg("-s").arg("-S")
        .arg("-X").arg("POST")
        .arg("-H").arg("Content-Type: application/json")
        .arg("-d").arg(&body_str)
        .arg(&url)
        .output()
        .map_err(|e| RuntimeError {
            message: format!("Failed to call Ollama: {}", e),
        })?;

    let response_text = String::from_utf8_lossy(&output.stdout).to_string();
    let json_val: serde_json::Value = serde_json::from_str(&response_text)
        .map_err(|e| RuntimeError {
            message: format!("Invalid Ollama response: {}. Raw: {}", e, &response_text[..response_text.len().min(200)]),
        })?;

    if let Some(err) = json_val.get("error") {
        return Err(RuntimeError {
            message: format!("Ollama error: {}", err),
        });
    }

    let text = json_val["message"]["content"]
        .as_str()
        .unwrap_or("")
        .to_string();

    Ok(Value::Str(text))
}

fn generate(args: &[Value], kwargs: &HashMap<String, Value>) -> Result<Value, RuntimeError> {
    let prompt = match args.first() {
        Some(Value::Str(s)) => s.clone(),
        _ => return Err(RuntimeError {
            message: "ollama.generate() requires a prompt string".to_string(),
        }),
    };

    let base_url = get_base_url(kwargs);
    let model = get_model(kwargs);

    let body = serde_json::json!({
        "model": model,
        "prompt": prompt,
        "stream": false,
    });

    let body_str = serde_json::to_string(&body).unwrap();
    let url = format!("{}/api/generate", base_url);

    let output = Command::new("curl")
        .arg("-s").arg("-S")
        .arg("-X").arg("POST")
        .arg("-H").arg("Content-Type: application/json")
        .arg("-d").arg(&body_str)
        .arg(&url)
        .output()
        .map_err(|e| RuntimeError {
            message: format!("Failed to call Ollama: {}", e),
        })?;

    let response_text = String::from_utf8_lossy(&output.stdout).to_string();
    let json_val: serde_json::Value = serde_json::from_str(&response_text)
        .map_err(|e| RuntimeError {
            message: format!("Invalid Ollama response: {}", e),
        })?;

    let text = json_val["response"]
        .as_str()
        .unwrap_or("")
        .to_string();

    Ok(Value::Str(text))
}

fn embed(args: &[Value], kwargs: &HashMap<String, Value>) -> Result<Value, RuntimeError> {
    let input = match args.first() {
        Some(Value::Str(s)) => s.clone(),
        _ => return Err(RuntimeError {
            message: "ollama.embed() requires a string".to_string(),
        }),
    };

    let base_url = get_base_url(kwargs);
    let model = match kwargs.get("model") {
        Some(Value::Str(s)) => s.clone(),
        _ => "nomic-embed-text".to_string(),
    };

    let body = serde_json::json!({
        "model": model,
        "input": input,
    });

    let body_str = serde_json::to_string(&body).unwrap();
    let url = format!("{}/api/embed", base_url);

    let output = Command::new("curl")
        .arg("-s").arg("-S")
        .arg("-X").arg("POST")
        .arg("-H").arg("Content-Type: application/json")
        .arg("-d").arg(&body_str)
        .arg(&url)
        .output()
        .map_err(|e| RuntimeError {
            message: format!("Failed to call Ollama: {}", e),
        })?;

    let response_text = String::from_utf8_lossy(&output.stdout).to_string();
    let json_val: serde_json::Value = serde_json::from_str(&response_text)
        .map_err(|e| RuntimeError {
            message: format!("Invalid Ollama response: {}", e),
        })?;

    if let Some(embeddings) = json_val["embeddings"].as_array() {
        if let Some(first) = embeddings.first() {
            if let Some(vec) = first.as_array() {
                let values: Vec<Value> = vec.iter()
                    .filter_map(|v| v.as_f64().map(Value::Float))
                    .collect();
                return Ok(Value::List(values));
            }
        }
    }

    Ok(Value::List(Vec::new()))
}

fn list_models(kwargs: &HashMap<String, Value>) -> Result<Value, RuntimeError> {
    let base_url = get_base_url(kwargs);
    let url = format!("{}/api/tags", base_url);

    let output = Command::new("curl")
        .arg("-s").arg("-S")
        .arg(&url)
        .output()
        .map_err(|e| RuntimeError {
            message: format!("Failed to list Ollama models: {}", e),
        })?;

    let response_text = String::from_utf8_lossy(&output.stdout).to_string();
    let json_val: serde_json::Value = serde_json::from_str(&response_text)
        .map_err(|e| RuntimeError {
            message: format!("Invalid Ollama response: {}", e),
        })?;

    let mut models = Vec::new();
    if let Some(model_list) = json_val["models"].as_array() {
        for m in model_list {
            if let Some(name) = m["name"].as_str() {
                models.push(Value::Str(name.to_string()));
            }
        }
    }

    Ok(Value::List(models))
}

fn pull_model(args: &[Value], kwargs: &HashMap<String, Value>) -> Result<Value, RuntimeError> {
    let model = match args.first() {
        Some(Value::Str(s)) => s.clone(),
        _ => return Err(RuntimeError {
            message: "ollama.pull() requires a model name".to_string(),
        }),
    };

    let base_url = get_base_url(kwargs);
    let body = serde_json::json!({"name": model, "stream": false});
    let body_str = serde_json::to_string(&body).unwrap();
    let url = format!("{}/api/pull", base_url);

    let output = Command::new("curl")
        .arg("-s").arg("-S")
        .arg("-X").arg("POST")
        .arg("-H").arg("Content-Type: application/json")
        .arg("-d").arg(&body_str)
        .arg(&url)
        .output()
        .map_err(|e| RuntimeError {
            message: format!("Failed to pull model: {}", e),
        })?;

    let _response_text = String::from_utf8_lossy(&output.stdout).to_string();
    Ok(Value::Str(format!("Pulled model: {}", model)))
}
