use std::collections::HashSet;

use crate::ast::*;
use crate::verify::fingerprint::{fingerprint_contract, fingerprint_expressions, BehavioralFingerprint};
use crate::runtime::stdlib;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Info,
    Warning,
    Error,
    Critical,
}

#[derive(Debug, Clone)]
pub struct VerificationResult {
    pub severity: Severity,
    pub code: String,
    pub message: String,
    pub contract_name: String,
    pub file: String,
    pub line: usize,
}

impl std::fmt::Display for VerificationResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let sev = match self.severity {
            Severity::Info => "INFO",
            Severity::Warning => "WARNING",
            Severity::Error => "ERROR",
            Severity::Critical => "CRITICAL",
        };
        write!(
            f,
            "[{}] {}: {}:{}: contract '{}': {}",
            sev, self.code, self.file, self.line, self.contract_name, self.message
        )
    }
}

pub fn verify_contract(
    contract: &ContractDef,
    fingerprint: Option<&BehavioralFingerprint>,
    file: &str,
    declared_capabilities: Option<&[String]>,
    risk_level: RiskLevel,
    sibling_contracts: Option<&[String]>,
) -> Vec<VerificationResult> {
    let owned_fp;
    let fp = match fingerprint {
        Some(f) => f,
        None => {
            owned_fp = fingerprint_contract(contract);
            &owned_fp
        }
    };

    let mut results = Vec::new();
    let line = contract.loc.line;
    let name = &contract.name;

    let mut add = |severity: Severity, code: &str, message: String| {
        results.push(VerificationResult {
            severity,
            code: code.to_string(),
            message,
            contract_name: name.clone(),
            file: file.to_string(),
            line,
        });
    };

    // -- Structural completeness --

    if contract.body.is_none() {
        add(Severity::Error, "E004", "contract has no body".to_string());
        return results;
    }

    // At high/critical risk, missing sections are always errors.
    if contract.precondition.is_none() && matches!(risk_level, RiskLevel::High | RiskLevel::Critical) {
        add(
            Severity::Error,
            "W003",
            format!(
                "no precondition — required at {} risk. Add:\n  precondition:\n           <your input constraints here>\n  \
                 Or lower the file risk level if this contract doesn't need input validation.",
                risk_level
            ),
        );
    }

    if contract.postcondition.is_none() && matches!(risk_level, RiskLevel::High | RiskLevel::Critical) {
        add(
            Severity::Error,
            "W004",
            format!(
                "no postcondition — required at {} risk. Add:\n  postcondition:\n           <your output guarantees here>\n  \
                 Or lower the file risk level if this contract doesn't need output guarantees.",
                risk_level
            ),
        );
    }

    if contract.effects.is_none() && matches!(risk_level, RiskLevel::High | RiskLevel::Critical) {
        add(
            Severity::Error,
            "W005",
            format!(
                "no effects declaration — required at {} risk. Add:\n  effects:\n           touches_nothing_else\n  \
                 Or declare specific effects: modifies [...], emits Event, reads [...].",
                risk_level
            ),
        );
    }

    // Auto-escalation: at ANY risk level, if the body has external side effects
    // (mutations to dotted paths, emitted events) but no effects declaration,
    // require the developer to declare them. Pure/self-contained code is fine
    // without declarations, but code that impacts other contracts must be explicit.
    if contract.effects.is_none() && !matches!(risk_level, RiskLevel::High | RiskLevel::Critical) {
        let external_mutations: Vec<_> = fp.mutations.iter()
            .filter(|m| m.contains('.'))
            .collect();
        let has_emits = !fp.emitted_events.is_empty();

        if !external_mutations.is_empty() || has_emits {
            let mut reasons = Vec::new();
            if !external_mutations.is_empty() {
                reasons.push(format!(
                    "mutates {}",
                    external_mutations.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")
                ));
            }
            if has_emits {
                reasons.push(format!(
                    "emits {}",
                    fp.emitted_events.iter().cloned().collect::<Vec<_>>().join(", ")
                ));
            }

            // Build a suggested fix showing the exact effects block they need
            let mut fix_lines = Vec::new();
            if !external_mutations.is_empty() {
                let mod_targets: Vec<&str> = external_mutations.iter()
                    .map(|m| m.as_str())
                    .collect();
                fix_lines.push(format!("modifies [{}]", mod_targets.join(", ")));
            }
            if has_emits {
                for ev in &fp.emitted_events {
                    fix_lines.push(format!("emits {}", ev));
                }
            }
            let fix_block: String = fix_lines.iter()
                .map(|l| format!("           {}", l))
                .collect::<Vec<_>>()
                .join("\n");

            add(
                Severity::Error,
                "W005",
                format!(
                    "contract '{}' has external side effects ({}) but no effects: block. \
                     Add:\n  effects:\n{}\n  \
                     Or mark the contract `pure` if it should have no side effects.",
                    name, reasons.join("; "), fix_block,
                ),
            );
        }
    }

    // -- Effect completeness (E001/E002) --

    let declared_modifies = extract_declared_modifies(contract.effects.as_ref());
    let declared_reads = extract_declared_reads(contract.effects.as_ref());
    let declared_emits = extract_declared_emits(contract.effects.as_ref());
    let has_touches_nothing = has_touches_nothing_else(contract.effects.as_ref());

    for mutation in &fp.mutations {
        if !is_covered_by(mutation, &declared_modifies) {
            if has_touches_nothing {
                add(
                    Severity::Error,
                    "E002",
                    format!(
                        "touches_nothing_else violated: body mutates '{}' which is not in the modifies declaration",
                        mutation
                    ),
                );
            } else {
                add(
                    Severity::Warning,
                    "E001",
                    format!(
                        "body mutates '{}' but it is not listed in the effects modifies declaration",
                        mutation
                    ),
                );
            }
        }
    }

    // -- Effect soundness (W001) --
    // Check declared mutations against both direct assignments (fp.mutations)
    // and method calls that target the same root (heuristic: calling obj.method()
    // when modifies [obj.field] is declared counts as an indirect mutation)

    for declared in &declared_modifies {
        if !is_observed_in(declared, &fp.mutations)
            && !is_covered_by_call(declared, &fp.calls) {
            add(
                Severity::Warning,
                "W001",
                format!(
                    "effects declares modifies '{}' but the body does not appear to mutate it",
                    declared
                ),
            );
        }
    }

    // -- Emit completeness (E005) --

    for event in &fp.emitted_events {
        if !declared_emits.contains(event) {
            let sev = if has_touches_nothing {
                Severity::Error
            } else {
                Severity::Warning
            };
            add(
                sev,
                "E005",
                format!(
                    "body emits '{}' but it is not declared in the effects block",
                    event
                ),
            );
        }
    }

    // -- Emit soundness (W002) --

    for declared_event in &declared_emits {
        if !fp.emitted_events.contains(declared_event) {
            add(
                Severity::Warning,
                "W002",
                format!(
                    "effects declares emits '{}' but the body does not emit it",
                    declared_event
                ),
            );
        }
    }

    // -- touches_nothing_else (E003) --

    if has_touches_nothing {
        let mut allowed_call_prefixes: HashSet<String> = HashSet::new();
        for m in &declared_modifies {
            if let Some(root) = m.split('.').next() {
                allowed_call_prefixes.insert(root.to_string());
            }
        }
        for r in &declared_reads {
            if let Some(root) = r.split('.').next() {
                allowed_call_prefixes.insert(root.to_string());
            }
        }
        for param in &contract.params {
            allowed_call_prefixes.insert(param.name.clone());
        }
        if let Some(caps) = declared_capabilities {
            for cap in caps {
                if let Some(root) = cap.split('.').next() {
                    allowed_call_prefixes.insert(root.to_string());
                }
            }
        }
        // Stdlib modules are always safe to call
        for module in stdlib::STDLIB_MODULE_NAMES {
            allowed_call_prefixes.insert(module.to_string());
        }
        // Built-in functions
        allowed_call_prefixes.insert("print".to_string());
        allowed_call_prefixes.insert("len".to_string());
        allowed_call_prefixes.insert("range".to_string());
        allowed_call_prefixes.insert("str".to_string());
        allowed_call_prefixes.insert("int".to_string());
        allowed_call_prefixes.insert("float".to_string());
        // Contracts defined in the same file (local helpers)
        if let Some(siblings) = sibling_contracts {
            for name in siblings {
                allowed_call_prefixes.insert(name.clone());
            }
        }

        for call in &fp.calls {
            let root = call.split('.').next().unwrap_or("");
            if allowed_call_prefixes.contains(root) {
                continue;
            }
            if !root.is_empty() && root.chars().next().map_or(false, |c| c.is_uppercase()) {
                continue;
            }
            if fp.mutations.contains(root) {
                continue;
            }
            add(
                Severity::Error,
                "E003",
                format!(
                    "touches_nothing_else violated: body calls '{}' which is not covered by declared effects or parameters",
                    call
                ),
            );
        }
    }

    // -- Precondition relevance (W006) --

    if let Some(ref precondition) = contract.precondition {
        let param_names: HashSet<String> = contract.params.iter().map(|p| p.name.clone()).collect();
        let precond_fp = fingerprint_expressions(&precondition.conditions);
        let mut body_roots: HashSet<String> = HashSet::new();
        for r in &fp.reads {
            if let Some(root) = r.split('.').next() {
                body_roots.insert(root.to_string());
            }
        }
        for m in &fp.mutations {
            if let Some(root) = m.split('.').next() {
                body_roots.insert(root.to_string());
            }
        }

        for read in &precond_fp.reads {
            let root = read.split('.').next().unwrap_or("");
            if !root.is_empty() && root.chars().next().map_or(false, |c| c.is_uppercase()) {
                continue;
            }
            if !param_names.contains(root) && !body_roots.contains(root) {
                add(
                    Severity::Warning,
                    "W006",
                    format!(
                        "precondition references '{}' which is not a parameter and not used in the body",
                        read
                    ),
                );
            }
        }
    }

    // -- Postcondition achievability (W007) --

    if let Some(ref postcondition) = contract.postcondition {
        let postcond_fp = fingerprint_expressions(&postcondition.conditions);
        for old_ref in &postcond_fp.old_references {
            if !is_mutation_covered(old_ref, &fp.mutations)
                && !is_covered_by_call(old_ref, &fp.calls) {
                add(
                    Severity::Warning,
                    "W007",
                    format!(
                        "postcondition uses old({}) but the body does not appear to modify '{}'",
                        old_ref, old_ref
                    ),
                );
            }
        }
    }

    // -- Intent scope (W008) --

    if let Some(caps) = declared_capabilities {
        let cap_roots: HashSet<String> = caps
            .iter()
            .filter_map(|c| c.split('.').next().map(String::from))
            .collect();
        let param_names: HashSet<String> = contract.params.iter().map(|p| p.name.clone()).collect();

        for check in &fp.capability_checks {
            let parts: Vec<&str> = check.split(" has ").collect();
            if parts.len() == 2 {
                let cap_path = parts[1];
                let cap_root = cap_path.split('.').next().unwrap_or("");
                if !cap_roots.contains(cap_root) && !param_names.contains(cap_root) {
                    add(
                        Severity::Warning,
                        "W008",
                        format!(
                            "body checks capability '{}' but the file header only requires: {:?}",
                            cap_path, caps
                        ),
                    );
                }
            }
        }
    }

    // -- Informational --

    if fp.has_recursion {
        add(
            Severity::Info,
            "I001",
            "contract contains recursive self-calls".to_string(),
        );
    }

    if fp.max_nesting_depth >= 4 {
        add(
            Severity::Info,
            "I002",
            format!(
                "contract has nesting depth {} — consider simplifying for auditability",
                fp.max_nesting_depth
            ),
        );
    }

    results
}

