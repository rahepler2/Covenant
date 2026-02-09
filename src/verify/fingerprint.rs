use std::collections::BTreeSet;

use serde::Serialize;

use crate::ast::*;

#[derive(Debug, Clone, Serialize)]
pub struct BehavioralFingerprint {
    pub reads: BTreeSet<String>,
    pub mutations: BTreeSet<String>,
    pub calls: BTreeSet<String>,
    pub emitted_events: BTreeSet<String>,
    pub old_references: BTreeSet<String>,
    pub capability_checks: BTreeSet<String>,
    pub operators: Vec<String>,
    pub literals: Vec<String>,
    pub has_branching: bool,
    pub has_looping: bool,
    pub has_error_handling: bool,
    pub has_recursion: bool,
    pub return_count: usize,
    pub max_nesting_depth: usize,
}

impl BehavioralFingerprint {
    pub fn new() -> Self {
        Self {
            reads: BTreeSet::new(),
            mutations: BTreeSet::new(),
            calls: BTreeSet::new(),
            emitted_events: BTreeSet::new(),
            old_references: BTreeSet::new(),
            capability_checks: BTreeSet::new(),
            operators: Vec::new(),
            literals: Vec::new(),
            has_branching: false,
            has_looping: false,
            has_error_handling: false,
            has_recursion: false,
            return_count: 0,
            max_nesting_depth: 0,
        }
    }

    pub fn to_canonical_dict(&self) -> serde_json::Value {
        let mut ops = self.operators.clone();
        ops.sort();
        let mut lits = self.literals.clone();
        lits.sort();

        serde_json::json!({
            "reads": self.reads.iter().collect::<Vec<_>>(),
            "mutations": self.mutations.iter().collect::<Vec<_>>(),
            "calls": self.calls.iter().collect::<Vec<_>>(),
            "emitted_events": self.emitted_events.iter().collect::<Vec<_>>(),
            "old_references": self.old_references.iter().collect::<Vec<_>>(),
            "capability_checks": self.capability_checks.iter().collect::<Vec<_>>(),
            "operators": ops,
            "literals": lits,
            "has_branching": self.has_branching,
            "has_looping": self.has_looping,
            "has_error_handling": self.has_error_handling,
            "has_recursion": self.has_recursion,
            "return_count": self.return_count,
            "max_nesting_depth": self.max_nesting_depth,
        })
    }
}

pub fn fingerprint_contract(contract: &ContractDef) -> BehavioralFingerprint {
    let mut fp = BehavioralFingerprint::new();
    let mut walker = ASTWalker::new(&contract.name);

    if let Some(ref body) = contract.body {
        walker.walk_statements(&body.statements, 0, &mut fp);
    }

    if let Some(ref on_failure) = contract.on_failure {
        walker.walk_statements(&on_failure.statements, 0, &mut fp);
    }

    fp
}

pub fn fingerprint_expressions(exprs: &[Expr]) -> BehavioralFingerprint {
    let mut fp = BehavioralFingerprint::new();
    let mut walker = ASTWalker::new("");
    for expr in exprs {
        walker.walk_expr(expr, &mut fp);
    }
    fp
}

struct ASTWalker {
    contract_name: String,
}

impl ASTWalker {
    fn new(contract_name: &str) -> Self {
        Self {
            contract_name: contract_name.to_string(),
        }
    }

    fn walk_statements(&mut self, stmts: &[Statement], depth: usize, fp: &mut BehavioralFingerprint) {
        if depth > fp.max_nesting_depth {
            fp.max_nesting_depth = depth;
        }
        for stmt in stmts {
            self.walk_statement(stmt, depth, fp);
        }
    }

    fn walk_statement(&mut self, stmt: &Statement, depth: usize, fp: &mut BehavioralFingerprint) {
        match stmt {
            Statement::Assignment { target, value, .. } => {
                fp.mutations.insert(target.clone());
                self.walk_expr(value, fp);
            }
            Statement::Return { value, .. } => {
                fp.return_count += 1;
                self.walk_expr(value, fp);
            }
            Statement::Emit { event, .. } => {
                if let Some(name) = self.extract_event_name(event) {
                    fp.emitted_events.insert(name);
                }
                self.walk_expr(event, fp);
            }
            Statement::ExprStmt { expr, .. } => {
                self.walk_expr(expr, fp);
            }
            Statement::If {
                condition,
                then_body,
                else_body,
                ..
            } => {
                fp.has_branching = true;
                self.walk_expr(condition, fp);
                self.walk_statements(then_body, depth + 1, fp);
                if !else_body.is_empty() {
                    self.walk_statements(else_body, depth + 1, fp);
                }
            }
            Statement::For {
                iterable, body, ..
            } => {
                fp.has_looping = true;
                self.walk_expr(iterable, fp);
                self.walk_statements(body, depth + 1, fp);
            }
            Statement::While {
                condition, body, ..
            } => {
                fp.has_looping = true;
                self.walk_expr(condition, fp);
                self.walk_statements(body, depth + 1, fp);
            }
            Statement::Parallel { branches, .. } => {
                for branch in branches {
                    self.walk_statements(branch, depth + 1, fp);
                }
            }
            Statement::TryCatch { try_body, catch_body, finally_body, .. } => {
                fp.has_error_handling = true;
                fp.has_branching = true;
                self.walk_statements(try_body, depth + 1, fp);
                self.walk_statements(catch_body, depth + 1, fp);
                self.walk_statements(finally_body, depth + 1, fp);
            }
        }
    }

