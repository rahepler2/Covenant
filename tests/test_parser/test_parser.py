"""Tests for the Covenant recursive descent parser."""

import pytest

from covenant.ast.nodes import (
    Assignment,
    BinaryOp,
    Body,
    ContractDef,
    Effects,
    EmitsEffect,
    EmitStmt,
    ExprStmt,
    FieldAccess,
    FieldDef,
    FileHeader,
    FunctionCall,
    HasExpr,
    Identifier,
    IntentBlock,
    MethodCall,
    ModifiesEffect,
    NeverFlowsTo,
    NumberLiteral,
    OldExpr,
    Postcondition,
    Precondition,
    Program,
    RequiresContext,
    RequiresDecl,
    ReturnStmt,
    RiskDecl,
    RiskLevel,
    ScopeDecl,
    SimpleType,
    AnnotatedType,
    StringLiteral,
    TouchesNothingElse,
    TypeDef,
)
from covenant.lexer.lexer import Lexer
from covenant.parser.parser import ParseError, Parser


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def parse(source: str) -> Program:
    tokens = Lexer(source, "test.cov").tokenize()
    return Parser(tokens, "test.cov").parse()


# ---------------------------------------------------------------------------
# File header
# ---------------------------------------------------------------------------

class TestFileHeader:
    def test_intent_only(self):
        prog = parse('intent: "Do something useful"\n')
        assert prog.header is not None
        assert prog.header.intent is not None
        assert prog.header.intent.text == "Do something useful"

    def test_full_header(self):
        source = (
            'intent: "Transfer funds"\n'
            "scope: finance.transfers\n"
            "risk: high\n"
            "requires: [auth.verified, ledger.access]\n"
        )
        prog = parse(source)
        h = prog.header
        assert h is not None
        assert h.intent.text == "Transfer funds"
        assert h.scope.path == "finance.transfers"
        assert h.risk.level == RiskLevel.HIGH
        assert h.requires.capabilities == ["auth.verified", "ledger.access"]

    def test_no_header(self):
        source = (
            "contract noop() -> Void\n"
            "  body:\n"
            "    return Void()\n"
        )
        prog = parse(source)
        assert prog.header is None

    def test_all_risk_levels(self):
        for level_str, level_enum in [
            ("low", RiskLevel.LOW),
            ("medium", RiskLevel.MEDIUM),
            ("high", RiskLevel.HIGH),
            ("critical", RiskLevel.CRITICAL),
        ]:
            prog = parse(f"risk: {level_str}\n")
            assert prog.header.risk.level == level_enum


# ---------------------------------------------------------------------------
# Contract definitions
# ---------------------------------------------------------------------------

class TestContractDef:
    def test_minimal_contract(self):
        source = (
            "contract noop() -> Void\n"
            "  body:\n"
            "    return Void()\n"
        )
        prog = parse(source)
        assert len(prog.contracts) == 1
        c = prog.contracts[0]
        assert c.name == "noop"
        assert len(c.params) == 0
        assert isinstance(c.return_type, SimpleType)
        assert c.return_type.name == "Void"

    def test_contract_with_params(self):
        source = (
            "contract add(a: Integer, b: Integer) -> Integer\n"
            "  body:\n"
            "    return a + b\n"
        )
        prog = parse(source)
        c = prog.contracts[0]
        assert len(c.params) == 2
        assert c.params[0].name == "a"
        assert isinstance(c.params[0].type_expr, SimpleType)
        assert c.params[0].type_expr.name == "Integer"

    def test_contract_with_precondition(self):
        source = (
            "contract divide(a: Float, b: Float) -> Float\n"
            "  precondition:\n"
            "    b != 0.0\n"
            "  body:\n"
            "    return a / b\n"
        )
        prog = parse(source)
        c = prog.contracts[0]
        assert c.precondition is not None
        assert len(c.precondition.conditions) == 1

    def test_contract_with_postcondition(self):
        source = (
            "contract increment(x: Integer) -> Integer\n"
            "  postcondition:\n"
            "    result == old(x) + 1\n"
            "  body:\n"
            "    return x + 1\n"
        )
        prog = parse(source)
        c = prog.contracts[0]
        assert c.postcondition is not None
        assert len(c.postcondition.conditions) == 1
        # Verify old() expression is parsed
        cond = c.postcondition.conditions[0]
        assert isinstance(cond, BinaryOp)
        assert isinstance(cond.right, BinaryOp)
        assert isinstance(cond.right.left, OldExpr)

    def test_contract_with_effects(self):
        source = (
            "contract update(rec: Record) -> Void\n"
            "  effects:\n"
            "    modifies [rec.value]\n"
            "    emits UpdateEvent\n"
            "    touches_nothing_else\n"
            "  body:\n"
            "    rec.value = 42\n"
        )
        prog = parse(source)
        c = prog.contracts[0]
        assert c.effects is not None
        decls = c.effects.declarations
        assert len(decls) == 3
        assert isinstance(decls[0], ModifiesEffect)
        assert decls[0].targets == ["rec.value"]
        assert isinstance(decls[1], EmitsEffect)
        assert decls[1].event_type == "UpdateEvent"
        assert isinstance(decls[2], TouchesNothingElse)

    def test_contract_with_on_failure(self):
        source = (
            "contract risky(x: Integer) -> Result\n"
            "  body:\n"
            "    return Result.ok(x)\n"
            "  on_failure:\n"
            "    return Result.error()\n"
        )
        prog = parse(source)
        c = prog.contracts[0]
        assert c.on_failure is not None
        assert len(c.on_failure.statements) == 1


