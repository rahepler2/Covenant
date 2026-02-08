"""Tests for the intent-behavior consistency checker."""

import pytest

from covenant.lexer.lexer import Lexer
from covenant.parser.parser import Parser
from covenant.verify.checker import Severity, verify_contract, verify_program
from covenant.verify.fingerprint import fingerprint_contract
from covenant.ast.nodes import RiskLevel


def _parse_and_verify(source: str, risk_level=RiskLevel.LOW, capabilities=None):
    """Parse source, verify, and return results."""
    tokens = Lexer(source, "test.cov").tokenize()
    program = Parser(tokens, "test.cov").parse()
    contract = program.contracts[0]
    return verify_contract(
        contract,
        file="test.cov",
        declared_capabilities=capabilities,
        risk_level=risk_level,
    )


def _parse_and_verify_program(source: str):
    """Parse source, verify entire program."""
    tokens = Lexer(source, "test.cov").tokenize()
    program = Parser(tokens, "test.cov").parse()
    return verify_program(program, file="test.cov")


def _codes(results):
    """Extract just the error codes from results."""
    return [r.code for r in results]


def _errors(results):
    """Filter to ERROR/CRITICAL only."""
    return [r for r in results if r.severity in (Severity.ERROR, Severity.CRITICAL)]


def _warnings(results):
    """Filter to WARNING only."""
    return [r for r in results if r.severity == Severity.WARNING]


# ---------------------------------------------------------------------------
# Structural completeness
# ---------------------------------------------------------------------------

class TestStructuralCompleteness:
    def test_missing_body_is_error(self):
        results = _parse_and_verify(
            "contract f() -> Void\n"
            "  precondition:\n"
            "    true\n"
        )
        assert "E004" in _codes(results)

    def test_missing_precondition_warning_low_risk(self):
        results = _parse_and_verify(
            "contract f() -> Void\n"
            "  body:\n"
            "    return Void()\n"
        )
        codes = _codes(results)
        assert "W003" in codes
        # At low risk, missing precondition is a warning not error
        w003 = [r for r in results if r.code == "W003"]
        assert w003[0].severity == Severity.WARNING

    def test_missing_precondition_error_high_risk(self):
        results = _parse_and_verify(
            "contract f() -> Void\n"
            "  body:\n"
            "    return Void()\n",
            risk_level=RiskLevel.HIGH,
        )
        w003 = [r for r in results if r.code == "W003"]
        assert w003[0].severity == Severity.ERROR

    def test_missing_postcondition_warning(self):
        results = _parse_and_verify(
            "contract f() -> Void\n"
            "  body:\n"
            "    return Void()\n"
        )
        assert "W004" in _codes(results)

    def test_missing_effects_warning(self):
        results = _parse_and_verify(
            "contract f() -> Void\n"
            "  body:\n"
            "    return Void()\n"
        )
        assert "W005" in _codes(results)

    def test_complete_contract_no_structural_warnings(self):
        results = _parse_and_verify(
            "contract f(x: Int) -> Int\n"
            "  precondition:\n"
            "    x > 0\n"
            "  postcondition:\n"
            "    result == x + 1\n"
            "  effects:\n"
            "    touches_nothing_else\n"
            "  body:\n"
            "    return x + 1\n"
        )
        structural_codes = {"W003", "W004", "W005", "E004"}
        assert not structural_codes.intersection(_codes(results))


# ---------------------------------------------------------------------------
# Effect Completeness (E001, E002)
# ---------------------------------------------------------------------------

