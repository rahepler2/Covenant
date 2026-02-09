//! Covenant Virtual Machine — stack-based bytecode execution engine
//!
//! Design for cache efficiency:
//! - Instructions are Copy enums (~12 bytes) — fit in L1 cache line
//! - Value stack is a contiguous Vec — sequential access pattern
//! - Locals are a flat array per frame — no hash table lookups in hot path
//! - Main dispatch loop is a tight match — branch predictor friendly

use std::collections::HashMap;

use crate::runtime::{Value, RuntimeError};
use crate::runtime::stdlib;
use super::opcodes::{Constant, Instruction};
use super::bytecode::{Module, CompiledContract};

const MAX_CALL_DEPTH: usize = 256;
#[allow(dead_code)]
const MAX_STACK_SIZE: usize = 65536;
const MAX_RANGE_SIZE: i64 = 10_000_000;

/// A call frame on the VM call stack
struct CallFrame {
    code: Vec<Instruction>,
    ip: usize,
    locals: Vec<Value>,
    old_locals: Option<Vec<Value>>,
    local_names: Vec<String>,
    stack_base: usize,
    contract_name: String,
    return_type: Option<String>,
}

/// The Covenant Virtual Machine
pub struct VM {
    stack: Vec<Value>,
    frames: Vec<CallFrame>,
    contracts: Vec<CompiledContract>,
    contract_index: HashMap<String, usize>,
    constants: Vec<Constant>,
    events: Vec<(String, Vec<Value>)>,
    call_depth: usize,
}

impl VM {
    pub fn new(module: Module) -> Self {
        let mut contract_index = HashMap::new();
        for (i, c) in module.contracts.iter().enumerate() {
            contract_index.insert(c.name.clone(), i);
        }
        Self {
            stack: Vec::with_capacity(256),
            frames: Vec::with_capacity(16),
            contracts: module.contracts,
            contract_index,
            constants: module.constants,
            events: Vec::new(),
            call_depth: 0,
        }
    }

    pub fn run_contract(
        &mut self,
        name: &str,
        args: HashMap<String, Value>,
    ) -> Result<Value, RuntimeError> {
        let idx = *self.contract_index.get(name).ok_or_else(|| RuntimeError {
            message: format!("Contract '{}' not found", name),
        })?;

        let contract = &self.contracts[idx];
        let mut locals = vec![Value::Null; contract.local_count as usize];
        // Map named args to param slots
        for (key, val) in &args {
            if let Some(pos) = contract.params.iter().position(|p| p == key) {
                locals[pos] = val.clone();
            }
        }

        // Validate argument types
        for (i, type_name) in contract.param_types.iter().enumerate() {
            if type_name == "Any" || i >= locals.len() {
                continue;
            }
            if !value_matches_type_name(&locals[i], type_name) {
                return Err(RuntimeError {
                    message: format!(
                        "Type error in '{}': parameter '{}' expects {}, got {} ({})",
                        contract.name,
                        contract.params.get(i).map(|s| s.as_str()).unwrap_or("?"),
                        type_name, locals[i].type_name(), locals[i]
                    ),
                });
            }
        }

        let code = contract.code.clone();
        let local_names = contract.local_names.clone();
        let contract_name = contract.name.clone();
        let return_type = contract.return_type.clone();

        self.call_depth = 1;
        self.frames.push(CallFrame {
            code,
            ip: 0,
            locals,
            old_locals: None,
            local_names,
            stack_base: 0,
            contract_name,
            return_type,
        });

        self.run()
    }

    pub fn emitted_events(&self) -> &[(String, Vec<Value>)] {
        &self.events
    }

    // ── Main execution loop ──────────────────────────────────────────
    //
    // Tight loop: fetch instruction (Copy), advance IP, dispatch.
    // All hot data (stack, locals, IP) stays in L1 cache.

    fn run(&mut self) -> Result<Value, RuntimeError> {
        loop {
            if self.frames.is_empty() {
                return Ok(self.stack.pop().unwrap_or(Value::Null));
            }

            let frame = self.frames.last_mut().unwrap();
            if frame.ip >= frame.code.len() {
                // Implicit return null
                let base = frame.stack_base;
                self.frames.pop();
                self.stack.truncate(base);
                self.stack.push(Value::Null);
                self.call_depth = self.call_depth.saturating_sub(1);
                continue;
            }

            // Fetch + advance — single cache line read for small instructions
            let inst = frame.code[frame.ip];
            frame.ip += 1;

            // Drop the mutable borrow before dispatch
            self.dispatch(inst)?;
        }
    }