pub fn verify_program(program: &Program, file: &str) -> Vec<VerificationResult> {
    let mut results = Vec::new();

    let risk_level = program
        .header
        .as_ref()
        .and_then(|h| h.risk.as_ref())
        .map(|r| r.level)
        .unwrap_or(RiskLevel::Low);

    let declared_capabilities: Option<Vec<String>> = program
        .header
        .as_ref()
        .and_then(|h| h.requires.as_ref())
        .map(|r| r.capabilities.clone());

    // Phase 0: Scope namespace validation (S001-S003)
    results.extend(verify_scope(program, file));

    // Phase 2: Intent Verification Engine (E001-E005, W001-W008, I001-I002)
    let sibling_names: Vec<String> = program.contracts.iter().map(|c| c.name.clone()).collect();
    for contract in &program.contracts {
        let fp = fingerprint_contract(contract);
        results.extend(verify_contract(
            contract,
            Some(&fp),
            file,
            declared_capabilities.as_deref(),
            risk_level,
            Some(&sibling_names),
        ));
    }

    // Phase 3: Capability Type System / IFC (F001-F006)
    results.extend(super::capability::verify_capabilities(program, file));

    // Phase 4: Contract Verification (V001-V005)
    results.extend(super::contract_verify::verify_contracts(program, file));

    results
}