class TestEffectCompleteness:
    def test_undeclared_mutation_with_touches_nothing(self):
        results = _parse_and_verify(
            "contract f(rec: Record) -> Void\n"
            "  effects:\n"
            "    modifies [rec.name]\n"
            "    touches_nothing_else\n"
            "  body:\n"
            "    rec.name = \"new\"\n"
            "    rec.value = 42\n"  # undeclared!
        )
        assert "E002" in _codes(results)

    def test_undeclared_mutation_without_touches_nothing(self):
        results = _parse_and_verify(
            "contract f(rec: Record) -> Void\n"
            "  effects:\n"
            "    modifies [rec.name]\n"
            "  body:\n"
            "    rec.name = \"new\"\n"
            "    rec.value = 42\n"  # undeclared, but no touches_nothing_else
        )
        # Should be a warning (E001) not error
        e001 = [r for r in results if r.code == "E001"]
        assert len(e001) == 1
        assert e001[0].severity == Severity.WARNING

    def test_declared_mutation_matches(self):
        results = _parse_and_verify(
            "contract f(rec: Record) -> Void\n"
            "  effects:\n"
            "    modifies [rec.value]\n"
            "    touches_nothing_else\n"
            "  body:\n"
            "    rec.value = 42\n"
        )
        # No E001 or E002
        assert "E001" not in _codes(results)
        assert "E002" not in _codes(results)

    def test_parent_path_covers_children(self):
        """modifies [rec] should cover rec.value mutations."""
        results = _parse_and_verify(
            "contract f(rec: Record) -> Void\n"
            "  effects:\n"
            "    modifies [rec]\n"
            "    touches_nothing_else\n"
            "  body:\n"
            "    rec.value = 42\n"
        )
        assert "E001" not in _codes(results)
        assert "E002" not in _codes(results)

    def test_local_variables_ok(self):
        """Local variable assignments shouldn't trigger E001."""
        results = _parse_and_verify(
            "contract f() -> Int\n"
            "  effects:\n"
            "    touches_nothing_else\n"
            "  body:\n"
            "    x = 42\n"
            "    return x\n"
        )
        assert "E001" not in _codes(results)
        assert "E002" not in _codes(results)


# ---------------------------------------------------------------------------
# Effect Soundness (W001)
# ---------------------------------------------------------------------------

class TestEffectSoundness:
    def test_declared_but_unused_effect(self):
        results = _parse_and_verify(
            "contract f() -> Void\n"
            "  effects:\n"
            "    modifies [some.state]\n"
            "  body:\n"
            "    return Void()\n"
        )
        assert "W001" in _codes(results)

    def test_all_declared_effects_used(self):
        results = _parse_and_verify(
            "contract f(rec: Record) -> Void\n"
            "  effects:\n"
            "    modifies [rec.value]\n"
            "  body:\n"
            "    rec.value = 42\n"
        )
        assert "W001" not in _codes(results)


# ---------------------------------------------------------------------------
# Emit Completeness/Soundness (E005, W002)
# ---------------------------------------------------------------------------

class TestEmitVerification:
    def test_undeclared_emit(self):
        results = _parse_and_verify(
            "contract f() -> Void\n"
            "  effects:\n"
            "    touches_nothing_else\n"
            "  body:\n"
            "    emit SomeEvent()\n"
        )
        assert "E005" in _codes(results)

    def test_declared_emit_matches(self):
        results = _parse_and_verify(
            "contract f() -> Void\n"
            "  effects:\n"
            "    emits SomeEvent\n"
            "    touches_nothing_else\n"
            "  body:\n"
            "    emit SomeEvent()\n"
        )
        assert "E005" not in _codes(results)
        assert "W002" not in _codes(results)

    def test_declared_emit_not_emitted(self):
        results = _parse_and_verify(
            "contract f() -> Void\n"
            "  effects:\n"
            "    emits SomeEvent\n"
            "  body:\n"
            "    return Void()\n"
        )
        assert "W002" in _codes(results)


# ---------------------------------------------------------------------------
# touches_nothing_else (E003)
# ---------------------------------------------------------------------------

class TestTouchesNothingElse:
    def test_external_call_violation(self):
        results = _parse_and_verify(
            "contract f(x: Int) -> Void\n"
            "  effects:\n"
            "    touches_nothing_else\n"
            "  body:\n"
            "    external_function(x)\n"
        )
        assert "E003" in _codes(results)

    def test_constructor_call_allowed(self):
        """Capitalized names (constructors) are always allowed."""
        results = _parse_and_verify(
            "contract f() -> Result\n"
            "  effects:\n"
            "    touches_nothing_else\n"
            "  body:\n"
            "    return Result()\n"
        )
        assert "E003" not in _codes(results)

    def test_parameter_method_call_allowed(self):
        """Calls on parameter objects are allowed."""
        results = _parse_and_verify(
            "contract f(buf: Buffer) -> Data\n"
            "  effects:\n"
            "    reads [buf]\n"
            "    touches_nothing_else\n"
            "  body:\n"
            "    return buf.transform(42)\n"
        )
        assert "E003" not in _codes(results)

    def test_no_touches_nothing_no_e003(self):
        """Without touches_nothing_else, E003 is not produced."""
        results = _parse_and_verify(
            "contract f() -> Void\n"
            "  effects:\n"
            "    modifies [something]\n"
            "  body:\n"
            "    external_function()\n"
        )
        assert "E003" not in _codes(results)