    // ── Instruction dispatch ─────────────────────────────────────────

    fn dispatch(&mut self, inst: Instruction) -> Result<(), RuntimeError> {
        match inst {
            // ── Constants & Stack ────────────────────────────────────
            Instruction::LoadConst(idx) => {
                self.stack.push(self.const_to_value(idx));
            }
            Instruction::LoadNull => self.stack.push(Value::Null),
            Instruction::LoadTrue => self.stack.push(Value::Bool(true)),
            Instruction::LoadFalse => self.stack.push(Value::Bool(false)),
            Instruction::Pop => { self.stack.pop(); }
            Instruction::Dup => {
                let val = self.stack.last().cloned().unwrap_or(Value::Null);
                self.stack.push(val);
            }

            // ── Locals ──────────────────────────────────────────────
            Instruction::GetLocal(idx) => {
                let frame = self.frames.last().unwrap();
                let val = frame.locals.get(idx as usize).cloned().unwrap_or(Value::Null);
                self.stack.push(val);
            }
            Instruction::SetLocal(idx) => {
                let val = self.stack.pop().unwrap_or(Value::Null);
                let frame = self.frames.last_mut().unwrap();
                let i = idx as usize;
                if i >= frame.locals.len() {
                    frame.locals.resize(i + 1, Value::Null);
                }
                frame.locals[i] = val;
            }

            // ── Arithmetic ──────────────────────────────────────────
            Instruction::Add => {
                let b = self.pop();
                let a = self.pop();
                self.stack.push(self.eval_add(a, b)?);
            }
            Instruction::Sub => {
                let b = self.pop();
                let a = self.pop();
                self.stack.push(self.eval_sub(a, b)?);
            }
            Instruction::Mul => {
                let b = self.pop();
                let a = self.pop();
                self.stack.push(self.eval_mul(a, b)?);
            }
            Instruction::Div => {
                let b = self.pop();
                let a = self.pop();
                self.stack.push(self.eval_div(a, b)?);
            }
            Instruction::Negate => {
                let a = self.pop();
                match a {
                    Value::Int(n) => self.stack.push(Value::Int(
                        n.checked_neg().ok_or_else(|| RuntimeError {
                            message: "Integer overflow in negation".to_string(),
                        })?
                    )),
                    Value::Float(n) => self.stack.push(Value::Float(-n)),
                    _ => return Err(RuntimeError {
                        message: format!("Cannot negate {}", a.type_name()),
                    }),
                }
            }

            // ── Comparison ──────────────────────────────────────────
            Instruction::Equal => {
                let b = self.pop();
                let a = self.pop();
                self.stack.push(Value::Bool(self.values_equal(&a, &b)));
            }
            Instruction::NotEqual => {
                let b = self.pop();
                let a = self.pop();
                self.stack.push(Value::Bool(!self.values_equal(&a, &b)));
            }
            Instruction::Less => {
                let b = self.pop();
                let a = self.pop();
                self.stack.push(Value::Bool(self.values_less(&a, &b)?));
            }
            Instruction::LessEqual => {
                let b = self.pop();
                let a = self.pop();
                self.stack.push(Value::Bool(self.values_less(&a, &b)? || self.values_equal(&a, &b)));
            }
            Instruction::Greater => {
                let b = self.pop();
                let a = self.pop();
                self.stack.push(Value::Bool(self.values_less(&b, &a)?));
            }
            Instruction::GreaterEqual => {
                let b = self.pop();
                let a = self.pop();
                self.stack.push(Value::Bool(self.values_less(&b, &a)? || self.values_equal(&a, &b)));
            }
            Instruction::Not => {
                let a = self.pop();
                self.stack.push(Value::Bool(!a.is_truthy()));
            }

            // ── Control Flow ────────────────────────────────────────
            Instruction::Jump(offset) => {
                let frame = self.frames.last_mut().unwrap();
                frame.ip = (frame.ip as i32 + offset) as usize;
            }
            Instruction::JumpIfFalse(offset) => {
                let val = self.pop();
                if !val.is_truthy() {
                    let frame = self.frames.last_mut().unwrap();
                    frame.ip = (frame.ip as i32 + offset) as usize;
                }
            }
            Instruction::JumpIfTrue(offset) => {
                let val = self.pop();
                if val.is_truthy() {
                    let frame = self.frames.last_mut().unwrap();
                    frame.ip = (frame.ip as i32 + offset) as usize;
                }
            }

            // ── Contract Calls ──────────────────────────────────────
            Instruction::CallContract(name_idx, pos_count, kw_count) => {
                self.do_call_contract(name_idx, pos_count, kw_count)?;
            }
            Instruction::Return => {
                let value = self.pop();
                let frame = self.frames.pop().unwrap();
                // Validate return type if declared
                if let Some(ref ret_type) = frame.return_type {
                    if !value_matches_type_name(&value, ret_type) {
                        return Err(RuntimeError {
                            message: format!(
                                "Type error in '{}': expected return type {}, got {} ({})",
                                frame.contract_name, ret_type,
                                value.type_name(), value
                            ),
                        });
                    }
                }
                self.stack.truncate(frame.stack_base);
                self.stack.push(value);
                self.call_depth = self.call_depth.saturating_sub(1);
            }

            // ── Objects ─────────────────────────────────────────────
            Instruction::NewObject(type_idx, field_count) => {
                let type_name = self.get_const_str(type_idx);
                let mut fields = HashMap::new();
                for _ in 0..field_count {
                    let key_val = self.pop();
                    let value = self.pop();
                    if let Value::Str(key) = key_val {
                        fields.insert(key, value);
                    }
                }
                self.stack.push(Value::Object(type_name, fields));
            }
            Instruction::GetField(name_idx) => {
                let field_name = self.get_const_str(name_idx);
                let obj = self.pop();
                match &obj {
                    Value::Object(_, fields) => {
                        self.stack.push(
                            fields.get(&field_name).cloned().unwrap_or(Value::Null)
                        );
                    }
                    _ => return Err(RuntimeError {
                        message: format!("Cannot access field '{}' on {}", field_name, obj.type_name()),
                    }),
                }
            }
            Instruction::SetField(path_idx) => {
                let path = self.get_const_str(path_idx);
                let value = self.pop();
                self.do_set_field(&path, value)?;
            }

            // ── Lists ───────────────────────────────────────────────
            Instruction::NewList(count) => {
                let start = self.stack.len().saturating_sub(count as usize);
                let items: Vec<Value> = self.stack.drain(start..).collect();
                self.stack.push(Value::List(items));
            }
            Instruction::ListIndex => {
                let idx = self.pop();
                let list = self.pop();
                match (&list, &idx) {
                    (Value::List(items), Value::Int(i)) => {
                        self.stack.push(
                            items.get(*i as usize).cloned().unwrap_or(Value::Null)
                        );
                    }
                    _ => return Err(RuntimeError {
                        message: format!("Cannot index {} with {}", list.type_name(), idx.type_name()),
                    }),
                }
            }

            // ── Built-ins ───────────────────────────────────────────
            Instruction::Print(count) => {
                let start = self.stack.len().saturating_sub(count as usize);
                let args: Vec<Value> = self.stack.drain(start..).collect();
                let strs: Vec<String> = args.iter().map(|v| format!("{}", v)).collect();
                println!("{}", strs.join(" "));
            }
            Instruction::CallBuiltin(name_idx, arg_count) => {
                let name = self.get_const_str(name_idx);
                self.do_call_builtin(&name, arg_count)?;
            }

            // ── Standard Library ────────────────────────────────────
            Instruction::CallModule(mod_idx, method_idx, pos_count, kw_count) => {
                let module = self.get_const_str(mod_idx);
                let method = self.get_const_str(method_idx);
                self.do_call_module(&module, &method, pos_count, kw_count)?;
            }
            Instruction::CallMethod(method_idx, pos_count, kw_count) => {
                let method = self.get_const_str(method_idx);
                self.do_call_method(&method, pos_count, kw_count)?;
            }

            // ── Contract Enforcement ────────────────────────────────
            Instruction::CheckPre(idx) => {
                let val = self.pop();
                if !val.is_truthy() {
                    let name = self.frames.last()
                        .map(|f| f.contract_name.as_str())
                        .unwrap_or("?");
                    return Err(RuntimeError {
                        message: format!(
                            "Precondition {} failed in contract '{}'",
                            idx + 1, name
                        ),
                    });
                }
            }
            Instruction::CheckPost(idx) => {
                let val = self.pop();
                if !val.is_truthy() {
                    let name = self.frames.last()
                        .map(|f| f.contract_name.as_str())
                        .unwrap_or("?");
                    return Err(RuntimeError {
                        message: format!(
                            "Postcondition {} failed in contract '{}'",
                            idx + 1, name
                        ),
                    });
                }
            }
            Instruction::Snapshot => {
                let frame = self.frames.last_mut().unwrap();
                frame.old_locals = Some(frame.locals.clone());
            }
            Instruction::BeginOld => {
                let frame = self.frames.last_mut().unwrap();
                if let Some(ref mut old) = frame.old_locals {
                    std::mem::swap(&mut frame.locals, old);
                }
            }
            Instruction::EndOld => {
                let frame = self.frames.last_mut().unwrap();
                if let Some(ref mut old) = frame.old_locals {
                    std::mem::swap(&mut frame.locals, old);
                }
            }

            // ── Events ──────────────────────────────────────────────
            Instruction::EmitEvent(name_idx, arg_count) => {
                let name = self.get_const_str(name_idx);
                let start = self.stack.len().saturating_sub(arg_count as usize);
                let args: Vec<Value> = self.stack.drain(start..).collect();
                self.events.push((name, args));
            }

            // ── Capabilities ────────────────────────────────────────
            Instruction::HasCapability => {
                self.pop(); // capability
                self.pop(); // subject
                self.stack.push(Value::Bool(true)); // always granted at runtime
            }
        }
        Ok(())
    }