    fn walk_expr(&mut self, expr: &Expr, fp: &mut BehavioralFingerprint) {
        match expr {
            Expr::Identifier { name, .. } => {
                fp.reads.insert(name.clone());
            }
            Expr::FieldAccess { .. } => {
                let path = self.extract_dotted_path(expr);
                fp.reads.insert(path);
            }
            Expr::FunctionCall {
                function,
                arguments,
                keyword_args,
                ..
            } => {
                if let Some(call_name) = self.extract_call_name(function) {
                    if call_name == self.contract_name {
                        fp.has_recursion = true;
                    }
                    fp.calls.insert(call_name);
                }
                self.walk_expr(function, fp);
                for arg in arguments {
                    self.walk_expr(arg, fp);
                }
                for (_, val) in keyword_args {
                    self.walk_expr(val, fp);
                }
            }
            Expr::MethodCall {
                object,
                method,
                arguments,
                keyword_args,
                ..
            } => {
                let obj_path = self.extract_call_name(object).unwrap_or_default();
                let call_name = if !obj_path.is_empty() {
                    format!("{}.{}", obj_path, method)
                } else {
                    method.clone()
                };
                fp.calls.insert(call_name);
                self.walk_expr(object, fp);
                for arg in arguments {
                    self.walk_expr(arg, fp);
                }
                for (_, val) in keyword_args {
                    self.walk_expr(val, fp);
                }
            }
            Expr::BinaryOp {
                left, op, right, ..
            } => {
                fp.operators.push(op.clone());
                self.walk_expr(left, fp);
                self.walk_expr(right, fp);
            }
            Expr::UnaryOp { op, operand, .. } => {
                fp.operators.push(op.clone());
                self.walk_expr(operand, fp);
            }
            Expr::OldExpr { inner, .. } => {
                let path = self.extract_dotted_path(inner);
                fp.old_references.insert(path);
                self.walk_expr(inner, fp);
            }
            Expr::HasExpr {
                subject,
                capability,
                ..
            } => {
                let subj = self.extract_dotted_path(subject);
                let cap = self.extract_dotted_path(capability);
                fp.capability_checks.insert(format!("{} has {}", subj, cap));
            }
            Expr::ListLiteral { elements, .. } => {
                for elem in elements {
                    self.walk_expr(elem, fp);
                }
            }
            Expr::NumberLiteral {
                value, is_int, ..
            } => {
                if *is_int {
                    fp.literals.push(format!("{}", *value as i64));
                } else {
                    fp.literals.push(format!("{}", value));
                }
            }
            Expr::StringLiteral { value, .. } => {
                fp.literals.push(format!("{:?}", value));
            }
            Expr::BoolLiteral { value, .. } => {
                fp.literals.push(if *value {
                    "True".to_string()
                } else {
                    "False".to_string()
                });
            }
            Expr::IndexAccess { object, index, .. } => {
                self.walk_expr(object, fp);
                self.walk_expr(index, fp);
            }
            Expr::NullLiteral { .. } => {
                // No fingerprint data from null literal
            }
            Expr::AwaitExpr { inner, .. } => {
                self.walk_expr(inner, fp);
            }
        }
    }

    fn extract_dotted_path(&self, expr: &Expr) -> String {
        match expr {
            Expr::Identifier { name, .. } => name.clone(),
            Expr::FieldAccess {
                object, field_name, ..
            } => {
                let parent = self.extract_dotted_path(object);
                format!("{}.{}", parent, field_name)
            }
            Expr::MethodCall { object, method, .. } => {
                let obj = self.extract_call_name(object).unwrap_or_default();
                if !obj.is_empty() {
                    format!("{}.{}()", obj, method)
                } else {
                    format!("{}()", method)
                }
            }
            Expr::FunctionCall { function, .. } => {
                let name = self.extract_call_name(function).unwrap_or_default();
                format!("{}()", name)
            }
            _ => "<complex>".to_string(),
        }
    }

    fn extract_call_name(&self, expr: &Expr) -> Option<String> {
        match expr {
            Expr::Identifier { name, .. } => Some(name.clone()),
            Expr::FieldAccess {
                object, field_name, ..
            } => {
                let parent = self.extract_dotted_path(object);
                Some(format!("{}.{}", parent, field_name))
            }
            _ => None,
        }
    }

    fn extract_event_name(&self, expr: &Expr) -> Option<String> {
        match expr {
            Expr::FunctionCall { function, .. } => self.extract_call_name(function),
            Expr::Identifier { name, .. } => Some(name.clone()),
            _ => None,
        }
    }
}
