//! Runtime tests — interpreter execution, type checking, stdlib dispatch

use std::collections::HashMap;
use covenant_lang::lexer::Lexer;
use covenant_lang::parser::Parser;
use covenant_lang::runtime::{Interpreter, Value};

fn run(source: &str) -> Value {
    run_contract(source, "main", HashMap::new())
}

fn run_contract(source: &str, name: &str, args: HashMap<String, Value>) -> Value {
    let tokens = Lexer::new(source, "test.cov").tokenize().unwrap();
    let program = Parser::new(tokens, "test.cov").parse().unwrap();
    let mut interp = Interpreter::new();
    interp.register_contracts(&program);
    interp.run_contract(name, args).unwrap()
}

fn run_err(source: &str) -> String {
    let tokens = Lexer::new(source, "test.cov").tokenize().unwrap();
    let program = Parser::new(tokens, "test.cov").parse().unwrap();
    let mut interp = Interpreter::new();
    interp.register_contracts(&program);
    interp.run_contract("main", HashMap::new()).unwrap_err().message
}

fn args(pairs: &[(&str, Value)]) -> HashMap<String, Value> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.clone())).collect()
}

// ── Basic values ────────────────────────────────────────────

#[test]
fn return_integer() {
    assert_eq!(run("contract main() = 42"), Value::Int(42));
}

#[test]
fn return_float() {
    assert_eq!(run("contract main() = 3.14"), Value::Float(3.14));
}

#[test]
fn return_string() {
    assert_eq!(run("contract main() = \"hello\""), Value::Str("hello".into()));
}

#[test]
fn return_bool_true() {
    assert_eq!(run("contract main() = true"), Value::Bool(true));
}

#[test]
fn return_bool_false() {
    assert_eq!(run("contract main() = false"), Value::Bool(false));
}

#[test]
fn return_null() {
    assert_eq!(run("contract main() = null"), Value::Null);
}

// ── Arithmetic ──────────────────────────────────────────────

#[test]
fn add_integers() {
    assert_eq!(run("contract main() = 3 + 4"), Value::Int(7));
}

#[test]
fn subtract() {
    assert_eq!(run("contract main() = 10 - 3"), Value::Int(7));
}

#[test]
fn multiply() {
    assert_eq!(run("contract main() = 6 * 7"), Value::Int(42));
}

#[test]
fn divide() {
    // Non-exact integer division returns Float (10 % 3 != 0)
    assert_eq!(run("contract main() = 10 / 3"), Value::Float(10.0 / 3.0));
}

#[test]
fn float_arithmetic() {
    assert_eq!(run("contract main() = 1.5 + 2.5"), Value::Float(4.0));
}

#[test]
fn mixed_int_float() {
    assert_eq!(run("contract main() = 1 + 2.5"), Value::Float(3.5));
}

#[test]
fn string_concatenation() {
    assert_eq!(run("contract main() = \"a\" + \"b\""), Value::Str("ab".into()));
}

#[test]
fn operator_precedence() {
    assert_eq!(run("contract main() = 2 + 3 * 4"), Value::Int(14));
}

#[test]
fn parenthesized_expression() {
    assert_eq!(run("contract main() = (2 + 3) * 4"), Value::Int(20));
}

// ── Comparisons ─────────────────────────────────────────────

#[test]
fn equal() {
    assert_eq!(run("contract main() = 1 == 1"), Value::Bool(true));
}

#[test]
fn not_equal() {
    assert_eq!(run("contract main() = 1 != 2"), Value::Bool(true));
}

#[test]
fn less_than() {
    assert_eq!(run("contract main() = 1 < 2"), Value::Bool(true));
}

#[test]
fn greater_than() {
    assert_eq!(run("contract main() = 2 > 1"), Value::Bool(true));
}

// ── Logic ───────────────────────────────────────────────────

#[test]
fn and_true() {
    assert_eq!(run("contract main() = true and true"), Value::Bool(true));
}

#[test]
fn and_false() {
    assert_eq!(run("contract main() = true and false"), Value::Bool(false));
}

#[test]
fn or_true() {
    assert_eq!(run("contract main() = false or true"), Value::Bool(true));
}

#[test]
fn not_true() {
    assert_eq!(run("contract main() = not true"), Value::Bool(false));
}

// ── Variables and assignment ────────────────────────────────

#[test]
fn variable_assignment() {
    let src = "contract main()\n  body:\n    x = 42\n    return x";
    assert_eq!(run(src), Value::Int(42));
}

#[test]
fn variable_reassignment() {
    let src = "contract main()\n  body:\n    x = 1\n    x = 2\n    return x";
    assert_eq!(run(src), Value::Int(2));
}

// ── Control flow ────────────────────────────────────────────