# ---------------------------------------------------------------------------
# Precondition Relevance (W006)
# ---------------------------------------------------------------------------

class TestPreconditionRelevance:
    def test_irrelevant_precondition(self):
        results = _parse_and_verify(
            "contract f(x: Int) -> Int\n"
            "  precondition:\n"
            "    unrelated_thing > 0\n"
            "  body:\n"
            "    return x\n"
        )
        assert "W006" in _codes(results)

    def test_relevant_precondition(self):
        results = _parse_and_verify(
            "contract f(x: Int) -> Int\n"
            "  precondition:\n"
            "    x > 0\n"
            "  body:\n"
            "    return x + 1\n"
        )
        assert "W006" not in _codes(results)


# ---------------------------------------------------------------------------
# Postcondition Achievability (W007)
# ---------------------------------------------------------------------------

class TestPostconditionAchievability:
    def test_old_ref_not_modified(self):
        results = _parse_and_verify(
            "contract f(x: Int) -> Int\n"
            "  postcondition:\n"
            "    result == old(unmodified_state) + 1\n"
            "  body:\n"
            "    return x + 1\n"
        )
        assert "W007" in _codes(results)

    def test_old_ref_is_modified(self):
        results = _parse_and_verify(
            "contract f(rec: Record) -> Void\n"
            "  postcondition:\n"
            "    rec.value == old(rec.value) + 1\n"
            "  effects:\n"
            "    modifies [rec.value]\n"
            "  body:\n"
            "    rec.value = rec.value + 1\n"
        )
        assert "W007" not in _codes(results)


# ---------------------------------------------------------------------------
# Informational
# ---------------------------------------------------------------------------

class TestInformational:
    def test_deep_nesting_info(self):
        results = _parse_and_verify(
            "contract f(x: Int) -> Int\n"
            "  body:\n"
            "    if x > 0:\n"
            "      if x > 1:\n"
            "        if x > 2:\n"
            "          if x > 3:\n"
            "            return x\n"
        )
        assert "I002" in _codes(results)


# ---------------------------------------------------------------------------
# Risk level escalation
# ---------------------------------------------------------------------------

class TestRiskEscalation:
    def test_high_risk_missing_sections_are_errors(self):
        results = _parse_and_verify(
            "contract f() -> Void\n"
            "  body:\n"
            "    return Void()\n",
            risk_level=RiskLevel.HIGH,
        )
        for r in results:
            if r.code in ("W003", "W004", "W005"):
                assert r.severity == Severity.ERROR

    def test_low_risk_missing_sections_are_warnings(self):
        results = _parse_and_verify(
            "contract f() -> Void\n"
            "  body:\n"
            "    return Void()\n",
            risk_level=RiskLevel.LOW,
        )
        for r in results:
            if r.code in ("W003", "W004", "W005"):
                assert r.severity == Severity.WARNING


# ---------------------------------------------------------------------------
# Program-level verification
# ---------------------------------------------------------------------------

class TestProgramVerification:
    def test_full_program_verification(self):
        source = (
            'intent: "Transfer funds"\n'
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
        results = _parse_and_verify_program(source)
        errors = _errors(results)
        # A well-formed program should have zero errors
        assert len(errors) == 0, f"Unexpected errors: {[str(e) for e in errors]}"

    def test_clean_sensor_example(self):
        source = (
            'intent: "Process sensor data"\n'
            "scope: iot.sensors\n"
            "risk: low\n"
            "\n"
            "contract process_data(input: SensorReading) -> AnalysisResult\n"
            "  precondition:\n"
            "    input.size > 0\n"
            "\n"
            "  postcondition:\n"
            "    result.source_id == input.sensor_id\n"
            "\n"
            "  effects:\n"
            "    reads [input]\n"
            "    touches_nothing_else\n"
            "\n"
            "  body:\n"
            "    buffer = Buffer.allocate(input.size)\n"
            "    filtered = filter_noise(buffer, input)\n"
            "    result = analyze(filtered)\n"
            "    return result\n"
        )
        results = _parse_and_verify_program(source)
        errors = _errors(results)
        # filter_noise and analyze are external calls — they should
        # be caught by touches_nothing_else
        e003 = [r for r in results if r.code == "E003"]
        assert len(e003) >= 1  # filter_noise or analyze not covered