/// Verify scope namespace requirements (S001-S003)
fn verify_scope(program: &Program, file: &str) -> Vec<VerificationResult> {
    let mut results = Vec::new();
    let line = program
        .header
        .as_ref()
        .map(|h| h.loc.line)
        .unwrap_or(1);

    // S001: scope is required
    let scope_path = match program
        .header
        .as_ref()
        .and_then(|h| h.scope.as_ref())
    {
        Some(s) => &s.path,
        None => {
            results.push(VerificationResult {
                severity: Severity::Error,
                code: "S001".to_string(),
                message: "missing scope declaration — every file must declare scope: domain.module"
                    .to_string(),
                contract_name: "(file)".to_string(),
                file: file.to_string(),
                line,
            });
            return results;
        }
    };

    // S002: scope must have at least 2 segments (domain.module)
    let segments: Vec<&str> = scope_path.split('.').collect();
    if segments.len() < 2 {
        results.push(VerificationResult {
            severity: Severity::Error,
            code: "S002".to_string(),
            message: format!(
                "scope '{}' must have at least two segments (e.g., domain.module), got {}",
                scope_path,
                segments.len()
            ),
            contract_name: "(file)".to_string(),
            file: file.to_string(),
            line,
        });
    }

    // S002: scope segments must be lowercase identifiers
    for segment in &segments {
        if segment.is_empty() {
            results.push(VerificationResult {
                severity: Severity::Error,
                code: "S002".to_string(),
                message: format!("scope '{}' contains empty segment", scope_path),
                contract_name: "(file)".to_string(),
                file: file.to_string(),
                line,
            });
        } else if !segment.chars().all(|c| c.is_ascii_lowercase() || c == '_' || c.is_ascii_digit()) {
            results.push(VerificationResult {
                severity: Severity::Error,
                code: "S002".to_string(),
                message: format!(
                    "scope segment '{}' must be lowercase (a-z, 0-9, _)",
                    segment
                ),
                contract_name: "(file)".to_string(),
                file: file.to_string(),
                line,
            });
        }
    }

    // S003: scope should be consistent with intent
    if let Some(ref header) = program.header {
        if let (Some(ref intent), Some(ref scope)) = (&header.intent, &header.scope) {
            let intent_lower = intent.text.to_lowercase();
            // Check if any scope segment is reflected in the intent
            let scope_reflected = segments.iter().any(|seg| {
                seg.len() >= 3 && intent_lower.contains(*seg)
            });

            if !scope_reflected && intent.text.len() > 10 {
                results.push(VerificationResult {
                    severity: Severity::Warning,
                    code: "S003".to_string(),
                    message: format!(
                        "scope '{}' does not appear related to intent \"{}\". \
                         Scope should reflect the domain this code belongs to.",
                        scope.path, intent.text
                    ),
                    contract_name: "(file)".to_string(),
                    file: file.to_string(),
                    line: scope.loc.line,
                });
            }
        }
    }

    results
}

