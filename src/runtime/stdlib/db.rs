//! Database module — SQLite via sqlite3 subprocess
//!
//! Provides persistent storage for Covenant applications.
//! Uses the sqlite3 command-line tool (available on most systems).
//!
//! Usage:
//!   conn = db.open("myapp.db")
//!   db.execute(conn, "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT)")
//!   db.execute(conn, "INSERT INTO users (name) VALUES (?)", ["Alice"])
//!   rows = db.query(conn, "SELECT * FROM users")
//!   tables = db.tables(conn)

use super::super::{Value, RuntimeError};
use std::collections::HashMap;
use std::process::Command;

pub fn call(
    method: &str,
    args: Vec<Value>,
    kwargs: HashMap<String, Value>,
) -> Result<Value, RuntimeError> {
    match method {
        "open" => db_open(&args),
        "execute" => db_execute(&args, &kwargs),
        "query" => db_query(&args, &kwargs),
        "tables" => db_tables(&args),
        "close" => Ok(Value::Bool(true)), // no-op — sqlite3 CLI is stateless
        _ => Err(RuntimeError {
            message: format!("db.{}() not found. Available: open, execute, query, tables, close", method),
        }),
    }
}

/// db.open("path/to/database.db") -> Database connection object
fn db_open(args: &[Value]) -> Result<Value, RuntimeError> {
    let path = match args.first() {
        Some(Value::Str(s)) => s.clone(),
        _ => return Err(RuntimeError {
            message: "db.open() expects a file path string".to_string(),
        }),
    };

    // Verify sqlite3 is available
    let check = Command::new("sqlite3")
        .arg("--version")
        .output();

    if check.is_err() {
        return Err(RuntimeError {
            message: "sqlite3 not found. Install SQLite to use db module.".to_string(),
        });
    }

    // Create database file if it doesn't exist (sqlite3 does this automatically)
    // Return a connection object that stores the path
    let mut fields = HashMap::new();
    fields.insert("path".to_string(), Value::Str(path.clone()));
    fields.insert("open".to_string(), Value::Bool(true));

    Ok(Value::Object("Database".to_string(), fields))
}

/// Extract the database path from a connection object
fn get_db_path(args: &[Value]) -> Result<String, RuntimeError> {
    match args.first() {
        Some(Value::Object(type_name, fields)) if type_name == "Database" => {
            match fields.get("path") {
                Some(Value::Str(path)) => Ok(path.clone()),
                _ => Err(RuntimeError {
                    message: "Invalid Database object: missing path".to_string(),
                }),
            }
        }
        Some(Value::Str(path)) => Ok(path.clone()),
        _ => Err(RuntimeError {
            message: "First argument must be a Database connection (from db.open())".to_string(),
        }),
    }
}

/// Get SQL string from args[1]
fn get_sql(args: &[Value]) -> Result<String, RuntimeError> {
    match args.get(1) {
        Some(Value::Str(s)) => Ok(s.clone()),
        _ => Err(RuntimeError {
            message: "Second argument must be a SQL string".to_string(),
        }),
    }
}

/// Get optional params from args[2] or kwargs["params"]
fn get_params(args: &[Value], kwargs: &HashMap<String, Value>) -> Vec<String> {
    // Check args[2] first
    if let Some(Value::List(items)) = args.get(2) {
        return items.iter().map(|v| value_to_sql_param(v)).collect();
    }
    // Then check kwargs
    if let Some(Value::List(items)) = kwargs.get("params") {
        return items.iter().map(|v| value_to_sql_param(v)).collect();
    }
    Vec::new()
}

/// Convert a Value to a SQL parameter string
fn value_to_sql_param(v: &Value) -> String {
    match v {
        Value::Int(n) => n.to_string(),
        Value::Float(f) => f.to_string(),
        Value::Str(s) => s.clone(),
        Value::Bool(b) => if *b { "1".to_string() } else { "0".to_string() },
        Value::Null => "NULL".to_string(),
        _ => format!("{}", v),
    }
}

/// Substitute ? placeholders with params (safely quoted)
fn substitute_params(sql: &str, params: &[String]) -> String {
    let mut result = String::new();
    let mut param_idx = 0;
    let mut chars = sql.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '?' && param_idx < params.len() {
            let param = &params[param_idx];
            // Check if it's a number — don't quote numbers
            if param.parse::<i64>().is_ok() || param.parse::<f64>().is_ok() || param == "NULL" {
                result.push_str(param);
            } else {
                // Quote string values, escaping single quotes
                result.push('\'');
                result.push_str(&param.replace('\'', "''"));
                result.push('\'');
            }
            param_idx += 1;
        } else {
            result.push(ch);
        }
    }
    result
}

