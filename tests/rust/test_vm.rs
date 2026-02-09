//! VM tests -- bytecode compiler + stack-based virtual machine

use std::collections::HashMap;
use covenant_lang::lexer::Lexer;
use covenant_lang::parser::Parser;
use covenant_lang::vm::compiler::Compiler;
use covenant_lang::vm::machine::VM;
use covenant_lang::vm::bytecode::Module;
use covenant_lang::runtime::Value;

// ── Helpers ──────────────────────────────────────────────────────

fn compile_and_run(source: &str, contract_name: &str, args: HashMap<String, Value>) -> Value {
    let tokens = Lexer::new(source, "test.cov").tokenize().unwrap();
    let program = Parser::new(tokens, "test.cov").parse().unwrap();
    let mut compiler = Compiler::new();
    let module = compiler.compile(&program);
    let mut vm = VM::new(module);
    vm.run_contract(contract_name, args).unwrap()
}

fn run(source: &str) -> Value {
    compile_and_run(source, "test", HashMap::new())
}

fn compile_and_run_with_events(
    source: &str,
    contract_name: &str,
    args: HashMap<String, Value>,
) -> (Value, Vec<(String, Vec<Value>)>) {
    let tokens = Lexer::new(source, "test.cov").tokenize().unwrap();
    let program = Parser::new(tokens, "test.cov").parse().unwrap();
    let mut compiler = Compiler::new();
    let module = compiler.compile(&program);
    let mut vm = VM::new(module);
    let result = vm.run_contract(contract_name, args).unwrap();
    let events = vm.emitted_events().to_vec();
    (result, events)
}

#[allow(dead_code)]
fn run_err(source: &str, contract_name: &str) -> String {
    let tokens = Lexer::new(source, "test.cov").tokenize().unwrap();
    let program = Parser::new(tokens, "test.cov").parse().unwrap();
    let mut compiler = Compiler::new();
    let module = compiler.compile(&program);
    let mut vm = VM::new(module);
    vm.run_contract(contract_name, HashMap::new()).unwrap_err().message
}

fn args(pairs: &[(&str, Value)]) -> HashMap<String, Value> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.clone())).collect()
}

// ── 1. Basic values (5 tests) ───────────────────────────────────

#[test]
fn vm_return_integer() {
    assert_eq!(run("contract test() = 42"), Value::Int(42));
}

#[test]
fn vm_return_float() {
    assert_eq!(run("contract test() = 3.14"), Value::Float(3.14));
}

#[test]
fn vm_return_string() {
    assert_eq!(run("contract test() = \"hello\""), Value::Str("hello".into()));
}

#[test]
fn vm_return_bool_true() {
    assert_eq!(run("contract test() = true"), Value::Bool(true));
}

#[test]
fn vm_return_bool_false() {
    assert_eq!(run("contract test() = false"), Value::Bool(false));
}

// ── 2. Arithmetic (6 tests) ────────────────────────────────────

#[test]
fn vm_add() {
    assert_eq!(run("contract test() = 10 + 25"), Value::Int(35));
}

#[test]
fn vm_subtract() {
    assert_eq!(run("contract test() = 50 - 8"), Value::Int(42));
}

#[test]
fn vm_multiply() {
    assert_eq!(run("contract test() = 6 * 7"), Value::Int(42));
}

#[test]
fn vm_divide_exact() {
    assert_eq!(run("contract test() = 10 / 2"), Value::Int(5));
}

#[test]
fn vm_divide_inexact_returns_float() {
    match run("contract test() = 10 / 3") {
        Value::Float(f) => assert!((f - 3.333333333333333).abs() < 1e-10),
        other => panic!("Expected Float, got {:?}", other),
    }
}

#[test]
fn vm_negate() {
    assert_eq!(run("contract test() = -42"), Value::Int(-42));
}

// ── 3. Comparisons (4 tests) ───────────────────────────────────

#[test]
fn vm_equal() {
    assert_eq!(run("contract test() = 5 == 5"), Value::Bool(true));
}

#[test]
fn vm_not_equal() {
    assert_eq!(run("contract test() = 5 != 3"), Value::Bool(true));
}

#[test]
fn vm_less_than() {
    assert_eq!(run("contract test() = 3 < 10"), Value::Bool(true));
}

#[test]
fn vm_greater_equal() {
    assert_eq!(run("contract test() = 10 >= 10"), Value::Bool(true));
}

// ── 4. Logic (3 tests) ─────────────────────────────────────────

#[test]
fn vm_and() {
    assert_eq!(run("contract test() = true and false"), Value::Bool(false));
}

#[test]
fn vm_or() {
    assert_eq!(run("contract test() = false or true"), Value::Bool(true));
}

#[test]
fn vm_not() {
    assert_eq!(run("contract test() = not false"), Value::Bool(true));
}

// ── 5. Variables (3 tests) ──────────────────────────────────────

#[test]
fn vm_variable_assignment() {
    let src = "contract test()\n  body:\n    let x = 99\n    return x";
    assert_eq!(run(src), Value::Int(99));
}

