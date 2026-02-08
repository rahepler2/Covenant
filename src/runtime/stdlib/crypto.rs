//! Cryptographic operations using sha2

use super::super::{Value, RuntimeError};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

pub fn call(
    method: &str,
    args: Vec<Value>,
    _kwargs: HashMap<String, Value>,
) -> Result<Value, RuntimeError> {
    match method {
        "sha256" | "hash" => sha256_hash(&args),
        "hmac" => hmac_sha256(&args),
        _ => Err(RuntimeError {
            message: format!("crypto.{}() not found", method),
        }),
    }
}

fn sha256_hash(args: &[Value]) -> Result<Value, RuntimeError> {
    let input = match args.first() {
        Some(Value::Str(s)) => s.clone(),
        Some(v) => format!("{}", v),
        None => {
            return Err(RuntimeError {
                message: "crypto.sha256() requires an argument".to_string(),
            })
        }
    };

    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let result = hasher.finalize();
    Ok(Value::Str(format!("{:x}", result)))
}

fn hmac_sha256(args: &[Value]) -> Result<Value, RuntimeError> {
    if args.len() < 2 {
        return Err(RuntimeError {
            message: "crypto.hmac() requires (key, message)".to_string(),
        });
    }
    let key = match &args[0] {
        Value::Str(s) => s.as_bytes().to_vec(),
        _ => {
            return Err(RuntimeError {
                message: "crypto.hmac() key must be a string".to_string(),
            })
        }
    };
    let message = match &args[1] {
        Value::Str(s) => s.as_bytes().to_vec(),
        _ => {
            return Err(RuntimeError {
                message: "crypto.hmac() message must be a string".to_string(),
            })
        }
    };

    // HMAC-SHA256 (RFC 2104)
    let block_size = 64;
    let mut k = key;
    if k.len() > block_size {
        let mut hasher = Sha256::new();
        hasher.update(&k);
        k = hasher.finalize().to_vec();
    }
    while k.len() < block_size {
        k.push(0);
    }

    let i_key_pad: Vec<u8> = k.iter().map(|b| b ^ 0x36).collect();
    let o_key_pad: Vec<u8> = k.iter().map(|b| b ^ 0x5c).collect();

    let mut inner_hasher = Sha256::new();
    inner_hasher.update(&i_key_pad);
    inner_hasher.update(&message);
    let inner_hash = inner_hasher.finalize();

    let mut outer_hasher = Sha256::new();
    outer_hasher.update(&o_key_pad);
    outer_hasher.update(&inner_hash);
    let result = outer_hasher.finalize();

    Ok(Value::Str(format!("{:x}", result)))
}