# ---------------------------------------------------------------------------
# Expressions
# ---------------------------------------------------------------------------

class TestExpressions:
    def test_binary_comparison(self):
        source = (
            "contract check(x: Integer) -> Boolean\n"
            "  precondition:\n"
            "    x >= 0\n"
            "  body:\n"
            "    return x >= 0\n"
        )
        prog = parse(source)
        cond = prog.contracts[0].precondition.conditions[0]
        assert isinstance(cond, BinaryOp)
        assert cond.op == ">="

    def test_field_access(self):
        source = (
            "contract get(obj: Thing) -> Integer\n"
            "  body:\n"
            "    return obj.value\n"
        )
        prog = parse(source)
        ret = prog.contracts[0].body.statements[0]
        assert isinstance(ret, ReturnStmt)
        assert isinstance(ret.value, FieldAccess)
        assert ret.value.field_name == "value"

    def test_method_call(self):
        source = (
            "contract process(buf: Buffer) -> Data\n"
            "  body:\n"
            "    return buf.transform(42)\n"
        )
        prog = parse(source)
        ret = prog.contracts[0].body.statements[0]
        assert isinstance(ret.value, MethodCall)
        assert ret.value.method == "transform"
        assert len(ret.value.arguments) == 1

    def test_function_call(self):
        source = (
            "contract run() -> Result\n"
            "  body:\n"
            "    return compute(1, 2, 3)\n"
        )
        prog = parse(source)
        ret = prog.contracts[0].body.statements[0]
        assert isinstance(ret.value, FunctionCall)
        assert len(ret.value.arguments) == 3

    def test_has_expression(self):
        source = (
            "contract check(user: User) -> Boolean\n"
            "  precondition:\n"
            "    user has admin_role\n"
            "  body:\n"
            "    return true\n"
        )
        prog = parse(source)
        cond = prog.contracts[0].precondition.conditions[0]
        assert isinstance(cond, HasExpr)

    def test_old_expression(self):
        source = (
            "contract inc(x: Integer) -> Integer\n"
            "  postcondition:\n"
            "    result == old(x) + 1\n"
            "  body:\n"
            "    return x + 1\n"
        )
        prog = parse(source)
        cond = prog.contracts[0].postcondition.conditions[0]
        assert isinstance(cond, BinaryOp)
        assert isinstance(cond.right, BinaryOp)
        assert isinstance(cond.right.left, OldExpr)
        assert isinstance(cond.right.left.inner, Identifier)

    def test_arithmetic_precedence(self):
        source = (
            "contract calc(a: Integer, b: Integer) -> Integer\n"
            "  body:\n"
            "    return a + b * 2\n"
        )
        prog = parse(source)
        ret = prog.contracts[0].body.statements[0]
        # a + (b * 2), not (a + b) * 2
        expr = ret.value
        assert isinstance(expr, BinaryOp)
        assert expr.op == "+"
        assert isinstance(expr.right, BinaryOp)
        assert expr.right.op == "*"


# ---------------------------------------------------------------------------
# Statements
# ---------------------------------------------------------------------------

class TestStatements:
    def test_assignment(self):
        source = (
            "contract run() -> Void\n"
            "  body:\n"
            "    x = 42\n"
        )
        prog = parse(source)
        stmt = prog.contracts[0].body.statements[0]
        assert isinstance(stmt, Assignment)
        assert stmt.target == "x"
        assert isinstance(stmt.value, NumberLiteral)
        assert stmt.value.value == 42

    def test_emit_statement(self):
        source = (
            "contract run() -> Void\n"
            "  body:\n"
            "    emit SomeEvent()\n"
        )
        prog = parse(source)
        stmt = prog.contracts[0].body.statements[0]
        assert isinstance(stmt, EmitStmt)

    def test_multiple_statements(self):
        source = (
            "contract run() -> Result\n"
            "  body:\n"
            "    x = 1\n"
            "    y = 2\n"
            "    z = x + y\n"
            "    return z\n"
        )
        prog = parse(source)
        stmts = prog.contracts[0].body.statements
        assert len(stmts) == 4
        assert isinstance(stmts[0], Assignment)
        assert isinstance(stmts[1], Assignment)
        assert isinstance(stmts[2], Assignment)
        assert isinstance(stmts[3], ReturnStmt)


# ---------------------------------------------------------------------------
# Type definitions
# ---------------------------------------------------------------------------

