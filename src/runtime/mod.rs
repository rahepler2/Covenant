pub mod stdlib;

use std::collections::HashMap;
use std::fmt;

use crate::ast::*;

#[derive(Debug)]
pub struct RuntimeError {
    pub message: String,
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Runtime error: {}", self.message)
    }
}

impl std::error::Error for RuntimeError {}

#[derive(Debug, Clone)]
pub enum Value {
    Int(i64),
    Float(f64),
    Str(String),
    Bool(bool),
    List(Vec<Value>),
    Object(String, HashMap<String, Value>), // (type_name, fields)
    Null,
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Int(n) => write!(f, "{}", n),
            Value::Float(n) => {
                if *n == (*n as i64) as f64 && !n.is_nan() && !n.is_infinite() {
                    write!(f, "{:.1}", n)
                } else {
                    write!(f, "{}", n)
                }
            }
            Value::Str(s) => write!(f, "{}", s),
            Value::Bool(b) => write!(f, "{}", b),
            Value::List(items) => {
                write!(f, "[")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", item)?;
                }
                write!(f, "]")
            }
            Value::Object(name, fields) => {
                write!(f, "{}{{", name)?;
                for (i, (k, v)) in fields.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}: {}", k, v)?;
                }
                write!(f, "}}")
            }
            Value::Null => write!(f, "null"),
        }
    }
}

impl Value {
    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Bool(b) => *b,
            Value::Int(n) => *n != 0,
            Value::Float(n) => *n != 0.0,
            Value::Str(s) => !s.is_empty(),
            Value::List(l) => !l.is_empty(),
            Value::Null => false,
            Value::Object(..) => true,
        }
    }

    pub fn type_name(&self) -> &str {
        match self {
            Value::Int(_) => "Int",
            Value::Float(_) => "Float",
            Value::Str(_) => "String",
            Value::Bool(_) => "Bool",
            Value::List(_) => "List",
            Value::Object(name, _) => name,
            Value::Null => "Null",
        }
    }

    /// Check if this value matches the declared type expression.
    /// Returns true if the value is compatible with the type.
    pub fn matches_type(&self, type_expr: &TypeExpr) -> bool {
        let type_name = match type_expr {
            TypeExpr::Simple { name, .. } => name.as_str(),
            TypeExpr::Annotated { base, .. } => return self.matches_type(base),
            TypeExpr::Generic { name, params, .. } => {
                // List<T> — check that it's a list and elements match T
                if name == "List" {
                    if let Value::List(items) = self {
                        if let Some(elem_type) = params.first() {
                            return items.iter().all(|item| item.matches_type(elem_type));
                        }
                        return true;
                    }
                    return false;
                }
                // Map<K, V> — check object fields
                if name == "Map" {
                    return matches!(self, Value::Object(..));
                }
                // Optional<T> — null or matches T
                if name == "Optional" {
                    if matches!(self, Value::Null) {
                        return true;
                    }
                    if let Some(inner) = params.first() {
                        return self.matches_type(inner);
                    }
                    return true;
                }
                // Other generic types — check base name
                name.as_str()
            }
            TypeExpr::List { element_type, .. } => {
                if let Value::List(items) = self {
                    return items.iter().all(|item| item.matches_type(element_type));
                }
                return false;
            }
        };
        match type_name {
            "Any" => true,
            "Int" | "Integer" => matches!(self, Value::Int(_)),
            "Float" | "Double" => matches!(self, Value::Float(_) | Value::Int(_)),
            "String" | "Str" => matches!(self, Value::Str(_)),
            "Bool" | "Boolean" => matches!(self, Value::Bool(_)),
            "List" | "Array" => matches!(self, Value::List(_)),
            "Null" | "None" => matches!(self, Value::Null),
            "Number" | "Numeric" => matches!(self, Value::Int(_) | Value::Float(_)),
            _ => {
                // Custom types: check object type name
                if let Value::Object(obj_type, _) = self {
                    obj_type == type_name
                } else {
                    // At low risk, unknown types pass (gradual typing)
                    true
                }
            }
        }
    }

    fn as_number(&self) -> Result<f64, RuntimeError> {
        match self {
            Value::Int(n) => Ok(*n as f64),
            Value::Float(n) => Ok(*n),
            _ => Err(RuntimeError {
                message: format!("Expected number, got {}", self.type_name()),
            }),
        }
    }
}

