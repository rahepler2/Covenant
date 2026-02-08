//! Extended math operations

use super::super::{Value, RuntimeError};
use std::collections::HashMap;

pub fn call(
    method: &str,
    args: Vec<Value>,
    _kwargs: HashMap<String, Value>,
) -> Result<Value, RuntimeError> {
    match method {
        "sqrt" => unary_float_op(&args, f64::sqrt, "sqrt"),
        "pow" => binary_pow(&args),
        "floor" => unary_int_op(&args, f64::floor, "floor"),
        "ceil" => unary_int_op(&args, f64::ceil, "ceil"),
        "round" => unary_int_op(&args, f64::round, "round"),
        "sin" => unary_float_op(&args, f64::sin, "sin"),
        "cos" => unary_float_op(&args, f64::cos, "cos"),
        "tan" => unary_float_op(&args, f64::tan, "tan"),
        "log" => unary_float_op(&args, f64::ln, "log"),
        "log10" => unary_float_op(&args, f64::log10, "log10"),
        "exp" => unary_float_op(&args, f64::exp, "exp"),
        "pi" => Ok(Value::Float(std::f64::consts::PI)),
        "e" => Ok(Value::Float(std::f64::consts::E)),
        "random" => random_float(),
        _ => Err(RuntimeError {
            message: format!("math.{}() not found", method),
        }),
    }
}

fn get_number(args: &[Value], name: &str) -> Result<f64, RuntimeError> {
    match args.first() {
        Some(Value::Int(n)) => Ok(*n as f64),
        Some(Value::Float(n)) => Ok(*n),
        _ => Err(RuntimeError {
            message: format!("math.{}() requires a number", name),
        }),
    }
}

fn unary_float_op(args: &[Value], op: fn(f64) -> f64, name: &str) -> Result<Value, RuntimeError> {
    let n = get_number(args, name)?;
    Ok(Value::Float(op(n)))
}

fn unary_int_op(args: &[Value], op: fn(f64) -> f64, name: &str) -> Result<Value, RuntimeError> {
    let n = get_number(args, name)?;
    let result = op(n);
    if result.is_finite() && result >= i64::MIN as f64 && result <= i64::MAX as f64 {
        Ok(Value::Int(result as i64))
    } else {
        Ok(Value::Float(result))
    }
}

fn binary_pow(args: &[Value]) -> Result<Value, RuntimeError> {
    if args.len() < 2 {
        return Err(RuntimeError {
            message: "math.pow() requires (base, exponent)".to_string(),
        });
    }
    let base = match &args[0] {
        Value::Int(n) => *n as f64,
        Value::Float(n) => *n,
        _ => {
            return Err(RuntimeError {
                message: "math.pow() base must be a number".to_string(),
            })
        }
    };
    let exp = match &args[1] {
        Value::Int(n) => *n as f64,
        Value::Float(n) => *n,
        _ => {
            return Err(RuntimeError {
                message: "math.pow() exponent must be a number".to_string(),
            })
        }
    };
    let result = base.powf(exp);
    // Return Int when result is a whole number
    if result.is_finite()
        && result == (result as i64) as f64
        && result >= i64::MIN as f64
        && result <= i64::MAX as f64
    {
        Ok(Value::Int(result as i64))
    } else {
        Ok(Value::Float(result))
    }
}

fn random_float() -> Result<Value, RuntimeError> {
    // Simple time-seeded random — suitable for scripting, not crypto
    use std::time::{SystemTime, UNIX_EPOCH};
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos() as u64;
    let val = ((seed
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407))
        >> 33) as f64
        / (1u64 << 31) as f64;
    Ok(Value::Float(val))
}