#[test]
fn vm_variable_reassignment() {
    let src = "contract test()\n  body:\n    let x = 1\n    x = 2\n    return x";
    assert_eq!(run(src), Value::Int(2));
}

#[test]
fn vm_variable_in_expression() {
    let src = "contract test()\n  body:\n    let a = 10\n    let b = 20\n    return a + b";
    assert_eq!(run(src), Value::Int(30));
}

// ── 6. Control flow (5 tests) ──────────────────────────────────

#[test]
fn vm_if_true_branch() {
    let src = "contract test()\n  body:\n    if true:\n      return 1\n    return 0";
    assert_eq!(run(src), Value::Int(1));
}

#[test]
fn vm_if_false_branch() {
    let src = "contract test()\n  body:\n    if false:\n      return 1\n    return 0";
    assert_eq!(run(src), Value::Int(0));
}

#[test]
fn vm_if_else() {
    let src = "contract test()\n  body:\n    let x = 10\n    if x > 5:\n      return \"big\"\n    else:\n      return \"small\"";
    assert_eq!(run(src), Value::Str("big".into()));
}

#[test]
fn vm_while_loop() {
    let src = "contract test()\n  body:\n    let i = 0\n    let sum = 0\n    while i < 5:\n      sum = sum + i\n      i = i + 1\n    return sum";
    assert_eq!(run(src), Value::Int(10)); // 0+1+2+3+4 = 10
}

#[test]
fn vm_for_in_range() {
    let src = "contract test()\n  body:\n    let sum = 0\n    for x in range(5):\n      sum = sum + x\n    return sum";
    assert_eq!(run(src), Value::Int(10)); // 0+1+2+3+4 = 10
}

// ── 7. Contract calls (3 tests) ────────────────────────────────

#[test]
fn vm_call_another_contract() {
    let src = "contract helper()\n  body:\n    return 100\n\ncontract test()\n  body:\n    let v = helper()\n    return v";
    assert_eq!(run(src), Value::Int(100));
}

#[test]
fn vm_contract_with_args() {
    let src = "contract add(a, b)\n  body:\n    return a + b\n\ncontract test()\n  body:\n    return add(3, 4)";
    assert_eq!(run(src), Value::Int(7));
}

#[test]
fn vm_recursive_factorial() {
    let src = "contract factorial(n)\n  body:\n    if n <= 1:\n      return 1\n    return n * factorial(n - 1)";
    let result = compile_and_run(src, "factorial", args(&[("n", Value::Int(5))]));
    assert_eq!(result, Value::Int(120));
}

// ── 8. Lists (3 tests) ─────────────────────────────────────────

#[test]
fn vm_list_literal() {
    assert_eq!(
        run("contract test() = [1, 2, 3]"),
        Value::List(vec![Value::Int(1), Value::Int(2), Value::Int(3)])
    );
}

#[test]
fn vm_list_indexing() {
    let src = "contract test()\n  body:\n    let xs = [10, 20, 30]\n    return xs[1]";
    assert_eq!(run(src), Value::Int(20));
}

#[test]
fn vm_list_in_return() {
    let src = "contract test()\n  body:\n    let a = 1\n    let b = 2\n    return [a, b, a + b]";
    assert_eq!(
        run(src),
        Value::List(vec![Value::Int(1), Value::Int(2), Value::Int(3)])
    );
}

// ── 9. Objects (2 tests) ───────────────────────────────────────

#[test]
fn vm_create_object() {
    let src = "contract test()\n  body:\n    let r = Result(value: 42)\n    return r";
    match run(src) {
        Value::Object(name, fields) => {
            assert_eq!(name, "Result");
            assert_eq!(fields.get("value"), Some(&Value::Int(42)));
        }
        other => panic!("Expected Object, got {:?}", other),
    }
}

#[test]
fn vm_field_access() {
    let src = "contract test()\n  body:\n    let r = Result(value: 99)\n    return r.value";
    assert_eq!(run(src), Value::Int(99));
}

// ── 10. Precondition/postcondition (3 tests) ───────────────────

#[test]
fn vm_precondition_passes() {
    let src = "contract test(n)\n  precondition:\n    n > 0\n  body:\n    return n * 2";
    let result = compile_and_run(src, "test", args(&[("n", Value::Int(5))]));
    assert_eq!(result, Value::Int(10));
}

#[test]
fn vm_precondition_fails() {
    let src = "contract test(n)\n  precondition:\n    n > 0\n  body:\n    return n * 2";
    // Pass n = -1 so the precondition n > 0 fails cleanly
    let tokens = Lexer::new(src, "test.cov").tokenize().unwrap();
    let program = Parser::new(tokens, "test.cov").parse().unwrap();
    let mut compiler = Compiler::new();
    let module = compiler.compile(&program);
    let mut vm = VM::new(module);
    let err = vm.run_contract("test", args(&[("n", Value::Int(-1))])).unwrap_err().message;
    assert!(err.contains("Precondition"), "Expected precondition failure, got: {}", err);
}

