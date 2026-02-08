"""Covenant recursive descent parser.

Transforms a flat token stream from the lexer into an immutable AST.
Hand-written for clear diagnostics — parser generators produce opaque
error messages that are unhelpful for AI agents trying to fix their code.

Grammar reference (simplified EBNF):

    program        ::= file_header? (contract_def | type_def | shared_decl)*
    file_header    ::= intent_block scope_decl? risk_decl? requires_decl?
    contract_def   ::= 'contract' NAME '(' params ')' '->' type_expr NEWLINE
                       INDENT sections DEDENT
    sections       ::= precondition? postcondition? effects? permissions? body on_failure?
"""

from __future__ import annotations

from covenant.ast.nodes import (
    ASTNode,
    AnnotatedType,
    Assignment,
    BinaryOp,
    Body,
    BoolLiteral,
    ContractDef,
    Effects,
    EmitsEffect,
    EmitStmt,
    Expr,
    ExprStmt,
    FieldAccess,
    FieldDef,
    FileHeader,
    FlowConstraint,
    ForStmt,
    FunctionCall,
    GenericType,
    GrantsPermission,
    DeniesPermission,
    EscalationPolicy,
    HasExpr,
    Identifier,
    IfStmt,
    IntentBlock,
    ListLiteral,
    ListType,
    MethodCall,
    ModifiesEffect,
    NeverFlowsTo,
    NumberLiteral,
    OldExpr,
    OnFailure,
    Param,
    PermissionsBlock,
    Postcondition,
    Precondition,
    Program,
    ReadsEffect,
    RequiresContext,
    RequiresDecl,
    ReturnStmt,
    RiskDecl,
    RiskLevel,
    ScopeDecl,
    SharedDecl,
    SimpleType,
    SourceLocation,
    Statement,
    StringLiteral,
    TouchesNothingElse,
    TypeDef,
    TypeExpr,
    UnaryOp,
    WhileStmt,
)
from covenant.lexer.tokens import Token, TokenType


# Keyword token types that can appear in identifier positions (dotted names,
# field access, etc.).  This is necessary because words like "access",
# "audit", "grants" are keywords but also valid as parts of qualified names.
_KEYWORD_TOKEN_TYPES: frozenset[TokenType] = frozenset({
    TokenType.ACCESS, TokenType.AUDIT, TokenType.GRANTS, TokenType.DENIES,
    TokenType.ESCALATION, TokenType.ISOLATION, TokenType.SCOPE,
    TokenType.RISK, TokenType.LOW, TokenType.MEDIUM, TokenType.HIGH,
    TokenType.CRITICAL, TokenType.FIELDS, TokenType.SHOW, TokenType.ALL,
    TokenType.WHERE, TokenType.SINCE, TokenType.READS, TokenType.EMITS,
    TokenType.MODIFIES, TokenType.SHARED, TokenType.TYPE, TokenType.REQUIRES,
    TokenType.INTENT, TokenType.OLD, TokenType.BODY, TokenType.EFFECTS,
    TokenType.PRECONDITION, TokenType.POSTCONDITION, TokenType.PERMISSIONS,
})


class ParseError(Exception):
    """Raised on parse errors with human-readable diagnostics."""

    def __init__(self, message: str, token: Token):
        self.token = token
        loc = f"{token.file}:{token.line}:{token.column}"
        super().__init__(f"{loc}: {message}")


