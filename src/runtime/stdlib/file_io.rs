//! File I/O operations

use super::super::{Value, RuntimeError};
use std::collections::HashMap;

const MAX_READ_SIZE: u64 = 10 * 1024 * 1024; // 10MB

pub fn call(
    method: &str,
    args: Vec<Value>,
    _kwargs: HashMap<String, Value>,
) -> Result<Value, RuntimeError> {
    match method {
        "read" => read_file(&args),
        "write" => write_file(&args),
        "append" => append_file(&args),
        "exists" => file_exists(&args),
        "lines" => read_lines(&args),
        "delete" | "remove" => delete_file(&args),
        _ => Err(RuntimeError {
            message: format!("file.{}() not found", method),
        }),
    }
}

fn read_file(args: &[Value]) -> Result<Value, RuntimeError> {
    let path = get_path(args)?;

    let metadata = std::fs::metadata(&path).map_err(|e| RuntimeError {
        message: format!("Cannot access file '{}': {}", path, e),
    })?;
    if metadata.len() > MAX_READ_SIZE {
        return Err(RuntimeError {
            message: format!("File '{}' exceeds maximum read size of 10MB", path),
        });
    }

    let content = std::fs::read_to_string(&path).map_err(|e| RuntimeError {
        message: format!("Cannot read file '{}': {}", path, e),
    })?;
    Ok(Value::Str(content))
}

fn write_file(args: &[Value]) -> Result<Value, RuntimeError> {
    if args.len() < 2 {
        return Err(RuntimeError {
            message: "file.write() requires (path, content)".to_string(),
        });
    }
    let path = match &args[0] {
        Value::Str(s) => s.clone(),
        _ => {
            return Err(RuntimeError {
                message: "file.write() path must be a string".to_string(),
            })
        }
    };
    let content = format!("{}", &args[1]);

    std::fs::write(&path, &content).map_err(|e| RuntimeError {
        message: format!("Cannot write file '{}': {}", path, e),
    })?;
    Ok(Value::Bool(true))
}

fn append_file(args: &[Value]) -> Result<Value, RuntimeError> {
    if args.len() < 2 {
        return Err(RuntimeError {
            message: "file.append() requires (path, content)".to_string(),
        });
    }
    let path = match &args[0] {
        Value::Str(s) => s.clone(),
        _ => {
            return Err(RuntimeError {
                message: "file.append() path must be a string".to_string(),
            })
        }
    };
    let content = format!("{}", &args[1]);

    use std::io::Write;
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| RuntimeError {
            message: format!("Cannot open file '{}' for append: {}", path, e),
        })?;
    f.write_all(content.as_bytes()).map_err(|e| RuntimeError {
        message: format!("Cannot append to file '{}': {}", path, e),
    })?;
    Ok(Value::Bool(true))
}

fn file_exists(args: &[Value]) -> Result<Value, RuntimeError> {
    let path = get_path(args)?;
    Ok(Value::Bool(std::path::Path::new(&path).exists()))
}

fn read_lines(args: &[Value]) -> Result<Value, RuntimeError> {
    let content = match read_file(args)? {
        Value::Str(s) => s,
        _ => return Ok(Value::List(Vec::new())),
    };
    let lines: Vec<Value> = content.lines().map(|l| Value::Str(l.to_string())).collect();
    Ok(Value::List(lines))
}

fn delete_file(args: &[Value]) -> Result<Value, RuntimeError> {
    let path = get_path(args)?;
    std::fs::remove_file(&path).map_err(|e| RuntimeError {
        message: format!("Cannot delete file '{}': {}", path, e),
    })?;
    Ok(Value::Bool(true))
}

fn get_path(args: &[Value]) -> Result<String, RuntimeError> {
    match args.first() {
        Some(Value::Str(s)) => Ok(s.clone()),
        _ => Err(RuntimeError {
            message: "Expected a file path string".to_string(),
        }),
    }
}
