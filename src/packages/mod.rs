//! Covenant Package System
//!
//! Resolves `use <name>` declarations to either:
//! - Built-in stdlib/tier2 modules (Rust-backed)
//! - File-based packages (.cov files in covenant_packages/)
//!
//! Search order for file packages:
//! 1. ./covenant_packages/<name>/
//! 2. ~/.covenant/packages/<name>/
//!
//! Built-in modules are always available — `use` is declarative for them.

use std::path::{Path, PathBuf};

use crate::ast::{Program, UseDecl};
use crate::lexer::Lexer;
use crate::parser::Parser;

/// All built-in module names (stdlib tier 1 + tier 2)
const BUILTIN_MODULES: &[&str] = &[
    // Tier 1 — stdlib
    "web", "data", "json", "file", "ai", "crypto", "time", "math", "text", "env",
    // Tier 2 — AI-age libraries
    "http", "anthropic", "openai", "ollama", "grok", "mcp", "mcpx",
    "embeddings", "prompts", "guardrails",
];

/// Check if a name is a built-in module
pub fn is_builtin_module(name: &str) -> bool {
    BUILTIN_MODULES.contains(&name)
}

/// A loaded file-based package
#[derive(Debug, Clone)]
pub struct Package {
    pub name: String,
    pub path: PathBuf,
    pub program: Program,
}

/// Resolve a use declaration to a package path, or None if it's a built-in
pub fn resolve_package(name: &str, source_dir: &Path) -> Option<PathBuf> {
    if is_builtin_module(name) {
        return None; // Built-in, no file to load
    }

    // Check ./covenant_packages/<name>/
    let local = source_dir.join("covenant_packages").join(name);
    if local.is_dir() {
        return Some(local);
    }

    // Check ~/.covenant/packages/<name>/
    if let Some(home) = dirs_home() {
        let global = home.join(".covenant").join("packages").join(name);
        if global.is_dir() {
            return Some(global);
        }
    }

    None
}

/// Load a file-based package from a directory
pub fn load_package(name: &str, pkg_dir: &Path) -> Result<Package, String> {
    // Find all .cov files in the package directory
    let mut cov_files: Vec<PathBuf> = Vec::new();

    // mod.cov is the main entry point (loaded first)
    let mod_file = pkg_dir.join("mod.cov");
    if mod_file.exists() {
        cov_files.push(mod_file);
    }

    // Then any other .cov files
    let entries = std::fs::read_dir(pkg_dir)
        .map_err(|e| format!("Cannot read package '{}': {}", name, e))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("Error reading package dir: {}", e))?;
        let path = entry.path();
        if path.extension().map(|e| e == "cov").unwrap_or(false)
            && path.file_name().map(|n| n != "mod.cov").unwrap_or(false)
        {
            cov_files.push(path);
        }
    }

    if cov_files.is_empty() {
        return Err(format!("Package '{}' has no .cov files", name));
    }

    // Parse all files and merge into one program
    let mut all_contracts = Vec::new();
    let mut all_type_defs = Vec::new();

    for cov_file in &cov_files {
        let source = std::fs::read_to_string(cov_file)
            .map_err(|e| format!("Cannot read {}: {}", cov_file.display(), e))?;
        let filename = cov_file.to_string_lossy().to_string();

        let tokens = Lexer::new(&source, &filename)
            .tokenize()
            .map_err(|e| format!("Lexer error in {}: {}", filename, e))?;

        let program = Parser::new(tokens, &filename)
            .parse()
            .map_err(|e| format!("Parse error in {}: {}", filename, e))?;

        all_contracts.extend(program.contracts);
        all_type_defs.extend(program.type_defs);
    }

    let program = Program {
        header: None,
        uses: Vec::new(),
        contracts: all_contracts,
        type_defs: all_type_defs,
        shared_decls: Vec::new(),
        loc: crate::ast::SourceLocation::new(&pkg_dir.to_string_lossy(), 0, 0),
    };

    Ok(Package {
        name: name.to_string(),
        path: pkg_dir.to_path_buf(),
        program,
    })
}

/// Resolve all use declarations for a program
pub fn resolve_uses(
    uses: &[UseDecl],
    source_dir: &Path,
) -> Result<Vec<Package>, String> {
    let mut packages = Vec::new();

    for use_decl in uses {
        let name = use_decl.alias.as_deref().unwrap_or(&use_decl.name);
        let _ = name; // alias is for the caller to use

        if is_builtin_module(&use_decl.name) {
            continue; // Built-in — no loading needed
        }

        match resolve_package(&use_decl.name, source_dir) {
            Some(pkg_dir) => {
                let pkg = load_package(&use_decl.name, &pkg_dir)?;
                packages.push(pkg);
            }
            None => {
                return Err(format!(
                    "Package '{}' not found. Install with: covenant add {}",
                    use_decl.name, use_decl.name
                ));
            }
        }
    }

    Ok(packages)
}

/// Get the user's home directory
fn dirs_home() -> Option<PathBuf> {
    std::env::var("HOME")
        .ok()
        .map(PathBuf::from)
}

/// List all installed packages
pub fn list_packages(source_dir: &Path) -> Vec<(String, PathBuf)> {
    let mut result = Vec::new();

    // Local packages
    let local = source_dir.join("covenant_packages");
    if let Ok(entries) = std::fs::read_dir(&local) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                if let Some(name) = entry.file_name().to_str() {
                    result.push((name.to_string(), entry.path()));
                }
            }
        }
    }

    // Global packages
    if let Some(home) = dirs_home() {
        let global = home.join(".covenant").join("packages");
        if let Ok(entries) = std::fs::read_dir(&global) {
            for entry in entries.flatten() {
                if entry.path().is_dir() {
                    if let Some(name) = entry.file_name().to_str() {
                        if !result.iter().any(|(n, _)| n == name) {
                            result.push((name.to_string(), entry.path()));
                        }
                    }
                }
            }
        }
    }

    result
}

/// Initialize a new Covenant project in the given directory
pub fn init_project(dir: &Path) -> Result<(), String> {
    let pkg_dir = dir.join("covenant_packages");
    std::fs::create_dir_all(&pkg_dir)
        .map_err(|e| format!("Cannot create covenant_packages/: {}", e))?;

    // Create a starter main.cov if none exists
    let main_file = dir.join("main.cov");
    if !main_file.exists() {
        let template = r#"intent: "My Covenant project"
scope: app.main
risk: low

contract main() -> Int
  precondition:
    true

  postcondition:
    result == 0

  body:
    print("Hello from Covenant!")
    return 0
"#;
        std::fs::write(&main_file, template)
            .map_err(|e| format!("Cannot create main.cov: {}", e))?;
    }

    Ok(())
}
