//! Bytecode compiler: AST → Instruction stream
//!
//! Walks the AST and emits bytecode for each contract.
//! Each `return` in a contract body jumps to the postcondition section
//! so that contract enforcement happens before the actual return.

use crate::ast::*;
use super::opcodes::{Constant, Instruction};
use super::bytecode::{Module, CompiledContract};

/// Top-level compiler — compiles a full Program into a Module
pub struct Compiler {
    constants: Vec<Constant>,
    contract_names: Vec<String>,
}

impl Compiler {
    pub fn new() -> Self {
        Self {
            constants: Vec::new(),
            contract_names: Vec::new(),
        }
    }

    pub fn compile(&mut self, program: &Program) -> Module {
        // Collect contract names for call resolution
        self.contract_names = program.contracts.iter().map(|c| c.name.clone()).collect();

        let mut contracts = Vec::new();
        for contract in &program.contracts {
            contracts.push(self.compile_contract(contract));
        }

        Module {
            constants: std::mem::take(&mut self.constants),
            contracts,
        }
    }

    fn compile_contract(&mut self, contract: &ContractDef) -> CompiledContract {
        let mut cc = ContractCompiler::new(&mut self.constants, &self.contract_names);

        // Reserve local slots for parameters
        for param in &contract.params {
            cc.resolve_local(&param.name);
        }
        // Reserve "result" slot
        cc.result_slot = cc.resolve_local("result");

        // ── Preconditions ────────────────────────────────────────────
        if let Some(ref pre) = contract.precondition {
            for (i, cond) in pre.conditions.iter().enumerate() {
                cc.compile_expression(cond);
                cc.emit(Instruction::CheckPre(i as u16));
            }
        }

        // ── Snapshot for old() ───────────────────────────────────────
        cc.emit(Instruction::Snapshot);

        // ── Body ─────────────────────────────────────────────────────
        if let Some(ref body) = contract.body {
            cc.compile_statements(&body.statements);
        }

        // If body falls through without return, result = null
        cc.emit(Instruction::LoadNull);
        cc.emit(Instruction::SetLocal(cc.result_slot));

        // ── Postcondition landing pad ────────────────────────────────
        let post_start = cc.current_offset();
        // Patch all return jumps to land here
        for idx in std::mem::take(&mut cc.return_patches) {
            cc.patch_jump_to(idx, post_start);
        }

        if let Some(ref post) = contract.postcondition {
            for (i, cond) in post.conditions.iter().enumerate() {
                cc.compile_expression(cond);
                cc.emit(Instruction::CheckPost(i as u16));
            }
        }

        // Final return
        cc.emit(Instruction::GetLocal(cc.result_slot));
        cc.emit(Instruction::Return);

        let local_count = cc.locals.len() as u16;
        let local_names = cc.locals.clone();
        let params = contract.params.iter().map(|p| p.name.clone()).collect();

        CompiledContract {
            name: contract.name.clone(),
            params,
            local_count,
            local_names,
            code: cc.instructions,
        }
    }

    /// Add a constant, deduplicating
    fn add_constant_shared(constants: &mut Vec<Constant>, c: Constant) -> u16 {
        if let Some(pos) = constants.iter().position(|existing| existing == &c) {
            pos as u16
        } else {
            let idx = constants.len() as u16;
            constants.push(c);
            idx
        }
    }
}

// ── Per-contract compiler state ──────────────────────────────────────────

struct ContractCompiler<'a> {
    instructions: Vec<Instruction>,
    locals: Vec<String>,
    constants: &'a mut Vec<Constant>,
    #[allow(dead_code)]
    contract_names: &'a [String],
    return_patches: Vec<usize>,
    result_slot: u16,
    hidden_counter: u16,
}

impl<'a> ContractCompiler<'a> {
    fn new(constants: &'a mut Vec<Constant>, contract_names: &'a [String]) -> Self {
        Self {
            instructions: Vec::new(),
            locals: Vec::new(),
            constants,
            contract_names,
            return_patches: Vec::new(),
            result_slot: 0,
            hidden_counter: 0,
        }
    }

