//! AI/LLM integration — calls Anthropic or OpenAI APIs via curl

use super::super::{Value, RuntimeError};
use std::collections::HashMap;
use std::process::Command;

pub fn call(
    method: &str,
    args: Vec<Value>,
    kwargs: HashMap<String, Value>,
) -> Result<Value, RuntimeError> {
    match method {
        "prompt" | "ask" | "complete" => ai_prompt(&args, &kwargs),
        "classify" => ai_classify(&args, &kwargs),
        "extract" => ai_extract(&args, &kwargs),
        "summarize" => ai_summarize(&args, &kwargs),
        _ => Err(RuntimeError {
            message: format!("ai.{}() not found", method),
        }),
    }
}

fn ai_prompt(args: &[Value], kwargs: &HashMap<String, Value>) -> Result<Value, RuntimeError> {
    let prompt = match args.first() {
        Some(Value::Str(s)) => s.clone(),
        _ => {
            return Err(RuntimeError {
                message: "ai.prompt() requires a prompt string".to_string(),
            })
        }
    };

    let model = match kwargs.get("model") {
        Some(Value::Str(s)) => s.clone(),
        _ => "claude-sonnet-4-20250514".to_string(),
    };

    let api_key = std::env::var("ANTHROPIC_API_KEY")
        .or_else(|_| std::env::var("OPENAI_API_KEY"))
        .map_err(|_| RuntimeError {
            message: "No API key found. Set ANTHROPIC_API_KEY or OPENAI_API_KEY".to_string(),
        })?;

    let is_anthropic = std::env::var("ANTHROPIC_API_KEY").is_ok();

    let (url, body) = if is_anthropic {
        let body = serde_json::json!({
            "model": model,
            "max_tokens": 1024,
            "messages": [{"role": "user", "content": prompt}]
        });
        ("https://api.anthropic.com/v1/messages", body)
    } else {
        let body = serde_json::json!({
            "model": model,
            "messages": [{"role": "user", "content": prompt}]
        });
        ("https://api.openai.com/v1/chat/completions", body)
    };

    let body_str = serde_json::to_string(&body).unwrap();

    let mut cmd = Command::new("curl");
    cmd.arg("-s")
        .arg("-S")
        .arg("-X")
        .arg("POST")
        .arg("-H")
        .arg("Content-Type: application/json");

    if is_anthropic {
        cmd.arg("-H")
            .arg(format!("x-api-key: {}", api_key))
            .arg("-H")
            .arg("anthropic-version: 2023-06-01");
    } else {
        cmd.arg("-H")
            .arg(format!("Authorization: Bearer {}", api_key));
    }

    cmd.arg("-d").arg(&body_str).arg(url);

    let output = cmd.output().map_err(|e| RuntimeError {
        message: format!("Failed to call AI API: {}", e),
    })?;

    let response_text = String::from_utf8_lossy(&output.stdout).to_string();

    let json_val: serde_json::Value =
        serde_json::from_str(&response_text).map_err(|e| RuntimeError {
            message: format!("Invalid API response: {}", e),
        })?;

    // Extract text from response
    let text = if is_anthropic {
        json_val["content"][0]["text"]
            .as_str()
            .unwrap_or("")
            .to_string()
    } else {
        json_val["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string()
    };

    Ok(Value::Str(text))
}

fn ai_classify(args: &[Value], kwargs: &HashMap<String, Value>) -> Result<Value, RuntimeError> {
    if args.len() < 2 {
        return Err(RuntimeError {
            message: "ai.classify() requires (text, categories)".to_string(),
        });
    }
    let text = match &args[0] {
        Value::Str(s) => s.clone(),
        _ => {
            return Err(RuntimeError {
                message: "ai.classify() first arg must be a string".to_string(),
            })
        }
    };
    let categories = match &args[1] {
        Value::List(items) => {
            let cats: Vec<String> = items.iter().map(|i| format!("{}", i)).collect();
            cats.join(", ")
        }
        _ => {
            return Err(RuntimeError {
                message: "ai.classify() second arg must be a list of categories".to_string(),
            })
        }
    };

    let prompt = format!(
        "Classify the following text into exactly one of these categories: [{}]. \
         Respond with ONLY the category name, nothing else.\n\nText: {}",
        categories, text
    );

    let mut new_kwargs = kwargs.clone();
    if !new_kwargs.contains_key("model") {
        new_kwargs.insert(
            "model".to_string(),
            Value::Str("claude-sonnet-4-20250514".to_string()),
        );
    }

    ai_prompt(&[Value::Str(prompt)], &new_kwargs)
}

fn ai_extract(args: &[Value], kwargs: &HashMap<String, Value>) -> Result<Value, RuntimeError> {
    if args.len() < 2 {
        return Err(RuntimeError {
            message: "ai.extract() requires (text, fields)".to_string(),
        });
    }
    let text = match &args[0] {
        Value::Str(s) => s.clone(),
        _ => {
            return Err(RuntimeError {
                message: "ai.extract() first arg must be a string".to_string(),
            })
        }
    };
    let fields = match &args[1] {
        Value::List(items) => {
            let fs: Vec<String> = items.iter().map(|i| format!("{}", i)).collect();
            fs.join(", ")
        }
        _ => {
            return Err(RuntimeError {
                message: "ai.extract() second arg must be a list of field names".to_string(),
            })
        }
    };

    let prompt = format!(
        "Extract the following fields from the text: [{}]. \
         Respond with ONLY valid JSON object with these fields as keys.\n\nText: {}",
        fields, text
    );

    let mut new_kwargs = kwargs.clone();
    if !new_kwargs.contains_key("model") {
        new_kwargs.insert(
            "model".to_string(),
            Value::Str("claude-sonnet-4-20250514".to_string()),
        );
    }

    let result = ai_prompt(&[Value::Str(prompt)], &new_kwargs)?;

    // Try to parse the JSON response
    if let Value::Str(ref s) = result {
        if let Ok(parsed) = super::json_mod::parse_json_string(s) {
            return Ok(parsed);
        }
    }
    Ok(result)
}

fn ai_summarize(args: &[Value], kwargs: &HashMap<String, Value>) -> Result<Value, RuntimeError> {
    let text = match args.first() {
        Some(Value::Str(s)) => s.clone(),
        _ => {
            return Err(RuntimeError {
                message: "ai.summarize() requires a text string".to_string(),
            })
        }
    };

    let prompt = format!(
        "Summarize the following text concisely in 2-3 sentences:\n\n{}",
        text
    );

    let mut new_kwargs = kwargs.clone();
    if !new_kwargs.contains_key("model") {
        new_kwargs.insert(
            "model".to_string(),
            Value::Str("claude-sonnet-4-20250514".to_string()),
        );
    }

    ai_prompt(&[Value::Str(prompt)], &new_kwargs)
}
