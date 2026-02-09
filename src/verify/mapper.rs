use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::ast::*;
use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::verify::fingerprint::fingerprint_contract;

/// Information about a single contract, extracted for the map.
#[derive(Debug, Clone)]
pub struct ContractInfo {
    pub name: String,
    pub scope: String,
    pub file: String,
    pub risk: String,
    pub params: Vec<String>,
    pub return_type: String,
    pub modifies: Vec<String>,
    pub reads: Vec<String>,
    pub emits: Vec<String>,
    pub calls: Vec<String>,
    pub has_shared_state: bool,
}

/// The full project map.
#[derive(Debug)]
pub struct ProjectMap {
    /// scope -> list of contracts
    pub scopes: BTreeMap<String, Vec<ContractInfo>>,
    /// contract name -> list of contracts it calls (cross-reference)
    pub call_graph: BTreeMap<String, BTreeSet<String>>,
    /// contract name -> list of contracts that call it
    pub callers: BTreeMap<String, BTreeSet<String>>,
    /// shared state name -> list of contracts that modify it
    pub shared_state_writers: BTreeMap<String, BTreeSet<String>>,
    /// shared state name -> list of contracts that read it
    pub shared_state_readers: BTreeMap<String, BTreeSet<String>>,
    /// All known contract names
    pub all_contracts: BTreeSet<String>,
}

/// Scan a directory for .cov files and build the project map.
pub fn build_project_map(dir: &Path) -> ProjectMap {
    let cov_files = find_cov_files(dir);
    let mut infos: Vec<ContractInfo> = Vec::new();

    for path in &cov_files {
        if let Some(file_infos) = parse_file_contracts(path) {
            infos.extend(file_infos);
        }
    }

    build_map_from_infos(infos)
}

/// Build map from a single file.
pub fn build_file_map(path: &Path) -> ProjectMap {
    let infos = parse_file_contracts(path).unwrap_or_default();
    build_map_from_infos(infos)
}

fn build_map_from_infos(infos: Vec<ContractInfo>) -> ProjectMap {
    let mut scopes: BTreeMap<String, Vec<ContractInfo>> = BTreeMap::new();
    let mut call_graph: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut callers: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut shared_state_writers: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut shared_state_readers: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut all_contracts: BTreeSet<String> = BTreeSet::new();

    for info in &infos {
        all_contracts.insert(info.name.clone());
        scopes
            .entry(info.scope.clone())
            .or_default()
            .push(info.clone());

        // Build call graph
        let mut outgoing = BTreeSet::new();
        for call in &info.calls {
            // Extract root function name from dotted paths
            let call_name = call.split('.').next().unwrap_or(call);
            outgoing.insert(call_name.to_string());
            callers
                .entry(call_name.to_string())
                .or_default()
                .insert(info.name.clone());
        }
        call_graph.insert(info.name.clone(), outgoing);

        // Track shared state access
        for m in &info.modifies {
            shared_state_writers
                .entry(m.clone())
                .or_default()
                .insert(info.name.clone());
        }
        for r in &info.reads {
            shared_state_readers
                .entry(r.clone())
                .or_default()
                .insert(info.name.clone());
        }
    }

    ProjectMap {
        scopes,
        call_graph,
        callers,
        shared_state_writers,
        shared_state_readers,
        all_contracts,
    }
}

/// Find all .cov files in a directory (recursively).
fn find_cov_files(dir: &Path) -> Vec<PathBuf> {
    let mut results = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                // Skip hidden dirs and covenant_packages
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                if !name.starts_with('.') && name != "target" && name != "covenant_packages" {
                    results.extend(find_cov_files(&path));
                }
            } else if path.extension().map_or(false, |e| e == "cov") {
                results.push(path);
            }
        }
    }
    results.sort();
    results
}

