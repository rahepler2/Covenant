//! MCPX — MCP Extensions for multi-agent coordination
//!
//! Methods: router, chain, parallel, fallback
//! Builds on the mcp module for multi-server orchestration

use super::super::{Value, RuntimeError};
use std::collections::HashMap;

pub fn call(
    method: &str,
    args: Vec<Value>,
    kwargs: HashMap<String, Value>,
) -> Result<Value, RuntimeError> {
    match method {
        "router" => router(&args, &kwargs),
        "chain" => chain(&args, &kwargs),
        "parallel" => parallel(&args, &kwargs),
        "fallback" => fallback(&args, &kwargs),
        _ => Err(RuntimeError {
            message: format!("mcpx.{}() not found", method),
        }),
    }
}

/// Create a router that dispatches tool calls to the right MCP server
fn router(args: &[Value], _kwargs: &HashMap<String, Value>) -> Result<Value, RuntimeError> {
    // args[0] = list of server configs [{name, server, tools}]
    let servers = match args.first() {
        Some(Value::List(l)) => l.clone(),
        _ => return Err(RuntimeError {
            message: "mcpx.router() requires a list of server configs".to_string(),
        }),
    };

    let mut fields = HashMap::new();
    fields.insert("type".to_string(), Value::Str("router".to_string()));
    fields.insert("servers".to_string(), Value::List(servers));

    Ok(Value::Object("McpxRouter".to_string(), fields))
}

/// Create a chain that pipes output through multiple MCP tools
fn chain(args: &[Value], _kwargs: &HashMap<String, Value>) -> Result<Value, RuntimeError> {
    let steps = match args.first() {
        Some(Value::List(l)) => l.clone(),
        _ => return Err(RuntimeError {
            message: "mcpx.chain() requires a list of steps".to_string(),
        }),
    };

    let mut fields = HashMap::new();
    fields.insert("type".to_string(), Value::Str("chain".to_string()));
    fields.insert("steps".to_string(), Value::List(steps));

    Ok(Value::Object("McpxChain".to_string(), fields))
}

/// Execute multiple MCP tool calls in parallel
fn parallel(args: &[Value], _kwargs: &HashMap<String, Value>) -> Result<Value, RuntimeError> {
    let tasks = match args.first() {
        Some(Value::List(l)) => l.clone(),
        _ => return Err(RuntimeError {
            message: "mcpx.parallel() requires a list of tasks".to_string(),
        }),
    };

    // Execute each task via mcp.call_tool sequentially (true parallelism needs async)
    let mut results = Vec::new();
    for task in &tasks {
        if let Value::Object(_, fields) = task {
            let server = fields.get("server").cloned().unwrap_or(Value::Null);
            let tool = fields.get("tool").cloned().unwrap_or(Value::Null);
            let task_args: HashMap<String, Value> = fields.iter()
                .filter(|(k, _)| k.as_str() != "server" && k.as_str() != "tool")
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();

            let call_args = vec![server, tool];
            match super::mcp_mod::call("call_tool", call_args, task_args) {
                Ok(result) => results.push(result),
                Err(e) => results.push(Value::Str(format!("Error: {}", e.message))),
            }
        }
    }

    Ok(Value::List(results))
}

/// Try multiple servers/tools, return first success
fn fallback(args: &[Value], _kwargs: &HashMap<String, Value>) -> Result<Value, RuntimeError> {
    let alternatives = match args.first() {
        Some(Value::List(l)) => l.clone(),
        _ => return Err(RuntimeError {
            message: "mcpx.fallback() requires a list of alternatives".to_string(),
        }),
    };

    for alt in &alternatives {
        if let Value::Object(_, fields) = alt {
            let server = fields.get("server").cloned().unwrap_or(Value::Null);
            let tool = fields.get("tool").cloned().unwrap_or(Value::Null);
            let alt_args: HashMap<String, Value> = fields.iter()
                .filter(|(k, _)| k.as_str() != "server" && k.as_str() != "tool")
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();

            let call_args = vec![server, tool];
            if let Ok(result) = super::mcp_mod::call("call_tool", call_args, alt_args) {
                return Ok(result);
            }
        }
    }

    Err(RuntimeError {
        message: "All fallback alternatives failed".to_string(),
    })
}
