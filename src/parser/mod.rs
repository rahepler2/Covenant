use crate::ast::*;
use crate::lexer::tokens::{Token, TokenType};
use std::fmt;

#[derive(Debug)]
pub struct ParseError {
    pub message: String,
    pub line: usize,
    pub column: usize,
    pub file: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}:{}: {}", self.file, self.line, self.column, self.message)
    }
}

impl std::error::Error for ParseError {}

const MAX_PARSER_DEPTH: usize = 256;

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    #[allow(dead_code)]
    filename: String,
    depth: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>, filename: &str) -> Self {
        Self {
            tokens,
            pos: 0,
            filename: filename.to_string(),
            depth: 0,
        }
    }

    fn enter_depth(&mut self) -> Result<(), ParseError> {
        self.depth += 1;
        if self.depth > MAX_PARSER_DEPTH {
            let tok = self.current();
            Err(ParseError {
                message: format!(
                    "Maximum nesting depth ({}) exceeded — expression is too deeply nested",
                    MAX_PARSER_DEPTH
                ),
                line: tok.line,
                column: tok.column,
                file: tok.file.clone(),
            })
        } else {
            Ok(())
        }
    }

    fn exit_depth(&mut self) {
        self.depth -= 1;
    }

    // ── Public API ──────────────────────────────────────────────────────

    pub fn parse(&mut self) -> Result<Program, ParseError> {
        let header = self.parse_file_header()?;
        let mut contracts = Vec::new();
        let mut type_defs = Vec::new();
        let mut shared_decls = Vec::new();

        loop {
            self.skip_newlines();
            if self.at_end() {
                break;
            }

            if self.check(TokenType::Contract) {
                contracts.push(self.parse_contract_def()?);
            } else if self.check(TokenType::Type) {
                type_defs.push(self.parse_type_def()?);
            } else if self.check(TokenType::Shared) {
                shared_decls.push(self.parse_shared_decl()?);
            } else if self.check(TokenType::Eof) {
                break;
            } else {
                let cur = self.current();
                return Err(ParseError {
                    message: format!(
                        "Expected 'contract', 'type', or 'shared' at top level, got {:?}",
                        cur.token_type
                    ),
                    line: cur.line,
                    column: cur.column,
                    file: cur.file.clone(),
                });
            }
        }

        Ok(Program {
            loc: self.loc(),
            header,
            contracts,
            type_defs,
            shared_decls,
        })
    }

    // ── File header ─────────────────────────────────────────────────────

    fn parse_file_header(&mut self) -> Result<Option<FileHeader>, ParseError> {
        self.skip_newlines();

        let mut intent = None;
        let mut scope = None;
        let mut risk = None;
        let mut requires = None;

        if self.check(TokenType::Intent) {
            intent = Some(self.parse_intent_block()?);
            self.skip_newlines();
        }
        if self.check(TokenType::Scope) {
            scope = Some(self.parse_scope_decl()?);
            self.skip_newlines();
        }
        if self.check(TokenType::Risk) {
            risk = Some(self.parse_risk_decl()?);
            self.skip_newlines();
        }
        if self.check(TokenType::Requires) {
            requires = Some(self.parse_requires_decl()?);
            self.skip_newlines();
        }

        if intent.is_none() && scope.is_none() && risk.is_none() && requires.is_none() {
            return Ok(None);
        }

        Ok(Some(FileHeader {
            loc: self.loc(),
            intent,
            scope,
            risk,
            requires,
        }))
    }

    fn parse_intent_block(&mut self) -> Result<IntentBlock, ParseError> {
        let loc = self.loc();
        self.expect(TokenType::Intent)?;
        self.expect(TokenType::Colon)?;
        let text_token = self.expect(TokenType::StringLit)?;
        Ok(IntentBlock {
            loc,
            text: text_token.value.clone(),
        })
    }

    fn parse_scope_decl(&mut self) -> Result<ScopeDecl, ParseError> {
        let loc = self.loc();
        self.expect(TokenType::Scope)?;
        self.expect(TokenType::Colon)?;
        let path = self.parse_dotted_name()?;
        Ok(ScopeDecl { loc, path })
    }

    fn parse_risk_decl(&mut self) -> Result<RiskDecl, ParseError> {
        let loc = self.loc();
        self.expect(TokenType::Risk)?;
        self.expect(TokenType::Colon)?;
        let token = self.current().clone();
        let level = match token.token_type {
            TokenType::Low => RiskLevel::Low,
            TokenType::Medium => RiskLevel::Medium,
            TokenType::High => RiskLevel::High,
            TokenType::Critical => RiskLevel::Critical,
            _ => {
                return Err(ParseError {
                    message: format!(
                        "Expected risk level (low, medium, high, critical), got {:?}",
                        token.value
                    ),
                    line: token.line,
                    column: token.column,
                    file: token.file.clone(),
                });
            }
        };
        self.advance();
        Ok(RiskDecl { loc, level })
    }

    fn parse_requires_decl(&mut self) -> Result<RequiresDecl, ParseError> {
        let loc = self.loc();
        self.expect(TokenType::Requires)?;
        self.expect(TokenType::Colon)?;
        let capabilities = self.parse_bracketed_list(|s| s.parse_dotted_name())?;
        Ok(RequiresDecl { loc, capabilities })
    }

    // ── Contract definition ─────────────────────────────────────────────

    fn parse_contract_def(&mut self) -> Result<ContractDef, ParseError> {
        let loc = self.loc();
        self.expect(TokenType::Contract)?;
        let name = self.expect(TokenType::Identifier)?.value.clone();

        self.expect(TokenType::LParen)?;
        let params = self.parse_param_list()?;
        self.expect(TokenType::RParen)?;

        self.expect(TokenType::Arrow)?;
        let return_type = Some(self.parse_type_expr()?);

        self.expect(TokenType::Newline)?;
        self.expect(TokenType::Indent)?;

        let mut precondition = None;
        let mut postcondition = None;
        let mut effects = None;
        let mut permissions = None;
        let mut body = None;
        let mut on_failure = None;

        while !self.check(TokenType::Dedent) && !self.at_end() {
            self.skip_newlines();
            if self.check(TokenType::Dedent) || self.at_end() {
                break;
            }

            if self.check(TokenType::Precondition) {
                precondition = Some(self.parse_precondition()?);
            } else if self.check(TokenType::Postcondition) {
                postcondition = Some(self.parse_postcondition()?);
            } else if self.check(TokenType::Effects) {
                effects = Some(self.parse_effects()?);
            } else if self.check(TokenType::Permissions) {
                permissions = Some(self.parse_permissions()?);
            } else if self.check(TokenType::Body) {
                body = Some(self.parse_body()?);
            } else if self.check(TokenType::OnFailure) {
                on_failure = Some(self.parse_on_failure()?);
            } else {
                let cur = self.current();
                return Err(ParseError {
                    message: format!(
                        "Expected contract section (precondition, postcondition, effects, \
                         permissions, body, on_failure), got {:?}",
                        cur.token_type
                    ),
                    line: cur.line,
                    column: cur.column,
                    file: cur.file.clone(),
                });
            }
            self.skip_newlines();
        }

        self.expect(TokenType::Dedent)?;

        Ok(ContractDef {
            loc,
            name,
            params,
            return_type,
            precondition,
            postcondition,
            effects,
            permissions,
            body,
            on_failure,
        })
    }

    // ── Contract sections ───────────────────────────────────────────────

    fn parse_precondition(&mut self) -> Result<Precondition, ParseError> {
        let loc = self.loc();
        self.expect(TokenType::Precondition)?;
        self.expect(TokenType::Colon)?;
        self.expect(TokenType::Newline)?;
        self.expect(TokenType::Indent)?;
        let conditions = self.parse_expression_list_block()?;
        self.expect(TokenType::Dedent)?;
        Ok(Precondition { loc, conditions })
    }

    fn parse_postcondition(&mut self) -> Result<Postcondition, ParseError> {
        let loc = self.loc();
        self.expect(TokenType::Postcondition)?;
        self.expect(TokenType::Colon)?;
        self.expect(TokenType::Newline)?;
        self.expect(TokenType::Indent)?;
        let conditions = self.parse_expression_list_block()?;
        self.expect(TokenType::Dedent)?;
        Ok(Postcondition { loc, conditions })
    }

    fn parse_effects(&mut self) -> Result<Effects, ParseError> {
        let loc = self.loc();
        self.expect(TokenType::Effects)?;
        self.expect(TokenType::Colon)?;
        self.expect(TokenType::Newline)?;
        self.expect(TokenType::Indent)?;

        let mut declarations = Vec::new();
        while !self.check(TokenType::Dedent) && !self.at_end() {
            self.skip_newlines();
            if self.check(TokenType::Dedent) {
                break;
            }

            if self.check(TokenType::Modifies) {
                let eloc = self.loc();
                self.advance();
                let targets = self.parse_bracketed_list(|s| s.parse_dotted_name())?;
                declarations.push(EffectDecl::Modifies { targets, loc: eloc });
            } else if self.check(TokenType::Reads) {
                let eloc = self.loc();
                self.advance();
                let targets = self.parse_bracketed_list(|s| s.parse_dotted_name())?;
                declarations.push(EffectDecl::Reads { targets, loc: eloc });
            } else if self.check(TokenType::Emits) {
                let eloc = self.loc();
                self.advance();
                let event_name = self.expect(TokenType::Identifier)?.value.clone();
                declarations.push(EffectDecl::Emits {
                    event_type: event_name,
                    loc: eloc,
                });
            } else if self.check(TokenType::TouchesNothingElse) {
                let eloc = self.loc();
                self.advance();
                declarations.push(EffectDecl::TouchesNothingElse { loc: eloc });
            } else {
                let cur = self.current();
                return Err(ParseError {
                    message: format!(
                        "Expected effect declaration (modifies, reads, emits, \
                         touches_nothing_else), got {:?}",
                        cur.token_type
                    ),
                    line: cur.line,
                    column: cur.column,
                    file: cur.file.clone(),
                });
            }
            self.skip_newlines();
        }

        self.expect(TokenType::Dedent)?;
        Ok(Effects { loc, declarations })
    }

    fn parse_permissions(&mut self) -> Result<PermissionsBlock, ParseError> {
        let loc = self.loc();
        self.expect(TokenType::Permissions)?;
        self.expect(TokenType::Colon)?;
        self.expect(TokenType::Newline)?;
        self.expect(TokenType::Indent)?;

        let mut grants = None;
        let mut denies = None;
        let mut escalation = None;

        while !self.check(TokenType::Dedent) && !self.at_end() {
            self.skip_newlines();
            if self.check(TokenType::Dedent) {
                break;
            }

            if self.check(TokenType::Grants) {
                let gloc = self.loc();
                self.advance();
                self.expect(TokenType::Colon)?;
                let perms = self.parse_bracketed_list(|s| s.parse_permission_expr())?;
                grants = Some(GrantsPermission {
                    permissions: perms,
                    loc: gloc,
                });
            } else if self.check(TokenType::Denies) {
                let dloc = self.loc();
                self.advance();
                self.expect(TokenType::Colon)?;
                let perms = self.parse_bracketed_list(|s| s.parse_permission_expr())?;
                denies = Some(DeniesPermission {
                    permissions: perms,
                    loc: dloc,
                });
            } else if self.check(TokenType::Escalation) {
                let eloc = self.loc();
                self.advance();
                self.expect(TokenType::Colon)?;
                let mut policy_parts = Vec::new();
                while !self.check(TokenType::Newline)
                    && !self.check(TokenType::Dedent)
                    && !self.at_end()
                {
                    policy_parts.push(self.current().value.clone());
                    self.advance();
                }
                escalation = Some(EscalationPolicy {
                    policy: policy_parts.join(" "),
                    loc: eloc,
                });
            } else {
                let cur = self.current();
                return Err(ParseError {
                    message: format!(
                        "Expected permission declaration (grants, denies, escalation), got {:?}",
                        cur.token_type
                    ),
                    line: cur.line,
                    column: cur.column,
                    file: cur.file.clone(),
                });
            }
            self.skip_newlines();
        }

        self.expect(TokenType::Dedent)?;
        Ok(PermissionsBlock {
            loc,
            grants,
            denies,
            escalation,
        })
    }

    fn parse_body(&mut self) -> Result<Body, ParseError> {
        let loc = self.loc();
        self.expect(TokenType::Body)?;
        self.expect(TokenType::Colon)?;
        self.expect(TokenType::Newline)?;
        self.expect(TokenType::Indent)?;
        let statements = self.parse_statement_block()?;
        self.expect(TokenType::Dedent)?;
        Ok(Body { loc, statements })
    }

    fn parse_on_failure(&mut self) -> Result<OnFailure, ParseError> {
        let loc = self.loc();
        self.expect(TokenType::OnFailure)?;
        self.expect(TokenType::Colon)?;
        self.expect(TokenType::Newline)?;
        self.expect(TokenType::Indent)?;
        let statements = self.parse_statement_block()?;
        self.expect(TokenType::Dedent)?;
        Ok(OnFailure { loc, statements })
    }

    // ── Statements ──────────────────────────────────────────────────────

    fn parse_statement_block(&mut self) -> Result<Vec<Statement>, ParseError> {
        let mut stmts = Vec::new();
        while !self.check(TokenType::Dedent) && !self.at_end() {
            self.skip_newlines();
            if self.check(TokenType::Dedent) || self.at_end() {
                break;
            }
            stmts.push(self.parse_statement()?);
            self.skip_newlines();
        }
        Ok(stmts)
    }

    fn parse_statement(&mut self) -> Result<Statement, ParseError> {
        let loc = self.loc();

        if self.check(TokenType::Return) {
            return self.parse_return_stmt();
        }
        if self.check(TokenType::Emit) {
            return self.parse_emit_stmt();
        }
        if self.check(TokenType::If) {
            return self.parse_if_stmt();
        }
        if self.check(TokenType::For) {
            return self.parse_for_stmt();
        }
        if self.check(TokenType::While) {
            return self.parse_while_stmt();
        }

        // Parse expression first, then decide if it's an assignment
        let expr = self.parse_expression()?;

        if self.check(TokenType::Assign) {
            self.advance(); // consume =
            let target = Self::expr_to_assignment_target(&expr)?;
            let value = self.parse_expression()?;
            return Ok(Statement::Assignment {
                loc,
                target,
                value,
            });
        }

        Ok(Statement::ExprStmt { loc, expr })
    }

    fn expr_to_assignment_target(expr: &Expr) -> Result<String, ParseError> {
        match expr {
            Expr::Identifier { name, .. } => Ok(name.clone()),
            Expr::FieldAccess { .. } => {
                let mut parts = Vec::new();
                let mut current = expr;
                while let Expr::FieldAccess {
                    object, field_name, ..
                } = current
                {
                    parts.push(field_name.clone());
                    current = object;
                }
                if let Expr::Identifier { name, .. } = current {
                    parts.push(name.clone());
                    parts.reverse();
                    Ok(parts.join("."))
                } else {
                    let loc = expr.loc();
                    Err(ParseError {
                        message: "Invalid assignment target".to_string(),
                        line: loc.line,
                        column: loc.column,
                        file: loc.file.clone(),
                    })
                }
            }
            _ => {
                let loc = expr.loc();
                Err(ParseError {
                    message: "Invalid assignment target".to_string(),
                    line: loc.line,
                    column: loc.column,
                    file: loc.file.clone(),
                })
            }
        }
    }

    fn parse_return_stmt(&mut self) -> Result<Statement, ParseError> {
        let loc = self.loc();
        self.expect(TokenType::Return)?;
        let value = self.parse_expression()?;
        Ok(Statement::Return { loc, value })
    }

    fn parse_emit_stmt(&mut self) -> Result<Statement, ParseError> {
        let loc = self.loc();
        self.expect(TokenType::Emit)?;
        let event = self.parse_expression()?;
        Ok(Statement::Emit { loc, event })
    }

    fn parse_if_stmt(&mut self) -> Result<Statement, ParseError> {
        let loc = self.loc();
        self.expect(TokenType::If)?;
        let condition = self.parse_expression()?;
        self.expect(TokenType::Colon)?;
        self.expect(TokenType::Newline)?;
        self.expect(TokenType::Indent)?;
        let then_body = self.parse_statement_block()?;
        self.expect(TokenType::Dedent)?;

        let mut else_body = Vec::new();
        self.skip_newlines();
        if self.check(TokenType::Else) {
            self.advance();
            self.expect(TokenType::Colon)?;
            self.expect(TokenType::Newline)?;
            self.expect(TokenType::Indent)?;
            else_body = self.parse_statement_block()?;
            self.expect(TokenType::Dedent)?;
        }

        Ok(Statement::If {
            loc,
            condition,
            then_body,
            else_body,
        })
    }

    fn parse_for_stmt(&mut self) -> Result<Statement, ParseError> {
        let loc = self.loc();
        self.expect(TokenType::For)?;
        let var = self.expect(TokenType::Identifier)?.value.clone();
        self.expect(TokenType::In)?;
        let iterable = self.parse_expression()?;
        self.expect(TokenType::Colon)?;
        self.expect(TokenType::Newline)?;
        self.expect(TokenType::Indent)?;
        let body = self.parse_statement_block()?;
        self.expect(TokenType::Dedent)?;
        Ok(Statement::For {
            loc,
            var,
            iterable,
            body,
        })
    }

    fn parse_while_stmt(&mut self) -> Result<Statement, ParseError> {
        let loc = self.loc();
        self.expect(TokenType::While)?;
        let condition = self.parse_expression()?;
        self.expect(TokenType::Colon)?;
        self.expect(TokenType::Newline)?;
        self.expect(TokenType::Indent)?;
        let body = self.parse_statement_block()?;
        self.expect(TokenType::Dedent)?;
        Ok(Statement::While {
            loc,
            condition,
            body,
        })
    }

    // ── Expressions (precedence climbing) ───────────────────────────────

    fn parse_expression(&mut self) -> Result<Expr, ParseError> {
        self.enter_depth()?;
        let result = self.parse_or_expr();
        self.exit_depth();
        result
    }

    fn parse_or_expr(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_and_expr()?;
        while self.check(TokenType::Or) {
            self.advance();
            let right = self.parse_and_expr()?;
            let loc = left.loc().clone();
            left = Expr::BinaryOp {
                loc,
                left: Box::new(left),
                op: "or".to_string(),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_and_expr(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_not_expr()?;
        while self.check(TokenType::And) {
            self.advance();
            let right = self.parse_not_expr()?;
            let loc = left.loc().clone();
            left = Expr::BinaryOp {
                loc,
                left: Box::new(left),
                op: "and".to_string(),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_not_expr(&mut self) -> Result<Expr, ParseError> {
        if self.check(TokenType::Not) {
            let loc = self.loc();
            self.advance();
            let operand = self.parse_not_expr()?;
            return Ok(Expr::UnaryOp {
                loc,
                op: "not".to_string(),
                operand: Box::new(operand),
            });
        }
        self.parse_comparison()
    }

    fn parse_comparison(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_has_expr()?;
        loop {
            let op = match self.current().token_type {
                TokenType::Equals => "==",
                TokenType::NotEquals => "!=",
                TokenType::LessThan => "<",
                TokenType::LessEqual => "<=",
                TokenType::GreaterThan => ">",
                TokenType::GreaterEqual => ">=",
                _ => break,
            };
            self.advance();
            let right = self.parse_has_expr()?;
            let loc = left.loc().clone();
            left = Expr::BinaryOp {
                loc,
                left: Box::new(left),
                op: op.to_string(),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_has_expr(&mut self) -> Result<Expr, ParseError> {
        let left = self.parse_additive()?;
        if self.check(TokenType::Has) {
            self.advance();
            let right = self.parse_additive()?;
            let loc = left.loc().clone();
            return Ok(Expr::HasExpr {
                loc,
                subject: Box::new(left),
                capability: Box::new(right),
            });
        }
        Ok(left)
    }

    fn parse_additive(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_multiplicative()?;
        while self.check(TokenType::Plus) || self.check(TokenType::Minus) {
            let op = if self.current().token_type == TokenType::Plus {
                "+"
            } else {
                "-"
            };
            self.advance();
            let right = self.parse_multiplicative()?;
            let loc = left.loc().clone();
            left = Expr::BinaryOp {
                loc,
                left: Box::new(left),
                op: op.to_string(),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_multiplicative(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_unary()?;
        while self.check(TokenType::Star) || self.check(TokenType::Slash) {
            let op = if self.current().token_type == TokenType::Star {
                "*"
            } else {
                "/"
            };
            self.advance();
            let right = self.parse_unary()?;
            let loc = left.loc().clone();
            left = Expr::BinaryOp {
                loc,
                left: Box::new(left),
                op: op.to_string(),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<Expr, ParseError> {
        if self.check(TokenType::Minus) {
            let loc = self.loc();
            self.advance();
            let operand = self.parse_unary()?;
            return Ok(Expr::UnaryOp {
                loc,
                op: "-".to_string(),
                operand: Box::new(operand),
            });
        }
        self.parse_postfix()
    }

    fn parse_postfix(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_primary()?;

        loop {
            if self.check(TokenType::Dot) {
                self.advance();
                let field_name = self.expect_identifier_or_keyword()?.value.clone();
                if self.check(TokenType::LParen) {
                    // Method call: obj.method(args)
                    self.advance();
                    let (args, kwargs) = self.parse_argument_list()?;
                    self.expect(TokenType::RParen)?;
                    let loc = expr.loc().clone();
                    expr = Expr::MethodCall {
                        loc,
                        object: Box::new(expr),
                        method: field_name,
                        arguments: args,
                        keyword_args: kwargs,
                    };
                } else {
                    let loc = expr.loc().clone();
                    expr = Expr::FieldAccess {
                        loc,
                        object: Box::new(expr),
                        field_name,
                    };
                }
            } else if self.check(TokenType::LParen) {
                // Function call: func(args)
                self.advance();
                let (args, kwargs) = self.parse_argument_list()?;
                self.expect(TokenType::RParen)?;
                let loc = expr.loc().clone();
                expr = Expr::FunctionCall {
                    loc,
                    function: Box::new(expr),
                    arguments: args,
                    keyword_args: kwargs,
                };
            } else {
                break;
            }
        }

        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<Expr, ParseError> {
        let loc = self.loc();
        let tok = self.current().clone();

        match tok.token_type {
            TokenType::Old => {
                self.advance();
                self.expect(TokenType::LParen)?;
                let inner = self.parse_expression()?;
                self.expect(TokenType::RParen)?;
                Ok(Expr::OldExpr {
                    loc,
                    inner: Box::new(inner),
                })
            }
            TokenType::StringLit => {
                self.advance();
                Ok(Expr::StringLiteral {
                    loc,
                    value: tok.value.clone(),
                })
            }
            TokenType::Integer => {
                self.advance();
                let val: i64 = tok.value.parse().map_err(|_| ParseError {
                    message: format!("Invalid integer literal: {}", tok.value),
                    line: tok.line,
                    column: tok.column,
                    file: tok.file.clone(),
                })?;
                Ok(Expr::NumberLiteral {
                    loc,
                    value: val as f64,
                    is_int: true,
                })
            }
            TokenType::Float => {
                self.advance();
                let val: f64 = tok.value.parse().map_err(|_| ParseError {
                    message: format!("Invalid float literal: {}", tok.value),
                    line: tok.line,
                    column: tok.column,
                    file: tok.file.clone(),
                })?;
                Ok(Expr::NumberLiteral {
                    loc,
                    value: val,
                    is_int: false,
                })
            }
            TokenType::True => {
                self.advance();
                Ok(Expr::BoolLiteral { loc, value: true })
            }
            TokenType::False => {
                self.advance();
                Ok(Expr::BoolLiteral { loc, value: false })
            }
            TokenType::Identifier => {
                self.advance();
                Ok(Expr::Identifier {
                    loc,
                    name: tok.value.clone(),
                })
            }
            TokenType::LBracket => self.parse_list_literal(),
            TokenType::LParen => {
                self.advance();
                let expr = self.parse_expression()?;
                self.expect(TokenType::RParen)?;
                Ok(expr)
            }
            _ => Err(ParseError {
                message: format!(
                    "Expected expression, got {:?} ({:?})",
                    tok.token_type, tok.value
                ),
                line: tok.line,
                column: tok.column,
                file: tok.file.clone(),
            }),
        }
    }

    fn parse_list_literal(&mut self) -> Result<Expr, ParseError> {
        let loc = self.loc();
        self.expect(TokenType::LBracket)?;
        let mut elements = Vec::new();
        if !self.check(TokenType::RBracket) {
            elements.push(self.parse_expression()?);
            while self.check(TokenType::Comma) {
                self.advance();
                if self.check(TokenType::RBracket) {
                    break;
                }
                elements.push(self.parse_expression()?);
            }
        }
        self.expect(TokenType::RBracket)?;
        Ok(Expr::ListLiteral { loc, elements })
    }

    fn parse_argument_list(&mut self) -> Result<(Vec<Expr>, Vec<(String, Expr)>), ParseError> {
        let mut args = Vec::new();
        let mut kwargs = Vec::new();

        if !self.check(TokenType::RParen) {
            self.parse_single_argument(&mut args, &mut kwargs)?;
            while self.check(TokenType::Comma) {
                self.advance();
                if self.check(TokenType::RParen) {
                    break;
                }
                self.parse_single_argument(&mut args, &mut kwargs)?;
            }
        }

        Ok((args, kwargs))
    }

    fn parse_single_argument(
        &mut self,
        args: &mut Vec<Expr>,
        kwargs: &mut Vec<(String, Expr)>,
    ) -> Result<(), ParseError> {
        // Check for keyword argument: IDENTIFIER COLON expr
        let cur = self.current();
        let is_keyword_candidate =
            cur.token_type == TokenType::Identifier || cur.token_type.can_be_identifier();
        if is_keyword_candidate && self.peek_type(1) == Some(TokenType::Colon) {
            let name = self.advance().value.clone();
            self.advance(); // consume colon
            let value = self.parse_expression()?;
            kwargs.push((name, value));
        } else {
            args.push(self.parse_expression()?);
        }
        Ok(())
    }

    fn parse_expression_list_block(&mut self) -> Result<Vec<Expr>, ParseError> {
        let mut exprs = Vec::new();
        while !self.check(TokenType::Dedent) && !self.at_end() {
            self.skip_newlines();
            if self.check(TokenType::Dedent) || self.at_end() {
                break;
            }
            exprs.push(self.parse_expression()?);
            self.skip_newlines();
        }
        Ok(exprs)
    }

    // ── Type definitions ────────────────────────────────────────────────

    fn parse_type_def(&mut self) -> Result<TypeDef, ParseError> {
        let loc = self.loc();
        self.expect(TokenType::Type)?;
        let name = self.expect(TokenType::Identifier)?.value.clone();
        self.expect(TokenType::Assign)?;
        let base_type = self.expect(TokenType::Identifier)?.value.clone();
        self.expect(TokenType::Newline)?;
        self.expect(TokenType::Indent)?;

        let mut fields = Vec::new();
        let mut flow_constraints = Vec::new();

        while !self.check(TokenType::Dedent) && !self.at_end() {
            self.skip_newlines();
            if self.check(TokenType::Dedent) {
                break;
            }

            if self.check(TokenType::Fields) {
                self.advance();
                self.expect(TokenType::Colon)?;
                self.expect(TokenType::Newline)?;
                self.expect(TokenType::Indent)?;
                while !self.check(TokenType::Dedent) && !self.at_end() {
                    self.skip_newlines();
                    if self.check(TokenType::Dedent) {
                        break;
                    }
                    fields.push(self.parse_field_def()?);
                    self.skip_newlines();
                }
                self.expect(TokenType::Dedent)?;
            } else if self.check(TokenType::FlowConstraints) {
                self.advance();
                self.expect(TokenType::Colon)?;
                self.expect(TokenType::Newline)?;
                self.expect(TokenType::Indent)?;
                while !self.check(TokenType::Dedent) && !self.at_end() {
                    self.skip_newlines();
                    if self.check(TokenType::Dedent) {
                        break;
                    }
                    flow_constraints.push(self.parse_flow_constraint()?);
                    self.skip_newlines();
                }
                self.expect(TokenType::Dedent)?;
            } else {
                let cur = self.current();
                return Err(ParseError {
                    message: format!(
                        "Expected 'fields' or 'flow_constraints' in type definition, got {:?}",
                        cur.token_type
                    ),
                    line: cur.line,
                    column: cur.column,
                    file: cur.file.clone(),
                });
            }
            self.skip_newlines();
        }

        self.expect(TokenType::Dedent)?;
        Ok(TypeDef {
            loc,
            name,
            base_type,
            fields,
            flow_constraints,
        })
    }

    fn parse_field_def(&mut self) -> Result<FieldDef, ParseError> {
        let loc = self.loc();
        let name = self.expect(TokenType::Identifier)?.value.clone();
        self.expect(TokenType::Colon)?;
        let type_expr = self.parse_type_expr()?;
        Ok(FieldDef {
            loc,
            name,
            type_expr,
        })
    }

    fn parse_flow_constraint(&mut self) -> Result<FlowConstraint, ParseError> {
        let loc = self.loc();
        if self.check(TokenType::NeverFlowsTo) {
            self.advance();
            self.expect(TokenType::Colon)?;
            let destinations = self.parse_bracketed_list(|s| s.parse_identifier_string())?;
            Ok(FlowConstraint::NeverFlowsTo { loc, destinations })
        } else if self.check(TokenType::RequiresContext) {
            self.advance();
            self.expect(TokenType::Colon)?;
            let context = self.expect(TokenType::Identifier)?.value.clone();
            Ok(FlowConstraint::RequiresContext { loc, context })
        } else {
            let cur = self.current();
            Err(ParseError {
                message: format!(
                    "Expected flow constraint (never_flows_to, requires_context), got {:?}",
                    cur.token_type
                ),
                line: cur.line,
                column: cur.column,
                file: cur.file.clone(),
            })
        }
    }

    // ── Shared declarations ─────────────────────────────────────────────

    fn parse_shared_decl(&mut self) -> Result<SharedDecl, ParseError> {
        let loc = self.loc();
        self.expect(TokenType::Shared)?;
        let name = self.expect(TokenType::Identifier)?.value.clone();
        self.expect(TokenType::Colon)?;
        let type_name = self.expect(TokenType::Identifier)?.value.clone();
        self.expect(TokenType::Newline)?;
        self.expect(TokenType::Indent)?;

        let mut access = String::new();
        let mut isolation = String::new();
        let mut audit = String::new();

        while !self.check(TokenType::Dedent) && !self.at_end() {
            self.skip_newlines();
            if self.check(TokenType::Dedent) {
                break;
            }

            if self.check(TokenType::Access) {
                self.advance();
                self.expect(TokenType::Colon)?;
                access = self.expect(TokenType::Identifier)?.value.clone();
            } else if self.check(TokenType::Isolation) {
                self.advance();
                self.expect(TokenType::Colon)?;
                isolation = self.expect(TokenType::Identifier)?.value.clone();
            } else if self.check(TokenType::Audit) {
                self.advance();
                self.expect(TokenType::Colon)?;
                audit = self.expect(TokenType::Identifier)?.value.clone();
            } else {
                let cur = self.current();
                return Err(ParseError {
                    message: format!(
                        "Expected shared declaration property (access, isolation, audit), got {:?}",
                        cur.token_type
                    ),
                    line: cur.line,
                    column: cur.column,
                    file: cur.file.clone(),
                });
            }
            self.skip_newlines();
        }

        self.expect(TokenType::Dedent)?;
        Ok(SharedDecl {
            loc,
            name,
            type_name,
            access,
            isolation,
            audit,
        })
    }

    // ── Type expressions ────────────────────────────────────────────────

    fn parse_type_expr(&mut self) -> Result<TypeExpr, ParseError> {
        let loc = self.loc();
        let name = self.expect(TokenType::Identifier)?.value.clone();
        let base = TypeExpr::Simple { loc: loc.clone(), name };

        if self.check(TokenType::LBracket) {
            self.advance();
            let mut annotations = Vec::new();
            annotations.push(self.expect(TokenType::Identifier)?.value.clone());
            while self.check(TokenType::Comma) {
                self.advance();
                annotations.push(self.expect(TokenType::Identifier)?.value.clone());
            }
            self.expect(TokenType::RBracket)?;
            return Ok(TypeExpr::Annotated {
                loc,
                base: Box::new(base),
                annotations,
            });
        }

        Ok(base)
    }

    // ── Parameters ──────────────────────────────────────────────────────

    fn parse_param_list(&mut self) -> Result<Vec<Param>, ParseError> {
        let mut params = Vec::new();
        if !self.check(TokenType::RParen) {
            params.push(self.parse_param()?);
            while self.check(TokenType::Comma) {
                self.advance();
                if self.check(TokenType::RParen) {
                    break;
                }
                params.push(self.parse_param()?);
            }
        }
        Ok(params)
    }

    fn parse_param(&mut self) -> Result<Param, ParseError> {
        let loc = self.loc();
        let name = self.expect(TokenType::Identifier)?.value.clone();
        self.expect(TokenType::Colon)?;
        let type_expr = self.parse_type_expr()?;
        Ok(Param {
            loc,
            name,
            type_expr,
        })
    }

    // ── Utility parsers ─────────────────────────────────────────────────

    fn parse_dotted_name(&mut self) -> Result<String, ParseError> {
        let mut parts = vec![self.expect_identifier_or_keyword()?.value.clone()];
        while self.check(TokenType::Dot) {
            self.advance();
            parts.push(self.expect_identifier_or_keyword()?.value.clone());
        }
        Ok(parts.join("."))
    }

    fn parse_identifier_string(&mut self) -> Result<String, ParseError> {
        self.parse_dotted_name()
    }

    fn parse_permission_expr(&mut self) -> Result<String, ParseError> {
        let mut parts = Vec::new();
        let mut depth = 0;
        while !self.at_end() {
            let tok = self.current();
            match tok.token_type {
                TokenType::LParen => {
                    depth += 1;
                    parts.push(tok.value.clone());
                    self.advance();
                }
                TokenType::RParen => {
                    if depth == 0 {
                        break;
                    }
                    depth -= 1;
                    parts.push(tok.value.clone());
                    self.advance();
                }
                TokenType::Comma if depth == 0 => break,
                TokenType::RBracket if depth == 0 => break,
                _ => {
                    parts.push(tok.value.clone());
                    self.advance();
                }
            }
        }
        Ok(parts.join(""))
    }

    fn parse_bracketed_list<T, F>(&mut self, mut item_parser: F) -> Result<Vec<T>, ParseError>
    where
        F: FnMut(&mut Self) -> Result<T, ParseError>,
    {
        self.expect(TokenType::LBracket)?;
        let mut items = Vec::new();
        if !self.check(TokenType::RBracket) {
            items.push(item_parser(self)?);
            while self.check(TokenType::Comma) {
                self.advance();
                if self.check(TokenType::RBracket) {
                    break;
                }
                items.push(item_parser(self)?);
            }
        }
        self.expect(TokenType::RBracket)?;
        Ok(items)
    }

    // ── Token stream helpers ────────────────────────────────────────────

    fn current(&self) -> &Token {
        if self.pos >= self.tokens.len() {
            &self.tokens[self.tokens.len() - 1] // EOF
        } else {
            &self.tokens[self.pos]
        }
    }

    fn advance(&mut self) -> &Token {
        let pos = self.pos;
        if self.pos < self.tokens.len() {
            self.pos += 1;
        }
        &self.tokens[pos]
    }

    fn check(&self, token_type: TokenType) -> bool {
        self.current().token_type == token_type
    }

    fn expect(&mut self, token_type: TokenType) -> Result<&Token, ParseError> {
        let tok = self.current();
        if tok.token_type != token_type {
            return Err(ParseError {
                message: format!(
                    "Expected {:?}, got {:?} ({:?})",
                    token_type, tok.token_type, tok.value
                ),
                line: tok.line,
                column: tok.column,
                file: tok.file.clone(),
            });
        }
        Ok(self.advance())
    }

    fn expect_identifier_or_keyword(&mut self) -> Result<&Token, ParseError> {
        let tok = self.current();
        if tok.token_type == TokenType::Identifier || tok.token_type.can_be_identifier() {
            Ok(self.advance())
        } else {
            Err(ParseError {
                message: format!(
                    "Expected identifier, got {:?} ({:?})",
                    tok.token_type, tok.value
                ),
                line: tok.line,
                column: tok.column,
                file: tok.file.clone(),
            })
        }
    }

    fn peek_type(&self, offset: usize) -> Option<TokenType> {
        let idx = self.pos + offset;
        if idx >= self.tokens.len() {
            None
        } else {
            Some(self.tokens[idx].token_type)
        }
    }

    fn at_end(&self) -> bool {
        self.current().token_type == TokenType::Eof
    }

    fn skip_newlines(&mut self) {
        while self.check(TokenType::Newline) {
            self.advance();
        }
    }

    fn loc(&self) -> SourceLocation {
        let tok = self.current();
        SourceLocation::new(&tok.file, tok.line, tok.column)
    }
}