    // ── Stack helpers ────────────────────────────────────────────────

    #[inline(always)]
    fn pop(&mut self) -> Value {
        self.stack.pop().unwrap_or(Value::Null)
    }

    fn const_to_value(&self, idx: u16) -> Value {
        match self.constants.get(idx as usize) {
            Some(Constant::Int(n)) => Value::Int(*n),
            Some(Constant::Float(n)) => Value::Float(*n),
            Some(Constant::Str(s)) => Value::Str(s.clone()),
            Some(Constant::Bool(b)) => Value::Bool(*b),
            Some(Constant::Null) | None => Value::Null,
        }
    }

    fn get_const_str(&self, idx: u16) -> String {
        match self.constants.get(idx as usize) {
            Some(Constant::Str(s)) => s.clone(),
            _ => String::new(),
        }
    }

    // ── Arithmetic implementation ────────────────────────────────────
    // Inlined for the hot path. Checked arithmetic prevents overflow.

    #[inline]
    fn eval_add(&self, a: Value, b: Value) -> Result<Value, RuntimeError> {
        match (&a, &b) {
            (Value::Int(x), Value::Int(y)) => x.checked_add(*y)
                .map(Value::Int)
                .ok_or_else(|| RuntimeError { message: "Integer overflow in addition".to_string() }),
            (Value::Float(x), Value::Float(y)) => Ok(Value::Float(x + y)),
            (Value::Int(x), Value::Float(y)) => Ok(Value::Float(*x as f64 + y)),
            (Value::Float(x), Value::Int(y)) => Ok(Value::Float(x + *y as f64)),
            (Value::Str(x), Value::Str(y)) => Ok(Value::Str(format!("{}{}", x, y))),
            (Value::List(x), Value::List(y)) => {
                let mut combined = x.clone();
                combined.extend(y.iter().cloned());
                Ok(Value::List(combined))
            }
            _ => Err(RuntimeError {
                message: format!("Cannot add {} and {}", a.type_name(), b.type_name()),
            }),
        }
    }

