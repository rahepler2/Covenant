//! String/text processing with regex support

use super::super::{Value, RuntimeError};
use regex::Regex;
use std::collections::HashMap;

pub fn call(
    method: &str,
    args: Vec<Value>,
    _kwargs: HashMap<String, Value>,
) -> Result<Value, RuntimeError> {
    match method {
        "split" => text_split(&args),
        "join" => text_join(&args),
        "replace" => text_replace(&args),
        "matches" => text_matches(&args),
        "find_all" => text_find_all(&args),
        "trim" => text_trim(&args),
        "upper" => text_upper(&args),
        "lower" => text_lower(&args),
        "starts_with" => text_starts_with(&args),
        "ends_with" => text_ends_with(&args),
        "contains" => text_contains(&args),
        "repeat" => text_repeat(&args),
        "reverse" => text_reverse(&args),
        "length" | "len" => text_length(&args),
        "slice" | "substring" => text_slice(&args),
        _ => Err(RuntimeError {
            message: format!("text.{}() not found", method),
        }),
    }
}

fn get_string(args: &[Value], idx: usize, name: &str) -> Result<String, RuntimeError> {
    match args.get(idx) {
        Some(Value::Str(s)) => Ok(s.clone()),
        _ => Err(RuntimeError {
            message: format!("text.{}() argument {} must be a string", name, idx + 1),
        }),
    }
}

fn text_split(args: &[Value]) -> Result<Value, RuntimeError> {
    let s = get_string(args, 0, "split")?;
    let delim = match args.get(1) {
        Some(Value::Str(d)) => d.clone(),
        _ => " ".to_string(),
    };
    let parts: Vec<Value> = s.split(&delim).map(|p| Value::Str(p.to_string())).collect();
    Ok(Value::List(parts))
}

fn text_join(args: &[Value]) -> Result<Value, RuntimeError> {
    let sep = get_string(args, 0, "join")?;
    let items = match args.get(1) {
        Some(Value::List(l)) => l,
        _ => {
            return Err(RuntimeError {
                message: "text.join() second arg must be a list".to_string(),
            })
        }
    };
    let strs: Vec<String> = items.iter().map(|v| format!("{}", v)).collect();
    Ok(Value::Str(strs.join(&sep)))
}

fn text_replace(args: &[Value]) -> Result<Value, RuntimeError> {
    let s = get_string(args, 0, "replace")?;
    let from = get_string(args, 1, "replace")?;
    let to = get_string(args, 2, "replace")?;
    Ok(Value::Str(s.replace(&from, &to)))
}

fn text_matches(args: &[Value]) -> Result<Value, RuntimeError> {
    let s = get_string(args, 0, "matches")?;
    let pattern = get_string(args, 1, "matches")?;
    let re = Regex::new(&pattern).map_err(|e| RuntimeError {
        message: format!("Invalid regex '{}': {}", pattern, e),
    })?;
    Ok(Value::Bool(re.is_match(&s)))
}

fn text_find_all(args: &[Value]) -> Result<Value, RuntimeError> {
    let s = get_string(args, 0, "find_all")?;
    let pattern = get_string(args, 1, "find_all")?;
    let re = Regex::new(&pattern).map_err(|e| RuntimeError {
        message: format!("Invalid regex '{}': {}", pattern, e),
    })?;
    let matches: Vec<Value> = re
        .find_iter(&s)
        .map(|m| Value::Str(m.as_str().to_string()))
        .collect();
    Ok(Value::List(matches))
}

fn text_trim(args: &[Value]) -> Result<Value, RuntimeError> {
    let s = get_string(args, 0, "trim")?;
    Ok(Value::Str(s.trim().to_string()))
}

fn text_upper(args: &[Value]) -> Result<Value, RuntimeError> {
    let s = get_string(args, 0, "upper")?;
    Ok(Value::Str(s.to_uppercase()))
}

fn text_lower(args: &[Value]) -> Result<Value, RuntimeError> {
    let s = get_string(args, 0, "lower")?;
    Ok(Value::Str(s.to_lowercase()))
}

fn text_starts_with(args: &[Value]) -> Result<Value, RuntimeError> {
    let s = get_string(args, 0, "starts_with")?;
    let prefix = get_string(args, 1, "starts_with")?;
    Ok(Value::Bool(s.starts_with(&prefix)))
}

fn text_ends_with(args: &[Value]) -> Result<Value, RuntimeError> {
    let s = get_string(args, 0, "ends_with")?;
    let suffix = get_string(args, 1, "ends_with")?;
    Ok(Value::Bool(s.ends_with(&suffix)))
}

fn text_contains(args: &[Value]) -> Result<Value, RuntimeError> {
    let s = get_string(args, 0, "contains")?;
    let sub = get_string(args, 1, "contains")?;
    Ok(Value::Bool(s.contains(&sub)))
}

fn text_repeat(args: &[Value]) -> Result<Value, RuntimeError> {
    let s = get_string(args, 0, "repeat")?;
    let n = match args.get(1) {
        Some(Value::Int(n)) => {
            if *n < 0 || *n > 10_000 {
                return Err(RuntimeError {
                    message: "text.repeat() count must be 0-10000".to_string(),
                });
            }
            *n as usize
        }
        _ => {
            return Err(RuntimeError {
                message: "text.repeat() requires (string, count)".to_string(),
            })
        }
    };
    Ok(Value::Str(s.repeat(n)))
}

fn text_reverse(args: &[Value]) -> Result<Value, RuntimeError> {
    let s = get_string(args, 0, "reverse")?;
    Ok(Value::Str(s.chars().rev().collect()))
}

fn text_length(args: &[Value]) -> Result<Value, RuntimeError> {
    let s = get_string(args, 0, "length")?;
    Ok(Value::Int(s.len() as i64))
}

fn text_slice(args: &[Value]) -> Result<Value, RuntimeError> {
    let s = get_string(args, 0, "slice")?;
    let start = match args.get(1) {
        Some(Value::Int(n)) => *n as usize,
        _ => 0,
    };
    let end = match args.get(2) {
        Some(Value::Int(n)) => *n as usize,
        _ => s.len(),
    };
    let chars: Vec<char> = s.chars().collect();
    let start = start.min(chars.len());
    let end = end.min(chars.len());
    Ok(Value::Str(chars[start..end].iter().collect()))
}