    // ── Helpers ──────────────────────────────────────────────────────

    fn emit(&mut self, inst: Instruction) {
        self.instructions.push(inst);
    }

    fn current_offset(&self) -> usize {
        self.instructions.len()
    }

    fn emit_jump(&mut self, inst: Instruction) -> usize {
        let idx = self.instructions.len();
        self.instructions.push(inst);
        idx
    }

    /// Patch a jump instruction at `idx` to target `self.current_offset()`
    fn patch_jump(&mut self, idx: usize) {
        let target = self.instructions.len();
        self.patch_jump_to(idx, target);
    }

    fn patch_jump_to(&mut self, idx: usize, target: usize) {
        let offset = (target as i32) - (idx as i32) - 1;
        match &mut self.instructions[idx] {
            Instruction::Jump(ref mut o)
            | Instruction::JumpIfFalse(ref mut o)
            | Instruction::JumpIfTrue(ref mut o) => *o = offset,
            _ => panic!("patch_jump on non-jump instruction"),
        }
    }

    fn emit_loop(&mut self, loop_start: usize) {
        let current = self.instructions.len();
        let offset = (loop_start as i32) - (current as i32) - 1;
        self.emit(Instruction::Jump(offset));
    }

    fn resolve_local(&mut self, name: &str) -> u16 {
        if let Some(pos) = self.locals.iter().position(|n| n == name) {
            pos as u16
        } else {
            let idx = self.locals.len() as u16;
            self.locals.push(name.to_string());
            idx
        }
    }

    fn add_constant(&mut self, c: Constant) -> u16 {
        Compiler::add_constant_shared(self.constants, c)
    }

    fn hidden_local(&mut self, prefix: &str) -> u16 {
        let name = format!("_{}_{}", prefix, self.hidden_counter);
        self.hidden_counter += 1;
        self.resolve_local(&name)
    }

    fn extract_name(expr: &Expr) -> String {
        match expr {
            Expr::Identifier { name, .. } => name.clone(),
            Expr::FieldAccess { object, field_name, .. } => {
                let parent = Self::extract_name(object);
                format!("{}.{}", parent, field_name)
            }
            _ => "<indirect>".to_string(),
        }
    }

    fn is_stdlib_module(name: &str) -> bool {
        matches!(name, "web" | "data" | "json" | "file" | "ai" | "crypto" | "time" | "math" | "text" | "env"
            | "http" | "anthropic" | "openai" | "ollama" | "grok" | "mcp" | "mcpx"
            | "embeddings" | "prompts" | "guardrails")
    }

    fn is_builtin(name: &str) -> bool {
        matches!(name, "len" | "abs" | "min" | "max" | "range" | "str" | "string" | "int" | "integer" | "float" | "type")
    }

    // ── Statement compilation ────────────────────────────────────────

    fn compile_statements(&mut self, stmts: &[Statement]) {
        for stmt in stmts {
            self.compile_statement(stmt);
        }
    }