#[test]
fn if_true_branch() {
    let src = "contract main()\n  body:\n    if true:\n      return 1\n    return 0";
    assert_eq!(run(src), Value::Int(1));
}

#[test]
fn if_false_branch() {
    let src = "contract main()\n  body:\n    if false:\n      return 1\n    return 0";
    assert_eq!(run(src), Value::Int(0));
}

#[test]
fn if_else() {
    let src = "contract main()\n  body:\n    if false:\n      return 1\n    else:\n      return 2";
    assert_eq!(run(src), Value::Int(2));
}

#[test]
fn for_loop_sum() {
    let src = "\
contract main()
  body:
    total = 0
    for i in [1, 2, 3, 4, 5]:
      total = total + i
    return total";
    assert_eq!(run(src), Value::Int(15));
}

#[test]
fn while_loop() {
    let src = "\
contract main()
  body:
    x = 0
    while x < 5:
      x = x + 1
    return x";
    assert_eq!(run(src), Value::Int(5));
}

// ── Contract calls ──────────────────────────────────────────

#[test]
fn call_another_contract() {
    let src = "\
contract square(n: Int) -> Int = n * n
contract main() = square(5)";
    assert_eq!(run(src), Value::Int(25));
}

#[test]
fn multiple_contract_calls() {
    let src = "\
contract double(n: Int) -> Int = n * 2
contract add(a: Int, b: Int) -> Int = a + b
contract main() = add(double(3), double(4))";
    assert_eq!(run(src), Value::Int(14));
}

#[test]
fn recursive_contract() {
    let src = "\
contract factorial(n: Int) -> Int
  body:
    if n <= 1:
      return 1
    return n * factorial(n - 1)
contract main() = factorial(5)";
    assert_eq!(run(src), Value::Int(120));
}

// ── Contract arguments ──────────────────────────────────────

#[test]
fn contract_with_args() {
    let src = "contract add(a: Int, b: Int) -> Int = a + b";
    let result = run_contract(src, "add", args(&[("a", Value::Int(3)), ("b", Value::Int(4))]));
    assert_eq!(result, Value::Int(7));
}

// ── Lists ───────────────────────────────────────────────────

#[test]
fn list_literal() {
    let src = "contract main() = [1, 2, 3]";
    let result = run(src);
    assert_eq!(result, Value::List(vec![Value::Int(1), Value::Int(2), Value::Int(3)]));
}

#[test]
fn list_indexing() {
    let src = "\
contract main()
  body:
    items = [10, 20, 30]
    return items[1]";
    assert_eq!(run(src), Value::Int(20));
}

#[test]
fn empty_list() {
    assert_eq!(run("contract main() = []"), Value::List(vec![]));
}

// ── Objects ─────────────────────────────────────────────────

#[test]
fn constructor() {
    let src = "\
contract main()
  body:
    p = Point(x: 1, y: 2)
    return p.x";
    assert_eq!(run(src), Value::Int(1));
}

#[test]
fn field_access() {
    let src = "\
contract main()
  body:
    p = Point(x: 10, y: 20)
    return p.y";
    assert_eq!(run(src), Value::Int(20));
}

// ── Builtins ────────────────────────────────────────────────

#[test]
fn builtin_len() {
    let src = "contract main() = len([1, 2, 3])";
    assert_eq!(run(src), Value::Int(3));
}

#[test]
fn builtin_str() {
    let src = "contract main() = str(42)";
    assert_eq!(run(src), Value::Str("42".into()));
}

#[test]
fn builtin_int() {
    let src = "contract main() = int(\"42\")";
    assert_eq!(run(src), Value::Int(42));
}

#[test]
fn builtin_range() {
    let src = "\
contract main()
  body:
    total = 0
    for i in range(5):
      total = total + i
    return total";
    assert_eq!(run(src), Value::Int(10)); // 0+1+2+3+4
}

#[test]
fn builtin_type_check() {
    // `type` is a keyword in Covenant, so `type(42)` cannot be parsed
    // as a function call expression — the parser rejects it
    let src = "contract main() = type(42)";
    let tokens = Lexer::new(src, "test.cov").tokenize().unwrap();
    let result = Parser::new(tokens, "test.cov").parse();
    assert!(result.is_err());
}

// ── Preconditions ───────────────────────────────────────────

#[test]
fn precondition_passes() {
    let src = "contract f(x: Int) -> Int\n  precondition:\n    x > 0\n  body:\n    return x";
    let result = run_contract(src, "f", args(&[("x", Value::Int(5))]));
    assert_eq!(result, Value::Int(5));
}

#[test]
fn precondition_fails() {
    let src = "contract f(x: Int) -> Int\n  precondition:\n    x > 0\n  body:\n    return x";
    let tokens = Lexer::new(src, "test.cov").tokenize().unwrap();
    let program = Parser::new(tokens, "test.cov").parse().unwrap();
    let mut interp = Interpreter::new();
    interp.register_contracts(&program);
    let err = interp.run_contract("f", args(&[("x", Value::Int(-1))])).unwrap_err();
    assert!(err.message.contains("Precondition"), "Expected precondition error: {}", err.message);
}