enum StmtResult {
    Continue,
    Return(Value),
}

const MAX_CALL_DEPTH: usize = 256;
const MAX_RANGE_SIZE: i64 = 10_000_000;

pub struct Interpreter {
    scopes: Vec<HashMap<String, Value>>,
    contracts: HashMap<String, ContractDef>,
    events: Vec<(String, Vec<Value>)>,
    old_snapshot: Option<HashMap<String, Value>>,
    call_depth: usize,
}

impl Interpreter {
    pub fn new() -> Self {
        Self {
            scopes: vec![HashMap::new()],
            contracts: HashMap::new(),
            events: Vec::new(),
            old_snapshot: None,
            call_depth: 0,
        }
    }

    pub fn register_contracts(&mut self, program: &Program) {
        for contract in &program.contracts {
            self.contracts.insert(contract.name.clone(), contract.clone());
        }
    }

    pub fn run_contract(
        &mut self,
        contract_name: &str,
        args: HashMap<String, Value>,
    ) -> Result<Value, RuntimeError> {
        if self.call_depth >= MAX_CALL_DEPTH {
            return Err(RuntimeError {
                message: format!(
                    "Maximum call depth ({}) exceeded — possible infinite recursion",
                    MAX_CALL_DEPTH
                ),
            });
        }
        self.call_depth += 1;

        let contract = self.contracts.get(contract_name).cloned().ok_or_else(|| {
            RuntimeError {
                message: format!("Contract '{}' not found", contract_name),
            }
        })?;

        // Push a new scope and validate argument types
        let mut scope = HashMap::new();
        for param in &contract.params {
            if let Some(value) = args.get(&param.name) {
                // Validate type if declared (skip "Any" for untyped params)
                if param.type_expr.display_name() != "Any" && !value.matches_type(&param.type_expr) {
                    self.call_depth -= 1;
                    return Err(RuntimeError {
                        message: format!(
                            "Type error in '{}': parameter '{}' expects {}, got {} ({})",
                            contract_name, param.name,
                            param.type_expr.display_name(),
                            value.type_name(), value
                        ),
                    });
                }
                scope.insert(param.name.clone(), value.clone());
            }
        }
        // Also insert any extra args not in params (positional overflow)
        for (name, value) in &args {
            if !scope.contains_key(name) {
                scope.insert(name.clone(), value.clone());
            }
        }
        self.scopes.push(scope);

        // Check preconditions
        if let Some(ref precondition) = contract.precondition {
            for (i, condition) in precondition.conditions.iter().enumerate() {
                let result = self.eval_expr(condition)?;
                if !result.is_truthy() {
                    self.call_depth -= 1;
                    self.scopes.pop();
                    return Err(RuntimeError {
                        message: format!(
                            "Precondition {} failed in contract '{}'",
                            i + 1,
                            contract_name
                        ),
                    });
                }
            }
        }

        // Snapshot for old() references
        let snapshot: HashMap<String, Value> = self.current_scope().clone();
        self.old_snapshot = Some(snapshot);

        // Execute body
        let return_value = if let Some(ref body) = contract.body {
            match self.exec_statements(&body.statements)? {
                StmtResult::Return(val) => val,
                StmtResult::Continue => Value::Null,
            }
        } else {
            Value::Null
        };

        // Bind result for postcondition checking
        self.set_var("result", return_value.clone());

        // Check postconditions
        if let Some(ref postcondition) = contract.postcondition {
            for (i, condition) in postcondition.conditions.iter().enumerate() {
                let result = self.eval_expr(condition)?;
                if !result.is_truthy() {
                    self.old_snapshot = None;
                    self.call_depth -= 1;
                    self.scopes.pop();
                    return Err(RuntimeError {
                        message: format!(
                            "Postcondition {} failed in contract '{}'",
                            i + 1,
                            contract_name
                        ),
                    });
                }
            }
        }

        // Validate return type if declared
        if let Some(ref ret_type) = contract.return_type {
            if !return_value.matches_type(ret_type) {
                self.old_snapshot = None;
                self.call_depth -= 1;
                self.scopes.pop();
                return Err(RuntimeError {
                    message: format!(
                        "Type error in '{}': expected return type {}, got {} ({})",
                        contract_name, ret_type.display_name(),
                        return_value.type_name(), return_value
                    ),
                });
            }
        }

        self.old_snapshot = None;
        self.call_depth -= 1;
        self.scopes.pop();
        Ok(return_value)
    }

