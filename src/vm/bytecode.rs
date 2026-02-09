//! Covenant bytecode module format and serialization
//!
//! A .covc file contains a serialized Module: constant pool + compiled contracts.

use super::opcodes::{Constant, Instruction};

/// A compiled Covenant module — the content of a .covc file
#[derive(Debug, Clone)]
pub struct Module {
    pub constants: Vec<Constant>,
    pub contracts: Vec<CompiledContract>,
}

/// A single compiled contract
#[derive(Debug, Clone)]
pub struct CompiledContract {
    pub name: String,
    pub params: Vec<String>,
    pub param_types: Vec<String>,    // Type names for each param ("Any" if untyped)
    pub return_type: Option<String>, // Return type name if declared
    pub local_count: u16,
    pub local_names: Vec<String>,
    pub code: Vec<Instruction>,
}

// ── Binary serialization ─────────────────────────────────────────────────

const MAGIC: &[u8; 4] = b"COVC";
const VERSION: u8 = 2;

// Constant type tags
const TAG_NULL: u8 = 0;
const TAG_INT: u8 = 1;
const TAG_FLOAT: u8 = 2;
const TAG_STR: u8 = 3;
const TAG_BOOL: u8 = 4;

// Instruction opcode bytes
const OP_LOAD_CONST: u8 = 0x01;
const OP_LOAD_NULL: u8 = 0x02;
const OP_LOAD_TRUE: u8 = 0x03;
const OP_LOAD_FALSE: u8 = 0x04;
const OP_POP: u8 = 0x05;
const OP_DUP: u8 = 0x06;
const OP_GET_LOCAL: u8 = 0x07;
const OP_SET_LOCAL: u8 = 0x08;
const OP_ADD: u8 = 0x10;
const OP_SUB: u8 = 0x11;
const OP_MUL: u8 = 0x12;
const OP_DIV: u8 = 0x13;
const OP_NEGATE: u8 = 0x14;
const OP_EQUAL: u8 = 0x20;
const OP_NOT_EQUAL: u8 = 0x21;
const OP_LESS: u8 = 0x22;
const OP_LESS_EQUAL: u8 = 0x23;
const OP_GREATER: u8 = 0x24;
const OP_GREATER_EQUAL: u8 = 0x25;
const OP_NOT: u8 = 0x26;
const OP_JUMP: u8 = 0x30;
const OP_JUMP_IF_FALSE: u8 = 0x31;
const OP_JUMP_IF_TRUE: u8 = 0x32;
const OP_CALL_CONTRACT: u8 = 0x40;
const OP_RETURN: u8 = 0x41;
const OP_NEW_OBJECT: u8 = 0x50;
const OP_GET_FIELD: u8 = 0x51;
const OP_SET_FIELD: u8 = 0x52;
const OP_NEW_LIST: u8 = 0x53;
const OP_LIST_INDEX: u8 = 0x54;
const OP_PRINT: u8 = 0x60;
const OP_CALL_BUILTIN: u8 = 0x61;
const OP_CALL_MODULE: u8 = 0x62;
const OP_CALL_METHOD: u8 = 0x63;
const OP_CHECK_PRE: u8 = 0x70;
const OP_CHECK_POST: u8 = 0x71;
const OP_SNAPSHOT: u8 = 0x72;
const OP_BEGIN_OLD: u8 = 0x73;
const OP_END_OLD: u8 = 0x74;
const OP_EMIT_EVENT: u8 = 0x80;
const OP_HAS_CAPABILITY: u8 = 0x90;

impl Module {
    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::new();

        // Header
        buf.extend_from_slice(MAGIC);
        buf.push(VERSION);

        // Constants
        write_u32(&mut buf, self.constants.len() as u32);
        for c in &self.constants {
            serialize_constant(&mut buf, c);
        }

        // Contracts
        write_u32(&mut buf, self.contracts.len() as u32);
        for contract in &self.contracts {
            serialize_contract(&mut buf, contract);
        }

