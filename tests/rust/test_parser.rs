//! Parser tests — AST construction, contract parsing, expressions

use covenant_lang::lexer::Lexer;
use covenant_lang::parser::Parser;
use covenant_lang::ast::*;

fn parse(source: &str) -> Program {
    let tokens = Lexer::new(source, "test.cov").tokenize().unwrap();
    Parser::new(tokens, "test.cov").parse().unwrap()
}

fn parse_err(source: &str) -> String {
    let tokens = Lexer::new(source, "test.cov").tokenize().unwrap();
    Parser::new(tokens, "test.cov").parse().unwrap_err().to_string()
}

// ── Header parsing ──────────────────────────────────────────

#[test]
fn parse_intent() {
    let p = parse("intent: \"My intent\"\nscope: a.b\nrisk: low\ncontract x()\n  body:\n    return 0");
    let header = p.header.as_ref().unwrap();
    assert_eq!(header.intent.as_ref().unwrap().text, "My intent");
}

#[test]
fn parse_scope() {
    let p = parse("intent: \"test\"\nscope: finance.transfers\nrisk: low\ncontract x()\n  body:\n    return 0");
    let header = p.header.as_ref().unwrap();
    assert_eq!(header.scope.as_ref().unwrap().path, "finance.transfers");
}

#[test]
fn parse_risk_levels() {
    for (level_str, expected) in [("low", RiskLevel::Low), ("medium", RiskLevel::Medium),
                                    ("high", RiskLevel::High), ("critical", RiskLevel::Critical)] {
        let src = format!("intent: \"t\"\nscope: a.b\nrisk: {}\ncontract x()\n  body:\n    return 0", level_str);
        let p = parse(&src);
        assert_eq!(p.header.as_ref().unwrap().risk.as_ref().unwrap().level, expected);
    }
}

// ── Contract parsing ────────────────────────────────────────

#[test]
fn simple_contract() {
    let p = parse("contract add(a: Int, b: Int) -> Int\n  body:\n    return a + b");
    assert_eq!(p.contracts.len(), 1);
    assert_eq!(p.contracts[0].name, "add");
    assert_eq!(p.contracts[0].params.len(), 2);
    assert_eq!(p.contracts[0].params[0].name, "a");
    assert_eq!(p.contracts[0].params[1].name, "b");
}

#[test]
fn expression_body() {
    let p = parse("contract square(n: Int) -> Int = n * n");
    assert_eq!(p.contracts[0].name, "square");
    assert!(p.contracts[0].body.is_some());
}

#[test]
fn no_params() {
    let p = parse("contract main()\n  body:\n    return 0");
    assert_eq!(p.contracts[0].params.len(), 0);
}

#[test]
fn optional_param_types() {
    let p = parse("contract add(a, b) = a + b");
    assert_eq!(p.contracts[0].params[0].type_expr.display_name(), "Any");
    assert_eq!(p.contracts[0].params[1].type_expr.display_name(), "Any");
}

#[test]
fn return_type() {
    let p = parse("contract f() -> String\n  body:\n    return \"hi\"");
    assert_eq!(p.contracts[0].return_type.as_ref().unwrap().display_name(), "String");
}

#[test]
fn no_return_type() {
    let p = parse("contract f()\n  body:\n    return 0");
    assert!(p.contracts[0].return_type.is_none());
}

#[test]
fn generic_type_in_params() {
    let p = parse("contract f(items: List<Int>) -> List<Int>\n  body:\n    return items");
    let param_type = &p.contracts[0].params[0].type_expr;
    assert!(matches!(param_type, TypeExpr::Generic { name, params, .. }
        if name == "List" && params.len() == 1));
}

#[test]
fn multiple_contracts() {
    let src = "contract a() = 1\ncontract b() = 2\ncontract c() = 3";
    let p = parse(src);
    assert_eq!(p.contracts.len(), 3);
    assert_eq!(p.contracts[0].name, "a");
    assert_eq!(p.contracts[1].name, "b");
    assert_eq!(p.contracts[2].name, "c");
}

#[test]
fn async_contract() {
    let p = parse("async contract fetch()\n  body:\n    return 0");
    assert!(p.contracts[0].is_async);
}

#[test]
fn pure_keyword() {
    let p = parse("contract f(x: Int) -> Int\n  pure\n  body:\n    return x");
    // pure sets effects to touches_nothing_else
    assert!(p.contracts[0].effects.is_some());
}

// ── Contract sections ───────────────────────────────────────

#[test]
fn precondition() {
    let src = "contract f(x: Int)\n  precondition:\n    x > 0\n  body:\n    return x";
    let p = parse(src);
    assert!(p.contracts[0].precondition.is_some());
    assert_eq!(p.contracts[0].precondition.as_ref().unwrap().conditions.len(), 1);
}

#[test]
fn postcondition() {
    let src = "contract f() -> Int\n  postcondition:\n    result > 0\n  body:\n    return 1";
    let p = parse(src);
    assert!(p.contracts[0].postcondition.is_some());
}

