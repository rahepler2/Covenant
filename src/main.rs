use std::collections::HashMap;
use std::path::PathBuf;
use std::process;

use clap::{Parser as ClapParser, Subcommand};

use covenant_lang::ast::*;
use covenant_lang::lexer::Lexer;
use covenant_lang::parser::Parser;
use covenant_lang::runtime::{Interpreter, Value};
use covenant_lang::verify::checker::{verify_program, Severity};
use covenant_lang::verify::fingerprint::fingerprint_contract;
use covenant_lang::verify::hasher::compute_intent_hash;

#[derive(ClapParser)]
#[command(name = "covenant", version, about = "The Covenant programming language compiler")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Display the token stream (debug)
    Tokenize {
        /// Path to .cov file
        file: PathBuf,
    },
    /// Parse and display the AST
    Parse {
        /// Path to .cov file
        file: PathBuf,
    },
    /// Run Stage 1 verification (intent + effects)
    Check {
        /// Path to .cov file
        file: PathBuf,
    },
    /// Show behavioral fingerprints for all contracts
    Fingerprint {
        /// Path to .cov file
        file: PathBuf,
    },
    /// Execute a contract
    Run {
        /// Path to .cov file
        file: PathBuf,
        /// Contract name to execute (defaults to first contract)
        #[arg(short, long)]
        contract: Option<String>,
        /// Arguments as key=value pairs
        #[arg(short, long, value_parser = parse_arg)]
        arg: Vec<(String, String)>,
    },
}

fn parse_arg(s: &str) -> Result<(String, String), String> {
    let parts: Vec<&str> = s.splitn(2, '=').collect();
    if parts.len() != 2 {
        Err(format!("Invalid argument format '{}', expected key=value", s))
    } else {
        Ok((parts[0].to_string(), parts[1].to_string()))
    }
}

fn main() {
    let cli = Cli::parse();
    let exit_code = match cli.command {
        Commands::Tokenize { file } => cmd_tokenize(&file),
        Commands::Parse { file } => cmd_parse(&file),
        Commands::Check { file } => cmd_check(&file),
        Commands::Fingerprint { file } => cmd_fingerprint(&file),
        Commands::Run {
            file,
            contract,
            arg,
        } => cmd_run(&file, contract.as_deref(), &arg),
    };
    process::exit(exit_code);
}

const MAX_SOURCE_SIZE: u64 = 10 * 1024 * 1024; // 10 MB

fn read_source(path: &PathBuf) -> Result<(String, String), i32> {
    let filename = path.to_string_lossy().to_string();

    // Check file size before reading
    match std::fs::metadata(path) {
        Ok(meta) => {
            if meta.len() > MAX_SOURCE_SIZE {
                eprintln!(
                    "Error: file {} is too large ({} bytes, max {} bytes)",
                    filename,
                    meta.len(),
                    MAX_SOURCE_SIZE
                );
                return Err(1);
            }
        }
        Err(e) => {
            eprintln!("Error: cannot read file {}: {}", filename, e);
            return Err(1);
        }
    }

    match std::fs::read_to_string(path) {
        Ok(source) => Ok((source, filename)),
        Err(e) => {
            eprintln!("Error: cannot read file {}: {}", filename, e);
            Err(1)
        }
    }
}

fn lex_and_parse(path: &PathBuf) -> Result<(Program, String), i32> {
    let (source, filename) = read_source(path)?;

    let tokens = match Lexer::new(&source, &filename).tokenize() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Lexer error: {}", e);
            return Err(1);
        }
    };

    let program = match Parser::new(tokens, &filename).parse() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Parse error: {}", e);
            return Err(1);
        }
    };

    Ok((program, filename))
}

fn cmd_tokenize(path: &PathBuf) -> i32 {
    let (source, filename) = match read_source(path) {
        Ok(r) => r,
        Err(code) => return code,
    };

    let tokens = match Lexer::new(&source, &filename).tokenize() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Lexer error: {}", e);
            return 1;
        }
    };

    for tok in &tokens {
        println!("{}", tok);
    }
    0
}

fn cmd_parse(path: &PathBuf) -> i32 {
    let (program, _) = match lex_and_parse(path) {
        Ok(r) => r,
        Err(code) => return code,
    };

    print_program(&program);
    0
}