    #[inline]
    fn eval_sub(&self, a: Value, b: Value) -> Result<Value, RuntimeError> {
        match (&a, &b) {
            (Value::Int(x), Value::Int(y)) => x.checked_sub(*y)
                .map(Value::Int)
                .ok_or_else(|| RuntimeError { message: "Integer overflow in subtraction".to_string() }),
            (Value::Float(x), Value::Float(y)) => Ok(Value::Float(x - y)),
            (Value::Int(x), Value::Float(y)) => Ok(Value::Float(*x as f64 - y)),
            (Value::Float(x), Value::Int(y)) => Ok(Value::Float(x - *y as f64)),
            _ => Err(RuntimeError {
                message: format!("Cannot subtract {} and {}", a.type_name(), b.type_name()),
            }),
        }
    }

    #[inline]
    fn eval_mul(&self, a: Value, b: Value) -> Result<Value, RuntimeError> {
        match (&a, &b) {
            (Value::Int(x), Value::Int(y)) => x.checked_mul(*y)
                .map(Value::Int)
                .ok_or_else(|| RuntimeError { message: "Integer overflow in multiplication".to_string() }),
            (Value::Float(x), Value::Float(y)) => Ok(Value::Float(x * y)),
            (Value::Int(x), Value::Float(y)) => Ok(Value::Float(*x as f64 * y)),
            (Value::Float(x), Value::Int(y)) => Ok(Value::Float(x * *y as f64)),
            _ => Err(RuntimeError {
                message: format!("Cannot multiply {} and {}", a.type_name(), b.type_name()),
            }),
        }
    }