        buf
    }

    pub fn deserialize(data: &[u8]) -> Result<Self, String> {
        // Header
        if data.len() < 5 {
            return Err("File too small".to_string());
        }
        if &data[0..4] != MAGIC {
            return Err("Invalid magic number — not a .covc file".to_string());
        }
        let mut pos = 4;
        let version = data[pos];
        pos += 1;
        if version != VERSION {
            return Err(format!("Unsupported bytecode version: {}", version));
        }

        // Constants
        let const_count = read_u32(data, &mut pos)? as usize;
        let mut constants = Vec::with_capacity(const_count);
        for _ in 0..const_count {
            constants.push(deserialize_constant(data, &mut pos)?);
        }

        // Contracts
        let contract_count = read_u32(data, &mut pos)? as usize;
        let mut contracts = Vec::with_capacity(contract_count);
        for _ in 0..contract_count {
            contracts.push(deserialize_contract(data, &mut pos)?);
        }

        Ok(Module {
            constants,
            contracts,
        })
    }
}

// ── Serialization helpers ────────────────────────────────────────────────

fn write_u16(buf: &mut Vec<u8>, v: u16) {
    buf.extend_from_slice(&v.to_le_bytes());
}

fn write_u32(buf: &mut Vec<u8>, v: u32) {
    buf.extend_from_slice(&v.to_le_bytes());
}

fn write_i32(buf: &mut Vec<u8>, v: i32) {
    buf.extend_from_slice(&v.to_le_bytes());
}

fn write_i64(buf: &mut Vec<u8>, v: i64) {
    buf.extend_from_slice(&v.to_le_bytes());
}

fn write_f64(buf: &mut Vec<u8>, v: f64) {
    buf.extend_from_slice(&v.to_le_bytes());
}

fn write_string(buf: &mut Vec<u8>, s: &str) {
    write_u32(buf, s.len() as u32);
    buf.extend_from_slice(s.as_bytes());
}

fn serialize_constant(buf: &mut Vec<u8>, c: &Constant) {
    match c {
        Constant::Null => buf.push(TAG_NULL),
        Constant::Int(n) => {
            buf.push(TAG_INT);
            write_i64(buf, *n);
        }
        Constant::Float(n) => {
            buf.push(TAG_FLOAT);
            write_f64(buf, *n);
        }
        Constant::Str(s) => {
            buf.push(TAG_STR);
            write_string(buf, s);
        }
        Constant::Bool(b) => {
            buf.push(TAG_BOOL);
            buf.push(if *b { 1 } else { 0 });
        }
    }
}

fn serialize_contract(buf: &mut Vec<u8>, contract: &CompiledContract) {
    write_string(buf, &contract.name);
    write_u16(buf, contract.params.len() as u16);
    for p in &contract.params {
        write_string(buf, p);
    }
    // Param types
    write_u16(buf, contract.param_types.len() as u16);
    for t in &contract.param_types {
        write_string(buf, t);
    }
    // Return type (0 = none, 1 = present)
    if let Some(ref rt) = contract.return_type {
        buf.push(1);
        write_string(buf, rt);
    } else {
        buf.push(0);
    }
    write_u16(buf, contract.local_count);
    write_u16(buf, contract.local_names.len() as u16);
    for name in &contract.local_names {
        write_string(buf, name);
    }
    write_u32(buf, contract.code.len() as u32);
    for inst in &contract.code {
        serialize_instruction(buf, inst);
    }
}