    fn compile_statement(&mut self, stmt: &Statement) {
        match stmt {
            Statement::Assignment { target, value, .. } => {
                self.compile_expression(value);
                if target.contains('.') {
                    let path_idx = self.add_constant(Constant::Str(target.clone()));
                    self.emit(Instruction::SetField(path_idx));
                } else {
                    let slot = self.resolve_local(target);
                    self.emit(Instruction::SetLocal(slot));
                }
            }

            Statement::Return { value, .. } => {
                self.compile_expression(value);
                self.emit(Instruction::SetLocal(self.result_slot));
                let jump_idx = self.emit_jump(Instruction::Jump(0));
                self.return_patches.push(jump_idx);
            }

            Statement::Emit { event, .. } => {
                self.compile_emit(event);
            }

            Statement::ExprStmt { expr, .. } => {
                self.compile_expression(expr);
                self.emit(Instruction::Pop);
            }

            Statement::If { condition, then_body, else_body, .. } => {
                self.compile_expression(condition);
                let else_jump = self.emit_jump(Instruction::JumpIfFalse(0));

                self.compile_statements(then_body);

                if !else_body.is_empty() {
                    let end_jump = self.emit_jump(Instruction::Jump(0));
                    self.patch_jump(else_jump);
                    self.compile_statements(else_body);
                    self.patch_jump(end_jump);
                } else {
                    self.patch_jump(else_jump);
                }
            }

            Statement::For { var, iterable, body, .. } => {
                // Compile iterable and cache length
                let iter_slot = self.hidden_local("iter");
                let len_slot = self.hidden_local("len");
                let idx_slot = self.hidden_local("idx");
                let var_slot = self.resolve_local(var);

                self.compile_expression(iterable);
                self.emit(Instruction::SetLocal(iter_slot));

                // len = builtin len(iter)
                self.emit(Instruction::GetLocal(iter_slot));
                let len_name = self.add_constant(Constant::Str("len".to_string()));
                self.emit(Instruction::CallBuiltin(len_name, 1));
                self.emit(Instruction::SetLocal(len_slot));

                // idx = 0
                let zero = self.add_constant(Constant::Int(0));
                self.emit(Instruction::LoadConst(zero));
                self.emit(Instruction::SetLocal(idx_slot));

                // loop:
                let loop_start = self.current_offset();
                self.emit(Instruction::GetLocal(idx_slot));
                self.emit(Instruction::GetLocal(len_slot));
                self.emit(Instruction::Less);
                let end_jump = self.emit_jump(Instruction::JumpIfFalse(0));

                // var = iter[idx]
                self.emit(Instruction::GetLocal(iter_slot));
                self.emit(Instruction::GetLocal(idx_slot));
                self.emit(Instruction::ListIndex);
                self.emit(Instruction::SetLocal(var_slot));

                self.compile_statements(body);

                // idx += 1
                self.emit(Instruction::GetLocal(idx_slot));
                let one = self.add_constant(Constant::Int(1));
                self.emit(Instruction::LoadConst(one));
                self.emit(Instruction::Add);
                self.emit(Instruction::SetLocal(idx_slot));

                self.emit_loop(loop_start);
                self.patch_jump(end_jump);
            }

            Statement::While { condition, body, .. } => {
                let loop_start = self.current_offset();
                self.compile_expression(condition);
                let end_jump = self.emit_jump(Instruction::JumpIfFalse(0));
                self.compile_statements(body);
                self.emit_loop(loop_start);
                self.patch_jump(end_jump);
            }
        }
    }

    fn compile_emit(&mut self, event: &Expr) {
        match event {
            Expr::FunctionCall { function, arguments, .. } => {
                let name = Self::extract_name(function);
                let name_idx = self.add_constant(Constant::Str(name));
                for arg in arguments {
                    self.compile_expression(arg);
                }
                self.emit(Instruction::EmitEvent(name_idx, arguments.len() as u16));
            }
            Expr::Identifier { name, .. } => {
                let name_idx = self.add_constant(Constant::Str(name.clone()));
                self.emit(Instruction::EmitEvent(name_idx, 0));
            }
            _ => {
                self.compile_expression(event);
                let name_idx = self.add_constant(Constant::Str("event".to_string()));
                self.emit(Instruction::EmitEvent(name_idx, 1));
            }
        }
    }

    // ── Expression compilation ───────────────────────────────────────