    #[inline]
    fn eval_div(&self, a: Value, b: Value) -> Result<Value, RuntimeError> {
        match (&a, &b) {
            (Value::Int(_), Value::Int(0)) | (Value::Float(_), Value::Int(0)) => {
                Err(RuntimeError { message: "Division by zero".to_string() })
            }
            (_, Value::Float(y)) if *y == 0.0 => {
                Err(RuntimeError { message: "Division by zero".to_string() })
            }
            (Value::Int(x), Value::Int(y)) => {
                if x % y == 0 {
                    x.checked_div(*y).map(Value::Int).ok_or_else(|| RuntimeError {
                        message: "Integer overflow in division".to_string(),
                    })
                } else {
                    Ok(Value::Float(*x as f64 / *y as f64))
                }
            }
            (Value::Float(x), Value::Float(y)) => Ok(Value::Float(x / y)),
            (Value::Int(x), Value::Float(y)) => Ok(Value::Float(*x as f64 / y)),
            (Value::Float(x), Value::Int(y)) => Ok(Value::Float(x / *y as f64)),
            _ => Err(RuntimeError {
                message: format!("Cannot divide {} by {}", a.type_name(), b.type_name()),
            }),
        }
    }

    fn values_equal(&self, a: &Value, b: &Value) -> bool {
        match (a, b) {
            (Value::Int(x), Value::Int(y)) => x == y,
            (Value::Float(x), Value::Float(y)) => x == y,
            (Value::Int(x), Value::Float(y)) => *x as f64 == *y,
            (Value::Float(x), Value::Int(y)) => *x == *y as f64,
            (Value::Str(x), Value::Str(y)) => x == y,
            (Value::Bool(x), Value::Bool(y)) => x == y,
            (Value::Null, Value::Null) => true,
            _ => false,
        }
    }

    fn values_less(&self, a: &Value, b: &Value) -> Result<bool, RuntimeError> {
        match (a, b) {
            (Value::Int(x), Value::Int(y)) => Ok(x < y),
            (Value::Float(x), Value::Float(y)) => Ok(x < y),
            (Value::Int(x), Value::Float(y)) => Ok((*x as f64) < *y),
            (Value::Float(x), Value::Int(y)) => Ok(*x < *y as f64),
            (Value::Str(x), Value::Str(y)) => Ok(x < y),
            _ => Err(RuntimeError {
                message: format!("Cannot compare {} and {}", a.type_name(), b.type_name()),
            }),
        }
    }

    // ── Complex operations ───────────────────────────────────────────

