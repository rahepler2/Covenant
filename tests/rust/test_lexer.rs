//! Lexer tests — tokenization, indentation, error handling

use covenant_lang::lexer::Lexer;
use covenant_lang::lexer::tokens::TokenType;

fn lex(source: &str) -> Vec<(TokenType, String)> {
    let tokens = Lexer::new(source, "test.cov").tokenize().unwrap();
    tokens.into_iter()
        .filter(|t| !matches!(t.token_type, TokenType::Eof))
        .map(|t| (t.token_type, t.value))
        .collect()
}

fn lex_types(source: &str) -> Vec<TokenType> {
    lex(source).into_iter().map(|(tt, _)| tt).collect()
}

fn lex_err(source: &str) -> String {
    Lexer::new(source, "test.cov").tokenize().unwrap_err().message
}

// ── Basic tokens ────────────────────────────────────────────

#[test]
fn identifier() {
    let tokens = lex("hello");
    assert_eq!(tokens.len(), 1); // Identifier only (no trailing newline in input)
    assert_eq!(tokens[0], (TokenType::Identifier, "hello".into()));
}

#[test]
fn integer_literal() {
    let tokens = lex("42");
    assert_eq!(tokens[0], (TokenType::Integer, "42".into()));
}

#[test]
fn float_literal() {
    let tokens = lex("3.14");
    assert_eq!(tokens[0], (TokenType::Float, "3.14".into()));
}

#[test]
fn string_literal() {
    let tokens = lex("\"hello world\"");
    assert_eq!(tokens[0], (TokenType::StringLit, "hello world".into()));
}

#[test]
fn string_escape_sequences() {
    let tokens = lex("\"line1\\nline2\\ttab\\\\backslash\"");
    assert_eq!(tokens[0].1, "line1\nline2\ttab\\backslash");
}

#[test]
fn boolean_literals() {
    let tokens = lex("true false");
    assert_eq!(tokens[0].0, TokenType::True);
    assert_eq!(tokens[1].0, TokenType::False);
}

#[test]
fn null_literal() {
    let tokens = lex("null");
    assert_eq!(tokens[0].0, TokenType::Null);
}

// ── Operators ───────────────────────────────────────────────

#[test]
fn arithmetic_operators() {
    let types = lex_types("+ - * /");
    assert_eq!(types[0], TokenType::Plus);
    assert_eq!(types[1], TokenType::Minus);
    assert_eq!(types[2], TokenType::Star);
    assert_eq!(types[3], TokenType::Slash);
}

#[test]
fn comparison_operators() {
    let types = lex_types("== != < <= > >=");
    assert_eq!(types[0], TokenType::Equals);
    assert_eq!(types[1], TokenType::NotEquals);
    assert_eq!(types[2], TokenType::LessThan);
    assert_eq!(types[3], TokenType::LessEqual);
    assert_eq!(types[4], TokenType::GreaterThan);
    assert_eq!(types[5], TokenType::GreaterEqual);
}

#[test]
fn assignment_vs_equality() {
    let types = lex_types("x = 1\ny == 2");
    // x = 1
    assert_eq!(types[0], TokenType::Identifier); // x
    assert_eq!(types[1], TokenType::Assign);      // =
    assert_eq!(types[2], TokenType::Integer);      // 1
    // newline
    assert_eq!(types[3], TokenType::Newline);
    // y == 2
    assert_eq!(types[4], TokenType::Identifier);   // y
    assert_eq!(types[5], TokenType::Equals);        // ==
    assert_eq!(types[6], TokenType::Integer);       // 2
}

#[test]
fn arrow_operator() {
    let types = lex_types("-> ");
    assert_eq!(types[0], TokenType::Arrow);
}

#[test]
fn brackets_and_parens() {
    let types = lex_types("()[]");
    assert_eq!(types[0], TokenType::LParen);
    assert_eq!(types[1], TokenType::RParen);
    assert_eq!(types[2], TokenType::LBracket);
    assert_eq!(types[3], TokenType::RBracket);
}

// ── Keywords ────────────────────────────────────────────────

#[test]
fn contract_keyword() {
    let types = lex_types("contract");
    assert_eq!(types[0], TokenType::Contract);
}

#[test]
fn all_major_keywords() {
    let src = "intent scope risk contract precondition postcondition effects body return";
    let types = lex_types(src);
    assert_eq!(types[0], TokenType::Intent);
    assert_eq!(types[1], TokenType::Scope);
    assert_eq!(types[2], TokenType::Risk);
    assert_eq!(types[3], TokenType::Contract);
    assert_eq!(types[4], TokenType::Precondition);
    assert_eq!(types[5], TokenType::Postcondition);
    assert_eq!(types[6], TokenType::Effects);
    assert_eq!(types[7], TokenType::Body);
    assert_eq!(types[8], TokenType::Return);
}

