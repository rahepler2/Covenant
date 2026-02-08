//! Embeddings — vector operations for RAG and semantic search
//!
//! Methods: cosine_similarity, dot_product, normalize, nearest, distance
//! Works with vectors from openai.embed(), ollama.embed(), etc.

use super::super::{Value, RuntimeError};
use std::collections::HashMap;

pub fn call(
    method: &str,
    args: Vec<Value>,
    kwargs: HashMap<String, Value>,
) -> Result<Value, RuntimeError> {
    match method {
        "cosine" | "cosine_similarity" => cosine_similarity(&args),
        "dot" | "dot_product" => dot_product(&args),
        "normalize" => normalize(&args),
        "distance" | "euclidean" => euclidean_distance(&args),
        "nearest" | "most_similar" => nearest(&args, &kwargs),
        "magnitude" | "norm" => magnitude(&args),
        "add" => vec_add(&args),
        "sub" | "subtract" => vec_sub(&args),
        "scale" => vec_scale(&args),
        "dim" | "dimensions" => dimensions(&args),
        _ => Err(RuntimeError {
            message: format!("embeddings.{}() not found", method),
        }),
    }
}

fn to_f64_vec(val: &Value) -> Result<Vec<f64>, RuntimeError> {
    match val {
        Value::List(items) => {
            let mut result = Vec::with_capacity(items.len());
            for item in items {
                match item {
                    Value::Float(f) => result.push(*f),
                    Value::Int(i) => result.push(*i as f64),
                    _ => return Err(RuntimeError {
                        message: "Vector elements must be numbers".to_string(),
                    }),
                }
            }
            Ok(result)
        }
        _ => Err(RuntimeError {
            message: "Expected a vector (list of numbers)".to_string(),
        }),
    }
}

fn f64_vec_to_value(v: Vec<f64>) -> Value {
    Value::List(v.into_iter().map(Value::Float).collect())
}

fn cosine_similarity(args: &[Value]) -> Result<Value, RuntimeError> {
    if args.len() < 2 {
        return Err(RuntimeError {
            message: "embeddings.cosine() requires two vectors".to_string(),
        });
    }
    let a = to_f64_vec(&args[0])?;
    let b = to_f64_vec(&args[1])?;

    if a.len() != b.len() {
        return Err(RuntimeError {
            message: format!("Vector dimension mismatch: {} vs {}", a.len(), b.len()),
        });
    }

    let dot: f64 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let mag_a: f64 = a.iter().map(|x| x * x).sum::<f64>().sqrt();
    let mag_b: f64 = b.iter().map(|x| x * x).sum::<f64>().sqrt();

    if mag_a == 0.0 || mag_b == 0.0 {
        return Ok(Value::Float(0.0));
    }

    Ok(Value::Float(dot / (mag_a * mag_b)))
}

fn dot_product(args: &[Value]) -> Result<Value, RuntimeError> {
    if args.len() < 2 {
        return Err(RuntimeError {
            message: "embeddings.dot() requires two vectors".to_string(),
        });
    }
    let a = to_f64_vec(&args[0])?;
    let b = to_f64_vec(&args[1])?;

    if a.len() != b.len() {
        return Err(RuntimeError {
            message: format!("Vector dimension mismatch: {} vs {}", a.len(), b.len()),
        });
    }

    let dot: f64 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    Ok(Value::Float(dot))
}

fn normalize(args: &[Value]) -> Result<Value, RuntimeError> {
    let v = to_f64_vec(args.first().ok_or_else(|| RuntimeError {
        message: "embeddings.normalize() requires a vector".to_string(),
    })?)?;

    let mag: f64 = v.iter().map(|x| x * x).sum::<f64>().sqrt();
    if mag == 0.0 {
        return Ok(f64_vec_to_value(v));
    }

    Ok(f64_vec_to_value(v.into_iter().map(|x| x / mag).collect()))
}

fn euclidean_distance(args: &[Value]) -> Result<Value, RuntimeError> {
    if args.len() < 2 {
        return Err(RuntimeError {
            message: "embeddings.distance() requires two vectors".to_string(),
        });
    }
    let a = to_f64_vec(&args[0])?;
    let b = to_f64_vec(&args[1])?;

    if a.len() != b.len() {
        return Err(RuntimeError {
            message: format!("Vector dimension mismatch: {} vs {}", a.len(), b.len()),
        });
    }

    let dist: f64 = a.iter().zip(b.iter())
        .map(|(x, y)| (x - y) * (x - y))
        .sum::<f64>()
        .sqrt();

    Ok(Value::Float(dist))
}