    fn do_call_contract(&mut self, name_idx: u16, pos_count: u16, kw_count: u16) -> Result<(), RuntimeError> {
        let name = self.get_const_str(name_idx);

        // Pop kwargs
        let mut kwargs = HashMap::new();
        for _ in 0..kw_count {
            let key_val = self.pop();
            let value = self.pop();
            if let Value::Str(key) = key_val {
                kwargs.insert(key, value);
            }
        }

        // Pop positional args (reverse — they were pushed left-to-right)
        let mut pos_args = Vec::with_capacity(pos_count as usize);
        for _ in 0..pos_count {
            pos_args.push(self.pop());
        }
        pos_args.reverse();

        // Check if it's a constructor (CapitalizedName)
        if name.starts_with(|c: char| c.is_uppercase()) {
            let mut fields = HashMap::new();
            for (i, val) in pos_args.into_iter().enumerate() {
                fields.insert(format!("_{}", i), val);
            }
            for (k, v) in kwargs {
                fields.insert(k, v);
            }
            self.stack.push(Value::Object(name, fields));
            return Ok(());
        }

        // Look up contract
        let idx = match self.contract_index.get(&name) {
            Some(i) => *i,
            None => {
                // Unknown function — push Null (lenient, like tree-walker)
                self.stack.push(Value::Null);
                return Ok(());
            }
        };

        if self.call_depth >= MAX_CALL_DEPTH {
            return Err(RuntimeError {
                message: format!("Maximum call depth ({}) exceeded", MAX_CALL_DEPTH),
            });
        }

        let contract = &self.contracts[idx];
        let mut locals = vec![Value::Null; contract.local_count as usize];

        // Assign positional args
        for (i, val) in pos_args.into_iter().enumerate() {
            if i < locals.len() {
                locals[i] = val;
            }
        }
        // Assign keyword args by name
        for (key, val) in kwargs {
            if let Some(pos) = contract.params.iter().position(|p| p == &key) {
                if pos < locals.len() {
                    locals[pos] = val;
                }
            }
        }

        // Validate argument types
        for (i, type_name) in contract.param_types.iter().enumerate() {
            if type_name == "Any" || i >= locals.len() {
                continue;
            }
            if !value_matches_type_name(&locals[i], type_name) {
                return Err(RuntimeError {
                    message: format!(
                        "Type error in '{}': parameter '{}' expects {}, got {} ({})",
                        contract.name,
                        contract.params.get(i).map(|s| s.as_str()).unwrap_or("?"),
                        type_name, locals[i].type_name(), locals[i]
                    ),
                });
            }
        }

        let code = contract.code.clone();
        let local_names = contract.local_names.clone();
        let contract_name = contract.name.clone();
        let return_type = contract.return_type.clone();

        self.call_depth += 1;
        self.frames.push(CallFrame {
            code,
            ip: 0,
            locals,
            old_locals: None,
            local_names,
            stack_base: self.stack.len(),
            contract_name,
            return_type,
        });

        Ok(())
    }

    fn do_call_builtin(&mut self, name: &str, arg_count: u16) -> Result<(), RuntimeError> {
        let start = self.stack.len().saturating_sub(arg_count as usize);
        let args: Vec<Value> = self.stack.drain(start..).collect();

        let result = match name {
            "len" => {
                match args.first() {
                    Some(Value::List(l)) => Value::Int(l.len() as i64),
                    Some(Value::Str(s)) => Value::Int(s.len() as i64),
                    _ => return Err(RuntimeError {
                        message: "len() requires a list or string".to_string(),
                    }),
                }
            }
            "abs" => {
                match args.first() {
                    Some(Value::Int(n)) => Value::Int(n.checked_abs().ok_or_else(|| RuntimeError {
                        message: "Integer overflow in abs()".to_string(),
                    })?),
                    Some(Value::Float(n)) => Value::Float(n.abs()),
                    _ => return Err(RuntimeError {
                        message: "abs() requires a number".to_string(),
                    }),
                }
            }
            "min" => {
                if args.len() >= 2 {
                    let a = as_f64(&args[0])?;
                    let b = as_f64(&args[1])?;
                    if a < b { args[0].clone() } else { args[1].clone() }
                } else {
                    return Err(RuntimeError { message: "min() requires two arguments".to_string() });
                }
            }
            "max" => {
                if args.len() >= 2 {
                    let a = as_f64(&args[0])?;
                    let b = as_f64(&args[1])?;
                    if a > b { args[0].clone() } else { args[1].clone() }
                } else {
                    return Err(RuntimeError { message: "max() requires two arguments".to_string() });
                }
            }
            "range" => {
                match args.first() {
                    Some(Value::Int(n)) => {
                        if *n < 0 {
                            return Err(RuntimeError { message: "range() requires non-negative integer".to_string() });
                        }
                        if *n > MAX_RANGE_SIZE {
                            return Err(RuntimeError { message: format!("range({}) exceeds maximum {}", n, MAX_RANGE_SIZE) });
                        }
                        Value::List((0..*n).map(Value::Int).collect())
                    }
                    _ => return Err(RuntimeError { message: "range() requires an integer".to_string() }),
                }
            }
            "str" | "string" => {
                if let Some(val) = args.first() {
                    Value::Str(format!("{}", val))
                } else {
                    Value::Str(String::new())
                }
            }
            "int" | "integer" => {
                match args.first() {
                    Some(Value::Int(n)) => Value::Int(*n),
                    Some(Value::Float(f)) => Value::Int(*f as i64),
                    Some(Value::Str(s)) => Value::Int(s.parse::<i64>().map_err(|_| RuntimeError {
                        message: format!("Cannot convert '{}' to int", s),
                    })?),
                    Some(Value::Bool(b)) => Value::Int(if *b { 1 } else { 0 }),
                    _ => return Err(RuntimeError {
                        message: "int() requires one argument".to_string(),
                    }),
                }
            }
            "float" => {
                match args.first() {
                    Some(Value::Float(f)) => Value::Float(*f),
                    Some(Value::Int(n)) => Value::Float(*n as f64),
                    Some(Value::Str(s)) => Value::Float(s.parse::<f64>().map_err(|_| RuntimeError {
                        message: format!("Cannot convert '{}' to float", s),
                    })?),
                    _ => return Err(RuntimeError {
                        message: "float() requires one argument".to_string(),
                    }),
                }
            }
            "type" => {
                if let Some(val) = args.first() {
                    Value::Str(val.type_name().to_string())
                } else {
                    return Err(RuntimeError {
                        message: "type() requires one argument".to_string(),
                    });
                }
            }
            _ => Value::Null,
        };

        self.stack.push(result);
        Ok(())
    }

