//! Covenant bytecode instruction set

use std::fmt;

/// Constants stored in the module's constant pool
#[derive(Debug, Clone, PartialEq)]
pub enum Constant {
    Int(i64),
    Float(f64),
    Str(String),
    Bool(bool),
    Null,
}

impl fmt::Display for Constant {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Constant::Int(n) => write!(f, "{}", n),
            Constant::Float(n) => write!(f, "{}", n),
            Constant::Str(s) => write!(f, "\"{}\"", s),
            Constant::Bool(b) => write!(f, "{}", b),
            Constant::Null => write!(f, "null"),
        }
    }
}

/// Bytecode instructions for the Covenant VM.
///
/// All operands are Copy types so instructions can be copied cheaply.
/// u16 indices reference the constant pool or local variable slots.
/// i32 offsets are relative jumps from the next instruction.
#[derive(Debug, Clone, Copy)]
pub enum Instruction {
    // ── Constants & Stack ────────────────────────────────────────────
    /// Push constants[idx] onto stack
    LoadConst(u16),
    /// Push null
    LoadNull,
    /// Push true
    LoadTrue,
    /// Push false
    LoadFalse,
    /// Discard top of stack
    Pop,
    /// Duplicate top of stack
    Dup,

    // ── Local Variables ──────────────────────────────────────────────
    /// Push locals[idx]
    GetLocal(u16),
    /// Pop → locals[idx]
    SetLocal(u16),

    // ── Arithmetic ───────────────────────────────────────────────────
    Add,
    Sub,
    Mul,
    Div,
    /// Unary negation
    Negate,

    // ── Comparison ───────────────────────────────────────────────────
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,

    // ── Logic ────────────────────────────────────────────────────────
    Not,

    // ── Control Flow ─────────────────────────────────────────────────
    /// Unconditional relative jump
    Jump(i32),
    /// Pop; jump if falsy
    JumpIfFalse(i32),
    /// Pop; jump if truthy
    JumpIfTrue(i32),

    // ── Contract Calls ───────────────────────────────────────────────
    /// Call contract: (name_const_idx, positional_args, keyword_args)
    CallContract(u16, u16, u16),
    /// Return from current contract (pop return value)
    Return,

    // ── Objects ──────────────────────────────────────────────────────
    /// Create object: (type_name_const_idx, field_count)
    /// Stack has field_count pairs of [value, key_string] (top-down)
    NewObject(u16, u16),
    /// Pop object, push object.field (field_name_const_idx)
    GetField(u16),
    /// Pop value; modify local variable at dotted path (path_const_idx)
    SetField(u16),

    // ── Lists ────────────────────────────────────────────────────────
    /// Pop N elements, push list (N = element_count)
    NewList(u16),
    /// Pop index, pop list, push list[index]
    ListIndex,

    // ── Built-in Functions ───────────────────────────────────────────
    /// Print N values from stack
    Print(u16),
    /// Call built-in: (name_const_idx, arg_count)
    CallBuiltin(u16, u16),

    // ── Standard Library ─────────────────────────────────────────────
    /// Call stdlib module.method: (module_const, method_const, pos_args, kw_args)
    CallModule(u16, u16, u16, u16),
    /// Call method on TOS object: (method_const, pos_args, kw_args)
    CallMethod(u16, u16, u16),

    // ── Contract Enforcement ─────────────────────────────────────────
    /// Pop bool; RuntimeError if false (precondition_index for message)
    CheckPre(u16),
    /// Pop bool; RuntimeError if false (postcondition_index for message)
    CheckPost(u16),
    /// Snapshot current locals for old() references
    Snapshot,
    /// Swap locals with snapshot (enter old scope)
    BeginOld,
    /// Swap locals back (exit old scope)
    EndOld,

    // ── Events ───────────────────────────────────────────────────────
    /// Emit event: (name_const_idx, arg_count)
    EmitEvent(u16, u16),

