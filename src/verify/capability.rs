//! Phase 3: Capability Type System — Information Flow Control (IFC)
//!
//! Verification codes:
//!   F001 — Information flow violation: labeled data flows to restricted sink
//!   F002 — Permission denied: body accesses data denied by permissions block
//!   F003 — Access not granted: body accesses data not covered by grants
//!   F004 — Context required: type with requires_context used outside required context
//!   F005 — Capability not declared: has-check references capability not in requires
//!   F006 — Grant-deny conflict: same access appears in both grants and denies

use std::collections::{BTreeSet, HashMap, HashSet};

use crate::ast::*;
use super::checker::{Severity, VerificationResult};
use super::fingerprint::{fingerprint_contract, BehavioralFingerprint};

/// Capability labels on data (e.g., {pii, no_log, encrypt_at_rest})
type Labels = BTreeSet<String>;

/// Information about a type definition and its capability constraints
#[derive(Debug, Clone)]
struct TypeInfo {
    #[allow(dead_code)]
    name: String,
    field_labels: HashMap<String, Labels>,
    all_labels: Labels, // union of all field labels
    never_flows_to: Vec<String>,
    requires_context: Option<String>,
}

/// Parsed access pattern from permission strings
#[derive(Debug, Clone)]
struct AccessPattern {
    kind: AccessKind,
    original: String,
}

#[derive(Debug, Clone)]
enum AccessKind {
    Read(String),
    Write(String),
    General(String),
}

// ── Public API ──────────────────────────────────────────────────────────

pub fn verify_capabilities(program: &Program, file: &str) -> Vec<VerificationResult> {
    let mut results = Vec::new();

    let type_registry = build_type_registry(&program.type_defs);

    let required_caps: HashSet<String> = program
        .header
        .as_ref()
        .and_then(|h| h.requires.as_ref())
        .map(|r| r.capabilities.iter().cloned().collect())
        .unwrap_or_default();

    let scope_context: Option<&str> = program
        .header
        .as_ref()
        .and_then(|h| h.scope.as_ref())
        .map(|s| s.path.as_str());

    // Build label→type mapping for flow violation attribution
    let label_to_types = build_label_to_types(&type_registry);

    for contract in &program.contracts {
        let fp = fingerprint_contract(contract);
        results.extend(verify_contract_capabilities(
            contract,
            &type_registry,
            &label_to_types,
            &required_caps,
            scope_context,
            &fp,
            file,
        ));
    }

    results
}

// ── Type registry ───────────────────────────────────────────────────────

fn build_type_registry(type_defs: &[TypeDef]) -> HashMap<String, TypeInfo> {
    let mut registry = HashMap::new();

    for td in type_defs {
        let mut field_labels: HashMap<String, Labels> = HashMap::new();
        let mut all_labels = Labels::new();

        for field in &td.fields {
            let labels = extract_type_labels(&field.type_expr);
            if !labels.is_empty() {
                all_labels.extend(labels.iter().cloned());
                field_labels.insert(field.name.clone(), labels);
            }
        }

        let never_flows_to: Vec<String> = td
            .flow_constraints
            .iter()
            .filter_map(|fc| match fc {
                FlowConstraint::NeverFlowsTo { destinations, .. } => Some(destinations.clone()),
                _ => None,
            })
            .flatten()
            .collect();

        let requires_context = td.flow_constraints.iter().find_map(|fc| match fc {
            FlowConstraint::RequiresContext { context, .. } => Some(context.clone()),
            _ => None,
        });

        registry.insert(
            td.name.clone(),
            TypeInfo {
                name: td.name.clone(),
                field_labels,
                all_labels,
                never_flows_to,
                requires_context,
            },
        );
    }

    registry
}

fn build_label_to_types(
    type_registry: &HashMap<String, TypeInfo>,
) -> HashMap<String, HashSet<String>> {
    let mut map: HashMap<String, HashSet<String>> = HashMap::new();
    for (type_name, info) in type_registry {
        for label in &info.all_labels {
            map.entry(label.clone())
                .or_default()
                .insert(type_name.clone());
        }
    }
    map
}

fn extract_type_labels(type_expr: &TypeExpr) -> Labels {
    match type_expr {
        TypeExpr::Annotated { annotations, .. } => annotations.iter().cloned().collect(),
        _ => BTreeSet::new(),
    }
}

// ── Per-contract verification ───────────────────────────────────────────