#[test]
fn logic_keywords() {
    let types = lex_types("and or not");
    assert_eq!(types[0], TokenType::And);
    assert_eq!(types[1], TokenType::Or);
    assert_eq!(types[2], TokenType::Not);
}

#[test]
fn control_flow_keywords() {
    let types = lex_types("if else for in while");
    assert_eq!(types[0], TokenType::If);
    assert_eq!(types[1], TokenType::Else);
    assert_eq!(types[2], TokenType::For);
    assert_eq!(types[3], TokenType::In);
    assert_eq!(types[4], TokenType::While);
}

#[test]
fn async_keywords() {
    let types = lex_types("async await parallel");
    assert_eq!(types[0], TokenType::Async);
    assert_eq!(types[1], TokenType::Await);
    assert_eq!(types[2], TokenType::Parallel);
}

#[test]
fn pure_keyword() {
    let types = lex_types("pure");
    assert_eq!(types[0], TokenType::Pure);
}

// ── Comments ────────────────────────────────────────────────

#[test]
fn comment_skipped() {
    let tokens = lex("x -- this is a comment");
    assert_eq!(tokens[0], (TokenType::Identifier, "x".into()));
    assert_eq!(tokens.len(), 1); // x only (no trailing newline in input)
}

#[test]
fn comment_only_line_skipped() {
    let tokens = lex("-- just a comment\nx");
    let types: Vec<_> = tokens.iter().map(|(t, _)| t.clone()).collect();
    assert!(types.contains(&TokenType::Identifier));
}

// ── Indentation ─────────────────────────────────────────────

#[test]
fn indent_dedent() {
    let src = "a:\n  b\nc";
    let types = lex_types(src);
    assert!(types.contains(&TokenType::Indent));
    assert!(types.contains(&TokenType::Dedent));
}

#[test]
fn nested_indent() {
    let src = "a:\n  b:\n    c\nd";
    let types = lex_types(src);
    let indent_count = types.iter().filter(|t| **t == TokenType::Indent).count();
    let dedent_count = types.iter().filter(|t| **t == TokenType::Dedent).count();
    assert_eq!(indent_count, 2);
    assert_eq!(dedent_count, 2);
}

#[test]
fn tab_is_error() {
    let err = lex_err("\tx");
    assert!(err.contains("Tab"), "Expected tab error, got: {}", err);
}

#[test]
fn odd_indent_is_error() {
    let src = "a:\n   b"; // 3 spaces (not multiple of 2)
    let result = Lexer::new(src, "test.cov").tokenize();
    assert!(result.is_err());
}

// ── Dot access ──────────────────────────────────────────────

#[test]
fn dotted_identifier() {
    let types = lex_types("a.b.c");
    assert_eq!(types[0], TokenType::Identifier); // a
    assert_eq!(types[1], TokenType::Dot);
    assert_eq!(types[2], TokenType::Identifier); // b
    assert_eq!(types[3], TokenType::Dot);
    assert_eq!(types[4], TokenType::Identifier); // c
}

#[test]
fn keyword_as_field_name() {
    // `access` is a keyword — the lexer always emits Access, not Identifier.
    // The parser's can_be_identifier() / expect_identifier_or_keyword()
    // handles promotion to identifier position.
    let tokens = lex("obj.access");
    assert_eq!(tokens[0], (TokenType::Identifier, "obj".into()));
    assert_eq!(tokens[1].0, TokenType::Dot);
    assert_eq!(tokens[2], (TokenType::Access, "access".into()));
}

// ── Complex expressions ─────────────────────────────────────

#[test]
fn function_call() {
    let types = lex_types("f(x, y)");
    assert_eq!(types[0], TokenType::Identifier); // f
    assert_eq!(types[1], TokenType::LParen);
    assert_eq!(types[2], TokenType::Identifier); // x
    assert_eq!(types[3], TokenType::Comma);
    assert_eq!(types[4], TokenType::Identifier); // y
    assert_eq!(types[5], TokenType::RParen);
}