// ── Type checking ───────────────────────────────────────────

#[test]
fn type_check_int_param() {
    let src = "contract f(x: Int) -> Int\n  body:\n    return x";
    let result = run_contract(src, "f", args(&[("x", Value::Int(5))]));
    assert_eq!(result, Value::Int(5));
}

#[test]
fn type_check_wrong_type() {
    let src = "contract main()\n  body:\n    return f(\"not a number\")\ncontract f(x: Int) -> Int\n  body:\n    return x";
    let err = run_err(src);
    assert!(err.contains("Type error") || err.contains("type"), "Expected type error: {}", err);
}

// ── Error cases ─────────────────────────────────────────────

#[test]
fn division_by_zero() {
    let err = run_err("contract main() = 1 / 0");
    assert!(err.contains("zero") || err.contains("divide"), "Expected division error: {}", err);
}

#[test]
fn undefined_variable() {
    // Undefined variables return Null in lenient mode (not an error)
    let result = run("contract main()\n  body:\n    return undefined_var");
    assert_eq!(result, Value::Null);
}

// ── Stdlib ──────────────────────────────────────────────────

#[test]
fn stdlib_math_sqrt() {
    let src = "contract main() = math.sqrt(16.0)";
    assert_eq!(run(src), Value::Float(4.0));
}

#[test]
fn stdlib_text_upper() {
    let src = "contract main() = text.upper(\"hello\")";
    assert_eq!(run(src), Value::Str("HELLO".into()));
}

#[test]
fn stdlib_text_split() {
    let src = "contract main() = text.split(\"a,b,c\", \",\")";
    let result = run(src);
    assert_eq!(result, Value::List(vec![
        Value::Str("a".into()),
        Value::Str("b".into()),
        Value::Str("c".into()),
    ]));
}

#[test]
fn stdlib_json_parse() {
    let src = "contract main()\n  body:\n    obj = json.parse(\"{\\\"x\\\": 42}\")\n    return obj.x";
    assert_eq!(run(src), Value::Int(42));
}

#[test]
fn stdlib_crypto_sha256() {
    let src = "contract main() = crypto.sha256(\"test\")";
    let result = run(src);
    if let Value::Str(hash) = result {
        assert_eq!(hash.len(), 64); // SHA-256 is 64 hex chars
    } else {
        panic!("Expected string hash");
    }
}

#[test]
fn stdlib_math_pi() {
    // math.pi() must be called as a method (not field access) to go through
    // the stdlib dispatcher
    let src = "contract main() = math.pi()";
    if let Value::Float(f) = run(src) {
        assert!((f - std::f64::consts::PI).abs() < 0.001);
    } else {
        panic!("Expected float");
    }
}

// ── try/catch/finally ──────────────────────────────────────────────────

#[test]
fn try_catch_basic() {
    let src = "contract main()\n  body:\n    try:\n      let x = 1 / 0\n    catch e:\n      return e\n    return \"no error\"";
    assert_eq!(run(src), Value::Str("Division by zero".into()));
}

#[test]
fn try_no_error() {
    let src = "contract main()\n  body:\n    let r = \"before\"\n    try:\n      r = \"in try\"\n    catch e:\n      r = \"in catch\"\n    return r";
    assert_eq!(run(src), Value::Str("in try".into()));
}

#[test]
fn try_catch_finally_all_run() {
    let src = "contract main()\n  body:\n    let log = \"start\"\n    try:\n      let x = 1 / 0\n    catch e:\n      log = log + \",catch\"\n    finally:\n      log = log + \",finally\"\n    return log";
    assert_eq!(run(src), Value::Str("start,catch,finally".into()));
}

#[test]
fn try_finally_no_catch() {
    let src = "contract main()\n  body:\n    let log = \"start\"\n    try:\n      log = log + \",try\"\n    finally:\n      log = log + \",finally\"\n    return log";
    assert_eq!(run(src), Value::Str("start,try,finally".into()));
}

#[test]
fn try_catch_var_binding() {
    let src = "contract main()\n  body:\n    try:\n      let x = 1 / 0\n    catch err:\n      return err\n    return \"unreachable\"";
    let result = run(src);
    if let Value::Str(msg) = result {
        assert!(msg.contains("Division"), "Expected division error, got: {}", msg);
    } else {
        panic!("Expected string error message");
    }
}

#[test]
fn try_success_skips_catch() {
    let src = "contract main()\n  body:\n    let reached = false\n    try:\n      let x = 42\n    catch e:\n      reached = true\n    return reached";
    assert_eq!(run(src), Value::Bool(false));
}
