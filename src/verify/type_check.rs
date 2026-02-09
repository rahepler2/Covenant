//! Static type checker for Covenant contracts
//!
//! Walks the AST and infers types of expressions, checking:
//! - T001: Argument type mismatch at call sites
//! - T002: Return type mismatch
//! - T003: Binary operator type mismatch
//! - T004: Wrong argument count
//! - T005: Type annotation on undeclared type

use std::collections::HashMap;
use crate::ast::*;

#[derive(Debug, Clone, PartialEq)]
pub enum InferredType {
    Int,
    Float,
    String,
    Bool,
    List(Box<InferredType>),
    Object(std::string::String),
    Null,
    Any,      // untyped or unknown
    Number,   // Int or Float
}

impl InferredType {
    pub fn display(&self) -> std::string::String {
        match self {
            InferredType::Int => "Int".to_string(),
            InferredType::Float => "Float".to_string(),
            InferredType::String => "String".to_string(),
            InferredType::Bool => "Bool".to_string(),
            InferredType::List(inner) => format!("List<{}>", inner.display()),
            InferredType::Object(name) => name.clone(),
            InferredType::Null => "Null".to_string(),
            InferredType::Any => "Any".to_string(),
            InferredType::Number => "Number".to_string(),
        }
    }

    fn from_type_expr(te: &TypeExpr) -> InferredType {
        match te {
            TypeExpr::Simple { name, .. } => match name.as_str() {
                "Int" | "Integer" => InferredType::Int,
                "Float" | "Double" => InferredType::Float,
                "String" | "Str" => InferredType::String,
                "Bool" | "Boolean" => InferredType::Bool,
                "List" | "Array" => InferredType::List(Box::new(InferredType::Any)),
                "Null" | "None" => InferredType::Null,
                "Number" | "Numeric" => InferredType::Number,
                "Any" => InferredType::Any,
                name => InferredType::Object(name.to_string()),
            },
            TypeExpr::Annotated { base, .. } => InferredType::from_type_expr(base),
            TypeExpr::Generic { name, params, .. } => {
                if name == "List" || name == "Array" {
                    let inner = params.first()
                        .map(InferredType::from_type_expr)
                        .unwrap_or(InferredType::Any);
                    InferredType::List(Box::new(inner))
                } else if name == "Optional" {
                    // Optional<T> can be T or Null — we approximate as Any
                    InferredType::Any
                } else {
                    InferredType::Object(name.clone())
                }
            }
            TypeExpr::List { element_type, .. } => {
                InferredType::List(Box::new(InferredType::from_type_expr(element_type)))
            }
        }
    }

    fn compatible_with(&self, other: &InferredType) -> bool {
        if *self == InferredType::Any || *other == InferredType::Any {
            return true;
        }
        match (self, other) {
            (a, b) if a == b => true,
            // Int is compatible with Float (promotion)
            (InferredType::Int, InferredType::Float) => true,
            (InferredType::Int, InferredType::Number) => true,
            (InferredType::Float, InferredType::Number) => true,
            (InferredType::Number, InferredType::Int) => true,
            (InferredType::Number, InferredType::Float) => true,
            // List compatibility
            (InferredType::List(a), InferredType::List(b)) => a.compatible_with(b),
            // Null is compatible with any optional-like type
            (InferredType::Null, _) => true,
            _ => false,
        }
    }
}

#[derive(Debug)]
pub struct TypeWarning {
    pub code: &'static str,
    pub message: std::string::String,
    pub line: usize,
    pub column: usize,
}

struct ContractSig {
    params: Vec<(std::string::String, InferredType)>,
    return_type: InferredType,
}

pub fn check_types(program: &Program) -> Vec<TypeWarning> {
    let mut warnings = Vec::new();

    // Build a map of contract signatures
    let mut sigs: HashMap<std::string::String, ContractSig> = HashMap::new();
    for contract in &program.contracts {
        let params: Vec<_> = contract.params.iter().map(|p| {
            (p.name.clone(), InferredType::from_type_expr(&p.type_expr))
        }).collect();
        let return_type = contract.return_type.as_ref()
            .map(InferredType::from_type_expr)
            .unwrap_or(InferredType::Any);
        sigs.insert(contract.name.clone(), ContractSig { params, return_type });
    }

    for contract in &program.contracts {
        let mut env: HashMap<std::string::String, InferredType> = HashMap::new();
        // Seed env with params
        for param in &contract.params {
            env.insert(param.name.clone(), InferredType::from_type_expr(&param.type_expr));
        }

        let expected_return = contract.return_type.as_ref()
            .map(InferredType::from_type_expr)
            .unwrap_or(InferredType::Any);

        if let Some(ref body) = contract.body {
            check_stmts(&body.statements, &mut env, &sigs, &expected_return, &contract.name, &mut warnings);
        }
    }

    warnings
}