fn nearest(args: &[Value], kwargs: &HashMap<String, Value>) -> Result<Value, RuntimeError> {
    // args[0] = query vector, args[1] = list of vectors (or list of {vector, label} objects)
    if args.len() < 2 {
        return Err(RuntimeError {
            message: "embeddings.nearest() requires (query, candidates)".to_string(),
        });
    }

    let query = to_f64_vec(&args[0])?;
    let k = match kwargs.get("k") {
        Some(Value::Int(n)) => *n as usize,
        _ => 1,
    };

    let candidates = match &args[1] {
        Value::List(items) => items.clone(),
        _ => return Err(RuntimeError {
            message: "Second argument must be a list of vectors".to_string(),
        }),
    };

    // Score each candidate
    let mut scored: Vec<(usize, f64)> = Vec::new();
    for (i, candidate) in candidates.iter().enumerate() {
        let vec = match candidate {
            Value::List(_) => to_f64_vec(candidate)?,
            Value::Object(_, fields) => {
                if let Some(v) = fields.get("vector").or(fields.get("embedding")) {
                    to_f64_vec(v)?
                } else {
                    continue;
                }
            }
            _ => continue,
        };

        if vec.len() != query.len() {
            continue;
        }

        let dot: f64 = query.iter().zip(vec.iter()).map(|(a, b)| a * b).sum();
        let mag_q: f64 = query.iter().map(|x| x * x).sum::<f64>().sqrt();
        let mag_v: f64 = vec.iter().map(|x| x * x).sum::<f64>().sqrt();
        let similarity = if mag_q == 0.0 || mag_v == 0.0 { 0.0 } else { dot / (mag_q * mag_v) };

        scored.push((i, similarity));
    }

    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let results: Vec<Value> = scored.iter().take(k).map(|(i, score)| {
        let mut fields = HashMap::new();
        fields.insert("index".to_string(), Value::Int(*i as i64));
        fields.insert("score".to_string(), Value::Float(*score));
        fields.insert("item".to_string(), candidates[*i].clone());
        Value::Object("SearchResult".to_string(), fields)
    }).collect();

    if k == 1 && results.len() == 1 {
        Ok(results.into_iter().next().unwrap())
    } else {
        Ok(Value::List(results))
    }
}

fn magnitude(args: &[Value]) -> Result<Value, RuntimeError> {
    let v = to_f64_vec(args.first().ok_or_else(|| RuntimeError {
        message: "embeddings.magnitude() requires a vector".to_string(),
    })?)?;

    let mag: f64 = v.iter().map(|x| x * x).sum::<f64>().sqrt();
    Ok(Value::Float(mag))
}

fn vec_add(args: &[Value]) -> Result<Value, RuntimeError> {
    if args.len() < 2 {
        return Err(RuntimeError {
            message: "embeddings.add() requires two vectors".to_string(),
        });
    }
    let a = to_f64_vec(&args[0])?;
    let b = to_f64_vec(&args[1])?;

    if a.len() != b.len() {
        return Err(RuntimeError {
            message: "Vector dimension mismatch".to_string(),
        });
    }

    Ok(f64_vec_to_value(a.iter().zip(b.iter()).map(|(x, y)| x + y).collect()))
}

fn vec_sub(args: &[Value]) -> Result<Value, RuntimeError> {
    if args.len() < 2 {
        return Err(RuntimeError {
            message: "embeddings.sub() requires two vectors".to_string(),
        });
    }
    let a = to_f64_vec(&args[0])?;
    let b = to_f64_vec(&args[1])?;

    if a.len() != b.len() {
        return Err(RuntimeError {
            message: "Vector dimension mismatch".to_string(),
        });
    }

    Ok(f64_vec_to_value(a.iter().zip(b.iter()).map(|(x, y)| x - y).collect()))
}

fn vec_scale(args: &[Value]) -> Result<Value, RuntimeError> {
    if args.len() < 2 {
        return Err(RuntimeError {
            message: "embeddings.scale() requires (vector, scalar)".to_string(),
        });
    }
    let v = to_f64_vec(&args[0])?;
    let scalar = match &args[1] {
        Value::Float(f) => *f,
        Value::Int(i) => *i as f64,
        _ => return Err(RuntimeError {
            message: "Scale factor must be a number".to_string(),
        }),
    };

    Ok(f64_vec_to_value(v.into_iter().map(|x| x * scalar).collect()))
}

fn dimensions(args: &[Value]) -> Result<Value, RuntimeError> {
    let v = to_f64_vec(args.first().ok_or_else(|| RuntimeError {
        message: "embeddings.dim() requires a vector".to_string(),
    })?)?;
    Ok(Value::Int(v.len() as i64))
}
