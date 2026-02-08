//! JSON parse/stringify using serde_json

use super::super::{Value, RuntimeError};
use std::collections::HashMap;

pub fn call(
    method: &str,
    args: Vec<Value>,
    _kwargs: HashMap<String, Value>,
) -> Result<Value, RuntimeError> {
    match method {
        "parse" => parse(&args),
        "stringify" | "to_string" => stringify(&args),
        _ => Err(RuntimeError {
            message: format!("json.{}() not found", method),
        }),
    }
}

fn parse(args: &[Value]) -> Result<Value, RuntimeError> {
    let text = match args.first() {
        Some(Value::Str(s)) => s.clone(),
        _ => {
            return Err(RuntimeError {
                message: "json.parse() requires a string".to_string(),
            })
        }
    };
    parse_json_string(&text)
}

pub fn parse_json_string(s: &str) -> Result<Value, RuntimeError> {
    let json_val: serde_json::Value = serde_json::from_str(s).map_err(|e| RuntimeError {
        message: format!("Invalid JSON: {}", e),
    })?;
    Ok(json_to_value(&json_val))
}

fn json_to_value(v: &serde_json::Value) -> Value {
    match v {
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
        serde_json::Value::Array(arr) => Value::List(arr.iter().map(json_to_value).collect()),
        serde_json::Value::Object(obj) => {
            let fields: HashMap<String, Value> =
                obj.iter().map(|(k, v)| (k.clone(), json_to_value(v))).collect();
            Value::Object("JsonObject".to_string(), fields)
        }
    }
}

fn stringify(args: &[Value]) -> Result<Value, RuntimeError> {
    let val = match args.first() {
        Some(v) => v,
        None => {
            return Err(RuntimeError {
                message: "json.stringify() requires an argument".to_string(),
            })
        }
    };
    let json_val = value_to_json(val);
    Ok(Value::Str(
        serde_json::to_string(&json_val).unwrap_or_else(|_| "null".to_string()),
    ))
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