    pub fn emitted_events(&self) -> &[(String, Vec<Value>)] {
        &self.events
    }

    // ── Statement execution ─────────────────────────────────────────────

    fn exec_statements(&mut self, stmts: &[Statement]) -> Result<StmtResult, RuntimeError> {
        for stmt in stmts {
            match self.exec_statement(stmt)? {
                StmtResult::Return(val) => return Ok(StmtResult::Return(val)),
                StmtResult::Continue => {}
            }
        }
        Ok(StmtResult::Continue)
    }

    fn exec_statement(&mut self, stmt: &Statement) -> Result<StmtResult, RuntimeError> {
        match stmt {
            Statement::Assignment { target, value, .. } => {
                let val = self.eval_expr(value)?;
                if target.contains('.') {
                    self.set_field(target, val)?;
                } else {
                    self.set_var(target, val);
                }
                Ok(StmtResult::Continue)
            }
            Statement::Return { value, .. } => {
                let val = self.eval_expr(value)?;
                Ok(StmtResult::Return(val))
            }
            Statement::Emit { event, .. } => {
                let (name, args) = self.eval_emit_expr(event)?;
                self.events.push((name, args));
                Ok(StmtResult::Continue)
            }
            Statement::ExprStmt { expr, .. } => {
                self.eval_expr(expr)?;
                Ok(StmtResult::Continue)
            }
            Statement::If {
                condition,
                then_body,
                else_body,
                ..
            } => {
                let cond = self.eval_expr(condition)?;
                if cond.is_truthy() {
                    self.exec_statements(then_body)
                } else if !else_body.is_empty() {
                    self.exec_statements(else_body)
                } else {
                    Ok(StmtResult::Continue)
                }
            }
            Statement::For {
                var,
                iterable,
                body,
                ..
            } => {
                let iter_val = self.eval_expr(iterable)?;
                match iter_val {
                    Value::List(items) => {
                        for item in items {
                            self.set_var(var, item);
                            match self.exec_statements(body)? {
                                StmtResult::Return(val) => return Ok(StmtResult::Return(val)),
                                StmtResult::Continue => {}
                            }
                        }
                        Ok(StmtResult::Continue)
                    }
                    _ => Err(RuntimeError {
                        message: format!("Cannot iterate over {}", iter_val.type_name()),
                    }),
                }
            }
            Statement::While {
                condition, body, ..
            } => {
                let mut iterations = 0;
                let max_iterations = 1_000_000;
                loop {
                    let cond = self.eval_expr(condition)?;
                    if !cond.is_truthy() {
                        break;
                    }
                    match self.exec_statements(body)? {
                        StmtResult::Return(val) => return Ok(StmtResult::Return(val)),
                        StmtResult::Continue => {}
                    }
                    iterations += 1;
                    if iterations > max_iterations {
                        return Err(RuntimeError {
                            message: "Loop exceeded maximum iteration limit".to_string(),
                        });
                    }
                }
                Ok(StmtResult::Continue)
            }
        }
    }

    // ── Expression evaluation ───────────────────────────────────────────

