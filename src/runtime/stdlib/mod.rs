//! Covenant Standard Library
//!
//! Modules: web, data, json, file, ai, crypto, time, math, text, env

pub mod web;
pub mod data;
pub mod json_mod;
pub mod file_io;
pub mod ai;
pub mod crypto;
pub mod time_mod;
pub mod math;
pub mod text;
pub mod env;

use super::{Value, RuntimeError};
use std::collections::HashMap;

const STDLIB_MODULES: &[&str] = &[
    "web", "data", "json", "file", "ai", "crypto", "time", "math", "text", "env",
];

/// Check if a name refers to a stdlib module
pub fn is_stdlib_module(name: &str) -> bool {
    STDLIB_MODULES.contains(&name)
}

/// Dispatch a method call on a stdlib module: module.method(args, kwargs)
pub fn call_module_method(
    module: &str,
    method: &str,
    args: Vec<Value>,
    kwargs: HashMap<String, Value>,
) -> Result<Value, RuntimeError> {
    match module {
        "web" => web::call(method, args, kwargs),
        "data" => data::call(method, args, kwargs),
        "json" => json_mod::call(method, args, kwargs),
        "file" => file_io::call(method, args, kwargs),
        "ai" => ai::call(method, args, kwargs),
        "crypto" => crypto::call(method, args, kwargs),
        "time" => time_mod::call(method, args, kwargs),
        "math" => math::call(method, args, kwargs),
        "text" => text::call(method, args, kwargs),
        "env" => env::call(method, args, kwargs),
        _ => Err(RuntimeError {
            message: format!("Unknown stdlib module: {}", module),
        }),
    }
}

/// Types created by stdlib that have methods (DataFrame, HttpResponse)
const STDLIB_TYPES: &[&str] = &["DataFrame", "HttpResponse"];

/// Check if a type name belongs to the stdlib
pub fn is_stdlib_type(type_name: &str) -> bool {
    STDLIB_TYPES.contains(&type_name)
}

/// Dispatch a method call on a stdlib-created object
pub fn call_type_method(
    type_name: &str,
    obj_fields: &HashMap<String, Value>,
    method: &str,
    args: Vec<Value>,
    kwargs: HashMap<String, Value>,
) -> Result<Value, RuntimeError> {
    match type_name {
        "DataFrame" => data::call_method(obj_fields, method, args, kwargs),
        "HttpResponse" => web::call_response_method(obj_fields, method, args, kwargs),
        _ => Err(RuntimeError {
            message: format!("No methods for stdlib type: {}", type_name),
        }),
    }
}