fn verify_contract_capabilities(
    contract: &ContractDef,
    type_registry: &HashMap<String, TypeInfo>,
    label_to_types: &HashMap<String, HashSet<String>>,
    required_caps: &HashSet<String>,
    scope_context: Option<&str>,
    fp: &BehavioralFingerprint,
    file: &str,
) -> Vec<VerificationResult> {
    let mut results = Vec::new();
    let line = contract.loc.line;
    let name = &contract.name;

    // Build param→type mapping
    let param_types = build_param_types(contract);

    // ── F004: requires_context ──────────────────────────────────────

    for (param_name, type_name) in &param_types {
        if let Some(type_info) = type_registry.get(type_name) {
            if let Some(ref required_context) = type_info.requires_context {
                if !context_satisfied(scope_context, required_context) {
                    results.push(VerificationResult {
                        severity: Severity::Error,
                        code: "F004".to_string(),
                        message: format!(
                            "parameter '{}' has type '{}' which requires_context '{}' \
                             but the file scope is '{}'",
                            param_name,
                            type_name,
                            required_context,
                            scope_context.unwrap_or("(none)")
                        ),
                        contract_name: name.clone(),
                        file: file.to_string(),
                        line,
                    });
                }
            }
        }
    }

    // ── F001: Information flow analysis (taint tracking) ────────────

    if let Some(ref body) = contract.body {
        let mut tracker = FlowTracker {
            type_registry,
            label_to_types,
            param_types: &param_types,
            var_labels: HashMap::new(),
            results: Vec::new(),
            contract_name: name.clone(),
            file: file.to_string(),
            contract_line: line,
        };

        // Initialize param labels (whole-object labels from type)
        for (param_name, type_name) in &param_types {
            if let Some(type_info) = type_registry.get(type_name) {
                if !type_info.all_labels.is_empty() {
                    tracker
                        .var_labels
                        .insert(param_name.clone(), type_info.all_labels.clone());
                }
            }
        }

        tracker.track_statements(&body.statements);
        results.extend(tracker.results);
    }

    // ── F002/F003: Permission verification ─────────────────────────

    if let Some(ref permissions) = contract.permissions {
        let granted: Vec<AccessPattern> = permissions
            .grants
            .as_ref()
            .map(|g| g.permissions.iter().map(|p| parse_access_pattern(p)).collect())
            .unwrap_or_default();

        let denied: Vec<AccessPattern> = permissions
            .denies
            .as_ref()
            .map(|d| d.permissions.iter().map(|p| parse_access_pattern(p)).collect())
            .unwrap_or_default();

        // F006: grant-deny conflict
        for g in &granted {
            for d in &denied {
                if access_patterns_overlap(g, d) {
                    results.push(VerificationResult {
                        severity: Severity::Warning,
                        code: "F006".to_string(),
                        message: format!(
                            "permission conflict: '{}' overlaps between grants and denies",
                            g.original
                        ),
                        contract_name: name.clone(),
                        file: file.to_string(),
                        line,
                    });
                }
            }
        }

        // F002: Check denies — body must not access denied data
        for deny in &denied {
            match &deny.kind {
                AccessKind::Read(target) => {
                    if fp.reads.contains(target) || is_prefix_in(target, &fp.reads) {
                        results.push(VerificationResult {
                            severity: Severity::Error,
                            code: "F002".to_string(),
                            message: format!(
                                "permission denied: body reads '{}' which is denied",
                                target
                            ),
                            contract_name: name.clone(),
                            file: file.to_string(),
                            line,
                        });
                    }
                }
                AccessKind::Write(target) => {
                    if fp.mutations.contains(target) || is_prefix_in(target, &fp.mutations) {
                        results.push(VerificationResult {
                            severity: Severity::Error,
                            code: "F002".to_string(),
                            message: format!(
                                "permission denied: body writes '{}' which is denied",
                                target
                            ),
                            contract_name: name.clone(),
                            file: file.to_string(),
                            line,
                        });
                    }
                }
                AccessKind::General(cap) => {
                    for call in &fp.calls {
                        if call_matches_destination(call, cap) {
                            results.push(VerificationResult {
                                severity: Severity::Error,
                                code: "F002".to_string(),
                                message: format!(
                                    "permission denied: body calls '{}' which requires \
                                     denied capability '{}'",
                                    call, cap
                                ),
                                contract_name: name.clone(),
                                file: file.to_string(),
                                line,
                            });
                        }
                    }
                }
            }
        }

        // F003: Check grants — if grants exist, only granted access is allowed
        if !granted.is_empty() {
            for read in &fp.reads {
                // Only check parameter-rooted field access (not local variables)
                if read.contains('.') {
                    let root = read.split('.').next().unwrap_or("");
                    let is_param = contract.params.iter().any(|p| p.name == root);
                    if is_param && !is_access_granted(read, "read", &granted) {
                        results.push(VerificationResult {
                            severity: Severity::Warning,
                            code: "F003".to_string(),
                            message: format!(
                                "access not explicitly granted: body reads '{}' \
                                 which is not covered by any grants permission",
                                read
                            ),
                            contract_name: name.clone(),
                            file: file.to_string(),
                            line,
                        });
                    }
                }
            }
        }
    }

    // ── F005: has-checks reference declared capabilities ────────────

    if !required_caps.is_empty() {
        for check in &fp.capability_checks {
            let parts: Vec<&str> = check.split(" has ").collect();
            if parts.len() == 2 {
                let cap_path = parts[1];
                let is_available = required_caps
                    .iter()
                    .any(|rc| cap_path.starts_with(rc.as_str()) || rc.starts_with(cap_path));
                if !is_available {
                    results.push(VerificationResult {
                        severity: Severity::Warning,
                        code: "F005".to_string(),
                        message: format!(
                            "capability '{}' checked but not declared in requires",
                            cap_path
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

// ── Flow tracker (taint analysis) ───────────────────────────────────────

struct FlowTracker<'a> {
    type_registry: &'a HashMap<String, TypeInfo>,
    label_to_types: &'a HashMap<String, HashSet<String>>,
    param_types: &'a HashMap<String, String>,
    var_labels: HashMap<String, Labels>,
    results: Vec<VerificationResult>,
    contract_name: String,
    file: String,
    contract_line: usize,
}

impl<'a> FlowTracker<'a> {
    fn track_statements(&mut self, stmts: &[Statement]) {
        for stmt in stmts {
            self.track_statement(stmt);
        }
    }

    fn track_statement(&mut self, stmt: &Statement) {
        match stmt {
            Statement::Assignment { target, value, .. } => {
                let labels = self.compute_labels(value);
                let root = target.split('.').next().unwrap_or(target);
                if !labels.is_empty() {
                    // Merge labels (taint is additive)
                    let entry = self.var_labels.entry(root.to_string()).or_default();
                    entry.extend(labels);
                }
                // Also check if the value expression sends tainted data somewhere
                self.check_expression_flow(value);
            }
            Statement::Return { value, .. } => {
                self.check_expression_flow(value);
            }
            Statement::Emit { event, .. } => {
                self.check_expression_flow(event);
            }
            Statement::ExprStmt { expr, .. } => {
                self.check_expression_flow(expr);
            }
            Statement::If {
                condition,
                then_body,
                else_body,
                ..
            } => {
                self.check_expression_flow(condition);
                self.track_statements(then_body);
                self.track_statements(else_body);
            }
            Statement::For {
                var,
                iterable,
                body,
                ..
            } => {
                let labels = self.compute_labels(iterable);
                if !labels.is_empty() {
                    self.var_labels.insert(var.clone(), labels);
                }
                self.track_statements(body);
            }
            Statement::While {
                condition, body, ..
            } => {
                self.check_expression_flow(condition);
                self.track_statements(body);
            }
            Statement::Parallel { branches, .. } => {
                for branch in branches {
                    self.track_statements(branch);
                }
            }
            Statement::TryCatch { try_body, catch_body, finally_body, .. } => {
                self.track_statements(try_body);
                self.track_statements(catch_body);
                self.track_statements(finally_body);
            }
        }
    }

    /// Compute the taint labels carried by an expression
    fn compute_labels(&self, expr: &Expr) -> Labels {
        match expr {
            Expr::Identifier { name, .. } => {
                self.var_labels.get(name).cloned().unwrap_or_default()
            }
            Expr::FieldAccess {
                object, field_name, ..
            } => {
                // If this is a field of a typed parameter, use field-specific labels
                if let Some(obj_name) = get_ident_name(object) {
                    if let Some(type_name) = self.param_types.get(obj_name) {
                        if let Some(type_info) = self.type_registry.get(type_name) {
                            if let Some(field_labels) = type_info.field_labels.get(field_name) {
                                return field_labels.clone();
                            }
                        }
                    }
                }
                // Fall back to parent object labels
                self.compute_labels(object)
            }
            Expr::FunctionCall {
                arguments,
                keyword_args,
                ..
            } => {
                let mut labels = Labels::new();
                for arg in arguments {
                    labels.extend(self.compute_labels(arg));
                }
                for (_, v) in keyword_args {
                    labels.extend(self.compute_labels(v));
                }
                labels
            }
            Expr::MethodCall {
                object,
                arguments,
                keyword_args,
                ..
            } => {
                let mut labels = self.compute_labels(object);
                for arg in arguments {
                    labels.extend(self.compute_labels(arg));
                }
                for (_, v) in keyword_args {
                    labels.extend(self.compute_labels(v));
                }
                labels
            }
            Expr::BinaryOp { left, right, .. } => {
                let mut labels = self.compute_labels(left);
                labels.extend(self.compute_labels(right));
                labels
            }
            Expr::UnaryOp { operand, .. } => self.compute_labels(operand),
            Expr::OldExpr { inner, .. } => self.compute_labels(inner),
            Expr::ListLiteral { elements, .. } => {
                let mut labels = Labels::new();
                for e in elements {
                    labels.extend(self.compute_labels(e));
                }
                labels
            }
            Expr::IndexAccess { object, index, .. } => {
                let mut labels = self.compute_labels(object);
                labels.extend(self.compute_labels(index));
                labels
            }
            Expr::HasExpr { .. }
            | Expr::StringLiteral { .. }
            | Expr::NumberLiteral { .. }
            | Expr::BoolLiteral { .. }
            | Expr::NullLiteral { .. } => Labels::new(),
            Expr::AwaitExpr { inner, .. } => self.compute_labels(inner),
        }
    }

    /// Walk an expression to check if tainted data flows to a restricted sink
    fn check_expression_flow(&mut self, expr: &Expr) {
        match expr {
            Expr::FunctionCall {
                function,
                arguments,
                keyword_args,
                ..
            } => {
                let func_name = extract_call_name(function);

                // Collect labels from all arguments
                let mut all_labels = Labels::new();
                for arg in arguments {
                    all_labels.extend(self.compute_labels(arg));
                }
                for (_, v) in keyword_args {
                    all_labels.extend(self.compute_labels(v));
                }

                if !all_labels.is_empty() {
                    self.check_flow_destination(&func_name, &all_labels);
                }

                // Recurse into subexpressions
                for arg in arguments {
                    self.check_expression_flow(arg);
                }
                for (_, v) in keyword_args {
                    self.check_expression_flow(v);
                }
            }
            Expr::MethodCall {
                object,
                method,
                arguments,
                keyword_args,
                ..
            } => {
                let obj_name = extract_call_name(object);
                let full_name = format!("{}.{}", obj_name, method);

                let mut all_labels = self.compute_labels(object);
                for arg in arguments {
                    all_labels.extend(self.compute_labels(arg));
                }
                for (_, v) in keyword_args {
                    all_labels.extend(self.compute_labels(v));
                }

                if !all_labels.is_empty() {
                    self.check_flow_destination(&full_name, &all_labels);
                }

                self.check_expression_flow(object);
                for arg in arguments {
                    self.check_expression_flow(arg);
                }
                for (_, v) in keyword_args {
                    self.check_expression_flow(v);
                }
            }
            Expr::BinaryOp { left, right, .. } => {
                self.check_expression_flow(left);
                self.check_expression_flow(right);
            }
            Expr::UnaryOp { operand, .. } => {
                self.check_expression_flow(operand);
            }
            _ => {}
        }
    }

    /// Check if a function call destination violates any never_flows_to constraint
    fn check_flow_destination(&mut self, call_name: &str, labels: &Labels) {
        // Skip constructors (capitalized names)
        if call_name
            .chars()
            .next()
            .map_or(false, |c| c.is_uppercase())
        {
            return;
        }

        let mut reported = HashSet::new();
        for label in labels {
            if let Some(types) = self.label_to_types.get(label) {
                for type_name in types {
                    if let Some(type_info) = self.type_registry.get(type_name) {
                        for dest in &type_info.never_flows_to {
                            if call_matches_destination(call_name, dest) {
                                let key = format!("{}:{}:{}", type_name, call_name, dest);
                                if reported.insert(key) {
                                    self.results.push(VerificationResult {
                                        severity: Severity::Error,
                                        code: "F001".to_string(),
                                        message: format!(
                                            "information flow violation: labeled data (type '{}') \
                                             flows to '{}' which is in never_flows_to [{}]",
                                            type_name,
                                            call_name,
                                            type_info.never_flows_to.join(", ")
                                        ),
                                        contract_name: self.contract_name.clone(),
                                        file: self.file.clone(),
                                        line: self.contract_line,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

// ── Helper functions ────────────────────────────────────────────────────

fn build_param_types(contract: &ContractDef) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for param in &contract.params {
        let type_name = match &param.type_expr {
            TypeExpr::Simple { name, .. } => name.clone(),
            TypeExpr::Annotated { base, .. } => {
                if let TypeExpr::Simple { name, .. } = base.as_ref() {
                    name.clone()
                } else {
                    continue;
                }
            }
            _ => continue,
        };
        map.insert(param.name.clone(), type_name);
    }
    map
}

fn context_satisfied(scope: Option<&str>, required_context: &str) -> bool {
    let scope = match scope {
        Some(s) => s,
        None => return false,
    };

    // Split scope by dots and context by underscores
    let scope_parts: HashSet<&str> = scope.split('.').collect();
    let context_parts: Vec<&str> = required_context.split('_').collect();

    // At least one context component must appear in the scope path
    context_parts.iter().any(|cp| scope_parts.contains(cp))
}

fn call_matches_destination(call: &str, dest: &str) -> bool {
    let call_lower = call.to_lowercase();
    let dest_lower = dest.to_lowercase();

    // Exact match
    if call_lower == dest_lower {
        return true;
    }
    // Call starts with destination followed by a dot
    if call_lower.starts_with(&format!("{}.", dest_lower)) {
        return true;
    }
    // Destination is a component of the call path
    for part in call_lower.split('.') {
        if part == dest_lower {
            return true;
        }
    }
    false
}

fn get_ident_name(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Identifier { name, .. } => Some(name),
        _ => None,
    }
}

fn extract_call_name(expr: &Expr) -> String {
    match expr {
        Expr::Identifier { name, .. } => name.clone(),
        Expr::FieldAccess {
            object, field_name, ..
        } => {
            let parent = extract_call_name(object);
            format!("{}.{}", parent, field_name)
        }
        _ => "<indirect>".to_string(),
    }
}

fn parse_access_pattern(s: &str) -> AccessPattern {
    if let Some(target) = s.strip_prefix("read(").and_then(|s| s.strip_suffix(')')) {
        AccessPattern {
            kind: AccessKind::Read(target.to_string()),
            original: s.to_string(),
        }
    } else if let Some(target) = s.strip_prefix("write(").and_then(|s| s.strip_suffix(')')) {
        AccessPattern {
            kind: AccessKind::Write(target.to_string()),
            original: s.to_string(),
        }
    } else {
        AccessPattern {
            kind: AccessKind::General(s.to_string()),
            original: s.to_string(),
        }
    }
}

fn access_patterns_overlap(a: &AccessPattern, b: &AccessPattern) -> bool {
    match (&a.kind, &b.kind) {
        (AccessKind::Read(t1), AccessKind::Read(t2)) => t1 == t2,
        (AccessKind::Write(t1), AccessKind::Write(t2)) => t1 == t2,
        (AccessKind::General(g1), AccessKind::General(g2)) => g1 == g2,
        _ => false,
    }
}

fn is_prefix_in(target: &str, set: &BTreeSet<String>) -> bool {
    set.iter()
        .any(|s| s.starts_with(&format!("{}.", target)) || target.starts_with(&format!("{}.", s)))
}

fn is_access_granted(read_path: &str, access_type: &str, grants: &[AccessPattern]) -> bool {
    for grant in grants {
        match &grant.kind {
            AccessKind::Read(target) if access_type == "read" => {
                if read_path == target
                    || read_path.starts_with(&format!("{}.", target))
                    || target.starts_with(&format!("{}.", read_path))
                {
                    return true;
                }
            }
            AccessKind::Write(target) if access_type == "write" => {
                if read_path == target
                    || read_path.starts_with(&format!("{}.", target))
                    || target.starts_with(&format!("{}.", read_path))
                {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}