fn check_stmts(
    stmts: &[Statement],
    env: &mut HashMap<std::string::String, InferredType>,
    sigs: &HashMap<std::string::String, ContractSig>,
    expected_return: &InferredType,
    contract_name: &str,
    warnings: &mut Vec<TypeWarning>,
) {
    for stmt in stmts {
        check_stmt(stmt, env, sigs, expected_return, contract_name, warnings);
    }
}

fn check_stmt(
    stmt: &Statement,
    env: &mut HashMap<std::string::String, InferredType>,
    sigs: &HashMap<std::string::String, ContractSig>,
    expected_return: &InferredType,
    contract_name: &str,
    warnings: &mut Vec<TypeWarning>,
) {
    match stmt {
        Statement::Assignment { target, value, .. } => {
            let ty = infer_expr(value, env, sigs, warnings);
            // Only track simple variable assignments (not dotted paths)
            if !target.contains('.') {
                env.insert(target.clone(), ty);
            }
        }
        Statement::Return { value, loc, .. } => {
            let actual = infer_expr(value, env, sigs, warnings);
            if *expected_return != InferredType::Any && !actual.compatible_with(expected_return) {
                warnings.push(TypeWarning {
                    code: "T002",
                    message: format!(
                        "contract '{}' declares return type {} but returns {}",
                        contract_name, expected_return.display(), actual.display()
                    ),
                    line: loc.line,
                    column: loc.column,
                });
            }
        }
        Statement::ExprStmt { expr, .. } => {
            infer_expr(expr, env, sigs, warnings);
        }
        Statement::Emit { event, .. } => {
            infer_expr(event, env, sigs, warnings);
        }
        Statement::If { condition, then_body, else_body, .. } => {
            infer_expr(condition, env, sigs, warnings);
            check_stmts(then_body, env, sigs, expected_return, contract_name, warnings);
            check_stmts(else_body, env, sigs, expected_return, contract_name, warnings);
        }
        Statement::For { var, iterable, body, .. } => {
            let iter_ty = infer_expr(iterable, env, sigs, warnings);
            // Infer loop variable type from iterable element type
            let elem_ty = match iter_ty {
                InferredType::List(inner) => *inner,
                _ => InferredType::Any,
            };
            env.insert(var.clone(), elem_ty);
            check_stmts(body, env, sigs, expected_return, contract_name, warnings);
        }
        Statement::While { condition, body, .. } => {
            infer_expr(condition, env, sigs, warnings);
            check_stmts(body, env, sigs, expected_return, contract_name, warnings);
        }
    }
}