    fn eval_expr(&mut self, expr: &Expr) -> Result<Value, RuntimeError> {
        match expr {
            Expr::Identifier { name, .. } => self.lookup_var(name),
            Expr::StringLiteral { value, .. } => Ok(Value::Str(value.clone())),
            Expr::NumberLiteral {
                value, is_int, ..
            } => {
                if *is_int {
                    Ok(Value::Int(*value as i64))
                } else {
                    Ok(Value::Float(*value))
                }
            }
            Expr::BoolLiteral { value, .. } => Ok(Value::Bool(*value)),
            Expr::ListLiteral { elements, .. } => {
                let items: Result<Vec<Value>, _> =
                    elements.iter().map(|e| self.eval_expr(e)).collect();
                Ok(Value::List(items?))
            }
            Expr::BinaryOp {
                left, op, right, ..
            } => {
                // Short-circuit for logical operators
                if op == "and" {
                    let l = self.eval_expr(left)?;
                    if !l.is_truthy() {
                        return Ok(l);
                    }
                    return self.eval_expr(right);
                }
                if op == "or" {
                    let l = self.eval_expr(left)?;
                    if l.is_truthy() {
                        return Ok(l);
                    }
                    return self.eval_expr(right);
                }

                let l = self.eval_expr(left)?;
                let r = self.eval_expr(right)?;
                self.eval_binop(&l, op, &r)
            }
            Expr::UnaryOp { op, operand, .. } => {
                let val = self.eval_expr(operand)?;
                match op.as_str() {
                    "-" => match val {
                        Value::Int(n) => n.checked_neg().map(Value::Int).ok_or_else(|| RuntimeError {
                            message: "Integer overflow in negation".to_string(),
                        }),
                        Value::Float(n) => Ok(Value::Float(-n)),
                        _ => Err(RuntimeError {
                            message: format!("Cannot negate {}", val.type_name()),
                        }),
                    },
                    "not" => Ok(Value::Bool(!val.is_truthy())),
                    _ => Err(RuntimeError {
                        message: format!("Unknown unary operator: {}", op),
                    }),
                }
            }
            Expr::FieldAccess {
                object, field_name, ..
            } => {
                let obj = self.eval_expr(object)?;
                match &obj {
                    Value::Object(_, fields) => {
                        fields.get(field_name).cloned().ok_or_else(|| RuntimeError {
                            message: format!(
                                "Object has no field '{}'",
                                field_name
                            ),
                        })
                    }
                    _ => Err(RuntimeError {
                        message: format!(
                            "Cannot access field '{}' on {}",
                            field_name,
                            obj.type_name()
                        ),
                    }),
                }
            }
            Expr::FunctionCall {
                function,
                arguments,
                keyword_args,
                ..
            } => {
                let func_name = self.extract_call_name_from_expr(function);
                let mut args: Vec<Value> = arguments
                    .iter()
                    .map(|a| self.eval_expr(a))
                    .collect::<Result<_, _>>()?;
                let mut kwarg_map: HashMap<String, Value> = HashMap::new();
                for (k, v) in keyword_args {
                    kwarg_map.insert(k.clone(), self.eval_expr(v)?);
                }

                // Check for built-in functions
                match func_name.as_str() {
                    "print" => {
                        let strs: Vec<String> = args.iter().map(|v| format!("{}", v)).collect();
                        println!("{}", strs.join(" "));
                        Ok(Value::Null)
                    }
                    "len" => {
                        if let Some(val) = args.first() {
                            match val {
                                Value::List(l) => Ok(Value::Int(l.len() as i64)),
                                Value::Str(s) => Ok(Value::Int(s.len() as i64)),
                                _ => Err(RuntimeError {
                                    message: format!("len() not supported for {}", val.type_name()),
                                }),
                            }
                        } else {
                            Err(RuntimeError {
                                message: "len() requires one argument".to_string(),
                            })
                        }
                    }
                    "abs" => {
                        if let Some(val) = args.first() {
                            match val {
                                Value::Int(n) => n.checked_abs().map(Value::Int).ok_or_else(|| RuntimeError {
                                    message: "Integer overflow in abs()".to_string(),
                                }),
                                Value::Float(n) => Ok(Value::Float(n.abs())),
                                _ => Err(RuntimeError {
                                    message: format!("abs() not supported for {}", val.type_name()),
                                }),
                            }
                        } else {
                            Err(RuntimeError {
                                message: "abs() requires one argument".to_string(),
                            })
                        }
                    }
                    "min" => {
                        if args.len() == 2 {
                            let a = args[0].as_number()?;
                            let b = args[1].as_number()?;
                            if a < b {
                                Ok(args.remove(0))
                            } else {
                                Ok(args.remove(1))
                            }
                        } else {
                            Err(RuntimeError {
                                message: "min() requires two arguments".to_string(),
                            })
                        }
                    }
                    "max" => {
                        if args.len() == 2 {
                            let a = args[0].as_number()?;
                            let b = args[1].as_number()?;
                            if a > b {
                                Ok(args.remove(0))
                            } else {
                                Ok(args.remove(1))
                            }
                        } else {
                            Err(RuntimeError {
                                message: "max() requires two arguments".to_string(),
                            })
                        }
                    }
                    "range" => {
                        if let Some(val) = args.first() {
                            let n = match val {
                                Value::Int(n) => *n,
                                _ => {
                                    return Err(RuntimeError {
                                        message: "range() requires an integer".to_string(),
                                    })
                                }
                            };
                            if n < 0 {
                                return Err(RuntimeError {
                                    message: "range() requires a non-negative integer".to_string(),
                                });
                            }
                            if n > MAX_RANGE_SIZE {
                                return Err(RuntimeError {
                                    message: format!(
                                        "range({}) exceeds maximum size of {}",
                                        n, MAX_RANGE_SIZE
                                    ),
                                });
                            }
                            Ok(Value::List((0..n).map(Value::Int).collect()))
                        } else {
                            Err(RuntimeError {
                                message: "range() requires one argument".to_string(),
                            })
                        }
                    }
                    "str" | "string" => {
                        if let Some(val) = args.first() {
                            Ok(Value::Str(format!("{}", val)))
                        } else {
                            Ok(Value::Str(String::new()))
                        }
                    }
                    "int" | "integer" => {
                        match args.first() {
                            Some(Value::Int(n)) => Ok(Value::Int(*n)),
                            Some(Value::Float(f)) => Ok(Value::Int(*f as i64)),
                            Some(Value::Str(s)) => s.parse::<i64>().map(Value::Int).map_err(|_| RuntimeError {
                                message: format!("Cannot convert '{}' to int", s),
                            }),
                            Some(Value::Bool(b)) => Ok(Value::Int(if *b { 1 } else { 0 })),
                            _ => Err(RuntimeError {
                                message: "int() requires one argument".to_string(),
                            }),
                        }
                    }
                    "float" => {
                        match args.first() {
                            Some(Value::Float(f)) => Ok(Value::Float(*f)),
                            Some(Value::Int(n)) => Ok(Value::Float(*n as f64)),
                            Some(Value::Str(s)) => s.parse::<f64>().map(Value::Float).map_err(|_| RuntimeError {
                                message: format!("Cannot convert '{}' to float", s),
                            }),
                            _ => Err(RuntimeError {
                                message: "float() requires one argument".to_string(),
                            }),
                        }
                    }
                    "type" => {
                        if let Some(val) = args.first() {
                            Ok(Value::Str(val.type_name().to_string()))
                        } else {
                            Err(RuntimeError {
                                message: "type() requires one argument".to_string(),
                            })
                        }
                    }
                    _ => {
                        // Check if it's a contract call
                        if self.contracts.contains_key(&func_name) {
                            let contract = self.contracts.get(&func_name).cloned().unwrap();
                            let mut call_args = HashMap::new();
                            for (i, param) in contract.params.iter().enumerate() {
                                if let Some(val) = args.get(i) {
                                    call_args.insert(param.name.clone(), val.clone());
                                } else if let Some(val) = kwarg_map.get(&param.name) {
                                    call_args.insert(param.name.clone(), val.clone());
                                }
                            }
                            return self.run_contract(&func_name, call_args);
                        }

                        // Constructor call: CapitalizedName(args...) creates an object
                        if func_name.chars().next().map_or(false, |c| c.is_uppercase()) {
                            let mut fields = HashMap::new();
                            for (i, val) in args.into_iter().enumerate() {
                                fields.insert(format!("_{}", i), val);
                            }
                            for (k, v) in kwarg_map {
                                fields.insert(k, v);
                            }
                            return Ok(Value::Object(func_name, fields));
                        }

                        // Unknown function — return Null with a warning
                        Ok(Value::Null)
                    }
                }
            }
            Expr::MethodCall {
                object,
                method,
                arguments,
                keyword_args,
                ..
            } => {
                // Check for stdlib module call: web.get(), data.frame(), etc.
                let obj_name = self.extract_call_name_from_expr(object);
                if stdlib::is_stdlib_module(&obj_name) {
                    let args: Vec<Value> = arguments
                        .iter()
                        .map(|a| self.eval_expr(a))
                        .collect::<Result<_, _>>()?;
                    let mut kwarg_map = HashMap::new();
                    for (k, v) in keyword_args {
                        kwarg_map.insert(k.clone(), self.eval_expr(v)?);
                    }
                    return stdlib::call_module_method(&obj_name, method, args, kwarg_map);
                }

                // Check for static method / constructor pattern: TypeName.method(args)
                if obj_name.chars().next().map_or(false, |c| c.is_uppercase()) {
                    // Static constructor call, e.g. TransferResult.success(...)
                    let mut fields = HashMap::new();
                    fields.insert("_method".to_string(), Value::Str(method.clone()));
                    for (i, arg) in arguments.iter().enumerate() {
                        let val = self.eval_expr(arg)?;
                        fields.insert(format!("_{}", i), val);
                    }
                    for (k, v) in keyword_args {
                        let val = self.eval_expr(v)?;
                        fields.insert(k.clone(), val);
                    }
                    return Ok(Value::Object(
                        format!("{}.{}", obj_name, method),
                        fields,
                    ));
                }

                let obj = self.eval_expr(object)?;
                let args: Vec<Value> = arguments
                    .iter()
                    .map(|a| self.eval_expr(a))
                    .collect::<Result<_, _>>()?;
                let mut kwarg_map = HashMap::new();
                for (k, v) in keyword_args {
                    kwarg_map.insert(k.clone(), self.eval_expr(v)?);
                }

                // Built-in methods
                match (&obj, method.as_str()) {
                    (Value::List(l), "append") => {
                        if let Some(val) = args.first() {
                            let mut new_list = l.clone();
                            new_list.push(val.clone());
                            Ok(Value::List(new_list))
                        } else {
                            Err(RuntimeError {
                                message: "append() requires one argument".to_string(),
                            })
                        }
                    }
                    (Value::List(l), "length") | (Value::List(l), "len") => {
                        Ok(Value::Int(l.len() as i64))
                    }
                    (Value::Str(s), "length") | (Value::Str(s), "len") => {
                        Ok(Value::Int(s.len() as i64))
                    }
                    (Value::Str(s), "upper") => Ok(Value::Str(s.to_uppercase())),
                    (Value::Str(s), "lower") => Ok(Value::Str(s.to_lowercase())),
                    (Value::Str(s), "contains") => {
                        if let Some(Value::Str(sub)) = args.first() {
                            Ok(Value::Bool(s.contains(sub.as_str())))
                        } else {
                            Err(RuntimeError {
                                message: "contains() requires a string argument".to_string(),
                            })
                        }
                    }
                    (Value::Object(type_name, fields), _) => {
                        // Check for stdlib type methods (DataFrame, HttpResponse)
                        if stdlib::is_stdlib_type(type_name) {
                            return stdlib::call_type_method(
                                type_name, fields, method, args, kwarg_map,
                            );
                        }

                        // Generic object method: create a derived object
                        let mut result_fields = HashMap::new();
                        result_fields
                            .insert("_source".to_string(), Value::Str(method.to_string()));
                        for (i, val) in args.into_iter().enumerate() {
                            result_fields.insert(format!("_{}", i), val);
                        }
                        for (k, v) in kwarg_map {
                            result_fields.insert(k, v);
                        }
                        for (k, v) in fields {
                            if !result_fields.contains_key(k) {
                                result_fields.insert(k.clone(), v.clone());
                            }
                        }
                        Ok(Value::Object(
                            format!("{}.{}", obj.type_name(), method),
                            result_fields,
                        ))
                    }
                    _ => Err(RuntimeError {
                        message: format!(
                            "Method '{}' not found on {}",
                            method,
                            obj.type_name()
                        ),
                    }),
                }
            }
            Expr::OldExpr { inner, .. } => {
                // Evaluate the inner expression against the pre-execution snapshot
                if let Some(ref snapshot) = self.old_snapshot {
                    // Save current scope, temporarily replace with snapshot, eval, restore
                    let current_scope = self.current_scope().clone();
                    *self.current_scope_mut() = snapshot.clone();
                    let result = self.eval_expr(inner);
                    *self.current_scope_mut() = current_scope;
                    result
                } else {
                    Err(RuntimeError {
                        message: "old() can only be used in postconditions".to_string(),
                    })
                }
            }
            Expr::HasExpr {
                subject,
                capability,
                ..
            } => {
                // Capability checks default to true in the interpreter
                let _subj = self.eval_expr(subject)?;
                let _cap = self.eval_expr(capability)?;
                Ok(Value::Bool(true))
            }
            Expr::IndexAccess {
                object,
                index,
                ..
            } => {
                let obj = self.eval_expr(object)?;
                let idx = self.eval_expr(index)?;
                match (&obj, &idx) {
                    (Value::List(items), Value::Int(i)) => {
                        let i = *i;
                        if i < 0 || i as usize >= items.len() {
                            Err(RuntimeError {
                                message: format!("Index {} out of bounds (list length {})", i, items.len()),
                            })
                        } else {
                            Ok(items[i as usize].clone())
                        }
                    }
                    (Value::Str(s), Value::Int(i)) => {
                        let i = *i;
                        if i < 0 || i as usize >= s.len() {
                            Err(RuntimeError {
                                message: format!("Index {} out of bounds (string length {})", i, s.len()),
                            })
                        } else {
                            Ok(Value::Str(s.chars().nth(i as usize).unwrap().to_string()))
                        }
                    }
                    _ => Err(RuntimeError {
                        message: format!("Cannot index {} with {}", obj.type_name(), idx.type_name()),
                    }),
                }
            }
        }
    }

