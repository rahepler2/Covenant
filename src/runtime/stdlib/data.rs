//! DataFrame module — pandas-like data manipulation

use super::super::{Value, RuntimeError};
use std::collections::HashMap;

// ── Module-level functions ───────────────────────────────────────────────

pub fn call(
    method: &str,
    args: Vec<Value>,
    kwargs: HashMap<String, Value>,
) -> Result<Value, RuntimeError> {
    match method {
        "frame" | "from_records" => create_dataframe(args, kwargs),
        "read_csv" => read_csv(&args),
        _ => Err(RuntimeError {
            message: format!("data.{}() not found", method),
        }),
    }
}

fn create_dataframe(
    args: Vec<Value>,
    kwargs: HashMap<String, Value>,
) -> Result<Value, RuntimeError> {
    // data.frame(columns: [...], rows: [[...], ...])
    if let Some(Value::List(columns)) = kwargs.get("columns") {
        let col_names: Vec<String> = columns
            .iter()
            .map(|c| match c {
                Value::Str(s) => s.clone(),
                other => format!("{}", other),
            })
            .collect();
        let rows = match kwargs.get("rows") {
            Some(Value::List(r)) => r.clone(),
            _ => Vec::new(),
        };
        return Ok(make_dataframe(col_names, rows));
    }

    // data.frame(records) where records is a List of Objects
    if let Some(Value::List(records)) = args.first() {
        if let Some(Value::Object(_, fields)) = records.first() {
            let mut col_names: Vec<String> = fields.keys().cloned().collect();
            col_names.sort(); // deterministic column order
            let rows: Vec<Value> = records
                .iter()
                .map(|rec| match rec {
                    Value::Object(_, fields) => {
                        let row: Vec<Value> = col_names
                            .iter()
                            .map(|c| fields.get(c).cloned().unwrap_or(Value::Null))
                            .collect();
                        Value::List(row)
                    }
                    _ => Value::Null,
                })
                .collect();
            return Ok(make_dataframe(col_names, rows));
        }
    }

    // Empty dataframe
    Ok(make_dataframe(Vec::new(), Vec::new()))
}

fn make_dataframe(columns: Vec<String>, rows: Vec<Value>) -> Value {
    let nrows = rows.len() as i64;
    let mut fields = HashMap::new();
    fields.insert(
        "_columns".to_string(),
        Value::List(columns.iter().map(|c| Value::Str(c.clone())).collect()),
    );
    fields.insert("_rows".to_string(), Value::List(rows));
    fields.insert("_nrows".to_string(), Value::Int(nrows));
    Value::Object("DataFrame".to_string(), fields)
}

fn read_csv(args: &[Value]) -> Result<Value, RuntimeError> {
    let path = match args.first() {
        Some(Value::Str(s)) => s.clone(),
        _ => {
            return Err(RuntimeError {
                message: "data.read_csv() requires a file path string".to_string(),
            })
        }
    };

    let content = std::fs::read_to_string(&path).map_err(|e| RuntimeError {
        message: format!("Cannot read CSV file '{}': {}", path, e),
    })?;

    let mut lines = content.lines();
    let header_line = match lines.next() {
        Some(h) => h,
        None => return Ok(make_dataframe(Vec::new(), Vec::new())),
    };

    let columns: Vec<String> = header_line
        .split(',')
        .map(|s| s.trim().to_string())
        .collect();

    let rows: Vec<Value> = lines
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let cells: Vec<Value> = line
                .split(',')
                .map(|cell| {
                    let trimmed = cell.trim();
                    if let Ok(n) = trimmed.parse::<i64>() {
                        Value::Int(n)
                    } else if let Ok(f) = trimmed.parse::<f64>() {
                        Value::Float(f)
                    } else if trimmed == "true" {
                        Value::Bool(true)
                    } else if trimmed == "false" {
                        Value::Bool(false)
                    } else {
                        Value::Str(trimmed.to_string())
                    }
                })
                .collect();
            Value::List(cells)
        })
        .collect();

    Ok(make_dataframe(columns, rows))
}

// ── Methods on DataFrame objects ─────────────────────────────────────────

