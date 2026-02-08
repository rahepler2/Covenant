//! Environment variable access

use super::super::{Value, RuntimeError};
use std::collections::HashMap;

pub fn call(
    method: &str,
    args: Vec<Value>,
    _kwargs: HashMap<String, Value>,
) -> Result<Value, RuntimeError> {
    match method {
        "get" => env_get(&args),
        "set" => env_set(&args),
        "has" => env_has(&args),
        "all" => env_all(),
        _ => Err(RuntimeError {
            message: format!("env.{}() not found", method),
        }),
    }
}

fn env_get(args: &[Value]) -> Result<Value, RuntimeError> {
    let key = match args.first() {
        Some(Value::Str(s)) => s.clone(),
        _ => {
            return Err(RuntimeError {
                message: "env.get() requires a key string".to_string(),
            })
        }
    };
    let default = match args.get(1) {
        Some(val) => val.clone(),
        None => Value::Null,
    };
    match std::env::var(&key) {
        Ok(val) => Ok(Value::Str(val)),
        Err(_) => Ok(default),
    }
}

fn env_set(args: &[Value]) -> Result<Value, RuntimeError> {
    if args.len() < 2 {
        return Err(RuntimeError {
            message: "env.set() requires (key, value)".to_string(),
        });
    }
    let key = match &args[0] {
        Value::Str(s) => s.clone(),
        _ => {
            return Err(RuntimeError {
                message: "env.set() key must be a string".to_string(),
            })
        }
    };
    let val = format!("{}", &args[1]);
    unsafe {
        std::env::set_var(&key, &val);
    }
    Ok(Value::Bool(true))
}

fn env_has(args: &[Value]) -> Result<Value, RuntimeError> {
    let key = match args.first() {
        Some(Value::Str(s)) => s.clone(),
        _ => {
            return Err(RuntimeError {
                message: "env.has() requires a key string".to_string(),
            })
        }
    };
    Ok(Value::Bool(std::env::var(&key).is_ok()))
}

fn env_all() -> Result<Value, RuntimeError> {
    let mut fields = HashMap::new();
    for (key, val) in std::env::vars() {
        fields.insert(key, Value::Str(val));
    }
    Ok(Value::Object("Environment".to_string(), fields))
}