/// Parse a .cov file and extract contract info.
fn parse_file_contracts(path: &Path) -> Option<Vec<ContractInfo>> {
    let source = std::fs::read_to_string(path).ok()?;
    let filename = path.to_string_lossy().to_string();

    let tokens = Lexer::new(&source, &filename).tokenize().ok()?;
    let program = Parser::new(tokens, &filename).parse().ok()?;

    let scope = program
        .header
        .as_ref()
        .and_then(|h| h.scope.as_ref())
        .map(|s| s.path.clone())
        .unwrap_or_else(|| "(no scope)".to_string());

    let risk = program
        .header
        .as_ref()
        .and_then(|h| h.risk.as_ref())
        .map(|r| format!("{}", r.level))
        .unwrap_or_else(|| "unspecified".to_string());

    let mut infos = Vec::new();

    for contract in &program.contracts {
        let fp = fingerprint_contract(contract);

        let params: Vec<String> = contract
            .params
            .iter()
            .map(|p| format!("{}: {}", p.name, p.type_expr.display_name()))
            .collect();

        let return_type = contract
            .return_type
            .as_ref()
            .map(|t| t.display_name())
            .unwrap_or_else(|| "void".to_string());

        let modifies = extract_effect_targets(contract.effects.as_ref(), "modifies");
        let reads = extract_effect_targets(contract.effects.as_ref(), "reads");
        let emits = extract_effect_targets(contract.effects.as_ref(), "emits");

        let calls: Vec<String> = fp.calls.iter().cloned().collect();

        infos.push(ContractInfo {
            name: contract.name.clone(),
            scope: scope.clone(),
            file: filename.clone(),
            risk: risk.clone(),
            params,
            return_type,
            modifies,
            reads,
            emits,
            calls,
            has_shared_state: !program.shared_decls.is_empty(),
        });
    }

    Some(infos)
}

fn extract_effect_targets(effects: Option<&Effects>, kind: &str) -> Vec<String> {
    let mut result = Vec::new();
    if let Some(effects) = effects {
        for decl in &effects.declarations {
            match (kind, decl) {
                ("modifies", EffectDecl::Modifies { targets, .. }) => {
                    result.extend(targets.iter().cloned());
                }
                ("reads", EffectDecl::Reads { targets, .. }) => {
                    result.extend(targets.iter().cloned());
                }
                ("emits", EffectDecl::Emits { event_type, .. }) => {
                    result.push(event_type.clone());
                }
                _ => {}
            }
        }
    }
    result
}

// ── Output formatting ──────────────────────────────────────────────────────

/// Format the full project map as a string.
pub fn format_full_map(map: &ProjectMap) -> String {
    let mut out = String::new();
    out.push_str("=== Covenant Project Map ===\n\n");

    // Group by scope
    for (scope, contracts) in &map.scopes {
        let file = contracts.first().map(|c| c.file.as_str()).unwrap_or("?");
        let risk = contracts.first().map(|c| c.risk.as_str()).unwrap_or("?");
        out.push_str(&format!("[{}] {} (risk: {})\n", scope, file, risk));

        for c in contracts {
            let params_str = c.params.join(", ");
            out.push_str(&format!("  {} contract {}({}) -> {}\n", tree_char(true), c.name, params_str, c.return_type));

            let details: Vec<(&str, &Vec<String>)> = vec![
                ("MODIFIES", &c.modifies),
                ("READS", &c.reads),
                ("EMITS", &c.emits),
                ("CALLS", &c.calls),
            ];

            let non_empty: Vec<_> = details.iter().filter(|(_, v)| !v.is_empty()).collect();
            let last_idx = if non_empty.is_empty() { 0 } else { non_empty.len() - 1 };

            for (i, (label, values)) in non_empty.iter().enumerate() {
                let connector = if i == last_idx { end_char() } else { branch_char() };
                out.push_str(&format!("  {}   {} {}: {}\n", pipe_char(), connector, label, values.join(", ")));
            }

            if non_empty.is_empty() {
                out.push_str(&format!("  {}   {} (pure — no side effects declared)\n", pipe_char(), end_char()));
            }
        }
        out.push('\n');
    }

    // Cross-scope dependencies
    out.push_str("--- Cross-Scope Dependencies ---\n\n");
    let mut has_cross = false;
    for (scope, contracts) in &map.scopes {
        for c in contracts {
            for call in &c.calls {
                let call_root = call.split('.').next().unwrap_or(call);
                // Find which scope the called contract belongs to
                for (other_scope, other_contracts) in &map.scopes {
                    if other_scope != scope {
                        for oc in other_contracts {
                            if oc.name == call_root {
                                out.push_str(&format!(
                                    "  {} ({}) -> {} ({})\n",
                                    c.name, scope, oc.name, other_scope
                                ));
                                has_cross = true;
                            }
                        }
                    }
                }
            }
        }
    }
    if !has_cross {
        out.push_str("  (no cross-scope dependencies found)\n");
    }

    // Shared state contention
    out.push_str("\n--- Shared State ---\n\n");
    let mut has_shared = false;
    for (state, writers) in &map.shared_state_writers {
        if writers.len() > 1 {
            out.push_str(&format!("  {} modified by: {}\n", state, format_set(writers)));
            has_shared = true;
        }
    }
    // Show states that are both read and written
    for (state, writers) in &map.shared_state_writers {
        if let Some(readers) = map.shared_state_readers.get(state) {
            let both: Vec<_> = readers.difference(writers).cloned().collect();
            if !both.is_empty() {
                out.push_str(&format!(
                    "  {} written by [{}], read by [{}]\n",
                    state,
                    format_set(writers),
                    format_set(readers)
                ));
                has_shared = true;
            }
        }
    }
    if !has_shared {
        out.push_str("  (no shared state contention)\n");
    }

    out
}