/// db.execute(conn, "SQL", [params]) -> Bool (success)
fn db_execute(args: &[Value], kwargs: &HashMap<String, Value>) -> Result<Value, RuntimeError> {
    let path = get_db_path(args)?;
    let sql = get_sql(args)?;
    let params = get_params(args, kwargs);

    let final_sql = if params.is_empty() {
        sql
    } else {
        substitute_params(&sql, &params)
    };

    // Validate no dangerous operations without confirmation
    let sql_upper = final_sql.trim().to_uppercase();
    if sql_upper.starts_with("DROP DATABASE") {
        return Err(RuntimeError {
            message: "DROP DATABASE is not allowed for safety".to_string(),
        });
    }

    let output = Command::new("sqlite3")
        .arg(&path)
        .arg(&final_sql)
        .output()
        .map_err(|e| RuntimeError {
            message: format!("Failed to execute sqlite3: {}", e),
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(RuntimeError {
            message: format!("SQL error: {}", stderr.trim()),
        });
    }

    Ok(Value::Bool(true))
}

/// db.query(conn, "SQL", [params]) -> List of Objects
fn db_query(args: &[Value], kwargs: &HashMap<String, Value>) -> Result<Value, RuntimeError> {
    let path = get_db_path(args)?;
    let sql = get_sql(args)?;
    let params = get_params(args, kwargs);

    let final_sql = if params.is_empty() {
        sql
    } else {
        substitute_params(&sql, &params)
    };

    // Use JSON output mode for structured results
    let output = Command::new("sqlite3")
        .arg("-json")
        .arg(&path)
        .arg(&final_sql)
        .output()
        .map_err(|e| RuntimeError {
            message: format!("Failed to execute sqlite3: {}", e),
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(RuntimeError {
            message: format!("SQL error: {}", stderr.trim()),
        });
    }

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if stdout.is_empty() {
        return Ok(Value::List(Vec::new()));
    }

    // Parse JSON array of objects
    let json: serde_json::Value = serde_json::from_str(&stdout).map_err(|e| RuntimeError {
        message: format!("Failed to parse query results: {}", e),
    })?;

    match json {
        serde_json::Value::Array(rows) => {
            let values: Vec<Value> = rows.iter().map(|row| json_to_row(row)).collect();
            Ok(Value::List(values))
        }
        _ => Ok(Value::List(Vec::new())),
    }
}

/// Convert a JSON row to a Covenant Value::Object
fn json_to_row(json: &serde_json::Value) -> Value {
    match json {
        serde_json::Value::Object(obj) => {
            let mut fields = HashMap::new();
            for (key, val) in obj {
                fields.insert(key.clone(), json_value_to_value(val));
            }
            Value::Object("Row".to_string(), fields)
        }
        _ => Value::Null,
    }
}

/// Convert a serde_json::Value to a Covenant Value
fn json_value_to_value(json: &serde_json::Value) -> Value {
    match json {
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
        serde_json::Value::Array(arr) => {
            Value::List(arr.iter().map(json_value_to_value).collect())
        }
        serde_json::Value::Object(obj) => {
            let mut fields = HashMap::new();
            for (k, v) in obj {
                fields.insert(k.clone(), json_value_to_value(v));
            }
            Value::Object("Object".to_string(), fields)
        }
    }
}

/// db.tables(conn) -> List of table names
fn db_tables(args: &[Value]) -> Result<Value, RuntimeError> {
    let path = get_db_path(args)?;

    let output = Command::new("sqlite3")
        .arg(&path)
        .arg(".tables")
        .output()
        .map_err(|e| RuntimeError {
            message: format!("Failed to execute sqlite3: {}", e),
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(RuntimeError {
            message: format!("SQLite error: {}", stderr.trim()),
        });
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let tables: Vec<Value> = stdout
        .split_whitespace()
        .filter(|s| !s.is_empty())
        .map(|s| Value::Str(s.to_string()))
        .collect();

    Ok(Value::List(tables))
}

/// Method dispatch for Database objects
pub fn call_db_method(
    fields: &HashMap<String, Value>,
    method: &str,
    args: Vec<Value>,
    kwargs: HashMap<String, Value>,
) -> Result<Value, RuntimeError> {
    let path = match fields.get("path") {
        Some(Value::Str(s)) => s.clone(),
        _ => return Err(RuntimeError {
            message: "Invalid Database object".to_string(),
        }),
    };

    // Prepend the connection as first arg
    let mut full_args = vec![Value::Str(path)];
    full_args.extend(args);

    match method {
        "execute" => db_execute(&full_args, &kwargs),
        "query" => db_query(&full_args, &kwargs),
        "tables" => db_tables(&full_args),
        "close" => Ok(Value::Bool(true)),
        _ => Err(RuntimeError {
            message: format!("Database.{}() not found", method),
        }),
    }
}