class Parser:
    """Recursive descent parser for Covenant source code.

    Usage::

        from covenant.lexer import Lexer
        from covenant.parser import Parser

        tokens = Lexer(source, "example.cov").tokenize()
        ast = Parser(tokens, "example.cov").parse()
    """

    def __init__(self, tokens: list[Token], filename: str = "<unknown>") -> None:
        self.tokens = tokens
        self.filename = filename
        self.pos = 0

    # ------------------------------------------------------------------
    # Public API
    # ------------------------------------------------------------------

    def parse(self) -> Program:
        """Parse the entire token stream into a Program AST."""
        header = self._parse_file_header()
        contracts: list[ContractDef] = []
        type_defs: list[TypeDef] = []
        shared_decls: list[SharedDecl] = []

        while not self._at_end():
            self._skip_newlines()
            if self._at_end():
                break

            if self._check(TokenType.CONTRACT):
                contracts.append(self._parse_contract_def())
            elif self._check(TokenType.TYPE):
                type_defs.append(self._parse_type_def())
            elif self._check(TokenType.SHARED):
                shared_decls.append(self._parse_shared_decl())
            elif self._check(TokenType.EOF):
                break
            else:
                raise ParseError(
                    f"Expected 'contract', 'type', or 'shared' at top level, "
                    f"got {self._current().type.name}",
                    self._current(),
                )

        return Program(
            loc=self._loc(),
            header=header,
            contracts=contracts,
            type_defs=type_defs,
            shared_decls=shared_decls,
        )

    # ------------------------------------------------------------------
    # File header
    # ------------------------------------------------------------------

    def _parse_file_header(self) -> FileHeader | None:
        """Parse optional file header (intent, scope, risk, requires)."""
        self._skip_newlines()

        intent = None
        scope = None
        risk = None
        requires = None

        if self._check(TokenType.INTENT):
            intent = self._parse_intent_block()
            self._skip_newlines()

        if self._check(TokenType.SCOPE):
            scope = self._parse_scope_decl()
            self._skip_newlines()

        if self._check(TokenType.RISK):
            risk = self._parse_risk_decl()
            self._skip_newlines()

        if self._check(TokenType.REQUIRES):
            requires = self._parse_requires_decl()
            self._skip_newlines()

        if intent is None and scope is None and risk is None and requires is None:
            return None

        return FileHeader(
            loc=self._loc(),
            intent=intent,
            scope=scope,
            risk=risk,
            requires=requires,
        )

    def _parse_intent_block(self) -> IntentBlock:
        loc = self._loc()
        self._expect(TokenType.INTENT)
        self._expect(TokenType.COLON)
        text_token = self._expect(TokenType.STRING)
        return IntentBlock(loc=loc, text=text_token.value)

    def _parse_scope_decl(self) -> ScopeDecl:
        loc = self._loc()
        self._expect(TokenType.SCOPE)
        self._expect(TokenType.COLON)
        path = self._parse_dotted_name()
        return ScopeDecl(loc=loc, path=path)

    def _parse_risk_decl(self) -> RiskDecl:
        loc = self._loc()
        self._expect(TokenType.RISK)
        self._expect(TokenType.COLON)
        level_map = {
            TokenType.LOW: RiskLevel.LOW,
            TokenType.MEDIUM: RiskLevel.MEDIUM,
            TokenType.HIGH: RiskLevel.HIGH,
            TokenType.CRITICAL: RiskLevel.CRITICAL,
        }
        token = self._current()
        if token.type not in level_map:
            raise ParseError(
                f"Expected risk level (low, medium, high, critical), got {token.value!r}",
                token,
            )
        self._advance()
        return RiskDecl(loc=loc, level=level_map[token.type])

    def _parse_requires_decl(self) -> RequiresDecl:
        loc = self._loc()
        self._expect(TokenType.REQUIRES)
        self._expect(TokenType.COLON)
        capabilities = self._parse_bracketed_list(self._parse_dotted_name)
        return RequiresDecl(loc=loc, capabilities=capabilities)

    # ------------------------------------------------------------------
    # Contract definition
    # ------------------------------------------------------------------

    def _parse_contract_def(self) -> ContractDef:
        loc = self._loc()
        self._expect(TokenType.CONTRACT)
        name_token = self._expect(TokenType.IDENTIFIER)

        self._expect(TokenType.LPAREN)
        params = self._parse_param_list()
        self._expect(TokenType.RPAREN)

        self._expect(TokenType.ARROW)
        return_type = self._parse_type_expr()

        self._expect(TokenType.NEWLINE)
        self._expect(TokenType.INDENT)

        precondition = None
        postcondition = None
        effects = None
        permissions = None
        body = None
        on_failure = None

        while not self._check(TokenType.DEDENT) and not self._at_end():
            self._skip_newlines()
            if self._check(TokenType.DEDENT) or self._at_end():
                break

            if self._check(TokenType.PRECONDITION):
                precondition = self._parse_precondition()
            elif self._check(TokenType.POSTCONDITION):
                postcondition = self._parse_postcondition()
            elif self._check(TokenType.EFFECTS):
                effects = self._parse_effects()
            elif self._check(TokenType.PERMISSIONS):
                permissions = self._parse_permissions()
            elif self._check(TokenType.BODY):
                body = self._parse_body()
            elif self._check(TokenType.ON_FAILURE):
                on_failure = self._parse_on_failure()
            else:
                raise ParseError(
                    f"Expected contract section (precondition, postcondition, effects, "
                    f"permissions, body, on_failure), got {self._current().type.name}",
                    self._current(),
                )
            self._skip_newlines()

        self._expect(TokenType.DEDENT)

        return ContractDef(
            loc=loc,
            name=name_token.value,
            params=params,
            return_type=return_type,
            precondition=precondition,
            postcondition=postcondition,
            effects=effects,
            permissions=permissions,
            body=body,
            on_failure=on_failure,
        )

    # ------------------------------------------------------------------
    # Contract sections
    # ------------------------------------------------------------------

    def _parse_precondition(self) -> Precondition:
        loc = self._loc()
        self._expect(TokenType.PRECONDITION)
        self._expect(TokenType.COLON)
        self._expect(TokenType.NEWLINE)
        self._expect(TokenType.INDENT)
        conditions = self._parse_expression_list_block()
        self._expect(TokenType.DEDENT)
        return Precondition(loc=loc, conditions=conditions)

    def _parse_postcondition(self) -> Postcondition:
        loc = self._loc()
        self._expect(TokenType.POSTCONDITION)
        self._expect(TokenType.COLON)
        self._expect(TokenType.NEWLINE)
        self._expect(TokenType.INDENT)
        conditions = self._parse_expression_list_block()
        self._expect(TokenType.DEDENT)
        return Postcondition(loc=loc, conditions=conditions)

    def _parse_effects(self) -> Effects:
        loc = self._loc()
        self._expect(TokenType.EFFECTS)
        self._expect(TokenType.COLON)
        self._expect(TokenType.NEWLINE)
        self._expect(TokenType.INDENT)

        declarations = []
        while not self._check(TokenType.DEDENT) and not self._at_end():
            self._skip_newlines()
            if self._check(TokenType.DEDENT):
                break

            if self._check(TokenType.MODIFIES):
                self._advance()
                targets = self._parse_bracketed_list(self._parse_dotted_name)
                declarations.append(ModifiesEffect(loc=self._loc(), targets=targets))
            elif self._check(TokenType.READS):
                self._advance()
                targets = self._parse_bracketed_list(self._parse_dotted_name)
                declarations.append(ReadsEffect(loc=self._loc(), targets=targets))
            elif self._check(TokenType.EMITS):
                self._advance()
                event_name = self._expect(TokenType.IDENTIFIER)
                declarations.append(EmitsEffect(loc=self._loc(), event_type=event_name.value))
            elif self._check(TokenType.TOUCHES_NOTHING_ELSE):
                self._advance()
                declarations.append(TouchesNothingElse(loc=self._loc()))
            else:
                raise ParseError(
                    f"Expected effect declaration (modifies, reads, emits, "
                    f"touches_nothing_else), got {self._current().type.name}",
                    self._current(),
                )
            self._skip_newlines()

        self._expect(TokenType.DEDENT)
        return Effects(loc=loc, declarations=declarations)

    def _parse_permissions(self) -> PermissionsBlock:
        loc = self._loc()
        self._expect(TokenType.PERMISSIONS)
        self._expect(TokenType.COLON)
        self._expect(TokenType.NEWLINE)
        self._expect(TokenType.INDENT)

        grants = None
        denies = None
        escalation = None

        while not self._check(TokenType.DEDENT) and not self._at_end():
            self._skip_newlines()
            if self._check(TokenType.DEDENT):
                break

            if self._check(TokenType.GRANTS):
                self._advance()
                self._expect(TokenType.COLON)
                perms = self._parse_bracketed_list(self._parse_permission_expr)
                grants = GrantsPermission(loc=self._loc(), permissions=perms)
            elif self._check(TokenType.DENIES):
                self._advance()
                self._expect(TokenType.COLON)
                perms = self._parse_bracketed_list(self._parse_permission_expr)
                denies = DeniesPermission(loc=self._loc(), permissions=perms)
            elif self._check(TokenType.ESCALATION):
                self._advance()
                self._expect(TokenType.COLON)
                policy_parts = []
                while not self._check(TokenType.NEWLINE) and not self._check(TokenType.DEDENT) and not self._at_end():
                    policy_parts.append(self._current().value)
                    self._advance()
                escalation = EscalationPolicy(loc=self._loc(), policy=" ".join(policy_parts))
            else:
                raise ParseError(
                    f"Expected permission declaration (grants, denies, escalation), "
                    f"got {self._current().type.name}",
                    self._current(),
                )
            self._skip_newlines()

        self._expect(TokenType.DEDENT)
        return PermissionsBlock(loc=loc, grants=grants, denies=denies, escalation=escalation)

    def _parse_body(self) -> Body:
        loc = self._loc()
        self._expect(TokenType.BODY)
        self._expect(TokenType.COLON)
        self._expect(TokenType.NEWLINE)
        self._expect(TokenType.INDENT)
        stmts = self._parse_statement_block()
        self._expect(TokenType.DEDENT)
        return Body(loc=loc, statements=stmts)

    def _parse_on_failure(self) -> OnFailure:
        loc = self._loc()
        self._expect(TokenType.ON_FAILURE)
        self._expect(TokenType.COLON)
        self._expect(TokenType.NEWLINE)
        self._expect(TokenType.INDENT)
        stmts = self._parse_statement_block()
        self._expect(TokenType.DEDENT)
        return OnFailure(loc=loc, statements=stmts)

    # ------------------------------------------------------------------
    # Statements
    # ------------------------------------------------------------------

    def _parse_statement_block(self) -> list[Statement]:
        """Parse statements until DEDENT."""
        stmts = []
        while not self._check(TokenType.DEDENT) and not self._at_end():
            self._skip_newlines()
            if self._check(TokenType.DEDENT) or self._at_end():
                break
            stmts.append(self._parse_statement())
            self._skip_newlines()
        return stmts

    def _parse_statement(self) -> Statement:
        """Parse a single statement."""
        loc = self._loc()

        if self._check(TokenType.RETURN):
            return self._parse_return_stmt()

        if self._check(TokenType.EMIT):
            return self._parse_emit_stmt()

        if self._check(TokenType.IF):
            return self._parse_if_stmt()

        if self._check(TokenType.FOR):
            return self._parse_for_stmt()

        if self._check(TokenType.WHILE):
            return self._parse_while_stmt()

        # Parse expression first, then decide if it's an assignment
        expr = self._parse_expression()

        # If followed by =, this is an assignment (x = ..., obj.field = ...)
        if self._check(TokenType.ASSIGN):
            self._advance()  # consume =
            target = self._expr_to_assignment_target(expr)
            value = self._parse_expression()
            return Assignment(loc=loc, target=target, value=value)

        return ExprStmt(loc=loc, expr=expr)

    @staticmethod
    def _expr_to_assignment_target(expr: Expr) -> str:
        """Convert a parsed expression to an assignment target string.

        Supports simple identifiers (x) and dotted field access (obj.field).
        """
        if isinstance(expr, Identifier):
            return expr.name
        if isinstance(expr, FieldAccess):
            parts = []
            current = expr
            while isinstance(current, FieldAccess):
                parts.append(current.field_name)
                current = current.object
            if isinstance(current, Identifier):
                parts.append(current.name)
                return ".".join(reversed(parts))
        raise ParseError(
            f"Invalid assignment target",
            Token(TokenType.ASSIGN, "=", expr.loc.line, expr.loc.column, expr.loc.file),
        )

    def _parse_return_stmt(self) -> ReturnStmt:
        loc = self._loc()
        self._expect(TokenType.RETURN)
        value = self._parse_expression()
        return ReturnStmt(loc=loc, value=value)

    def _parse_emit_stmt(self) -> EmitStmt:
        loc = self._loc()
        self._expect(TokenType.EMIT)
        event = self._parse_expression()
        return EmitStmt(loc=loc, event=event)

    def _parse_if_stmt(self) -> IfStmt:
        loc = self._loc()
        self._expect(TokenType.IF)
        condition = self._parse_expression()
        self._expect(TokenType.COLON)
        self._expect(TokenType.NEWLINE)
        self._expect(TokenType.INDENT)
        then_body = self._parse_statement_block()
        self._expect(TokenType.DEDENT)

        else_body: list[Statement] = []
        self._skip_newlines()
        if self._check(TokenType.ELSE):
            self._advance()
            self._expect(TokenType.COLON)
            self._expect(TokenType.NEWLINE)
            self._expect(TokenType.INDENT)
            else_body = self._parse_statement_block()
            self._expect(TokenType.DEDENT)

        return IfStmt(loc=loc, condition=condition, then_body=then_body, else_body=else_body)

    def _parse_for_stmt(self) -> ForStmt:
        loc = self._loc()
        self._expect(TokenType.FOR)
        var = self._expect(TokenType.IDENTIFIER).value
        self._expect(TokenType.IN)
        iterable = self._parse_expression()
        self._expect(TokenType.COLON)
        self._expect(TokenType.NEWLINE)
        self._expect(TokenType.INDENT)
        loop_body = self._parse_statement_block()
        self._expect(TokenType.DEDENT)
        return ForStmt(loc=loc, var=var, iterable=iterable, loop_body=loop_body)

    def _parse_while_stmt(self) -> WhileStmt:
        loc = self._loc()
        self._expect(TokenType.WHILE)
        condition = self._parse_expression()
        self._expect(TokenType.COLON)
        self._expect(TokenType.NEWLINE)
        self._expect(TokenType.INDENT)
        loop_body = self._parse_statement_block()
        self._expect(TokenType.DEDENT)
        return WhileStmt(loc=loc, condition=condition, loop_body=loop_body)

    # ------------------------------------------------------------------
    # Expressions (precedence climbing)
    # ------------------------------------------------------------------

    def _parse_expression(self) -> Expr:
        """Parse an expression with operator precedence."""
        return self._parse_or_expr()

    def _parse_or_expr(self) -> Expr:
        left = self._parse_and_expr()
        while self._check(TokenType.OR):
            self._advance()
            right = self._parse_and_expr()
            left = BinaryOp(loc=left.loc, left=left, op="or", right=right)
        return left

    def _parse_and_expr(self) -> Expr:
        left = self._parse_not_expr()
        while self._check(TokenType.AND):
            self._advance()
            right = self._parse_not_expr()
            left = BinaryOp(loc=left.loc, left=left, op="and", right=right)
        return left

    def _parse_not_expr(self) -> Expr:
        if self._check(TokenType.NOT):
            loc = self._loc()
            self._advance()
            operand = self._parse_not_expr()
            return UnaryOp(loc=loc, op="not", operand=operand)
        return self._parse_comparison()

    def _parse_comparison(self) -> Expr:
        left = self._parse_has_expr()
        comparison_ops = {
            TokenType.EQUALS: "==",
            TokenType.NOT_EQUALS: "!=",
            TokenType.LESS_THAN: "<",
            TokenType.LESS_EQUAL: "<=",
            TokenType.GREATER_THAN: ">",
            TokenType.GREATER_EQUAL: ">=",
        }
        while self._current().type in comparison_ops:
            op = comparison_ops[self._current().type]
            self._advance()
            right = self._parse_has_expr()
            left = BinaryOp(loc=left.loc, left=left, op=op, right=right)
        return left

    def _parse_has_expr(self) -> Expr:
        left = self._parse_additive()
        if self._check(TokenType.HAS):
            self._advance()
            right = self._parse_additive()
            return HasExpr(loc=left.loc, subject=left, capability=right)
        return left

    def _parse_additive(self) -> Expr:
        left = self._parse_multiplicative()
        while self._check(TokenType.PLUS) or self._check(TokenType.MINUS):
            op = "+" if self._current().type == TokenType.PLUS else "-"
            self._advance()
            right = self._parse_multiplicative()
            left = BinaryOp(loc=left.loc, left=left, op=op, right=right)
        return left

    def _parse_multiplicative(self) -> Expr:
        left = self._parse_unary()
        while self._check(TokenType.STAR) or self._check(TokenType.SLASH):
            op = "*" if self._current().type == TokenType.STAR else "/"
            self._advance()
            right = self._parse_unary()
            left = BinaryOp(loc=left.loc, left=left, op=op, right=right)
        return left

    def _parse_unary(self) -> Expr:
        if self._check(TokenType.MINUS):
            loc = self._loc()
            self._advance()
            operand = self._parse_unary()
            return UnaryOp(loc=loc, op="-", operand=operand)
        return self._parse_postfix()

    def _parse_postfix(self) -> Expr:
        """Parse postfix operations: field access and function/method calls."""
        expr = self._parse_primary()

        while True:
            if self._check(TokenType.DOT):
                self._advance()
                field_name = self._expect_identifier_or_keyword().value
                if self._check(TokenType.LPAREN):
                    # Method call: obj.method(args)
                    self._advance()
                    args, kwargs = self._parse_argument_list()
                    self._expect(TokenType.RPAREN)
                    expr = MethodCall(
                        loc=expr.loc, object=expr, method=field_name,
                        arguments=args, keyword_args=kwargs,
                    )
                else:
                    expr = FieldAccess(loc=expr.loc, object=expr, field_name=field_name)
            elif self._check(TokenType.LPAREN):
                # Function call: func(args)
                self._advance()
                args, kwargs = self._parse_argument_list()
                self._expect(TokenType.RPAREN)
                expr = FunctionCall(
                    loc=expr.loc, function=expr, arguments=args, keyword_args=kwargs,
                )
            else:
                break

        return expr

    def _parse_primary(self) -> Expr:
        """Parse a primary expression (literals, identifiers, grouping)."""
        loc = self._loc()
        tok = self._current()

        if tok.type == TokenType.OLD:
            self._advance()
            self._expect(TokenType.LPAREN)
            inner = self._parse_expression()
            self._expect(TokenType.RPAREN)
            return OldExpr(loc=loc, inner=inner)

        if tok.type == TokenType.STRING:
            self._advance()
            return StringLiteral(loc=loc, value=tok.value)

        if tok.type == TokenType.INTEGER:
            self._advance()
            return NumberLiteral(loc=loc, value=int(tok.value))

        if tok.type == TokenType.FLOAT:
            self._advance()
            return NumberLiteral(loc=loc, value=float(tok.value))

        if tok.type == TokenType.TRUE:
            self._advance()
            return BoolLiteral(loc=loc, value=True)

        if tok.type == TokenType.FALSE:
            self._advance()
            return BoolLiteral(loc=loc, value=False)

        if tok.type == TokenType.IDENTIFIER:
            self._advance()
            return Identifier(loc=loc, name=tok.value)

        if tok.type == TokenType.LBRACKET:
            return self._parse_list_literal()

        if tok.type == TokenType.LPAREN:
            self._advance()
            expr = self._parse_expression()
            self._expect(TokenType.RPAREN)
            return expr

        raise ParseError(
            f"Expected expression, got {tok.type.name} ({tok.value!r})",
            tok,
        )

    def _parse_list_literal(self) -> ListLiteral:
        loc = self._loc()
        self._expect(TokenType.LBRACKET)
        elements = []
        if not self._check(TokenType.RBRACKET):
            elements.append(self._parse_expression())
            while self._check(TokenType.COMMA):
                self._advance()
                if self._check(TokenType.RBRACKET):
                    break
                elements.append(self._parse_expression())
        self._expect(TokenType.RBRACKET)
        return ListLiteral(loc=loc, elements=elements)

    def _parse_argument_list(self) -> tuple[list[Expr], dict[str, Expr]]:
        """Parse comma-separated arguments inside parens.

        Supports both positional and keyword arguments:
            func(1, 2, name: value)
        """
        args: list[Expr] = []
        kwargs: dict[str, Expr] = {}

        if not self._check(TokenType.RPAREN):
            self._parse_single_argument(args, kwargs)
            while self._check(TokenType.COMMA):
                self._advance()
                if self._check(TokenType.RPAREN):
                    break
                self._parse_single_argument(args, kwargs)

        return args, kwargs

    def _parse_single_argument(
        self, args: list[Expr], kwargs: dict[str, Expr]
    ) -> None:
        """Parse one argument, detecting keyword form (name: value)."""
        # Check for keyword argument: IDENTIFIER COLON expr
        if (
            (self._check(TokenType.IDENTIFIER) or self._current().type in _KEYWORD_TOKEN_TYPES)
            and self._peek_type(1) == TokenType.COLON
        ):
            name = self._advance().value
            self._advance()  # consume colon
            value = self._parse_expression()
            kwargs[name] = value
        else:
            args.append(self._parse_expression())

    def _parse_expression_list_block(self) -> list[Expr]:
        """Parse a block of expressions (one per line) until DEDENT."""
        exprs = []
        while not self._check(TokenType.DEDENT) and not self._at_end():
            self._skip_newlines()
            if self._check(TokenType.DEDENT) or self._at_end():
                break
            exprs.append(self._parse_expression())
            self._skip_newlines()
        return exprs

    # ------------------------------------------------------------------
    # Type definitions
    # ------------------------------------------------------------------

    def _parse_type_def(self) -> TypeDef:
        loc = self._loc()
        self._expect(TokenType.TYPE)
        name = self._expect(TokenType.IDENTIFIER).value
        self._expect(TokenType.ASSIGN)
        base_type = self._expect(TokenType.IDENTIFIER).value
        self._expect(TokenType.NEWLINE)
        self._expect(TokenType.INDENT)

        fields: list[FieldDef] = []
        flow_constraints: list[FlowConstraint] = []

        while not self._check(TokenType.DEDENT) and not self._at_end():
            self._skip_newlines()
            if self._check(TokenType.DEDENT):
                break

            if self._check(TokenType.FIELDS):
                self._advance()
                self._expect(TokenType.COLON)
                self._expect(TokenType.NEWLINE)
                self._expect(TokenType.INDENT)
                while not self._check(TokenType.DEDENT) and not self._at_end():
                    self._skip_newlines()
                    if self._check(TokenType.DEDENT):
                        break
                    fields.append(self._parse_field_def())
                    self._skip_newlines()
                self._expect(TokenType.DEDENT)
            elif self._check(TokenType.FLOW_CONSTRAINTS):
                self._advance()
                self._expect(TokenType.COLON)
                self._expect(TokenType.NEWLINE)
                self._expect(TokenType.INDENT)
                while not self._check(TokenType.DEDENT) and not self._at_end():
                    self._skip_newlines()
                    if self._check(TokenType.DEDENT):
                        break
                    flow_constraints.append(self._parse_flow_constraint())
                    self._skip_newlines()
                self._expect(TokenType.DEDENT)
            else:
                raise ParseError(
                    f"Expected 'fields' or 'flow_constraints' in type definition, "
                    f"got {self._current().type.name}",
                    self._current(),
                )
            self._skip_newlines()

        self._expect(TokenType.DEDENT)
        return TypeDef(
            loc=loc, name=name, base_type=base_type,
            fields=fields, flow_constraints=flow_constraints,
        )

    def _parse_field_def(self) -> FieldDef:
        loc = self._loc()
        name = self._expect(TokenType.IDENTIFIER).value
        self._expect(TokenType.COLON)
        type_expr = self._parse_type_expr()
        return FieldDef(loc=loc, name=name, type_expr=type_expr)

    def _parse_flow_constraint(self) -> FlowConstraint:
        loc = self._loc()
        if self._check(TokenType.NEVER_FLOWS_TO):
            self._advance()
            self._expect(TokenType.COLON)
            destinations = self._parse_bracketed_list(self._parse_identifier_string)
            return NeverFlowsTo(loc=loc, destinations=destinations)
        elif self._check(TokenType.REQUIRES_CONTEXT):
            self._advance()
            self._expect(TokenType.COLON)
            context = self._expect(TokenType.IDENTIFIER).value
            return RequiresContext(loc=loc, context=context)
        else:
            raise ParseError(
                f"Expected flow constraint (never_flows_to, requires_context), "
                f"got {self._current().type.name}",
                self._current(),
            )

    # ------------------------------------------------------------------
    # Shared declarations
    # ------------------------------------------------------------------

    def _parse_shared_decl(self) -> SharedDecl:
        loc = self._loc()
        self._expect(TokenType.SHARED)
        name = self._expect(TokenType.IDENTIFIER).value
        self._expect(TokenType.COLON)
        type_name = self._expect(TokenType.IDENTIFIER).value
        self._expect(TokenType.NEWLINE)
        self._expect(TokenType.INDENT)

        access = ""
        isolation = ""
        audit = ""

        while not self._check(TokenType.DEDENT) and not self._at_end():
            self._skip_newlines()
            if self._check(TokenType.DEDENT):
                break

            if self._check(TokenType.ACCESS):
                self._advance()
                self._expect(TokenType.COLON)
                access = self._expect(TokenType.IDENTIFIER).value
            elif self._check(TokenType.ISOLATION):
                self._advance()
                self._expect(TokenType.COLON)
                isolation = self._expect(TokenType.IDENTIFIER).value
            elif self._check(TokenType.AUDIT):
                self._advance()
                self._expect(TokenType.COLON)
                audit = self._expect(TokenType.IDENTIFIER).value
            else:
                raise ParseError(
                    f"Expected shared declaration property (access, isolation, audit), "
                    f"got {self._current().type.name}",
                    self._current(),
                )
            self._skip_newlines()

        self._expect(TokenType.DEDENT)
        return SharedDecl(
            loc=loc, name=name, type_name=type_name,
            access=access, isolation=isolation, audit=audit,
        )

    # ------------------------------------------------------------------
    # Type expressions
    # ------------------------------------------------------------------

    def _parse_type_expr(self) -> TypeExpr:
        """Parse a type expression, possibly with annotations."""
        loc = self._loc()
        name = self._expect(TokenType.IDENTIFIER).value
        base: TypeExpr = SimpleType(loc=loc, name=name)

        # Check for annotations: Type [ann1, ann2]
        if self._check(TokenType.LBRACKET):
            self._advance()
            annotations = []
            annotations.append(self._expect(TokenType.IDENTIFIER).value)
            while self._check(TokenType.COMMA):
                self._advance()
                annotations.append(self._expect(TokenType.IDENTIFIER).value)
            self._expect(TokenType.RBRACKET)
            return AnnotatedType(loc=loc, base=base, annotations=annotations)

        return base

    # ------------------------------------------------------------------
    # Parameters
    # ------------------------------------------------------------------

    def _parse_param_list(self) -> list[Param]:
        """Parse comma-separated parameter declarations."""
        params = []
        if not self._check(TokenType.RPAREN):
            params.append(self._parse_param())
            while self._check(TokenType.COMMA):
                self._advance()
                if self._check(TokenType.RPAREN):
                    break
                params.append(self._parse_param())
        return params

    def _parse_param(self) -> Param:
        loc = self._loc()
        name = self._expect(TokenType.IDENTIFIER).value
        self._expect(TokenType.COLON)
        type_expr = self._parse_type_expr()
        return Param(loc=loc, name=name, type_expr=type_expr)

    # ------------------------------------------------------------------
    # Utility parsers
    # ------------------------------------------------------------------

    def _parse_dotted_name(self) -> str:
        """Parse a dotted identifier path like 'finance.transfers'.

        Allows keywords in identifier positions since names like
        'ledger.access' or 'auth.grants' use words that are also keywords.
        """
        parts = [self._expect_identifier_or_keyword().value]
        while self._check(TokenType.DOT):
            self._advance()
            parts.append(self._expect_identifier_or_keyword().value)
        return ".".join(parts)

    def _parse_identifier_string(self) -> str:
        """Parse a single identifier or dotted name as a string."""
        return self._parse_dotted_name()

    def _parse_permission_expr(self) -> str:
        """Parse a permission expression like 'read(record.name)' or 'network_access'."""
        parts = []
        # Consume tokens until comma or bracket, handling nested parens
        depth = 0
        while not self._at_end():
            tok = self._current()
            if tok.type == TokenType.LPAREN:
                depth += 1
                parts.append(tok.value)
                self._advance()
            elif tok.type == TokenType.RPAREN:
                if depth == 0:
                    break
                depth -= 1
                parts.append(tok.value)
                self._advance()
            elif tok.type == TokenType.COMMA and depth == 0:
                break
            elif tok.type == TokenType.RBRACKET and depth == 0:
                break
            else:
                parts.append(tok.value)
                self._advance()
        return "".join(parts)

    def _parse_bracketed_list(self, item_parser) -> list:
        """Parse [item, item, ...] using the given item parser."""
        self._expect(TokenType.LBRACKET)
        items = []
        if not self._check(TokenType.RBRACKET):
            items.append(item_parser())
            while self._check(TokenType.COMMA):
                self._advance()
                if self._check(TokenType.RBRACKET):
                    break
                items.append(item_parser())
        self._expect(TokenType.RBRACKET)
        return items

    # ------------------------------------------------------------------
    # Token stream helpers
    # ------------------------------------------------------------------

    def _current(self) -> Token:
        """Return the current token."""
        if self.pos >= len(self.tokens):
            return self.tokens[-1]  # EOF
        return self.tokens[self.pos]

    def _advance(self) -> Token:
        """Consume and return the current token."""
        tok = self._current()
        self.pos += 1
        return tok

    def _check(self, token_type: TokenType) -> bool:
        """Check if current token matches without consuming."""
        return self._current().type == token_type

    def _expect(self, token_type: TokenType) -> Token:
        """Consume current token if it matches, otherwise raise ParseError."""
        tok = self._current()
        if tok.type != token_type:
            raise ParseError(
                f"Expected {token_type.name}, got {tok.type.name} ({tok.value!r})",
                tok,
            )
        return self._advance()

    def _expect_identifier_or_keyword(self) -> Token:
        """Consume an IDENTIFIER or a keyword token used in identifier position.

        Many Covenant keywords (access, audit, grants, etc.) are also valid
        as parts of dotted names and field references.
        """
        tok = self._current()
        if tok.type == TokenType.IDENTIFIER or tok.type in _KEYWORD_TOKEN_TYPES:
            return self._advance()
        raise ParseError(
            f"Expected identifier, got {tok.type.name} ({tok.value!r})",
            tok,
        )

    def _peek_type(self, offset: int) -> TokenType | None:
        """Look ahead at a token type without consuming."""
        idx = self.pos + offset
        if idx >= len(self.tokens):
            return None
        return self.tokens[idx].type

    def _at_end(self) -> bool:
        return self._current().type == TokenType.EOF

    def _skip_newlines(self) -> None:
        """Skip NEWLINE tokens."""
        while self._check(TokenType.NEWLINE):
            self._advance()

    def _loc(self) -> SourceLocation:
        """Build a SourceLocation from the current token."""
        tok = self._current()
        return SourceLocation(file=tok.file, line=tok.line, column=tok.column)