pub fn call_method(
    fields: &HashMap<String, Value>,
    method: &str,
    args: Vec<Value>,
    _kwargs: HashMap<String, Value>,
) -> Result<Value, RuntimeError> {
    let columns = get_columns(fields);
    let rows = get_rows(fields);

    match method {
        "head" => {
            let n = match args.first() {
                Some(Value::Int(n)) => *n as usize,
                _ => 5,
            };
            let head_rows: Vec<Value> = rows.into_iter().take(n).collect();
            Ok(make_dataframe(columns, head_rows))
        }
        "tail" => {
            let n = match args.first() {
                Some(Value::Int(n)) => *n as usize,
                _ => 5,
            };
            let len = rows.len();
            let start = if len > n { len - n } else { 0 };
            let tail_rows: Vec<Value> = rows.into_iter().skip(start).collect();
            Ok(make_dataframe(columns, tail_rows))
        }
        "select" => {
            let col_names = extract_column_names(&args);
            let col_indices: Vec<usize> = col_names
                .iter()
                .filter_map(|name| columns.iter().position(|c| c == name))
                .collect();

            let new_rows: Vec<Value> = rows
                .iter()
                .map(|row| {
                    if let Value::List(cells) = row {
                        let selected: Vec<Value> = col_indices
                            .iter()
                            .map(|&i| cells.get(i).cloned().unwrap_or(Value::Null))
                            .collect();
                        Value::List(selected)
                    } else {
                        Value::Null
                    }
                })
                .collect();

            Ok(make_dataframe(col_names, new_rows))
        }
        "filter" => {
            // df.filter("column", "op", value)
            if args.len() < 3 {
                return Err(RuntimeError {
                    message: "filter() requires (column, operator, value)".to_string(),
                });
            }
            let col_name = match &args[0] {
                Value::Str(s) => s.clone(),
                _ => {
                    return Err(RuntimeError {
                        message: "filter() first arg must be column name string".to_string(),
                    })
                }
            };
            let op = match &args[1] {
                Value::Str(s) => s.clone(),
                _ => {
                    return Err(RuntimeError {
                        message: "filter() second arg must be operator string".to_string(),
                    })
                }
            };
            let compare_val = &args[2];

            let col_idx =
                columns
                    .iter()
                    .position(|c| c == &col_name)
                    .ok_or_else(|| RuntimeError {
                        message: format!("Column '{}' not found", col_name),
                    })?;

            let filtered: Vec<Value> = rows
                .into_iter()
                .filter(|row| {
                    if let Value::List(cells) = row {
                        if let Some(cell) = cells.get(col_idx) {
                            compare_values(cell, &op, compare_val)
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                })
                .collect();

            Ok(make_dataframe(columns, filtered))
        }
        "sort_by" => {
            let col_name = match args.first() {
                Some(Value::Str(s)) => s.clone(),
                _ => {
                    return Err(RuntimeError {
                        message: "sort_by() requires a column name".to_string(),
                    })
                }
            };
            let descending = match args.get(1) {
                Some(Value::Str(s)) => s == "desc",
                _ => false,
            };

            let col_idx =
                columns
                    .iter()
                    .position(|c| c == &col_name)
                    .ok_or_else(|| RuntimeError {
                        message: format!("Column '{}' not found", col_name),
                    })?;

            let mut sorted = rows;
            sorted.sort_by(|a, b| {
                let va = extract_cell(a, col_idx);
                let vb = extract_cell(b, col_idx);
                let ord = compare_for_sort(&va, &vb);
                if descending {
                    ord.reverse()
                } else {
                    ord
                }
            });

            Ok(make_dataframe(columns, sorted))
        }
        "group_by" => {
            let col_name = match args.first() {
                Some(Value::Str(s)) => s.clone(),
                _ => {
                    return Err(RuntimeError {
                        message: "group_by() requires a column name".to_string(),
                    })
                }
            };

            let col_idx =
                columns
                    .iter()
                    .position(|c| c == &col_name)
                    .ok_or_else(|| RuntimeError {
                        message: format!("Column '{}' not found", col_name),
                    })?;

            // Use Vec to preserve insertion order
            let mut group_keys: Vec<String> = Vec::new();
            let mut group_map: HashMap<String, Vec<Value>> = HashMap::new();
            for row in &rows {
                let key = format!("{}", extract_cell(row, col_idx));
                if !group_map.contains_key(&key) {
                    group_keys.push(key.clone());
                }
                group_map.entry(key).or_default().push(row.clone());
            }

            let mut result_fields = HashMap::new();
            for key in group_keys {
                if let Some(group_rows) = group_map.remove(&key) {
                    result_fields.insert(key, make_dataframe(columns.clone(), group_rows));
                }
            }

            Ok(Value::Object("DataFrameGroups".to_string(), result_fields))
        }
        "count" => Ok(Value::Int(rows.len() as i64)),
        "sum" => {
            let col_name = match args.first() {
                Some(Value::Str(s)) => s.clone(),
                _ => {
                    return Err(RuntimeError {
                        message: "sum() requires a column name".to_string(),
                    })
                }
            };
            let col_idx =
                columns
                    .iter()
                    .position(|c| c == &col_name)
                    .ok_or_else(|| RuntimeError {
                        message: format!("Column '{}' not found", col_name),
                    })?;

            let mut total = 0.0_f64;
            let mut is_int = true;
            for row in &rows {
                match extract_cell(row, col_idx) {
                    Value::Int(n) => total += n as f64,
                    Value::Float(n) => {
                        total += n;
                        is_int = false;
                    }
                    _ => {}
                }
            }
            if is_int {
                Ok(Value::Int(total as i64))
            } else {
                Ok(Value::Float(total))
            }
        }
        "mean" | "avg" => {
            let col_name = match args.first() {
                Some(Value::Str(s)) => s.clone(),
                _ => {
                    return Err(RuntimeError {
                        message: "mean() requires a column name".to_string(),
                    })
                }
            };
            let col_idx =
                columns
                    .iter()
                    .position(|c| c == &col_name)
                    .ok_or_else(|| RuntimeError {
                        message: format!("Column '{}' not found", col_name),
                    })?;

            let mut total = 0.0_f64;
            let mut count = 0;
            for row in &rows {
                match extract_cell(row, col_idx) {
                    Value::Int(n) => {
                        total += n as f64;
                        count += 1;
                    }
                    Value::Float(n) => {
                        total += n;
                        count += 1;
                    }
                    _ => {}
                }
            }
            if count == 0 {
                Ok(Value::Float(0.0))
            } else {
                Ok(Value::Float(total / count as f64))
            }
        }
        "column" | "col" => {
            let col_name = match args.first() {
                Some(Value::Str(s)) => s.clone(),
                _ => {
                    return Err(RuntimeError {
                        message: "column() requires a column name".to_string(),
                    })
                }
            };
            let col_idx =
                columns
                    .iter()
                    .position(|c| c == &col_name)
                    .ok_or_else(|| RuntimeError {
                        message: format!("Column '{}' not found", col_name),
                    })?;

            let values: Vec<Value> = rows.iter().map(|row| extract_cell(row, col_idx)).collect();
            Ok(Value::List(values))
        }
        "add_column" => {
            // add_column("name", [values]) or add_column("name", default_value)
            if args.len() < 2 {
                return Err(RuntimeError {
                    message: "add_column() requires (name, values)".to_string(),
                });
            }
            let col_name = match &args[0] {
                Value::Str(s) => s.clone(),
                _ => {
                    return Err(RuntimeError {
                        message: "add_column() first arg must be column name".to_string(),
                    })
                }
            };

            let mut new_columns = columns;
            new_columns.push(col_name);

            let new_rows: Vec<Value> = match &args[1] {
                Value::List(values) => rows
                    .into_iter()
                    .enumerate()
                    .map(|(i, row)| {
                        if let Value::List(mut cells) = row {
                            cells.push(values.get(i).cloned().unwrap_or(Value::Null));
                            Value::List(cells)
                        } else {
                            row
                        }
                    })
                    .collect(),
                default_val => rows
                    .into_iter()
                    .map(|row| {
                        if let Value::List(mut cells) = row {
                            cells.push(default_val.clone());
                            Value::List(cells)
                        } else {
                            row
                        }
                    })
                    .collect(),
            };

            Ok(make_dataframe(new_columns, new_rows))
        }
        "to_csv" => {
            let mut out = String::new();
            out.push_str(&columns.join(","));
            out.push('\n');
            for row in &rows {
                if let Value::List(cells) = row {
                    let line: Vec<String> = cells.iter().map(|c| format!("{}", c)).collect();
                    out.push_str(&line.join(","));
                    out.push('\n');
                }
            }
            Ok(Value::Str(out))
        }
        "print" | "show" => {
            print_dataframe(&columns, &rows);
            Ok(Value::Null)
        }
        _ => Err(RuntimeError {
            message: format!("DataFrame.{}() not found", method),
        }),
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────

fn get_columns(fields: &HashMap<String, Value>) -> Vec<String> {
    match fields.get("_columns") {
        Some(Value::List(cols)) => cols
            .iter()
            .map(|c| match c {
                Value::Str(s) => s.clone(),
                other => format!("{}", other),
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn get_rows(fields: &HashMap<String, Value>) -> Vec<Value> {
    match fields.get("_rows") {
        Some(Value::List(rows)) => rows.clone(),
        _ => Vec::new(),
    }
}

fn extract_cell(row: &Value, idx: usize) -> Value {
    match row {
        Value::List(cells) => cells.get(idx).cloned().unwrap_or(Value::Null),
        _ => Value::Null,
    }
}

fn extract_column_names(args: &[Value]) -> Vec<String> {
    let mut names = Vec::new();
    for arg in args {
        match arg {
            Value::Str(s) => names.push(s.clone()),
            Value::List(items) => {
                for item in items {
                    if let Value::Str(s) = item {
                        names.push(s.clone());
                    }
                }
            }
            _ => {}
        }
    }
    names
}

fn compare_values(cell: &Value, op: &str, val: &Value) -> bool {
    match (cell, val) {
        (Value::Int(a), Value::Int(b)) => match op {
            "==" | "=" => a == b,
            "!=" => a != b,
            ">" => a > b,
            ">=" => a >= b,
            "<" => a < b,
            "<=" => a <= b,
            _ => false,
        },
        (Value::Float(a), Value::Float(b)) => match op {
            "==" | "=" => a == b,
            "!=" => a != b,
            ">" => a > b,
            ">=" => a >= b,
            "<" => a < b,
            "<=" => a <= b,
            _ => false,
        },
        (Value::Int(a), Value::Float(b)) => {
            let af = *a as f64;
            match op {
                "==" | "=" => af == *b,
                "!=" => af != *b,
                ">" => af > *b,
                ">=" => af >= *b,
                "<" => af < *b,
                "<=" => af <= *b,
                _ => false,
            }
        }
        (Value::Float(a), Value::Int(b)) => {
            let bf = *b as f64;
            match op {
                "==" | "=" => *a == bf,
                "!=" => *a != bf,
                ">" => *a > bf,
                ">=" => *a >= bf,
                "<" => *a < bf,
                "<=" => *a <= bf,
                _ => false,
            }
        }
        (Value::Str(a), Value::Str(b)) => match op {
            "==" | "=" => a == b,
            "!=" => a != b,
            "contains" => a.contains(b.as_str()),
            _ => false,
        },
        _ => false,
    }
}

fn compare_for_sort(a: &Value, b: &Value) -> std::cmp::Ordering {
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => x.cmp(y),
        (Value::Float(x), Value::Float(y)) => {
            x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal)
        }
        (Value::Int(x), Value::Float(y)) => {
            (*x as f64).partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal)
        }
        (Value::Float(x), Value::Int(y)) => {
            x.partial_cmp(&(*y as f64)).unwrap_or(std::cmp::Ordering::Equal)
        }
        (Value::Str(x), Value::Str(y)) => x.cmp(y),
        _ => std::cmp::Ordering::Equal,
    }
}

fn print_dataframe(columns: &[String], rows: &[Value]) {
    if columns.is_empty() {
        println!("(empty DataFrame)");
        return;
    }

    // Compute column widths
    let mut widths: Vec<usize> = columns.iter().map(|c| c.len()).collect();
    for row in rows {
        if let Value::List(cells) = row {
            for (i, cell) in cells.iter().enumerate() {
                if i < widths.len() {
                    let w = format!("{}", cell).len();
                    if w > widths[i] {
                        widths[i] = w;
                    }
                }
            }
        }
    }

    // Print header
    let header: Vec<String> = columns
        .iter()
        .enumerate()
        .map(|(i, c)| format!("{:width$}", c, width = widths[i]))
        .collect();
    println!("{}", header.join(" | "));

    let sep: Vec<String> = widths.iter().map(|w| "-".repeat(*w)).collect();
    println!("{}", sep.join("-+-"));

    // Print rows
    for row in rows {
        if let Value::List(cells) = row {
            let line: Vec<String> = cells
                .iter()
                .enumerate()
                .map(|(i, cell)| {
                    let w = if i < widths.len() { widths[i] } else { 0 };
                    format!("{:width$}", format!("{}", cell), width = w)
                })
                .collect();
            println!("{}", line.join(" | "));
        }
    }
    println!("({} rows)", rows.len());
}