    // ── Capabilities ─────────────────────────────────────────────────
    /// Pop capability, pop subject, push bool (always true at runtime)
    HasCapability,

    // ── Error Handling ────────────────────────────────────────────────
    /// Push exception handler (catch_offset is absolute instruction index)
    SetHandler(u32),
    /// Remove top exception handler
    ClearHandler,
    /// Store error string from stack into local slot
    CatchBind(u16),
}

impl fmt::Display for Instruction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Instruction::LoadConst(i) => write!(f, "LOAD_CONST {}", i),
            Instruction::LoadNull => write!(f, "LOAD_NULL"),
            Instruction::LoadTrue => write!(f, "LOAD_TRUE"),
            Instruction::LoadFalse => write!(f, "LOAD_FALSE"),
            Instruction::Pop => write!(f, "POP"),
            Instruction::Dup => write!(f, "DUP"),
            Instruction::GetLocal(i) => write!(f, "GET_LOCAL {}", i),
            Instruction::SetLocal(i) => write!(f, "SET_LOCAL {}", i),
            Instruction::Add => write!(f, "ADD"),
            Instruction::Sub => write!(f, "SUB"),
            Instruction::Mul => write!(f, "MUL"),
            Instruction::Div => write!(f, "DIV"),
            Instruction::Negate => write!(f, "NEGATE"),
            Instruction::Equal => write!(f, "EQUAL"),
            Instruction::NotEqual => write!(f, "NOT_EQUAL"),
            Instruction::Less => write!(f, "LESS"),
            Instruction::LessEqual => write!(f, "LESS_EQUAL"),
            Instruction::Greater => write!(f, "GREATER"),
            Instruction::GreaterEqual => write!(f, "GREATER_EQUAL"),
            Instruction::Not => write!(f, "NOT"),
            Instruction::Jump(o) => write!(f, "JUMP {}", o),
            Instruction::JumpIfFalse(o) => write!(f, "JUMP_IF_FALSE {}", o),
            Instruction::JumpIfTrue(o) => write!(f, "JUMP_IF_TRUE {}", o),
            Instruction::CallContract(n, p, k) => write!(f, "CALL_CONTRACT c{} pos={} kw={}", n, p, k),
            Instruction::Return => write!(f, "RETURN"),
            Instruction::NewObject(t, n) => write!(f, "NEW_OBJECT type={} fields={}", t, n),
            Instruction::GetField(i) => write!(f, "GET_FIELD {}", i),
            Instruction::SetField(i) => write!(f, "SET_FIELD {}", i),
            Instruction::NewList(n) => write!(f, "NEW_LIST {}", n),
            Instruction::ListIndex => write!(f, "LIST_INDEX"),
            Instruction::Print(n) => write!(f, "PRINT {}", n),
            Instruction::CallBuiltin(n, a) => write!(f, "CALL_BUILTIN c{} args={}", n, a),
            Instruction::CallModule(m, mt, p, k) => write!(f, "CALL_MODULE c{}.c{} pos={} kw={}", m, mt, p, k),
            Instruction::CallMethod(m, p, k) => write!(f, "CALL_METHOD c{} pos={} kw={}", m, p, k),
            Instruction::CheckPre(i) => write!(f, "CHECK_PRE {}", i),
            Instruction::CheckPost(i) => write!(f, "CHECK_POST {}", i),
            Instruction::Snapshot => write!(f, "SNAPSHOT"),
            Instruction::BeginOld => write!(f, "BEGIN_OLD"),
            Instruction::EndOld => write!(f, "END_OLD"),
            Instruction::EmitEvent(n, a) => write!(f, "EMIT_EVENT c{} args={}", n, a),
            Instruction::HasCapability => write!(f, "HAS_CAPABILITY"),
            Instruction::SetHandler(o) => write!(f, "SET_HANDLER @{}", o),
            Instruction::ClearHandler => write!(f, "CLEAR_HANDLER"),
            Instruction::CatchBind(i) => write!(f, "CATCH_BIND {}", i),
        }
    }
}