    fn compile_expression(&mut self, expr: &Expr) {
        match expr {
            Expr::Identifier { name, .. } => {
                let slot = self.resolve_local(name);
                self.emit(Instruction::GetLocal(slot));
            }

            Expr::StringLiteral { value, .. } => {
                let idx = self.add_constant(Constant::Str(value.clone()));
                self.emit(Instruction::LoadConst(idx));
            }

            Expr::NumberLiteral { value, is_int, .. } => {
                let c = if *is_int {
                    Constant::Int(*value as i64)
                } else {
                    Constant::Float(*value)
                };
                let idx = self.add_constant(c);
                self.emit(Instruction::LoadConst(idx));
            }

            Expr::BoolLiteral { value, .. } => {
                self.emit(if *value { Instruction::LoadTrue } else { Instruction::LoadFalse });
            }

            Expr::ListLiteral { elements, .. } => {
                for elem in elements {
                    self.compile_expression(elem);
                }
                self.emit(Instruction::NewList(elements.len() as u16));
            }

            Expr::BinaryOp { left, op, right, .. } => {
                // Short-circuit: and/or
                if op == "and" {
                    self.compile_expression(left);
                    self.emit(Instruction::Dup);
                    let end = self.emit_jump(Instruction::JumpIfFalse(0));
                    self.emit(Instruction::Pop);
                    self.compile_expression(right);
                    self.patch_jump(end);
                    return;
                }
                if op == "or" {
                    self.compile_expression(left);
                    self.emit(Instruction::Dup);
                    let end = self.emit_jump(Instruction::JumpIfTrue(0));
                    self.emit(Instruction::Pop);
                    self.compile_expression(right);
                    self.patch_jump(end);
                    return;
                }

                self.compile_expression(left);
                self.compile_expression(right);
                match op.as_str() {
                    "+" => self.emit(Instruction::Add),
                    "-" => self.emit(Instruction::Sub),
                    "*" => self.emit(Instruction::Mul),
                    "/" => self.emit(Instruction::Div),
                    "==" => self.emit(Instruction::Equal),
                    "!=" => self.emit(Instruction::NotEqual),
                    "<" => self.emit(Instruction::Less),
                    "<=" => self.emit(Instruction::LessEqual),
                    ">" => self.emit(Instruction::Greater),
                    ">=" => self.emit(Instruction::GreaterEqual),
                    _ => {} // unknown op — should not happen
                }
            }

            Expr::UnaryOp { op, operand, .. } => {
                self.compile_expression(operand);
                match op.as_str() {
                    "-" => self.emit(Instruction::Negate),
                    "not" => self.emit(Instruction::Not),
                    _ => {}
                }
            }

            Expr::FieldAccess { object, field_name, .. } => {
                self.compile_expression(object);
                let field_idx = self.add_constant(Constant::Str(field_name.clone()));
                self.emit(Instruction::GetField(field_idx));
            }

            Expr::FunctionCall { function, arguments, keyword_args, .. } => {
                self.compile_function_call(function, arguments, keyword_args);
            }

            Expr::MethodCall { object, method, arguments, keyword_args, .. } => {
                self.compile_method_call(object, method, arguments, keyword_args);
            }

            Expr::OldExpr { inner, .. } => {
                self.emit(Instruction::BeginOld);
                self.compile_expression(inner);
                self.emit(Instruction::EndOld);
            }

            Expr::HasExpr { subject, capability, .. } => {
                self.compile_expression(subject);
                self.compile_expression(capability);
                self.emit(Instruction::HasCapability);
            }
            Expr::IndexAccess { object, index, .. } => {
                self.compile_expression(object);
                self.compile_expression(index);
                self.emit(Instruction::ListIndex);
            }
        }
    }

    fn compile_function_call(
        &mut self,
        function: &Expr,
        arguments: &[Expr],
        keyword_args: &[(String, Expr)],
    ) {
        let name = Self::extract_name(function);

        // Built-in: print
        if name == "print" {
            for arg in arguments {
                self.compile_expression(arg);
            }
            self.emit(Instruction::Print(arguments.len() as u16));
            // print returns null — push it since caller expects a value
            self.emit(Instruction::LoadNull);
            return;
        }

        // Built-in functions: len, abs, min, max, range
        if Self::is_builtin(&name) {
            for arg in arguments {
                self.compile_expression(arg);
            }
            let name_idx = self.add_constant(Constant::Str(name));
            self.emit(Instruction::CallBuiltin(name_idx, arguments.len() as u16));
            return;
        }

        // Constructor: CapitalizedName(args...)
        if name.starts_with(|c: char| c.is_uppercase()) {
            self.compile_constructor(&name, arguments, keyword_args);
            return;
        }

        // Contract call
        // Push positional args
        for arg in arguments {
            self.compile_expression(arg);
        }
        // Push keyword args as [value, key_string] pairs
        for (key, value) in keyword_args {
            self.compile_expression(value);
            let key_idx = self.add_constant(Constant::Str(key.clone()));
            self.emit(Instruction::LoadConst(key_idx));
        }
        let name_idx = self.add_constant(Constant::Str(name));
        self.emit(Instruction::CallContract(
            name_idx,
            arguments.len() as u16,
            keyword_args.len() as u16,
        ));
    }