#[test]
fn list_literal() {
    let types = lex_types("[1, 2, 3]");
    assert_eq!(types[0], TokenType::LBracket);
    assert_eq!(types[1], TokenType::Integer);
    assert_eq!(types[2], TokenType::Comma);
    assert_eq!(types[3], TokenType::Integer);
    assert_eq!(types[4], TokenType::Comma);
    assert_eq!(types[5], TokenType::Integer);
    assert_eq!(types[6], TokenType::RBracket);
}

#[test]
fn contract_header() {
    let src = "contract add(a: Int, b: Int) -> Int";
    let types = lex_types(src);
    assert_eq!(types[0], TokenType::Contract);
    assert_eq!(types[1], TokenType::Identifier); // add
    assert_eq!(types[2], TokenType::LParen);
}

// ── Line numbers ────────────────────────────────────────────

#[test]
fn line_numbers_tracked() {
    let src = "a\nb\nc";
    let tokens = Lexer::new(src, "test.cov").tokenize().unwrap();
    let a = tokens.iter().find(|t| t.value == "a").unwrap();
    let b = tokens.iter().find(|t| t.value == "b").unwrap();
    let c = tokens.iter().find(|t| t.value == "c").unwrap();
    assert_eq!(a.line, 1);
    assert_eq!(b.line, 2);
    assert_eq!(c.line, 3);
}

#[test]
fn filename_preserved() {
    let tokens = Lexer::new("x", "myfile.cov").tokenize().unwrap();
    assert_eq!(tokens[0].file, "myfile.cov");
}

// ── Edge cases ──────────────────────────────────────────────

#[test]
fn empty_source() {
    let tokens = Lexer::new("", "test.cov").tokenize().unwrap();
    assert_eq!(tokens.len(), 1); // Just EOF
    assert_eq!(tokens[0].token_type, TokenType::Eof);
}

#[test]
fn only_comments() {
    let tokens = Lexer::new("-- comment\n-- another", "test.cov").tokenize().unwrap();
    assert_eq!(tokens.last().unwrap().token_type, TokenType::Eof);
}

#[test]
fn negative_number() {
    let types = lex_types("-42");
    assert_eq!(types[0], TokenType::Minus);
    assert_eq!(types[1], TokenType::Integer);
}

#[test]
fn emit_keyword() {
    let types = lex_types("emit TransferEvent(a, b)");
    assert_eq!(types[0], TokenType::Emit);
    assert_eq!(types[1], TokenType::Identifier);
    assert_eq!(types[2], TokenType::LParen);
}

#[test]
fn old_keyword() {
    let types = lex_types("old(x)");
    assert_eq!(types[0], TokenType::Old);
    assert_eq!(types[1], TokenType::LParen);
    assert_eq!(types[2], TokenType::Identifier);
    assert_eq!(types[3], TokenType::RParen);
}

#[test]
fn has_keyword() {
    let types = lex_types("x has permission");
    assert_eq!(types[0], TokenType::Identifier);
    assert_eq!(types[1], TokenType::Has);
    assert_eq!(types[2], TokenType::Identifier);
}

#[test]
fn use_keyword() {
    let types = lex_types("use math");
    assert_eq!(types[0], TokenType::Use);
    assert_eq!(types[1], TokenType::Identifier);
}

#[test]
fn risk_levels() {
    let types = lex_types("low medium high critical");
    assert_eq!(types[0], TokenType::Low);
    assert_eq!(types[1], TokenType::Medium);
    assert_eq!(types[2], TokenType::High);
    assert_eq!(types[3], TokenType::Critical);
}

// ── Full mini-program ───────────────────────────────────────

#[test]
fn full_expression_body_contract() {
    let src = "contract square(n: Int) -> Int = n * n";
    let tokens = Lexer::new(src, "test.cov").tokenize().unwrap();
    assert!(tokens.iter().any(|t| t.token_type == TokenType::Contract));
    assert!(tokens.iter().any(|t| t.token_type == TokenType::Arrow));
    assert!(tokens.iter().any(|t| t.token_type == TokenType::Assign));
    assert!(tokens.iter().any(|t| t.token_type == TokenType::Star));
}

#[test]
fn full_contract_with_body() {
    let src = "\
contract add(a: Int, b: Int) -> Int
  body:
    return a + b
";
    let tokens = Lexer::new(src, "test.cov").tokenize().unwrap();
    let types: Vec<_> = tokens.iter().map(|t| t.token_type.clone()).collect();
    assert!(types.contains(&TokenType::Contract));
    assert!(types.contains(&TokenType::Body));
    assert!(types.contains(&TokenType::Indent));
    assert!(types.contains(&TokenType::Dedent));
    assert!(types.contains(&TokenType::Return));
}
