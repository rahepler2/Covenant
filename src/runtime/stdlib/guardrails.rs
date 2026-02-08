//! Guardrails — output validation and content filtering
//!
//! Methods: validate_json, validate_schema, check_pii, check_length,
//!          check_toxicity, sanitize, assert_format
//! Enforces safety constraints on LLM outputs

use super::super::{Value, RuntimeError};
use std::collections::HashMap;

pub fn call(
    method: &str,
    args: Vec<Value>,
    kwargs: HashMap<String, Value>,
) -> Result<Value, RuntimeError> {
    match method {
        "validate_json" | "is_json" => validate_json(&args),
        "validate_schema" | "check_schema" => validate_schema(&args, &kwargs),
        "check_pii" | "has_pii" => check_pii(&args),
        "check_length" => check_length(&args, &kwargs),
        "sanitize" | "clean" => sanitize(&args, &kwargs),
        "assert_format" => assert_format(&args, &kwargs),
        "check_contains" => check_contains(&args, &kwargs),
        "check_not_contains" => check_not_contains(&args, &kwargs),
        "retry_parse" => retry_parse(&args, &kwargs),
        _ => Err(RuntimeError {
            message: format!("guardrails.{}() not found", method),
        }),
    }
}

/// Validate that a string is valid JSON
fn validate_json(args: &[Value]) -> Result<Value, RuntimeError> {
    let text = match args.first() {
        Some(Value::Str(s)) => s.clone(),
        _ => return Ok(Value::Bool(false)),
    };

    let is_valid = serde_json::from_str::<serde_json::Value>(&text).is_ok();
    Ok(Value::Bool(is_valid))
}

/// Validate that a JSON string or object matches a schema
/// guardrails.validate_schema(data, required: ["name", "age"], types: {name: "string", age: "number"})
fn validate_schema(args: &[Value], kwargs: &HashMap<String, Value>) -> Result<Value, RuntimeError> {
    let data = match args.first() {
        Some(v) => v.clone(),
        None => return Ok(Value::Bool(false)),
    };

    // Parse JSON string to object if needed
    let obj_fields = match &data {
        Value::Object(_, fields) => fields.clone(),
        Value::Str(s) => {
            match serde_json::from_str::<serde_json::Value>(s) {
                Ok(serde_json::Value::Object(map)) => {
                    map.iter().map(|(k, v)| (k.clone(), json_to_value(v))).collect()
                }
                _ => return Ok(Value::Bool(false)),
            }
        }
        _ => return Ok(Value::Bool(false)),
    };

    // Check required fields
    if let Some(Value::List(required)) = kwargs.get("required") {
        for req in required {
            if let Value::Str(key) = req {
                if !obj_fields.contains_key(key) {
                    return Ok(Value::Bool(false));
                }
            }
        }
    }

    // Check field types
    if let Some(Value::Object(_, type_map)) = kwargs.get("types") {
        for (field, expected_type) in type_map {
            if let (Some(value), Value::Str(type_name)) = (obj_fields.get(field), expected_type) {
                let matches = match type_name.as_str() {
                    "string" | "str" => matches!(value, Value::Str(_)),
                    "number" | "int" | "integer" => matches!(value, Value::Int(_)),
                    "float" | "decimal" => matches!(value, Value::Float(_) | Value::Int(_)),
                    "bool" | "boolean" => matches!(value, Value::Bool(_)),
                    "list" | "array" => matches!(value, Value::List(_)),
                    "object" => matches!(value, Value::Object(_, _)),
                    "null" => matches!(value, Value::Null),
                    _ => true,
                };
                if !matches {
                    return Ok(Value::Bool(false));
                }
            }
        }
    }

    Ok(Value::Bool(true))
}

/// Check if text contains PII patterns (emails, phones, SSNs)
fn check_pii(args: &[Value]) -> Result<Value, RuntimeError> {
    let text = match args.first() {
        Some(Value::Str(s)) => s.clone(),
        _ => return Ok(Value::Bool(false)),
    };

    let mut findings = Vec::new();

    // Email pattern
    if regex::Regex::new(r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}")
        .map(|re| re.is_match(&text))
        .unwrap_or(false)
    {
        findings.push(Value::Str("email".to_string()));
    }

    // Phone pattern (US)
    if regex::Regex::new(r"\b\d{3}[-.]?\d{3}[-.]?\d{4}\b")
        .map(|re| re.is_match(&text))
        .unwrap_or(false)
    {
        findings.push(Value::Str("phone".to_string()));
    }

    // SSN pattern
    if regex::Regex::new(r"\b\d{3}-\d{2}-\d{4}\b")
        .map(|re| re.is_match(&text))
        .unwrap_or(false)
    {
        findings.push(Value::Str("ssn".to_string()));
    }

    // Credit card pattern
    if regex::Regex::new(r"\b\d{4}[-\s]?\d{4}[-\s]?\d{4}[-\s]?\d{4}\b")
        .map(|re| re.is_match(&text))
        .unwrap_or(false)
    {
        findings.push(Value::Str("credit_card".to_string()));
    }

    if findings.is_empty() {
        Ok(Value::Bool(false))
    } else {
        Ok(Value::List(findings))
    }
}

/// Check text length constraints
fn check_length(args: &[Value], kwargs: &HashMap<String, Value>) -> Result<Value, RuntimeError> {
    let text = match args.first() {
        Some(Value::Str(s)) => s.clone(),
        _ => return Ok(Value::Bool(false)),
    };

    let len = text.len() as i64;

    if let Some(Value::Int(min)) = kwargs.get("min") {
        if len < *min {
            return Ok(Value::Bool(false));
        }
    }

    if let Some(Value::Int(max)) = kwargs.get("max") {
        if len > *max {
            return Ok(Value::Bool(false));
        }
    }

    Ok(Value::Bool(true))
}