fn serialize_instruction(buf: &mut Vec<u8>, inst: &Instruction) {
    match *inst {
        Instruction::LoadConst(i) => { buf.push(OP_LOAD_CONST); write_u16(buf, i); }
        Instruction::LoadNull => buf.push(OP_LOAD_NULL),
        Instruction::LoadTrue => buf.push(OP_LOAD_TRUE),
        Instruction::LoadFalse => buf.push(OP_LOAD_FALSE),
        Instruction::Pop => buf.push(OP_POP),
        Instruction::Dup => buf.push(OP_DUP),
        Instruction::GetLocal(i) => { buf.push(OP_GET_LOCAL); write_u16(buf, i); }
        Instruction::SetLocal(i) => { buf.push(OP_SET_LOCAL); write_u16(buf, i); }
        Instruction::Add => buf.push(OP_ADD),
        Instruction::Sub => buf.push(OP_SUB),
        Instruction::Mul => buf.push(OP_MUL),
        Instruction::Div => buf.push(OP_DIV),
        Instruction::Negate => buf.push(OP_NEGATE),
        Instruction::Equal => buf.push(OP_EQUAL),
        Instruction::NotEqual => buf.push(OP_NOT_EQUAL),
        Instruction::Less => buf.push(OP_LESS),
        Instruction::LessEqual => buf.push(OP_LESS_EQUAL),
        Instruction::Greater => buf.push(OP_GREATER),
        Instruction::GreaterEqual => buf.push(OP_GREATER_EQUAL),
        Instruction::Not => buf.push(OP_NOT),
        Instruction::Jump(o) => { buf.push(OP_JUMP); write_i32(buf, o); }
        Instruction::JumpIfFalse(o) => { buf.push(OP_JUMP_IF_FALSE); write_i32(buf, o); }
        Instruction::JumpIfTrue(o) => { buf.push(OP_JUMP_IF_TRUE); write_i32(buf, o); }
        Instruction::CallContract(n, p, k) => { buf.push(OP_CALL_CONTRACT); write_u16(buf, n); write_u16(buf, p); write_u16(buf, k); }
        Instruction::Return => buf.push(OP_RETURN),
        Instruction::NewObject(t, n) => { buf.push(OP_NEW_OBJECT); write_u16(buf, t); write_u16(buf, n); }
        Instruction::GetField(i) => { buf.push(OP_GET_FIELD); write_u16(buf, i); }
        Instruction::SetField(i) => { buf.push(OP_SET_FIELD); write_u16(buf, i); }
        Instruction::NewList(n) => { buf.push(OP_NEW_LIST); write_u16(buf, n); }
        Instruction::ListIndex => buf.push(OP_LIST_INDEX),
        Instruction::Print(n) => { buf.push(OP_PRINT); write_u16(buf, n); }
        Instruction::CallBuiltin(n, a) => { buf.push(OP_CALL_BUILTIN); write_u16(buf, n); write_u16(buf, a); }
        Instruction::CallModule(m, mt, p, k) => { buf.push(OP_CALL_MODULE); write_u16(buf, m); write_u16(buf, mt); write_u16(buf, p); write_u16(buf, k); }
        Instruction::CallMethod(m, p, k) => { buf.push(OP_CALL_METHOD); write_u16(buf, m); write_u16(buf, p); write_u16(buf, k); }
        Instruction::CheckPre(i) => { buf.push(OP_CHECK_PRE); write_u16(buf, i); }
        Instruction::CheckPost(i) => { buf.push(OP_CHECK_POST); write_u16(buf, i); }
        Instruction::Snapshot => buf.push(OP_SNAPSHOT),
        Instruction::BeginOld => buf.push(OP_BEGIN_OLD),
        Instruction::EndOld => buf.push(OP_END_OLD),
        Instruction::EmitEvent(n, a) => { buf.push(OP_EMIT_EVENT); write_u16(buf, n); write_u16(buf, a); }
        Instruction::HasCapability => buf.push(OP_HAS_CAPABILITY),
    }
}

// ── Deserialization helpers ──────────────────────────────────────────────

fn read_u16(data: &[u8], pos: &mut usize) -> Result<u16, String> {
    if *pos + 2 > data.len() {
        return Err("Unexpected end of bytecode".to_string());
    }
    let v = u16::from_le_bytes([data[*pos], data[*pos + 1]]);
    *pos += 2;
    Ok(v)
}

fn read_u32(data: &[u8], pos: &mut usize) -> Result<u32, String> {
    if *pos + 4 > data.len() {
        return Err("Unexpected end of bytecode".to_string());
    }
    let v = u32::from_le_bytes([data[*pos], data[*pos + 1], data[*pos + 2], data[*pos + 3]]);
    *pos += 4;
    Ok(v)
}

fn read_i32(data: &[u8], pos: &mut usize) -> Result<i32, String> {
    if *pos + 4 > data.len() {
        return Err("Unexpected end of bytecode".to_string());
    }
    let v = i32::from_le_bytes([data[*pos], data[*pos + 1], data[*pos + 2], data[*pos + 3]]);
    *pos += 4;
    Ok(v)
}