#[test]
fn effects_modifies() {
    let src = "contract f()\n  effects:\n    modifies [x, y]\n  body:\n    return 0";
    let p = parse(src);
    let effects = p.contracts[0].effects.as_ref().unwrap();
    assert!(!effects.declarations.is_empty());
}

#[test]
fn effects_emits() {
    let src = "contract f()\n  effects:\n    emits MyEvent\n  body:\n    emit MyEvent()";
    let p = parse(src);
    let effects = p.contracts[0].effects.as_ref().unwrap();
    assert!(effects.declarations.iter().any(|d| matches!(d, EffectDecl::Emits { event_type, .. } if event_type == "MyEvent")));
}

#[test]
fn effects_touches_nothing_else() {
    let src = "contract f()\n  effects:\n    touches_nothing_else\n  body:\n    return 0";
    let p = parse(src);
    let effects = p.contracts[0].effects.as_ref().unwrap();
    assert!(effects.declarations.iter().any(|d| matches!(d, EffectDecl::TouchesNothingElse { .. })));
}

#[test]
fn on_failure() {
    let src = "contract f()\n  body:\n    return 1\n  on_failure:\n    return 0";
    let p = parse(src);
    assert!(p.contracts[0].on_failure.is_some());
}

// ── Expressions ─────────────────────────────────────────────

#[test]
fn binary_expression() {
    let p = parse("contract f() = 1 + 2");
    // The body contains a return with a BinaryOp
    assert!(p.contracts[0].body.is_some());
}

#[test]
fn comparison_expression() {
    let p = parse("contract f(x: Int) -> Bool = x > 0");
    assert!(p.contracts[0].body.is_some());
}

#[test]
fn string_concatenation() {
    let p = parse("contract f() = \"a\" + \"b\"");
    assert!(p.contracts[0].body.is_some());
}

#[test]
fn list_literal() {
    let p = parse("contract f() = [1, 2, 3]");
    assert!(p.contracts[0].body.is_some());
}

#[test]
fn constructor_call() {
    let p = parse("contract f() = Point(x: 1, y: 2)");
    assert!(p.contracts[0].body.is_some());
}

#[test]
fn method_call() {
    let src = "contract f()\n  body:\n    x = math.sqrt(4)";
    let p = parse(src);
    assert!(p.contracts[0].body.is_some());
}

#[test]
fn old_expression() {
    let src = "contract f()\n  postcondition:\n    x == old(x) + 1\n  body:\n    return 0";
    let p = parse(src);
    assert!(p.contracts[0].postcondition.is_some());
}

// ── Statements ──────────────────────────────────────────────

#[test]
fn assignment() {
    let src = "contract f()\n  body:\n    x = 42\n    return x";
    let p = parse(src);
    let stmts = &p.contracts[0].body.as_ref().unwrap().statements;
    assert!(stmts.len() >= 2);
}

#[test]
fn if_statement() {
    let src = "contract f(x: Int)\n  body:\n    if x > 0:\n      return 1\n    return 0";
    let p = parse(src);
    assert!(p.contracts[0].body.is_some());
}

#[test]
fn if_else() {
    let src = "contract f(x: Int)\n  body:\n    if x > 0:\n      return 1\n    else:\n      return 0";
    let p = parse(src);
    assert!(p.contracts[0].body.is_some());
}

#[test]
fn for_loop() {
    let src = "contract f()\n  body:\n    for i in [1, 2, 3]:\n      x = i\n    return 0";
    let p = parse(src);
    assert!(p.contracts[0].body.is_some());
}

#[test]
fn while_loop() {
    let src = "contract f()\n  body:\n    x = 0\n    while x < 10:\n      x = x + 1\n    return x";
    let p = parse(src);
    assert!(p.contracts[0].body.is_some());
}

#[test]
fn emit_statement() {
    let src = "contract f()\n  effects:\n    emits E\n  body:\n    emit E(1, 2)";
    let p = parse(src);
    assert!(p.contracts[0].body.is_some());
}

#[test]
fn parallel_block() {
    let src = "contract f()\n  body:\n    parallel:\n      x = 1\n      y = 2\n    return 0";
    let p = parse(src);
    assert!(p.contracts[0].body.is_some());
}

// ── Type definitions ────────────────────────────────────────

#[test]
fn type_definition() {
    let src = "\
type Account = Record
  fields:
    balance: Currency
    owner: String
contract f() = 0";
    let p = parse(src);
    assert_eq!(p.type_defs.len(), 1);
    assert_eq!(p.type_defs[0].name, "Account");
}

// ── Use declarations ────────────────────────────────────────

#[test]
fn use_declaration() {
    let src = "use math\ncontract f() = math.pi";
    let p = parse(src);
    assert_eq!(p.uses.len(), 1);
    assert_eq!(p.uses[0].name, "math");
}

// ── Error cases ─────────────────────────────────────────────

#[test]
fn missing_body() {
    let err = parse_err("contract f()");
    assert!(!err.is_empty());
}

#[test]
fn unterminated_string() {
    let result = Lexer::new("\"unterminated", "test.cov").tokenize();
    assert!(result.is_err());
}