class TestTypeDef:
    def test_type_with_fields(self):
        source = (
            "type Point = Record\n"
            "  fields:\n"
            "    x: Float\n"
            "    y: Float\n"
        )
        prog = parse(source)
        assert len(prog.type_defs) == 1
        td = prog.type_defs[0]
        assert td.name == "Point"
        assert td.base_type == "Record"
        assert len(td.fields) == 2
        assert td.fields[0].name == "x"
        assert td.fields[1].name == "y"

    def test_type_with_annotated_fields(self):
        source = (
            "type Secret = Record\n"
            "  fields:\n"
            "    value: String [pii, no_log]\n"
        )
        prog = parse(source)
        td = prog.type_defs[0]
        field = td.fields[0]
        assert isinstance(field.type_expr, AnnotatedType)
        assert field.type_expr.annotations == ["pii", "no_log"]

    def test_type_with_flow_constraints(self):
        source = (
            "type Sensitive = Record\n"
            "  fields:\n"
            "    data: String\n"
            "  flow_constraints:\n"
            "    never_flows_to: [external_api, log_sink]\n"
            "    requires_context: secure_session\n"
        )
        prog = parse(source)
        td = prog.type_defs[0]
        assert len(td.flow_constraints) == 2
        assert isinstance(td.flow_constraints[0], NeverFlowsTo)
        assert td.flow_constraints[0].destinations == ["external_api", "log_sink"]
        assert isinstance(td.flow_constraints[1], RequiresContext)
        assert td.flow_constraints[1].context == "secure_session"


# ---------------------------------------------------------------------------
# Shared declarations
# ---------------------------------------------------------------------------

class TestSharedDecl:
    def test_shared_basic(self):
        source = (
            "shared ledger: Ledger\n"
            "  access: transactional\n"
            "  isolation: serializable\n"
            "  audit: full_history\n"
        )
        prog = parse(source)
        assert len(prog.shared_decls) == 1
        sd = prog.shared_decls[0]
        assert sd.name == "ledger"
        assert sd.type_name == "Ledger"
        assert sd.access == "transactional"
        assert sd.isolation == "serializable"
        assert sd.audit == "full_history"


# ---------------------------------------------------------------------------
# Multiple contracts
# ---------------------------------------------------------------------------

class TestMultipleContracts:
    def test_two_contracts(self):
        source = (
            "contract first() -> Void\n"
            "  body:\n"
            "    return Void()\n"
            "\n"
            "contract second(x: Integer) -> Integer\n"
            "  body:\n"
            "    return x + 1\n"
        )
        prog = parse(source)
        assert len(prog.contracts) == 2
        assert prog.contracts[0].name == "first"
        assert prog.contracts[1].name == "second"


# ---------------------------------------------------------------------------
# Integration: parse the transfer example
# ---------------------------------------------------------------------------

class TestIntegration:
    def test_parse_transfer_example(self):
        source = (
            'intent: "Transfer funds between two accounts"\n'
            "scope: finance.transfers\n"
            "risk: high\n"
            "requires: [auth.verified, ledger.write_access]\n"
            "\n"
            "contract transfer(from: Account, to: Account, amount: Currency) -> TransferResult\n"
            "  precondition:\n"
            "    from.balance >= amount\n"
            "    amount > Currency(0)\n"
            "\n"
            "  postcondition:\n"
            "    from.balance == old(from.balance) - amount\n"
            "    to.balance == old(to.balance) + amount\n"
            "\n"
            "  effects:\n"
            "    modifies [from.balance, to.balance]\n"
            "    emits TransferEvent\n"
            "    touches_nothing_else\n"
            "\n"
            "  body:\n"
            "    hold = ledger.escrow(from, amount)\n"
            "    ledger.deposit(to, hold)\n"
            "    emit TransferEvent(from, to, amount)\n"
            "    return TransferResult.success()\n"
            "\n"
            "  on_failure:\n"
            "    ledger.rollback(hold)\n"
            "    return TransferResult.insufficient_funds()\n"
        )
        prog = parse(source)
        assert prog.header is not None
        assert prog.header.intent.text == "Transfer funds between two accounts"
        assert prog.header.risk.level == RiskLevel.HIGH

        c = prog.contracts[0]
        assert c.name == "transfer"
        assert len(c.params) == 3
        assert c.precondition is not None
        assert len(c.precondition.conditions) == 2
        assert c.postcondition is not None
        assert len(c.postcondition.conditions) == 2
        assert c.effects is not None
        assert len(c.effects.declarations) == 3
        assert c.body is not None
        assert len(c.body.statements) == 4
        assert c.on_failure is not None
        assert len(c.on_failure.statements) == 2


# ---------------------------------------------------------------------------
# Error handling
# ---------------------------------------------------------------------------

class TestParseErrors:
    def test_missing_body(self):
        source = (
            "contract bad() -> Void\n"
            "  precondition:\n"
            "    true\n"
        )
        # Should parse without error — body is optional structurally
        # (semantic check would catch this later)
        prog = parse(source)
        assert prog.contracts[0].body is None

    def test_invalid_top_level(self):
        source = "return 42\n"
        with pytest.raises(ParseError):
            parse(source)