#[test]
fn vm_postcondition() {
    let src = "contract test()\n  body:\n    return 10\n  postcondition:\n    result > 0";
    assert_eq!(run(src), Value::Int(10));
}

// ── 11. Events (2 tests) ───────────────────────────────────────

#[test]
fn vm_emit_event() {
    let src = "contract test()\n  effects:\n    emits Transferred\n  body:\n    emit Transferred(100)\n    return true";
    let (result, events) = compile_and_run_with_events(src, "test", HashMap::new());
    assert_eq!(result, Value::Bool(true));
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].0, "Transferred");
    assert_eq!(events[0].1, vec![Value::Int(100)]);
}

#[test]
fn vm_multiple_events() {
    let src = "contract test()\n  effects:\n    emits Started\n    emits Finished\n  body:\n    emit Started(1)\n    emit Finished(2)\n    return true";
    let (_, events) = compile_and_run_with_events(src, "test", HashMap::new());
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].0, "Started");
    assert_eq!(events[1].0, "Finished");
}

// ── 12. Serialization (2 tests) ────────────────────────────────

#[test]
fn vm_serialize_deserialize_module() {
    let src = "contract test()\n  body:\n    return 42";
    let tokens = Lexer::new(src, "test.cov").tokenize().unwrap();
    let program = Parser::new(tokens, "test.cov").parse().unwrap();
    let mut compiler = Compiler::new();
    let module = compiler.compile(&program);

    let bytes = module.serialize();
    let restored = Module::deserialize(&bytes).unwrap();

    // The restored module should have the same contract count and constant count
    assert_eq!(restored.contracts.len(), module.contracts.len());
    assert_eq!(restored.constants.len(), module.constants.len());
    assert_eq!(restored.contracts[0].name, "test");
}

#[test]
fn vm_roundtrip_preserves_execution() {
    let src = "contract test()\n  body:\n    let x = 10\n    let y = 32\n    return x + y";
    let tokens = Lexer::new(src, "test.cov").tokenize().unwrap();
    let program = Parser::new(tokens, "test.cov").parse().unwrap();
    let mut compiler = Compiler::new();
    let module = compiler.compile(&program);

    // Serialize and deserialize
    let bytes = module.serialize();
    let restored = Module::deserialize(&bytes).unwrap();

    // Execute the restored module
    let mut vm = VM::new(restored);
    let result = vm.run_contract("test", HashMap::new()).unwrap();
    assert_eq!(result, Value::Int(42));
}

// ── 13. Error cases (2 tests) ──────────────────────────────────

#[test]
fn vm_unknown_contract_name() {
    let src = "contract test()\n  body:\n    return 1";
    let tokens = Lexer::new(src, "test.cov").tokenize().unwrap();
    let program = Parser::new(tokens, "test.cov").parse().unwrap();
    let mut compiler = Compiler::new();
    let module = compiler.compile(&program);
    let mut vm = VM::new(module);
    let err = vm.run_contract("nonexistent", HashMap::new()).unwrap_err();
    assert!(
        err.message.contains("not found"),
        "Expected 'not found' error, got: {}",
        err.message
    );
}

#[test]
fn vm_return_null_for_implicit_return() {
    // A contract body with no explicit return yields Null
    let src = "contract test()\n  body:\n    let x = 42";
    assert_eq!(run(src), Value::Null);
}

// ── try/catch/finally (VM) ─────────────────────────────────────────────

#[test]
fn vm_try_catch_basic() {
    let src = "contract test()\n  body:\n    try:\n      let x = 1 / 0\n    catch e:\n      return e\n    return \"no error\"";
    assert_eq!(run(src), Value::Str("Division by zero".into()));
}

#[test]
fn vm_try_no_error() {
    let src = "contract test()\n  body:\n    let r = \"before\"\n    try:\n      r = \"in try\"\n    catch e:\n      r = \"in catch\"\n    return r";
    assert_eq!(run(src), Value::Str("in try".into()));
}

#[test]
fn vm_try_catch_finally() {
    let src = "contract test()\n  body:\n    let log = \"start\"\n    try:\n      let x = 1 / 0\n    catch e:\n      log = log + \",catch\"\n    finally:\n      log = log + \",finally\"\n    return log";
    assert_eq!(run(src), Value::Str("start,catch,finally".into()));
}

#[test]
fn vm_try_finally_no_catch() {
    let src = "contract test()\n  body:\n    let log = \"start\"\n    try:\n      log = log + \",try\"\n    finally:\n      log = log + \",finally\"\n    return log";
    assert_eq!(run(src), Value::Str("start,try,finally".into()));
}

#[test]
fn vm_try_success_skips_catch() {
    let src = "contract test()\n  body:\n    let reached = false\n    try:\n      let x = 42\n    catch e:\n      reached = true\n    return reached";
    assert_eq!(run(src), Value::Bool(false));
}
