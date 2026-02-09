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
use covenant_lang::verify::mapper;
use covenant_lang::verify::type_check;
use covenant_lang::serve;
use covenant_lang::vm::compiler::Compiler as BytecodeCompiler;
use covenant_lang::vm::bytecode::Module as BytecodeModule;
use covenant_lang::vm::machine::VM;

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
    /// Execute a contract (via bytecode VM)
    Run {
        /// Path to .cov file
        file: PathBuf,
        /// Contract name to execute (defaults to first contract)
        #[arg(short, long)]
        contract: Option<String>,
        /// Arguments as key=value pairs
        #[arg(short, long, value_parser = parse_arg)]
        arg: Vec<(String, String)>,
        /// Use tree-walking interpreter instead of VM
        #[arg(long)]
        interpret: bool,
    },
    /// Compile .cov to bytecode (.covc)
    Build {
        /// Path to .cov file
        file: PathBuf,
        /// Output path (defaults to <input>.covc)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Execute pre-compiled bytecode (.covc)
    Exec {
        /// Path to .covc file
        file: PathBuf,
        /// Contract name to execute (defaults to first contract)
        #[arg(short, long)]
        contract: Option<String>,
        /// Arguments as key=value pairs
        #[arg(short, long, value_parser = parse_arg)]
        arg: Vec<(String, String)>,
    },
    /// Disassemble a .cov file (show bytecode)
    Disasm {
        /// Path to .cov file
        file: PathBuf,
    },
    /// Install a package
    Add {
        /// Package name
        name: String,
        /// Install globally instead of locally
        #[arg(long)]
        global: bool,
    },
    /// Initialize a new Covenant project
    Init,
    /// List installed packages
    Packages,
    /// Start HTTP server mapping contracts to API endpoints
    Serve {
        /// .cov files to serve (or directory to scan)
        #[arg(default_value = ".")]
        files: Vec<PathBuf>,
        /// Port to listen on
        #[arg(short, long, default_value = "8080")]
        port: u16,
        /// Host to bind to
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        /// Directory to serve static files from
        #[arg(short, long)]
        static_dir: Option<PathBuf>,
        /// API route prefix
        #[arg(long, default_value = "/api")]
        prefix: String,
    },
    /// Show impact map of contracts, scopes, and dependencies
    Map {
        /// Directory to scan (defaults to current directory)
        #[arg(default_value = ".")]
        dir: PathBuf,
        /// Show impact for a specific contract
        #[arg(short, long)]
        contract: Option<String>,
        /// Show impact for a specific file
        #[arg(short, long)]
        file: Option<PathBuf>,
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
            interpret,
        } => {
            if interpret {
                cmd_run_interpret(&file, contract.as_deref(), &arg)
            } else {
                cmd_run(&file, contract.as_deref(), &arg)
            }
        }
        Commands::Build { file, output } => cmd_build(&file, output.as_deref()),
        Commands::Exec {
            file,
            contract,
            arg,
        } => cmd_exec(&file, contract.as_deref(), &arg),
        Commands::Disasm { file } => cmd_disasm(&file),
        Commands::Add { name, global } => cmd_add(&name, global),
        Commands::Serve {
            files,
            port,
            host,
            static_dir,
            prefix,
        } => cmd_serve(&files, port, &host, static_dir.as_deref(), &prefix),
        Commands::Init => cmd_init(),
        Commands::Packages => cmd_packages(),
        Commands::Map { dir, contract, file } => cmd_map(&dir, contract.as_deref(), file.as_deref()),
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

    // Run static type checker
    let type_warnings = type_check::check_types(&program);

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
    for tw in &type_warnings {
        println!("  TYPE  {}: {} (line {})", tw.code, tw.message, tw.line);
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
    let total_errors = errors.len();
    let total_warnings = warnings.len() + type_warnings.len();
    if total_errors > 0 {
        println!(
            "{}: FAIL ({} error(s), {} warning(s))",
            filename, total_errors, total_warnings
        );
        1
    } else if total_warnings > 0 {
        println!("{}: WARN ({} warning(s))", filename, total_warnings);
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
        None => program.contracts.iter()
            .find(|c| c.name == "main")
            .unwrap_or(&program.contracts[0])
            .name.clone(),
    };

    // Parse arguments
    let mut arg_values: HashMap<String, Value> = HashMap::new();
    for (key, val) in args {
        arg_values.insert(key.clone(), parse_value(val));
    }

    // Compile to bytecode then execute on VM
    let mut compiler = BytecodeCompiler::new();
    let module = compiler.compile(&program);
    let mut vm = VM::new(module);

    match vm.run_contract(&target_name, arg_values) {
        Ok(result) => {
            println!("{}", result);

            let events = vm.emitted_events();
            if !events.is_empty() {
                println!("\nEmitted events:");
                for (name, event_args) in events {
                    let args_str: Vec<String> = event_args.iter().map(|v| format!("{}", v)).collect();
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

fn cmd_run_interpret(path: &PathBuf, contract_name: Option<&str>, args: &[(String, String)]) -> i32 {
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
        None => program.contracts.iter()
            .find(|c| c.name == "main")
            .unwrap_or(&program.contracts[0])
            .name.clone(),
    };

    let mut arg_values: HashMap<String, Value> = HashMap::new();
    for (key, val) in args {
        arg_values.insert(key.clone(), parse_value(val));
    }

    let mut interpreter = Interpreter::new();
    interpreter.register_contracts(&program);

    match interpreter.run_contract(&target_name, arg_values) {
        Ok(result) => {
            println!("{}", result);

            let events = interpreter.emitted_events();
            if !events.is_empty() {
                println!("\nEmitted events:");
                for (name, event_args) in events {
                    let args_str: Vec<String> = event_args.iter().map(|v| format!("{}", v)).collect();
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

fn cmd_build(path: &PathBuf, output: Option<&std::path::Path>) -> i32 {
    let (program, _) = match lex_and_parse(path) {
        Ok(r) => r,
        Err(code) => return code,
    };

    let mut compiler = BytecodeCompiler::new();
    let module = compiler.compile(&program);
    let bytes = module.serialize();

    let out_path = match output {
        Some(p) => p.to_path_buf(),
        None => path.with_extension("covc"),
    };

    match std::fs::write(&out_path, &bytes) {
        Ok(_) => {
            println!(
                "Compiled {} -> {} ({} bytes, {} contracts)",
                path.display(),
                out_path.display(),
                bytes.len(),
                module.contracts.len()
            );
            0
        }
        Err(e) => {
            eprintln!("Error writing {}: {}", out_path.display(), e);
            1
        }
    }
}

fn cmd_exec(path: &PathBuf, contract_name: Option<&str>, args: &[(String, String)]) -> i32 {
    let data = match std::fs::read(path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Error reading {}: {}", path.display(), e);
            return 1;
        }
    };

    let module = match BytecodeModule::deserialize(&data) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Error loading bytecode: {}", e);
            return 1;
        }
    };

    if module.contracts.is_empty() {
        eprintln!("Error: no contracts found in bytecode");
        return 1;
    }

    let target_name = match contract_name {
        Some(name) => name.to_string(),
        None => module.contracts.iter()
            .find(|c| c.name == "main")
            .unwrap_or(&module.contracts[0])
            .name.clone(),
    };

    let mut arg_values: HashMap<String, Value> = HashMap::new();
    for (key, val) in args {
        arg_values.insert(key.clone(), parse_value(val));
    }

    let mut vm = VM::new(module);

    match vm.run_contract(&target_name, arg_values) {
        Ok(result) => {
            println!("{}", result);

            let events = vm.emitted_events();
            if !events.is_empty() {
                println!("\nEmitted events:");
                for (name, event_args) in events {
                    let args_str: Vec<String> = event_args.iter().map(|v| format!("{}", v)).collect();
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

fn cmd_disasm(path: &PathBuf) -> i32 {
    let (program, _) = match lex_and_parse(path) {
        Ok(r) => r,
        Err(code) => return code,
    };

    let mut compiler = BytecodeCompiler::new();
    let module = compiler.compile(&program);

    println!("=== Constants ({}) ===", module.constants.len());
    for (i, c) in module.constants.iter().enumerate() {
        println!("  c{}: {}", i, c);
    }

    println!();
    for contract in &module.contracts {
        println!(
            "=== {} ({} locals, {} instructions) ===",
            contract.name,
            contract.local_count,
            contract.code.len()
        );
        println!("  params: {:?}", contract.params);
        println!("  locals: {:?}", contract.local_names);
        println!();
        for (i, inst) in contract.code.iter().enumerate() {
            println!("  {:4}: {}", i, inst);
        }
        println!();
    }
    0
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

fn cmd_serve(
    files: &[PathBuf],
    port: u16,
    host: &str,
    static_dir: Option<&std::path::Path>,
    prefix: &str,
) -> i32 {
    // Collect .cov files
    let mut cov_files: Vec<PathBuf> = Vec::new();

    for path in files {
        if path.is_dir() {
            // Scan directory for .cov files
            if let Ok(entries) = std::fs::read_dir(path) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.extension().and_then(|e| e.to_str()) == Some("cov") {
                        cov_files.push(p);
                    }
                }
            }
        } else if path.extension().and_then(|e| e.to_str()) == Some("cov") {
            cov_files.push(path.clone());
        }
    }

    if cov_files.is_empty() {
        eprintln!("Error: no .cov files found");
        return 1;
    }

    let config = serve::ServeConfig {
        port,
        host: host.to_string(),
        static_dir: static_dir.map(|p| p.to_path_buf()),
        api_prefix: prefix.to_string(),
    };

    match serve::start_server(&cov_files, &config) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("Server error: {}", e);
            1
        }
    }
}

fn cmd_add(name: &str, global: bool) -> i32 {
    use covenant_lang::packages;

    // Check if it's a built-in module
    if packages::is_builtin_module(name) {
        println!("'{}' is a built-in module — no installation needed. Just use: {}.method()", name, name);
        return 0;
    }

    // Determine install directory
    let install_dir = if global {
        match std::env::var("HOME") {
            Ok(home) => PathBuf::from(home).join(".covenant").join("packages").join(name),
            Err(_) => {
                eprintln!("Error: cannot determine home directory");
                return 1;
            }
        }
    } else {
        PathBuf::from("covenant_packages").join(name)
    };

    if install_dir.exists() {
        println!("Package '{}' already installed at {}", name, install_dir.display());
        return 0;
    }

    // Create package directory with a starter mod.cov
    if let Err(e) = std::fs::create_dir_all(&install_dir) {
        eprintln!("Error creating {}: {}", install_dir.display(), e);
        return 1;
    }

    let mod_template = format!(
        r#"-- {} package for Covenant
intent: "Package: {}"
scope: packages.{}
risk: low

contract hello() -> String
  precondition:
    true

  postcondition:
    result != ""

  body:
    return "Hello from {} package!"
"#,
        name, name, name, name
    );

    let mod_path = install_dir.join("mod.cov");
    if let Err(e) = std::fs::write(&mod_path, mod_template) {
        eprintln!("Error writing {}: {}", mod_path.display(), e);
        return 1;
    }

    println!("Installed package '{}' at {}", name, install_dir.display());
    println!("Edit {} to add contracts", mod_path.display());
    0
}

fn cmd_init() -> i32 {
    use covenant_lang::packages;

    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    match packages::init_project(&cwd) {
        Ok(_) => {
            println!("Initialized Covenant project in {}", cwd.display());
            if cwd.join("main.cov").exists() {
                println!("Created main.cov — run with: covenant run main.cov -c main");
            }
            println!("Created covenant_packages/ for local packages");
            0
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            1
        }
    }
}

fn cmd_packages() -> i32 {
    use covenant_lang::packages;

    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    println!("Built-in modules (always available):");
    println!("  Tier 1: web, data, json, file, ai, crypto, time, math, text, env, db");
    println!("  Tier 2: http, anthropic, openai, ollama, grok, mcp, mcpx, embeddings, prompts, guardrails");
    println!();

    let installed = packages::list_packages(&cwd);
    if installed.is_empty() {
        println!("No file-based packages installed.");
    } else {
        println!("Installed packages:");
        for (name, path) in &installed {
            println!("  {} ({})", name, path.display());
        }
    }

    0
}

fn cmd_map(dir: &PathBuf, contract: Option<&str>, file: Option<&std::path::Path>) -> i32 {
    let map = if let Some(file_path) = file {
        // Scan a specific file + the project directory for cross-references
        let project_map = mapper::build_project_map(dir);
        // If file not in project dir, also scan it
        let file_str = file_path.to_string_lossy().to_string();
        print!("{}", mapper::format_file_impact(&project_map, &file_str));
        return 0;
    } else {
        mapper::build_project_map(dir)
    };

    if let Some(contract_name) = contract {
        // Targeted: show impact for a specific contract
        print!("{}", mapper::format_contract_impact(&map, contract_name));
    } else {
        // Full project map
        print!("{}", mapper::format_full_map(&map));
    }
    0
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