    fn compile_method_call(
        &mut self,
        object: &Expr,
        method: &str,
        arguments: &[Expr],
        keyword_args: &[(String, Expr)],
    ) {
        let obj_name = Self::extract_name(object);

        // Stdlib module call: web.get(), data.frame(), etc.
        if Self::is_stdlib_module(&obj_name) {
            // Push positional args
            for arg in arguments {
                self.compile_expression(arg);
            }
            // Push kwargs
            for (key, value) in keyword_args {
                self.compile_expression(value);
                let key_idx = self.add_constant(Constant::Str(key.clone()));
                self.emit(Instruction::LoadConst(key_idx));
            }
            let mod_idx = self.add_constant(Constant::Str(obj_name));
            let method_idx = self.add_constant(Constant::Str(method.to_string()));
            self.emit(Instruction::CallModule(
                mod_idx,
                method_idx,
                arguments.len() as u16,
                keyword_args.len() as u16,
            ));
            return;
        }

        // Static constructor: TypeName.method(args...)
        if obj_name.starts_with(|c: char| c.is_uppercase()) {
            let type_name = format!("{}.{}", obj_name, method);
            let method_key = self.add_constant(Constant::Str("_method".to_string()));
            let method_val = self.add_constant(Constant::Str(method.to_string()));
            self.emit(Instruction::LoadConst(method_val));
            self.emit(Instruction::LoadConst(method_key));

            for (i, arg) in arguments.iter().enumerate() {
                self.compile_expression(arg);
                let key_idx = self.add_constant(Constant::Str(format!("_{}", i)));
                self.emit(Instruction::LoadConst(key_idx));
            }
            for (key, value) in keyword_args {
                self.compile_expression(value);
                let key_idx = self.add_constant(Constant::Str(key.clone()));
                self.emit(Instruction::LoadConst(key_idx));
            }

            let total = 1 + arguments.len() + keyword_args.len();
            let type_idx = self.add_constant(Constant::Str(type_name));
            self.emit(Instruction::NewObject(type_idx, total as u16));
            return;
        }

        // Instance method call: obj.method(args...)
        // Compile object first, then args, then emit CallMethod
        self.compile_expression(object);
        for arg in arguments {
            self.compile_expression(arg);
        }
        for (key, value) in keyword_args {
            self.compile_expression(value);
            let key_idx = self.add_constant(Constant::Str(key.clone()));
            self.emit(Instruction::LoadConst(key_idx));
        }
        let method_idx = self.add_constant(Constant::Str(method.to_string()));
        self.emit(Instruction::CallMethod(
            method_idx,
            arguments.len() as u16,
            keyword_args.len() as u16,
        ));
    }

    fn compile_constructor(
        &mut self,
        name: &str,
        arguments: &[Expr],
        keyword_args: &[(String, Expr)],
    ) {
        // Positional args become fields _0, _1, ...
        for (i, arg) in arguments.iter().enumerate() {
            self.compile_expression(arg);
            let key_idx = self.add_constant(Constant::Str(format!("_{}", i)));
            self.emit(Instruction::LoadConst(key_idx));
        }
        // Keyword args become named fields
        for (key, value) in keyword_args {
            self.compile_expression(value);
            let key_idx = self.add_constant(Constant::Str(key.clone()));
            self.emit(Instruction::LoadConst(key_idx));
        }
        let total = arguments.len() + keyword_args.len();
        let type_idx = self.add_constant(Constant::Str(name.to_string()));
        self.emit(Instruction::NewObject(type_idx, total as u16));
    }
}