    fn do_call_module(
        &mut self,
        module: &str,
        method: &str,
        pos_count: u16,
        kw_count: u16,
    ) -> Result<(), RuntimeError> {
        // Pop kwargs
        let mut kwargs = HashMap::new();
        for _ in 0..kw_count {
            let key_val = self.pop();
            let value = self.pop();
            if let Value::Str(key) = key_val {
                kwargs.insert(key, value);
            }
        }
        // Pop positional args
        let mut args = Vec::with_capacity(pos_count as usize);
        for _ in 0..pos_count {
            args.push(self.pop());
        }
        args.reverse();

        let result = stdlib::call_module_method(module, method, args, kwargs)?;
        self.stack.push(result);
        Ok(())
    }

    fn do_call_method(
        &mut self,
        method: &str,
        pos_count: u16,
        kw_count: u16,
    ) -> Result<(), RuntimeError> {
        // Pop kwargs
        let mut kwargs = HashMap::new();
        for _ in 0..kw_count {
            let key_val = self.pop();
            let value = self.pop();
            if let Value::Str(key) = key_val {
                kwargs.insert(key, value);
            }
        }
        // Pop positional args
        let mut args = Vec::with_capacity(pos_count as usize);
        for _ in 0..pos_count {
            args.push(self.pop());
        }
        args.reverse();

        // Pop the object (pushed before args by compiler)
        let obj = self.pop();

        // Dispatch based on object type
        let result = match &obj {
            // Stdlib types (DataFrame, HttpResponse)
            Value::Object(type_name, fields) if stdlib::is_stdlib_type(type_name) => {
                stdlib::call_type_method(type_name, fields, method, args, kwargs)?
            }

            // Built-in value methods
            Value::List(l) => match method {
                "append" => {
                    let mut new_list = l.clone();
                    if let Some(val) = args.first() {
                        new_list.push(val.clone());
                    }
                    Value::List(new_list)
                }
                "length" | "len" => Value::Int(l.len() as i64),
                _ => return Err(RuntimeError {
                    message: format!("List has no method '{}'", method),
                }),
            },
            Value::Str(s) => match method {
                "length" | "len" => Value::Int(s.len() as i64),
                "upper" => Value::Str(s.to_uppercase()),
                "lower" => Value::Str(s.to_lowercase()),
                "contains" => {
                    if let Some(Value::Str(sub)) = args.first() {
                        Value::Bool(s.contains(sub.as_str()))
                    } else {
                        Value::Bool(false)
                    }
                }
                _ => return Err(RuntimeError {
                    message: format!("String has no method '{}'", method),
                }),
            },

            // Generic object method — create derived object (like tree-walker)
            Value::Object(type_name, fields) => {
                let mut result_fields = HashMap::new();
                result_fields.insert("_source".to_string(), Value::Str(method.to_string()));
                for (i, val) in args.into_iter().enumerate() {
                    result_fields.insert(format!("_{}", i), val);
                }
                for (k, v) in kwargs {
                    result_fields.insert(k, v);
                }
                for (k, v) in fields {
                    if !result_fields.contains_key(k) {
                        result_fields.insert(k.clone(), v.clone());
                    }
                }
                Value::Object(format!("{}.{}", type_name, method), result_fields)
            }

            _ => return Err(RuntimeError {
                message: format!("Method '{}' not found on {}", method, obj.type_name()),
            }),
        };

        self.stack.push(result);
        Ok(())
    }