/// Sanitize text: strip code blocks, HTML, control characters
fn sanitize(args: &[Value], kwargs: &HashMap<String, Value>) -> Result<Value, RuntimeError> {
    let text = match args.first() {
        Some(Value::Str(s)) => s.clone(),
        _ => return Err(RuntimeError {
            message: "guardrails.sanitize() requires a string".to_string(),
        }),
    };

    let mut result = text;

    // Strip HTML tags
    if let Some(Value::Bool(true)) = kwargs.get("strip_html") {
        if let Ok(re) = regex::Regex::new(r"<[^>]+>") {
            result = re.replace_all(&result, "").to_string();
        }
    }

    // Strip code blocks
    if let Some(Value::Bool(true)) = kwargs.get("strip_code") {
        if let Ok(re) = regex::Regex::new(r"```[\s\S]*?```") {
            result = re.replace_all(&result, "[code removed]").to_string();
        }
    }

    // Strip control characters (keep newlines and tabs)
    result = result.chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\t' || *c == '\r')
        .collect();

    // Trim whitespace
    result = result.trim().to_string();

    Ok(Value::Str(result))
}

/// Assert that text matches a specific format
/// guardrails.assert_format(text, format: "json") or format: "email", "url", etc.
fn assert_format(args: &[Value], kwargs: &HashMap<String, Value>) -> Result<Value, RuntimeError> {
    let text = match args.first() {
        Some(Value::Str(s)) => s.clone(),
        _ => return Ok(Value::Bool(false)),
    };

    let format = match kwargs.get("format").or(args.get(1)) {
        Some(Value::Str(s)) => s.clone(),
        _ => return Ok(Value::Bool(true)),
    };

    let is_valid = match format.as_str() {
        "json" => serde_json::from_str::<serde_json::Value>(&text).is_ok(),
        "email" => regex::Regex::new(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$")
            .map(|re| re.is_match(&text))
            .unwrap_or(false),
        "url" => text.starts_with("http://") || text.starts_with("https://"),
        "number" => text.parse::<f64>().is_ok(),
        "integer" => text.parse::<i64>().is_ok(),
        "boolean" => text == "true" || text == "false",
        "nonempty" => !text.trim().is_empty(),
        _ => true,
    };

    Ok(Value::Bool(is_valid))
}

/// Check that text contains specific strings
fn check_contains(args: &[Value], kwargs: &HashMap<String, Value>) -> Result<Value, RuntimeError> {
    let text = match args.first() {
        Some(Value::Str(s)) => s.clone(),
        _ => return Ok(Value::Bool(false)),
    };

    if let Some(Value::List(required)) = kwargs.get("any").or(args.get(1).filter(|v| matches!(v, Value::List(_)))) {
        for item in required {
            if let Value::Str(s) = item {
                if text.contains(s.as_str()) {
                    return Ok(Value::Bool(true));
                }
            }
        }
        return Ok(Value::Bool(false));
    }

    if let Some(Value::List(required)) = kwargs.get("all") {
        for item in required {
            if let Value::Str(s) = item {
                if !text.contains(s.as_str()) {
                    return Ok(Value::Bool(false));
                }
            }
        }
    }

    Ok(Value::Bool(true))
}

/// Check that text does NOT contain specific strings
fn check_not_contains(args: &[Value], kwargs: &HashMap<String, Value>) -> Result<Value, RuntimeError> {
    let text = match args.first() {
        Some(Value::Str(s)) => s.to_lowercase(),
        _ => return Ok(Value::Bool(true)),
    };

    let banned = match kwargs.get("words").or(args.get(1).filter(|v| matches!(v, Value::List(_)))) {
        Some(Value::List(items)) => items.clone(),
        _ => Vec::new(),
    };

    for item in &banned {
        if let Value::Str(s) = item {
            if text.contains(&s.to_lowercase()) {
                return Ok(Value::Bool(false));
            }
        }
    }

    Ok(Value::Bool(true))
}

/// Try to extract JSON from a response that may have surrounding text
fn retry_parse(args: &[Value], _kwargs: &HashMap<String, Value>) -> Result<Value, RuntimeError> {
    let text = match args.first() {
        Some(Value::Str(s)) => s.clone(),
        _ => return Err(RuntimeError {
            message: "guardrails.retry_parse() requires a string".to_string(),
        }),
    };

    // Try direct parse first
    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&text) {
        return Ok(json_to_value(&parsed));
    }

    // Try to find JSON in code blocks
    if let Some(start) = text.find("```json") {
        if let Some(end) = text[start + 7..].find("```") {
            let json_str = text[start + 7..start + 7 + end].trim();
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(json_str) {
                return Ok(json_to_value(&parsed));
            }
        }
    }

    // Try to find JSON between { and }
    if let Some(start) = text.find('{') {
        if let Some(end) = text.rfind('}') {
            let json_str = &text[start..=end];
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(json_str) {
                return Ok(json_to_value(&parsed));
            }
        }
    }

    // Try to find JSON array between [ and ]
    if let Some(start) = text.find('[') {
        if let Some(end) = text.rfind(']') {
            let json_str = &text[start..=end];
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(json_str) {
                return Ok(json_to_value(&parsed));
            }
        }
    }

    Err(RuntimeError {
        message: "Could not extract valid JSON from response".to_string(),
    })
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
