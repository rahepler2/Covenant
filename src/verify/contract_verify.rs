//! Phase 4: Contract Verification
//!
//! Verification codes:
//!   V001 — Not all code paths return a value
//!   V002 — Dead code detected after return statement
//!   V003 — High/critical risk contract missing on_failure handler
//!   V004 — Postcondition references result but body may not return

use crate::ast::*;
use super::checker::{Severity, VerificationResult};
use super::fingerprint::fingerprint_expressions;

// ── Public API ──────────────────────────────────────────────────────────

pub fn verify_contracts(program: &Program, file: &str) -> Vec<VerificationResult> {
    let mut results = Vec::new();

    let risk_level = program
        .header
        .as_ref()
        .and_then(|h| h.risk.as_ref())
        .map(|r| r.level)
        .unwrap_or(RiskLevel::Low);

    let shared_names: Vec<String> = program.shared_decls.iter().map(|s| s.name.clone()).collect();

    for contract in &program.contracts {
        results.extend(verify_single_contract(
            contract,
            risk_level,
            &shared_names,
            file,
        ));
    }

    results
}

// ── Per-contract verification ───────────────────────────────────────────

fn verify_single_contract(
    contract: &ContractDef,
    risk_level: RiskLevel,
    shared_names: &[String],
    file: &str,
) -> Vec<VerificationResult> {
    let mut results = Vec::new();
    let line = contract.loc.line;
    let name = &contract.name;

    // Skip contracts without bodies
    let body = match &contract.body {
        Some(b) => b,
        None => return results,
    };

    // ── V001: Not all code paths return a value ────────────────────

    if contract.return_type.is_some() && !body.statements.is_empty() {
        if !always_returns(&body.statements) {
            results.push(VerificationResult {
                severity: Severity::Warning,
                code: "V001".to_string(),
                message: "not all code paths return a value".to_string(),
                contract_name: name.clone(),
                file: file.to_string(),
                line,
            });
        }
    }

    // ── V002: Dead code detection ──────────────────────────────────

    let dead_locs = find_dead_code(&body.statements);
    for dead_loc in dead_locs {
        results.push(VerificationResult {
            severity: Severity::Warning,
            code: "V002".to_string(),
            message: "unreachable code after return statement".to_string(),
            contract_name: name.clone(),
            file: file.to_string(),
            line: dead_loc.line,
        });
    }

    // ── V003: On-failure missing for high/critical risk ────────────

    if matches!(risk_level, RiskLevel::High | RiskLevel::Critical) && contract.on_failure.is_none()
    {
        results.push(VerificationResult {
            severity: Severity::Warning,
            code: "V003".to_string(),
            message: format!(
                "contract has {:?} risk level but no on_failure handler — \
                 high-risk contracts should handle failure gracefully",
                risk_level
            ),
            contract_name: name.clone(),
            file: file.to_string(),
            line,
        });
    }

    // ── V004: Postcondition uses result but body may not return ────

    if let Some(ref postcondition) = contract.postcondition {
        let postcond_fp = fingerprint_expressions(&postcondition.conditions);
        let references_result = postcond_fp.reads.contains("result");

        if references_result && !always_returns(&body.statements) {
            results.push(VerificationResult {
                severity: Severity::Warning,
                code: "V004".to_string(),
                message: "postcondition references 'result' but not all code paths \
                     return a value — postcondition may evaluate against null"
                    .to_string(),
                contract_name: name.clone(),
                file: file.to_string(),
                line,
            });
        }
    }

    // ── V005: Shared state access validation ───────────────────────

    if !shared_names.is_empty() {
        let fp = super::fingerprint::fingerprint_contract(contract);
        for shared_name in shared_names {
            let accessed = fp.reads.iter().any(|r| {
                r == shared_name || r.starts_with(&format!("{}.", shared_name))
            }) || fp.mutations.iter().any(|m| {
                m == shared_name || m.starts_with(&format!("{}.", shared_name))
            }) || fp.calls.iter().any(|c| {
                c.starts_with(&format!("{}.", shared_name))
            });

            if accessed {
                // Check if effects declare access to this shared state
                let declared = contract.effects.as_ref().map_or(false, |effects| {
                    effects.declarations.iter().any(|d| match d {
                        EffectDecl::Modifies { targets, .. } | EffectDecl::Reads { targets, .. } => {
                            targets.iter().any(|t| {
                                t == shared_name
                                    || t.starts_with(&format!("{}.", shared_name))
                            })
                        }
                        _ => false,
                    })
                });

                if !declared {
                    results.push(VerificationResult {
                        severity: Severity::Warning,
                        code: "V005".to_string(),
                        message: format!(
                            "contract accesses shared state '{}' but does not declare \
                             it in effects — shared state access should be explicit",
                            shared_name
                        ),
                        contract_name: name.clone(),
                        file: file.to_string(),
                        line,
                    });
                }
            }
        }
    }

    results
}

// ── Return path analysis ────────────────────────────────────────────────

/// Check if every code path through the statements ends with a return
fn always_returns(stmts: &[Statement]) -> bool {
    if stmts.is_empty() {
        return false;
    }

    // Check each statement — an early return counts
    for (i, stmt) in stmts.iter().enumerate() {
        match stmt {
            Statement::Return { .. } => return true,
            Statement::If {
                then_body,
                else_body,
                ..
            } => {
                // If both branches return, and this is exhaustive (has else), the whole block returns
                if !else_body.is_empty()
                    && always_returns(then_body)
                    && always_returns(else_body)
                {
                    return true;
                }
            }
            _ => {}
        }
        // For the last statement, if it's a non-returning statement, fall through
        let _ = i;
    }

    false
}

// ── Dead code detection ─────────────────────────────────────────────────

/// Find source locations of dead code (statements after a return)
fn find_dead_code(stmts: &[Statement]) -> Vec<SourceLocation> {
    let mut dead = Vec::new();
    let mut found_return = false;

    for stmt in stmts {
        if found_return {
            dead.push(stmt.loc().clone());
            continue; // Don't recurse into already-dead code
        }
        if matches!(stmt, Statement::Return { .. }) {
            found_return = true;
            continue;
        }
        // If an if/else both return, subsequent code is dead
        if let Statement::If {
            then_body,
            else_body,
            ..
        } = stmt
        {
            if !else_body.is_empty() && always_returns(then_body) && always_returns(else_body) {
                found_return = true;
            }
            // Recurse into branches to find nested dead code
            dead.extend(find_dead_code(then_body));
            dead.extend(find_dead_code(else_body));
        }
        // Recurse into loops
        if let Statement::For { body, .. } | Statement::While { body, .. } = stmt {
            dead.extend(find_dead_code(body));
        }
    }

    dead
}