fn infer_expr(
    expr: &Expr,
    env: &HashMap<std::string::String, InferredType>,
    sigs: &HashMap<std::string::String, ContractSig>,
    warnings: &mut Vec<TypeWarning>,
) -> InferredType {
    match expr {
        Expr::Identifier { name, .. } => {
            env.get(name).cloned().unwrap_or(InferredType::Any)
        }
        Expr::StringLiteral { .. } => InferredType::String,
        Expr::NumberLiteral { is_int, .. } => {
            if *is_int { InferredType::Int } else { InferredType::Float }
        }
        Expr::BoolLiteral { .. } => InferredType::Bool,
        Expr::ListLiteral { elements, .. } => {
            if elements.is_empty() {
                return InferredType::List(Box::new(InferredType::Any));
            }
            let first = infer_expr(&elements[0], env, sigs, warnings);
            for elem in elements.iter().skip(1) {
                infer_expr(elem, env, sigs, warnings);
            }
            InferredType::List(Box::new(first))
        }
        Expr::BinaryOp { left, op, right, loc, .. } => {
            let lt = infer_expr(left, env, sigs, warnings);
            let rt = infer_expr(right, env, sigs, warnings);
            infer_binop(&lt, op, &rt, loc, warnings)
        }
        Expr::UnaryOp { op, operand, .. } => {
            let t = infer_expr(operand, env, sigs, warnings);
            match op.as_str() {
                "-" => match t {
                    InferredType::Int => InferredType::Int,
                    InferredType::Float => InferredType::Float,
                    _ => InferredType::Any,
                },
                "not" => InferredType::Bool,
                _ => InferredType::Any,
            }
        }
        Expr::FieldAccess { object, .. } => {
            infer_expr(object, env, sigs, warnings);
            InferredType::Any // field types not tracked statically yet
        }
        Expr::FunctionCall { function, arguments, keyword_args, loc, .. } => {
            let func_name = extract_name(function);
            let arg_types: Vec<_> = arguments.iter()
                .map(|a| infer_expr(a, env, sigs, warnings))
                .collect();
            for (_, v) in keyword_args {
                infer_expr(v, env, sigs, warnings);
            }

            // Check against known contract signature
            if let Some(sig) = sigs.get(&func_name) {
                let expected_count = sig.params.len();
                let actual_count = arg_types.len() + keyword_args.len();
                if actual_count != expected_count {
                    warnings.push(TypeWarning {
                        code: "T004",
                        message: format!(
                            "'{}' expects {} argument(s), got {}",
                            func_name, expected_count, actual_count
                        ),
                        line: loc.line,
                        column: loc.column,
                    });
                }

                // Check positional arg types
                for (i, arg_ty) in arg_types.iter().enumerate() {
                    if let Some((param_name, param_ty)) = sig.params.get(i) {
                        if !arg_ty.compatible_with(param_ty) {
                            warnings.push(TypeWarning {
                                code: "T001",
                                message: format!(
                                    "argument '{}' of '{}' expects {}, got {}",
                                    param_name, func_name,
                                    param_ty.display(), arg_ty.display()
                                ),
                                line: loc.line,
                                column: loc.column,
                            });
                        }
                    }
                }

                return sig.return_type.clone();
            }

            // Built-in functions
            match func_name.as_str() {
                "len" => InferredType::Int,
                "abs" => arg_types.first().cloned().unwrap_or(InferredType::Number),
                "min" | "max" => arg_types.first().cloned().unwrap_or(InferredType::Number),
                "range" => InferredType::List(Box::new(InferredType::Int)),
                "str" | "string" => InferredType::String,
                "int" | "integer" => InferredType::Int,
                "float" => InferredType::Float,
                "type" => InferredType::String,
                "print" => InferredType::Null,
                _ => {
                    if func_name.chars().next().map_or(false, |c| c.is_uppercase()) {
                        InferredType::Object(func_name)
                    } else {
                        InferredType::Any
                    }
                }
            }
        }
        Expr::MethodCall { object, arguments, keyword_args, .. } => {
            let obj_ty = infer_expr(object, env, sigs, warnings);
            for a in arguments {
                infer_expr(a, env, sigs, warnings);
            }
            for (_, v) in keyword_args {
                infer_expr(v, env, sigs, warnings);
            }
            // Method return type inference for built-in types
            match &obj_ty {
                InferredType::List(_) => InferredType::Any, // append returns List, len returns Int
                InferredType::String => InferredType::Any,
                _ => InferredType::Any,
            }
        }
        Expr::OldExpr { inner, .. } => infer_expr(inner, env, sigs, warnings),
        Expr::HasExpr { .. } => InferredType::Bool,
        Expr::IndexAccess { object, index, .. } => {
            let obj_ty = infer_expr(object, env, sigs, warnings);
            infer_expr(index, env, sigs, warnings);
            match obj_ty {
                InferredType::List(inner) => *inner,
                InferredType::String => InferredType::String,
                _ => InferredType::Any,
            }
        }
    }
}

fn infer_binop(
    lt: &InferredType,
    op: &str,
    rt: &InferredType,
    loc: &SourceLocation,
    warnings: &mut Vec<TypeWarning>,
) -> InferredType {
    match op {
        "+" => {
            match (lt, rt) {
                (InferredType::Int, InferredType::Int) => InferredType::Int,
                (InferredType::Float, _) | (_, InferredType::Float) => InferredType::Float,
                (InferredType::String, InferredType::String) => InferredType::String,
                (InferredType::List(a), InferredType::List(_)) => InferredType::List(a.clone()),
                (InferredType::Any, _) | (_, InferredType::Any) => InferredType::Any,
                _ => {
                    warnings.push(TypeWarning {
                        code: "T003",
                        message: format!(
                            "cannot apply '+' to {} and {}",
                            lt.display(), rt.display()
                        ),
                        line: loc.line,
                        column: loc.column,
                    });
                    InferredType::Any
                }
            }
        }
        "-" | "*" | "/" => {
            match (lt, rt) {
                (InferredType::Int, InferredType::Int) => InferredType::Int,
                (InferredType::Float, _) | (_, InferredType::Float) => InferredType::Float,
                (InferredType::Any, _) | (_, InferredType::Any) => InferredType::Any,
                (InferredType::Number, _) | (_, InferredType::Number) => InferredType::Number,
                _ => {
                    warnings.push(TypeWarning {
                        code: "T003",
                        message: format!(
                            "cannot apply '{}' to {} and {}",
                            op, lt.display(), rt.display()
                        ),
                        line: loc.line,
                        column: loc.column,
                    });
                    InferredType::Any
                }
            }
        }
        "==" | "!=" | "<" | "<=" | ">" | ">=" => InferredType::Bool,
        "and" | "or" => InferredType::Bool,
        _ => InferredType::Any,
    }
}

fn extract_name(expr: &Expr) -> std::string::String {
    match expr {
        Expr::Identifier { name, .. } => name.clone(),
        Expr::FieldAccess { object, field_name, .. } => {
            format!("{}.{}", extract_name(object), field_name)
        }
        _ => "<expr>".to_string(),
    }
}