fn cmd_check(path: &PathBuf) -> i32 {
    let (program, filename) = match lex_and_parse(path) {
        Ok(r) => r,
        Err(code) => {
            eprintln!("FAIL");
            return code;
        }
    };

    let results = verify_program(&program, &filename);

    let errors: Vec<_> = results
        .iter()
        .filter(|r| matches!(r.severity, Severity::Error | Severity::Critical))
        .collect();
    let warnings: Vec<_> = results
        .iter()
        .filter(|r| matches!(r.severity, Severity::Warning))
        .collect();
    let infos: Vec<_> = results
        .iter()
        .filter(|r| matches!(r.severity, Severity::Info))
        .collect();

    for r in &errors {
        println!("  ERROR {}: {}", r.code, r.message);
    }
    for r in &warnings {
        println!("  WARN  {}: {}", r.code, r.message);
    }
    for r in &infos {
        println!("  INFO  {}: {}", r.code, r.message);
    }

    // Print intent hashes
    let intent_text = program
        .header
        .as_ref()
        .and_then(|h| h.intent.as_ref())
        .map(|i| i.text.as_str())
        .unwrap_or("");

    println!();
    for contract in &program.contracts {
        let fp = fingerprint_contract(contract);
        let ih = compute_intent_hash(contract, intent_text, Some(&fp));
        println!(
            "  {}: intent_hash={}...",
            contract.name,
            &ih.combined_hash[..16]
        );
    }

    println!();
    if !errors.is_empty() {
        println!(
            "{}: FAIL ({} error(s), {} warning(s))",
            filename,
            errors.len(),
            warnings.len()
        );
        1
    } else if !warnings.is_empty() {
        println!("{}: WARN ({} warning(s))", filename, warnings.len());
        0
    } else {
        println!("{}: OK", filename);
        0
    }
}

fn cmd_fingerprint(path: &PathBuf) -> i32 {
    let (program, _) = match lex_and_parse(path) {
        Ok(r) => r,
        Err(code) => return code,
    };

    let intent_text = program
        .header
        .as_ref()
        .and_then(|h| h.intent.as_ref())
        .map(|i| i.text.as_str())
        .unwrap_or("");

    for contract in &program.contracts {
        let fp = fingerprint_contract(contract);
        let ih = compute_intent_hash(contract, intent_text, Some(&fp));

        println!("Contract: {}", contract.name);
        println!(
            "  Reads:       {}",
            if fp.reads.is_empty() {
                "(none)".to_string()
            } else {
                format!("{:?}", fp.reads)
            }
        );
        println!(
            "  Mutations:   {}",
            if fp.mutations.is_empty() {
                "(none)".to_string()
            } else {
                format!("{:?}", fp.mutations)
            }
        );
        println!(
            "  Calls:       {}",
            if fp.calls.is_empty() {
                "(none)".to_string()
            } else {
                format!("{:?}", fp.calls)
            }
        );
        println!(
            "  Events:      {}",
            if fp.emitted_events.is_empty() {
                "(none)".to_string()
            } else {
                format!("{:?}", fp.emitted_events)
            }
        );
        println!(
            "  old() refs:  {}",
            if fp.old_references.is_empty() {
                "(none)".to_string()
            } else {
                format!("{:?}", fp.old_references)
            }
        );
        println!(
            "  Cap checks:  {}",
            if fp.capability_checks.is_empty() {
                "(none)".to_string()
            } else {
                format!("{:?}", fp.capability_checks)
            }
        );
        println!("  Branching:   {}", fp.has_branching);
        println!("  Looping:     {}", fp.has_looping);
        println!("  Recursion:   {}", fp.has_recursion);
        println!("  Returns:     {}", fp.return_count);
        println!("  Max depth:   {}", fp.max_nesting_depth);
        println!("  Intent hash: {}", ih.combined_hash);
        println!();
    }
    0
}