fn read_i64(data: &[u8], pos: &mut usize) -> Result<i64, String> {
    if *pos + 8 > data.len() {
        return Err("Unexpected end of bytecode".to_string());
    }
    let v = i64::from_le_bytes([
        data[*pos], data[*pos+1], data[*pos+2], data[*pos+3],
        data[*pos+4], data[*pos+5], data[*pos+6], data[*pos+7],
    ]);
    *pos += 8;
    Ok(v)
}

fn read_f64(data: &[u8], pos: &mut usize) -> Result<f64, String> {
    if *pos + 8 > data.len() {
        return Err("Unexpected end of bytecode".to_string());
    }
    let v = f64::from_le_bytes([
        data[*pos], data[*pos+1], data[*pos+2], data[*pos+3],
        data[*pos+4], data[*pos+5], data[*pos+6], data[*pos+7],
    ]);
    *pos += 8;
    Ok(v)
}

fn read_string(data: &[u8], pos: &mut usize) -> Result<String, String> {
    let len = read_u32(data, pos)? as usize;
    if *pos + len > data.len() {
        return Err("Unexpected end of bytecode in string".to_string());
    }
    let s = String::from_utf8(data[*pos..*pos + len].to_vec())
        .map_err(|e| format!("Invalid UTF-8 in bytecode: {}", e))?;
    *pos += len;
    Ok(s)
}

fn deserialize_constant(data: &[u8], pos: &mut usize) -> Result<Constant, String> {
    if *pos >= data.len() {
        return Err("Unexpected end of bytecode in constant".to_string());
    }
    let tag = data[*pos];
    *pos += 1;
    match tag {
        TAG_NULL => Ok(Constant::Null),
        TAG_INT => Ok(Constant::Int(read_i64(data, pos)?)),
        TAG_FLOAT => Ok(Constant::Float(read_f64(data, pos)?)),
        TAG_STR => Ok(Constant::Str(read_string(data, pos)?)),
        TAG_BOOL => {
            if *pos >= data.len() {
                return Err("Unexpected end of bytecode in bool".to_string());
            }
            let v = data[*pos] != 0;
            *pos += 1;
            Ok(Constant::Bool(v))
        }
        _ => Err(format!("Unknown constant tag: {}", tag)),
    }
}

fn deserialize_contract(data: &[u8], pos: &mut usize) -> Result<CompiledContract, String> {
    let name = read_string(data, pos)?;
    let param_count = read_u16(data, pos)? as usize;
    let mut params = Vec::with_capacity(param_count);
    for _ in 0..param_count {
        params.push(read_string(data, pos)?);
    }
    // Param types
    let type_count = read_u16(data, pos)? as usize;
    let mut param_types = Vec::with_capacity(type_count);
    for _ in 0..type_count {
        param_types.push(read_string(data, pos)?);
    }
    // Return type
    if *pos >= data.len() {
        return Err("Unexpected end of bytecode in return type flag".to_string());
    }
    let has_return = data[*pos];
    *pos += 1;
    let return_type = if has_return == 1 {
        Some(read_string(data, pos)?)
    } else {
        None
    };
    let local_count = read_u16(data, pos)?;
    let name_count = read_u16(data, pos)? as usize;
    let mut local_names = Vec::with_capacity(name_count);
    for _ in 0..name_count {
        local_names.push(read_string(data, pos)?);
    }
    let code_len = read_u32(data, pos)? as usize;
    let mut code = Vec::with_capacity(code_len);
    for _ in 0..code_len {
        code.push(deserialize_instruction(data, pos)?);
    }
    Ok(CompiledContract {
        name,
        params,
        param_types,
        return_type,
        local_count,
        local_names,
        code,
    })
}

