//! Prompt engineering library
//!
//! Methods: template, few_shot, system, chat_messages, format
//! Builds structured prompts for LLM calls

use super::super::{Value, RuntimeError};
use std::collections::HashMap;

pub fn call(
    method: &str,
    args: Vec<Value>,
    kwargs: HashMap<String, Value>,
) -> Result<Value, RuntimeError> {
    match method {
        "template" | "render" => template(&args, &kwargs),
        "few_shot" => few_shot(&args, &kwargs),
        "system" => system_msg(&args),
        "user" => user_msg(&args),
        "assistant" => assistant_msg(&args),
        "messages" | "chat" => build_messages(&args, &kwargs),
        "format" => format_prompt(&args, &kwargs),
        _ => Err(RuntimeError {
            message: format!("prompts.{}() not found", method),
        }),
    }
}

/// Render a template string with variable substitution
/// prompts.template("Hello {name}, you are {age} years old", name: "Alice", age: 30)
fn template(args: &[Value], kwargs: &HashMap<String, Value>) -> Result<Value, RuntimeError> {
    let template = match args.first() {
        Some(Value::Str(s)) => s.clone(),
        _ => return Err(RuntimeError {
            message: "prompts.template() requires a template string".to_string(),
        }),
    };

    let mut result = template;
    for (key, value) in kwargs {
        let placeholder = format!("{{{}}}", key);
        let replacement = format!("{}", value);
        result = result.replace(&placeholder, &replacement);
    }

    Ok(Value::Str(result))
}

/// Build a few-shot prompt from examples
/// prompts.few_shot(task: "classify", examples: [...], input: "text")
fn few_shot(args: &[Value], kwargs: &HashMap<String, Value>) -> Result<Value, RuntimeError> {
    let task = match kwargs.get("task").or(args.first()) {
        Some(Value::Str(s)) => s.clone(),
        _ => "complete the task".to_string(),
    };

    let examples = match kwargs.get("examples").or(args.get(1)) {
        Some(Value::List(items)) => items.clone(),
        _ => Vec::new(),
    };

    let input = match kwargs.get("input").or(args.get(2)) {
        Some(Value::Str(s)) => s.clone(),
        _ => String::new(),
    };

    let mut prompt = format!("Task: {}\n\n", task);

    for (i, example) in examples.iter().enumerate() {
        match example {
            Value::Object(_, fields) => {
                let inp = fields.get("input").map(|v| format!("{}", v)).unwrap_or_default();
                let out = fields.get("output").map(|v| format!("{}", v)).unwrap_or_default();
                prompt.push_str(&format!("Example {}:\nInput: {}\nOutput: {}\n\n", i + 1, inp, out));
            }
            Value::List(pair) if pair.len() >= 2 => {
                prompt.push_str(&format!("Example {}:\nInput: {}\nOutput: {}\n\n", i + 1, pair[0], pair[1]));
            }
            _ => {
                prompt.push_str(&format!("Example {}: {}\n\n", i + 1, example));
            }
        }
    }

    if !input.is_empty() {
        prompt.push_str(&format!("Now complete:\nInput: {}\nOutput:", input));
    }

    Ok(Value::Str(prompt))
}

/// Create a system message object
fn system_msg(args: &[Value]) -> Result<Value, RuntimeError> {
    let content = match args.first() {
        Some(Value::Str(s)) => s.clone(),
        _ => return Err(RuntimeError {
            message: "prompts.system() requires a string".to_string(),
        }),
    };

    let mut fields = HashMap::new();
    fields.insert("role".to_string(), Value::Str("system".to_string()));
    fields.insert("content".to_string(), Value::Str(content));
    Ok(Value::Object("Message".to_string(), fields))
}

/// Create a user message object
fn user_msg(args: &[Value]) -> Result<Value, RuntimeError> {
    let content = match args.first() {
        Some(Value::Str(s)) => s.clone(),
        _ => return Err(RuntimeError {
            message: "prompts.user() requires a string".to_string(),
        }),
    };

    let mut fields = HashMap::new();
    fields.insert("role".to_string(), Value::Str("user".to_string()));
    fields.insert("content".to_string(), Value::Str(content));
    Ok(Value::Object("Message".to_string(), fields))
}

/// Create an assistant message object
fn assistant_msg(args: &[Value]) -> Result<Value, RuntimeError> {
    let content = match args.first() {
        Some(Value::Str(s)) => s.clone(),
        _ => return Err(RuntimeError {
            message: "prompts.assistant() requires a string".to_string(),
        }),
    };

    let mut fields = HashMap::new();
    fields.insert("role".to_string(), Value::Str("assistant".to_string()));
    fields.insert("content".to_string(), Value::Str(content));
    Ok(Value::Object("Message".to_string(), fields))
}

/// Build a message list for chat APIs
/// prompts.messages(system: "You are helpful", messages: [...])
fn build_messages(args: &[Value], kwargs: &HashMap<String, Value>) -> Result<Value, RuntimeError> {
    let mut messages = Vec::new();

    // Add system message if provided
    if let Some(Value::Str(sys)) = kwargs.get("system") {
        let mut fields = HashMap::new();
        fields.insert("role".to_string(), Value::Str("system".to_string()));
        fields.insert("content".to_string(), Value::Str(sys.clone()));
        messages.push(Value::Object("Message".to_string(), fields));
    }

    // Add any existing messages from args or kwargs
    if let Some(Value::List(msgs)) = kwargs.get("history").or(args.first()) {
        messages.extend(msgs.clone());
    }

    // Add a new user message if provided
    if let Some(Value::Str(user)) = kwargs.get("user").or(args.get(1)) {
        let mut fields = HashMap::new();
        fields.insert("role".to_string(), Value::Str("user".to_string()));
        fields.insert("content".to_string(), Value::Str(user.clone()));
        messages.push(Value::Object("Message".to_string(), fields));
    }

    Ok(Value::List(messages))
}

/// Format a prompt with sections
/// prompts.format(context: "...", instructions: "...", constraints: "...")
fn format_prompt(_args: &[Value], kwargs: &HashMap<String, Value>) -> Result<Value, RuntimeError> {
    let mut parts = Vec::new();

    if let Some(Value::Str(ctx)) = kwargs.get("context") {
        parts.push(format!("Context:\n{}", ctx));
    }
    if let Some(Value::Str(inst)) = kwargs.get("instructions") {
        parts.push(format!("Instructions:\n{}", inst));
    }
    if let Some(Value::Str(constraints)) = kwargs.get("constraints") {
        parts.push(format!("Constraints:\n{}", constraints));
    }
    if let Some(Value::Str(output)) = kwargs.get("output_format") {
        parts.push(format!("Output format:\n{}", output));
    }
    if let Some(Value::Str(input)) = kwargs.get("input") {
        parts.push(format!("Input:\n{}", input));
    }

    Ok(Value::Str(parts.join("\n\n")))
}