fn cmd_run(path: &PathBuf, contract_name: Option<&str>, args: &[(String, String)]) -> i32 {
    let (program, _) = match lex_and_parse(path) {
        Ok(r) => r,
        Err(code) => return code,
    };

    if program.contracts.is_empty() {
        eprintln!("Error: no contracts found in file");
        return 1;
    }

    let target_name = match contract_name {
        Some(name) => name.to_string(),
        None => program.contracts[0].name.clone(),
    };

    // Parse arguments
    let mut arg_values: HashMap<String, Value> = HashMap::new();
    for (key, val) in args {
        arg_values.insert(key.clone(), parse_value(val));
    }

    let mut interpreter = Interpreter::new();
    interpreter.register_contracts(&program);

    match interpreter.run_contract(&target_name, arg_values) {
        Ok(result) => {
            println!("{}", result);

            // Print emitted events
            let events = interpreter.emitted_events();
            if !events.is_empty() {
                println!("\nEmitted events:");
                for (name, args) in events {
                    let args_str: Vec<String> = args.iter().map(|v| format!("{}", v)).collect();
                    println!("  {} ({})", name, args_str.join(", "));
                }
            }
            0
        }
        Err(e) => {
            eprintln!("{}", e);
            1
        }
    }
}

fn parse_value(s: &str) -> Value {
    // Try integer
    if let Ok(n) = s.parse::<i64>() {
        return Value::Int(n);
    }
    // Try float
    if let Ok(n) = s.parse::<f64>() {
        return Value::Float(n);
    }
    // Try boolean
    match s {
        "true" => return Value::Bool(true),
        "false" => return Value::Bool(false),
        "null" => return Value::Null,
        _ => {}
    }
    // Try JSON object
    if s.starts_with('{') {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(s) {
            return json_to_value(&json);
        }
    }
    // Default to string
    Value::Str(s.to_string())
}

fn json_to_value(json: &serde_json::Value) -> Value {
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
            Value::List(arr.iter().map(json_to_value).collect())
        }
        serde_json::Value::Object(obj) => {
            let fields: HashMap<String, Value> =
                obj.iter().map(|(k, v)| (k.clone(), json_to_value(v))).collect();
            Value::Object("Object".to_string(), fields)
        }
    }
}

fn print_program(program: &Program) {
    if let Some(ref header) = program.header {
        if let Some(ref intent) = header.intent {
            println!("Intent: \"{}\"", intent.text);
        }
        if let Some(ref scope) = header.scope {
            println!("Scope:  {}", scope.path);
        }
        if let Some(ref risk) = header.risk {
            println!("Risk:   {}", risk.level);
        }
        if let Some(ref requires) = header.requires {
            println!("Requires: {}", requires.capabilities.join(", "));
        }
        println!();
    }

    for td in &program.type_defs {
        println!("Type: {} = {}", td.name, td.base_type);
        for f in &td.fields {
            println!("  field: {}: {}", f.name, f.type_expr.display_name());
        }
        for fc in &td.flow_constraints {
            println!("  flow: {:?}", fc);
        }
        println!();
    }

    for sd in &program.shared_decls {
        println!("Shared: {}: {}", sd.name, sd.type_name);
        println!(
            "  access: {}, isolation: {}, audit: {}",
            sd.access, sd.isolation, sd.audit
        );
        println!();
    }

    for c in &program.contracts {
        let params: Vec<String> = c
            .params
            .iter()
            .map(|p| format!("{}: {}", p.name, p.type_expr.display_name()))
            .collect();
        let ret = c
            .return_type
            .as_ref()
            .map(|t| t.display_name())
            .unwrap_or_else(|| "?".to_string());
        println!("Contract: {}({}) -> {}", c.name, params.join(", "), ret);
        if let Some(ref pre) = c.precondition {
            println!("  preconditions: {}", pre.conditions.len());
        }
        if let Some(ref post) = c.postcondition {
            println!("  postconditions: {}", post.conditions.len());
        }
        if let Some(ref effects) = c.effects {
            println!("  effects: {}", effects.declarations.len());
        }
        if c.permissions.is_some() {
            println!("  permissions: defined");
        }
        if let Some(ref body) = c.body {
            println!("  body: {} statement(s)", body.statements.len());
        }
        if let Some(ref on_failure) = c.on_failure {
            println!("  on_failure: {} statement(s)", on_failure.statements.len());
        }
        println!();
    }
}