    fn eval_binop(&self, left: &Value, op: &str, right: &Value) -> Result<Value, RuntimeError> {
        match (left, right) {
            (Value::Int(a), Value::Int(b)) => match op {
                "+" => a.checked_add(*b).map(Value::Int).ok_or_else(|| RuntimeError {
                    message: "Integer overflow in addition".to_string(),
                }),
                "-" => a.checked_sub(*b).map(Value::Int).ok_or_else(|| RuntimeError {
                    message: "Integer overflow in subtraction".to_string(),
                }),
                "*" => a.checked_mul(*b).map(Value::Int).ok_or_else(|| RuntimeError {
                    message: "Integer overflow in multiplication".to_string(),
                }),
                "/" => {
                    if *b == 0 {
                        Err(RuntimeError {
                            message: "Division by zero".to_string(),
                        })
                    } else if a % b == 0 {
                        a.checked_div(*b).map(Value::Int).ok_or_else(|| RuntimeError {
                            message: "Integer overflow in division".to_string(),
                        })
                    } else {
                        Ok(Value::Float(*a as f64 / *b as f64))
                    }
                }
                "==" => Ok(Value::Bool(a == b)),
                "!=" => Ok(Value::Bool(a != b)),
                "<" => Ok(Value::Bool(a < b)),
                "<=" => Ok(Value::Bool(a <= b)),
                ">" => Ok(Value::Bool(a > b)),
                ">=" => Ok(Value::Bool(a >= b)),
                _ => Err(RuntimeError {
                    message: format!("Unknown operator: {}", op),
                }),
            },
            (Value::Float(_), Value::Float(_))
            | (Value::Int(_), Value::Float(_))
            | (Value::Float(_), Value::Int(_)) => {
                let a = left.as_number()?;
                let b = right.as_number()?;
                match op {
                    "+" => Ok(Value::Float(a + b)),
                    "-" => Ok(Value::Float(a - b)),
                    "*" => Ok(Value::Float(a * b)),
                    "/" => {
                        if b == 0.0 {
                            Err(RuntimeError {
                                message: "Division by zero".to_string(),
                            })
                        } else {
                            Ok(Value::Float(a / b))
                        }
                    }
                    "==" => Ok(Value::Bool(a == b)),
                    "!=" => Ok(Value::Bool(a != b)),
                    "<" => Ok(Value::Bool(a < b)),
                    "<=" => Ok(Value::Bool(a <= b)),
                    ">" => Ok(Value::Bool(a > b)),
                    ">=" => Ok(Value::Bool(a >= b)),
                    _ => Err(RuntimeError {
                        message: format!("Unknown operator: {}", op),
                    }),
                }
            }
            (Value::Str(a), Value::Str(b)) => match op {
                "+" => Ok(Value::Str(format!("{}{}", a, b))),
                "==" => Ok(Value::Bool(a == b)),
                "!=" => Ok(Value::Bool(a != b)),
                _ => Err(RuntimeError {
                    message: format!("Cannot apply '{}' to strings", op),
                }),
            },
            (Value::List(a), Value::List(b)) => match op {
                "+" => {
                    let mut combined = a.clone();
                    combined.extend(b.iter().cloned());
                    Ok(Value::List(combined))
                }
                _ => Err(RuntimeError {
                    message: format!("Cannot apply '{}' to lists", op),
                }),
            },
            (Value::Bool(a), Value::Bool(b)) => match op {
                "==" => Ok(Value::Bool(a == b)),
                "!=" => Ok(Value::Bool(a != b)),
                _ => Err(RuntimeError {
                    message: format!("Cannot apply '{}' to booleans", op),
                }),
            },
            (Value::Null, Value::Null) => match op {
                "==" => Ok(Value::Bool(true)),
                "!=" => Ok(Value::Bool(false)),
                _ => Err(RuntimeError {
                    message: format!("Cannot apply '{}' to null", op),
                }),
            },
            (Value::Null, _) | (_, Value::Null) => match op {
                "==" => Ok(Value::Bool(false)),
                "!=" => Ok(Value::Bool(true)),
                _ => Err(RuntimeError {
                    message: format!("Cannot apply '{}' with null", op),
                }),
            },
            _ => Err(RuntimeError {
                message: format!(
                    "Cannot apply '{}' to {} and {}",
                    op,
                    left.type_name(),
                    right.type_name()
                ),
            }),
        }
    }