fn deserialize_instruction(data: &[u8], pos: &mut usize) -> Result<Instruction, String> {
    if *pos >= data.len() {
        return Err("Unexpected end of bytecode in instruction".to_string());
    }
    let op = data[*pos];
    *pos += 1;
    match op {
        OP_LOAD_CONST => Ok(Instruction::LoadConst(read_u16(data, pos)?)),
        OP_LOAD_NULL => Ok(Instruction::LoadNull),
        OP_LOAD_TRUE => Ok(Instruction::LoadTrue),
        OP_LOAD_FALSE => Ok(Instruction::LoadFalse),
        OP_POP => Ok(Instruction::Pop),
        OP_DUP => Ok(Instruction::Dup),
        OP_GET_LOCAL => Ok(Instruction::GetLocal(read_u16(data, pos)?)),
        OP_SET_LOCAL => Ok(Instruction::SetLocal(read_u16(data, pos)?)),
        OP_ADD => Ok(Instruction::Add),
        OP_SUB => Ok(Instruction::Sub),
        OP_MUL => Ok(Instruction::Mul),
        OP_DIV => Ok(Instruction::Div),
        OP_NEGATE => Ok(Instruction::Negate),
        OP_EQUAL => Ok(Instruction::Equal),
        OP_NOT_EQUAL => Ok(Instruction::NotEqual),
        OP_LESS => Ok(Instruction::Less),
        OP_LESS_EQUAL => Ok(Instruction::LessEqual),
        OP_GREATER => Ok(Instruction::Greater),
        OP_GREATER_EQUAL => Ok(Instruction::GreaterEqual),
        OP_NOT => Ok(Instruction::Not),
        OP_JUMP => Ok(Instruction::Jump(read_i32(data, pos)?)),
        OP_JUMP_IF_FALSE => Ok(Instruction::JumpIfFalse(read_i32(data, pos)?)),
        OP_JUMP_IF_TRUE => Ok(Instruction::JumpIfTrue(read_i32(data, pos)?)),
        OP_CALL_CONTRACT => {
            let n = read_u16(data, pos)?;
            let p = read_u16(data, pos)?;
            let k = read_u16(data, pos)?;
            Ok(Instruction::CallContract(n, p, k))
        }
        OP_RETURN => Ok(Instruction::Return),
        OP_NEW_OBJECT => {
            let t = read_u16(data, pos)?;
            let n = read_u16(data, pos)?;
            Ok(Instruction::NewObject(t, n))
        }
        OP_GET_FIELD => Ok(Instruction::GetField(read_u16(data, pos)?)),
        OP_SET_FIELD => Ok(Instruction::SetField(read_u16(data, pos)?)),
        OP_NEW_LIST => Ok(Instruction::NewList(read_u16(data, pos)?)),
        OP_LIST_INDEX => Ok(Instruction::ListIndex),
        OP_PRINT => Ok(Instruction::Print(read_u16(data, pos)?)),
        OP_CALL_BUILTIN => {
            let n = read_u16(data, pos)?;
            let a = read_u16(data, pos)?;
            Ok(Instruction::CallBuiltin(n, a))
        }
        OP_CALL_MODULE => {
            let m = read_u16(data, pos)?;
            let mt = read_u16(data, pos)?;
            let p = read_u16(data, pos)?;
            let k = read_u16(data, pos)?;
            Ok(Instruction::CallModule(m, mt, p, k))
        }
        OP_CALL_METHOD => {
            let m = read_u16(data, pos)?;
            let p = read_u16(data, pos)?;
            let k = read_u16(data, pos)?;
            Ok(Instruction::CallMethod(m, p, k))
        }
        OP_CHECK_PRE => Ok(Instruction::CheckPre(read_u16(data, pos)?)),
        OP_CHECK_POST => Ok(Instruction::CheckPost(read_u16(data, pos)?)),
        OP_SNAPSHOT => Ok(Instruction::Snapshot),
        OP_BEGIN_OLD => Ok(Instruction::BeginOld),
        OP_END_OLD => Ok(Instruction::EndOld),
        OP_EMIT_EVENT => {
            let n = read_u16(data, pos)?;
            let a = read_u16(data, pos)?;
            Ok(Instruction::EmitEvent(n, a))
        }
        OP_HAS_CAPABILITY => Ok(Instruction::HasCapability),
        _ => Err(format!("Unknown opcode: 0x{:02x}", op)),
    }
}