    fn do_set_field(&mut self, path: &str, value: Value) -> Result<(), RuntimeError> {
        let parts: Vec<&str> = path.split('.').collect();
        if parts.len() < 2 {
            return Ok(());
        }

        let root = parts[0];
        let frame = self.frames.last_mut().unwrap();

        // Find root local
        let root_slot = frame.local_names.iter().position(|n| n == root);
        let root_slot = match root_slot {
            Some(s) => s,
            None => return Ok(()), // unknown variable
        };

        let mut obj = frame.locals[root_slot].clone();
        let field_parts = &parts[1..parts.len() - 1];
        let final_field = parts[parts.len() - 1];

        let mut obj_stack: Vec<(String, Value)> = vec![(root.to_string(), obj.clone())];
        for part in field_parts {
            match &obj {
                Value::Object(_, fields) => {
                    obj = fields.get(*part).cloned().unwrap_or(Value::Null);
                    obj_stack.push((part.to_string(), obj.clone()));
                }
                _ => return Err(RuntimeError {
                    message: format!("Cannot access field '{}' on {}", part, obj.type_name()),
                }),
            }
        }

        // Set the field on the innermost object
        match &mut obj {
            Value::Object(_, ref mut fields) => {
                fields.insert(final_field.to_string(), value);
            }
            _ => return Err(RuntimeError {
                message: format!("Cannot set field '{}' on {}", final_field, obj.type_name()),
            }),
        }

        // Rebuild chain
        let mut current = obj;
        let stack_len = obj_stack.len();
        for i in (0..stack_len - 1).rev() {
            let child_key = &obj_stack[i + 1].0;
            if let Value::Object(type_name, mut fields) = obj_stack[i].1.clone() {
                fields.insert(child_key.clone(), current);
                current = Value::Object(type_name, fields);
            }
        }

        let frame = self.frames.last_mut().unwrap();
        frame.locals[root_slot] = current;
        Ok(())
    }
}

fn as_f64(v: &Value) -> Result<f64, RuntimeError> {
    match v {
        Value::Int(n) => Ok(*n as f64),
        Value::Float(n) => Ok(*n),
        _ => Err(RuntimeError {
            message: format!("Expected number, got {}", v.type_name()),
        }),
    }
}

/// Check if a runtime Value matches a type name string (for VM type checking)
fn value_matches_type_name(value: &Value, type_name: &str) -> bool {
    match type_name {
        "Any" => true,
        "Int" | "Integer" => matches!(value, Value::Int(_)),
        "Float" | "Double" => matches!(value, Value::Float(_) | Value::Int(_)),
        "String" | "Str" => matches!(value, Value::Str(_)),
        "Bool" | "Boolean" => matches!(value, Value::Bool(_)),
        "List" | "Array" => matches!(value, Value::List(_)),
        "Null" | "None" => matches!(value, Value::Null),
        "Number" | "Numeric" => matches!(value, Value::Int(_) | Value::Float(_)),
        _ => {
            // Generic types like List<Int> — just check base
            if type_name.starts_with("List<") {
                return matches!(value, Value::List(_));
            }
            if type_name.starts_with("Optional<") {
                return matches!(value, Value::Null) || true; // accept anything for Optional
            }
            // Custom object types
            if let Value::Object(obj_type, _) = value {
                obj_type == type_name
            } else {
                true // unknown types pass (gradual typing)
            }
        }
    }
}