    // ── Variable access ─────────────────────────────────────────────────

    fn lookup_var(&self, name: &str) -> Result<Value, RuntimeError> {
        // Search scopes from innermost to outermost
        for scope in self.scopes.iter().rev() {
            if let Some(val) = scope.get(name) {
                return Ok(val.clone());
            }
        }
        // Return Null for undefined variables (lenient mode)
        Ok(Value::Null)
    }

    fn set_var(&mut self, name: &str, value: Value) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.to_string(), value);
        }
    }

    fn set_field(&mut self, path: &str, value: Value) -> Result<(), RuntimeError> {
        let parts: Vec<&str> = path.split('.').collect();
        if parts.len() < 2 {
            self.set_var(path, value);
            return Ok(());
        }

        // Get the root object
        let root = parts[0];
        let mut obj = self.lookup_var(root)?;

        // Navigate to the parent of the field to set
        let field_parts = &parts[1..parts.len() - 1];
        let final_field = parts[parts.len() - 1];

        let mut obj_stack: Vec<(String, Value)> = vec![(root.to_string(), obj.clone())];

        for part in field_parts {
            match &obj {
                Value::Object(_, fields) => {
                    obj = fields.get(*part).cloned().unwrap_or(Value::Null);
                    obj_stack.push((part.to_string(), obj.clone()));
                }
                _ => {
                    return Err(RuntimeError {
                        message: format!("Cannot access field '{}' on {}", part, obj.type_name()),
                    });
                }
            }
        }

        // Set the field on the innermost object
        match &mut obj {
            Value::Object(_, ref mut fields) => {
                fields.insert(final_field.to_string(), value);
            }
            _ => {
                return Err(RuntimeError {
                    message: format!(
                        "Cannot set field '{}' on {}",
                        final_field,
                        obj.type_name()
                    ),
                });
            }
        }

        // Rebuild the object chain from inside out
        let mut current = obj;
        let stack_len = obj_stack.len();
        for i in (0..stack_len - 1).rev() {
            let child_key = &obj_stack[i + 1].0;
            if let Value::Object(type_name, mut fields) = obj_stack[i].1.clone() {
                fields.insert(child_key.clone(), current);
                current = Value::Object(type_name, fields);
            }
        }

        self.set_var(root, current);
        Ok(())
    }

    fn current_scope(&self) -> &HashMap<String, Value> {
        self.scopes.last().unwrap()
    }

    fn current_scope_mut(&mut self) -> &mut HashMap<String, Value> {
        self.scopes.last_mut().unwrap()
    }

    fn extract_call_name_from_expr(&self, expr: &Expr) -> String {
        match expr {
            Expr::Identifier { name, .. } => name.clone(),
            Expr::FieldAccess {
                object, field_name, ..
            } => {
                let parent = self.extract_call_name_from_expr(object);
                format!("{}.{}", parent, field_name)
            }
            _ => "<indirect>".to_string(),
        }
    }

    fn eval_emit_expr(&mut self, expr: &Expr) -> Result<(String, Vec<Value>), RuntimeError> {
        match expr {
            Expr::FunctionCall {
                function,
                arguments,
                ..
            } => {
                let name = self.extract_call_name_from_expr(function);
                let args: Vec<Value> = arguments
                    .iter()
                    .map(|a| self.eval_expr(a))
                    .collect::<Result<_, _>>()?;
                Ok((name, args))
            }
            Expr::Identifier { name, .. } => Ok((name.clone(), vec![])),
            _ => {
                let val = self.eval_expr(expr)?;
                Ok(("event".to_string(), vec![val]))
            }
        }
    }
}