// ── Helpers ─────────────────────────────────────────────────────────────

fn extract_declared_modifies(effects: Option<&Effects>) -> HashSet<String> {
    let mut result = HashSet::new();
    if let Some(effects) = effects {
        for decl in &effects.declarations {
            if let EffectDecl::Modifies { targets, .. } = decl {
                result.extend(targets.iter().cloned());
            }
        }
    }
    result
}

fn extract_declared_reads(effects: Option<&Effects>) -> HashSet<String> {
    let mut result = HashSet::new();
    if let Some(effects) = effects {
        for decl in &effects.declarations {
            if let EffectDecl::Reads { targets, .. } = decl {
                result.extend(targets.iter().cloned());
            }
        }
    }
    result
}

fn extract_declared_emits(effects: Option<&Effects>) -> HashSet<String> {
    let mut result = HashSet::new();
    if let Some(effects) = effects {
        for decl in &effects.declarations {
            if let EffectDecl::Emits { event_type, .. } = decl {
                result.insert(event_type.clone());
            }
        }
    }
    result
}

fn has_touches_nothing_else(effects: Option<&Effects>) -> bool {
    effects
        .map(|e| {
            e.declarations
                .iter()
                .any(|d| matches!(d, EffectDecl::TouchesNothingElse { .. }))
        })
        .unwrap_or(false)
}

fn is_covered_by(actual: &str, declared: &HashSet<String>) -> bool {
    if declared.contains(actual) {
        return true;
    }
    for d in declared {
        if actual.starts_with(&format!("{}.", d)) {
            return true;
        }
    }
    // Local variable assignments (no dots) are generally OK
    if !actual.contains('.') {
        return true;
    }
    false
}

fn is_mutation_covered(reference: &str, mutations: &std::collections::BTreeSet<String>) -> bool {
    if mutations.contains(reference) {
        return true;
    }
    for m in mutations {
        if reference.starts_with(&format!("{}.", m)) || m.starts_with(&format!("{}.", reference)) {
            return true;
        }
    }
    false
}

fn is_observed_in(declared: &str, actual: &std::collections::BTreeSet<String>) -> bool {
    if actual.contains(declared) {
        return true;
    }
    for a in actual {
        if a.starts_with(&format!("{}.", declared)) {
            return true;
        }
        if declared.starts_with(&format!("{}.", a)) {
            return true;
        }
    }
    false
}

/// Heuristic: a declared mutation `obj.field` is "covered" if the body calls
/// a method on the same root object (e.g., `obj.method(...)` or `obj.sub.method(...)`).
/// This handles common patterns like `ledger.escrow(from, amount)` covering
/// `modifies [from.balance]` when `from` appears as a call argument.
fn is_covered_by_call(declared: &str, calls: &std::collections::BTreeSet<String>) -> bool {
    let root = declared.split('.').next().unwrap_or("");
    if root.is_empty() {
        return false;
    }
    for call in calls {
        let call_root = call.split('.').next().unwrap_or("");
        if call_root == root {
            return true;
        }
    }
    false
}
