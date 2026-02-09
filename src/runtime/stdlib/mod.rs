//! Covenant Standard Library
//!
//! Tier 1 (core): web, data, json, file, ai, crypto, time, math, text, env
//! Tier 2 (AI-age): http, anthropic, openai, ollama, grok, mcp, mcpx, embeddings, prompts, guardrails

// Tier 1 — core stdlib
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
pub mod db;

// Tier 2 — AI-age libraries
pub mod http_client;
pub mod anthropic;
pub mod openai_mod;
pub mod ollama;
pub mod grok;
pub mod mcp_mod;
pub mod mcpx;
pub mod embeddings;
pub mod prompts;
pub mod guardrails;

use super::{Value, RuntimeError};
use std::collections::HashMap;

const STDLIB_MODULES: &[&str] = &[
    // Tier 1
    "web", "data", "json", "file", "ai", "crypto", "time", "math", "text", "env", "db",
    // Tier 2
    "http", "anthropic", "openai", "ollama", "grok", "mcp", "mcpx",
    "embeddings", "prompts", "guardrails",
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
        // Tier 1
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
        "db" => db::call(method, args, kwargs),
        // Tier 2
        "http" => http_client::call(method, args, kwargs),
        "anthropic" => anthropic::call(method, args, kwargs),
        "openai" => openai_mod::call(method, args, kwargs),
        "ollama" => ollama::call(method, args, kwargs),
        "grok" => grok::call(method, args, kwargs),
        "mcp" => mcp_mod::call(method, args, kwargs),
        "mcpx" => mcpx::call(method, args, kwargs),
        "embeddings" => embeddings::call(method, args, kwargs),
        "prompts" => prompts::call(method, args, kwargs),
        "guardrails" => guardrails::call(method, args, kwargs),
        _ => Err(RuntimeError {
            message: format!("Unknown module: {}", module),
        }),
    }
}

/// Types created by stdlib that have methods (DataFrame, HttpResponse)
const STDLIB_TYPES: &[&str] = &["DataFrame", "HttpResponse", "Database"];

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
        "Database" => db::call_db_method(obj_fields, method, args, kwargs),
        _ => Err(RuntimeError {
            message: format!("No methods for stdlib type: {}", type_name),
        }),
    }
}
