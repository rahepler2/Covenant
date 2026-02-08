use std::collections::HashSet;

use crate::ast::*;
use crate::verify::fingerprint::{fingerprint_contract, fingerprint_expressions, BehavioralFingerprint};

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

    if contract.precondition.is_none() {
        let sev = if matches!(risk_level, RiskLevel::High | RiskLevel::Critical) {
            Severity::Error
        } else {
            Severity::Warning
        };
        add(
            sev,
            "W003",
            "no precondition — every contract should declare what must be true before execution"
                .to_string(),
        );
    }

    if contract.postcondition.is_none() {
        let sev = if matches!(risk_level, RiskLevel::High | RiskLevel::Critical) {
            Severity::Error
        } else {
            Severity::Warning
        };
        add(
            sev,
            "W004",
            "no postcondition — every contract should declare what will be true after execution"
                .to_string(),
        );
    }

    if contract.effects.is_none() {
        let sev = if matches!(risk_level, RiskLevel::High | RiskLevel::Critical) {
            Severity::Error
        } else {
            Severity::Warning
        };
        add(
            sev,
            "W005",
            "no effects declaration — every contract must declare its side effects".to_string(),
        );
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

    for declared in &declared_modifies {
        if !is_observed_in(declared, &fp.mutations) {
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
            if !is_mutation_covered(old_ref, &fp.mutations) {
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

    // Phase 2: Intent Verification Engine (E001-E005, W001-W008, I001-I002)
    for contract in &program.contracts {
        let fp = fingerprint_contract(contract);
        results.extend(verify_contract(
            contract,
            Some(&fp),
            file,
            declared_capabilities.as_deref(),
            risk_level,
        ));
    }

    // Phase 3: Capability Type System / IFC (F001-F006)
    results.extend(super::capability::verify_capabilities(program, file));

    // Phase 4: Contract Verification (V001-V005)
    results.extend(super::contract_verify::verify_contracts(program, file));

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