/// Format the impact view for a specific contract.
/// If scope_hint is provided, prefer contracts in that scope for name resolution.
pub fn format_contract_impact(map: &ProjectMap, contract_name: &str) -> String {
    format_contract_impact_scoped(map, contract_name, None)
}

fn format_contract_impact_scoped(map: &ProjectMap, contract_name: &str, scope_hint: Option<&str>) -> String {
    let mut out = String::new();

    // Find the contract, preferring the hinted scope
    let target = find_contract_scoped(map, contract_name, scope_hint);

    let c = match target {
        Some(c) => c,
        None => {
            out.push_str(&format!("Error: contract '{}' not found\n", contract_name));
            return out;
        }
    };

    out.push_str(&format!("=== Impact: {} ===\n\n", c.name));
    out.push_str(&format!("  Scope: {}\n", c.scope));
    out.push_str(&format!("  File:  {}\n", c.file));
    out.push_str(&format!("  Risk:  {}\n", c.risk));
    out.push_str(&format!("  Signature: {}({}) -> {}\n", c.name, c.params.join(", "), c.return_type));

    // Direct effects
    out.push_str("\n  Direct Effects:\n");
    if !c.modifies.is_empty() {
        out.push_str(&format!("    MODIFIES: {}\n", c.modifies.join(", ")));
    }
    if !c.reads.is_empty() {
        out.push_str(&format!("    READS:    {}\n", c.reads.join(", ")));
    }
    if !c.emits.is_empty() {
        out.push_str(&format!("    EMITS:    {}\n", c.emits.join(", ")));
    }
    if c.modifies.is_empty() && c.reads.is_empty() && c.emits.is_empty() {
        out.push_str("    (pure — no side effects declared)\n");
    }

    // Calls (first level)
    out.push_str("\n  Calls (Level 1):\n");
    if c.calls.is_empty() {
        out.push_str("    (none)\n");
    } else {
        for call in &c.calls {
            let call_root = call.split('.').next().unwrap_or(call);
            // Find info about the called contract (prefer same scope)
            let called = find_contract_scoped(map, call_root, scope_hint);
            match called {
                Some(ci) => {
                    out.push_str(&format!("    {} {} [{}]\n", branch_char(), call, ci.scope));
                    if !ci.modifies.is_empty() {
                        out.push_str(&format!("    {}   MODIFIES: {}\n", pipe_char(), ci.modifies.join(", ")));
                    }
                    if !ci.emits.is_empty() {
                        out.push_str(&format!("    {}   EMITS: {}\n", pipe_char(), ci.emits.join(", ")));
                    }

                    // Second level calls
                    if !ci.calls.is_empty() {
                        let filtered: Vec<_> = ci.calls.iter()
                            .filter(|c2| *c2 != &c.name) // avoid self-recursion in display
                            .collect();
                        if !filtered.is_empty() {
                            out.push_str(&format!("    {}   Calls (Level 2): {}\n", pipe_char(), filtered.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")));
                        }
                    }
                }
                None => {
                    out.push_str(&format!("    {} {} (external/builtin)\n", branch_char(), call));
                }
            }
        }
    }

    // Called by
    out.push_str("\n  Called By:\n");
    let called_by = map.callers.get(contract_name);
    match called_by {
        Some(set) if !set.is_empty() => {
            for caller in set {
                let ci = find_contract_info(map, caller);
                let scope = ci.map(|c| c.scope.as_str()).unwrap_or("?");
                out.push_str(&format!("    {} {} [{}]\n", branch_char(), caller, scope));
            }
        }
        _ => {
            out.push_str("    (none — this is a root entry point or unused)\n");
        }
    }

    // Shared state impact
    out.push_str("\n  Shared State Impact:\n");
    let mut has_impact = false;
    for m in &c.modifies {
        if let Some(readers) = map.shared_state_readers.get(m) {
            let others: Vec<_> = readers.iter().filter(|r| *r != &c.name).collect();
            if !others.is_empty() {
                out.push_str(&format!(
                    "    {} modifies '{}' which is read by: {}\n",
                    branch_char(),
                    m,
                    others.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")
                ));
                has_impact = true;
            }
        }
        if let Some(writers) = map.shared_state_writers.get(m) {
            let others: Vec<_> = writers.iter().filter(|w| *w != &c.name).collect();
            if !others.is_empty() {
                out.push_str(&format!(
                    "    {} modifies '{}' which is also modified by: {}\n",
                    branch_char(),
                    m,
                    others.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")
                ));
                has_impact = true;
            }
        }
    }
    if !has_impact {
        out.push_str("    (no shared state contention)\n");
    }

    out
}

/// Format the impact view for a specific file (all contracts in it).
pub fn format_file_impact(map: &ProjectMap, file_path: &str) -> String {
    let mut out = String::new();

    let mut found_contracts: Vec<&ContractInfo> = Vec::new();
    for contracts in map.scopes.values() {
        for c in contracts {
            if c.file == file_path || c.file.ends_with(file_path) {
                found_contracts.push(c);
            }
        }
    }

    if found_contracts.is_empty() {
        out.push_str(&format!("Error: no contracts found in '{}'\n", file_path));
        return out;
    }

    let scope = found_contracts.first().map(|c| c.scope.as_str()).unwrap_or("?");
    out.push_str(&format!("=== File Impact: {} [{}] ===\n\n", file_path, scope));
    out.push_str(&format!("  Contracts: {}\n\n", found_contracts.len()));

    for c in &found_contracts {
        out.push_str(&format_contract_impact_scoped(map, &c.name, Some(&c.scope)));
        out.push('\n');
    }

    out
}

fn find_contract_info<'a>(map: &'a ProjectMap, name: &str) -> Option<&'a ContractInfo> {
    find_contract_scoped(map, name, None)
}

/// Find a contract by name. If scope_hint is provided, prefer contracts in that scope.
fn find_contract_scoped<'a>(map: &'a ProjectMap, name: &str, scope_hint: Option<&str>) -> Option<&'a ContractInfo> {
    // First, try to find in the hinted scope
    if let Some(scope) = scope_hint {
        if let Some(contracts) = map.scopes.get(scope) {
            for c in contracts {
                if c.name == name {
                    return Some(c);
                }
            }
        }
    }

    // Fall back to global search
    for contracts in map.scopes.values() {
        for c in contracts {
            if c.name == name {
                return Some(c);
            }
        }
    }
    None
}

fn format_set(set: &BTreeSet<String>) -> String {
    set.iter().cloned().collect::<Vec<_>>().join(", ")
}

// Tree-drawing characters
fn tree_char(_is_last: bool) -> &'static str { "├──" }
fn branch_char() -> &'static str { "├──" }
fn end_char() -> &'static str { "└──" }
fn pipe_char() -> &'static str { "│" }
