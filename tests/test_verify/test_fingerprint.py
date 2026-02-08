"""Tests for behavioral fingerprinting."""

from covenant.lexer.lexer import Lexer
from covenant.parser.parser import Parser
from covenant.verify.fingerprint import BehavioralFingerprint, fingerprint_contract


def _parse_contract(source: str):
    """Parse source and return the first contract."""
    tokens = Lexer(source, "test.cov").tokenize()
    program = Parser(tokens, "test.cov").parse()
    assert len(program.contracts) >= 1
    return program.contracts[0]


class TestReads:
    def test_reads_identifiers(self):
        contract = _parse_contract(
            "contract f(x: Int) -> Int\n"
            "  body:\n"
            "    return x\n"
        )
        fp = fingerprint_contract(contract)
        assert "x" in fp.reads

    def test_reads_field_access(self):
        contract = _parse_contract(
            "contract f(obj: Thing) -> Int\n"
            "  body:\n"
            "    return obj.value\n"
        )
        fp = fingerprint_contract(contract)
        assert "obj.value" in fp.reads

    def test_reads_nested_field_access(self):
        contract = _parse_contract(
            "contract f(obj: Thing) -> Int\n"
            "  body:\n"
            "    return obj.inner.deep\n"
        )
        fp = fingerprint_contract(contract)
        assert "obj.inner.deep" in fp.reads

    def test_reads_from_body_not_precondition(self):
        """Body fingerprint includes body reads, not precondition reads."""
        contract = _parse_contract(
            "contract f(x: Int) -> Int\n"
            "  precondition:\n"
            "    x > 0\n"
            "  body:\n"
            "    return x\n"
        )
        fp = fingerprint_contract(contract)
        # x is read in the body (return x), so it's in reads
        assert "x" in fp.reads


class TestMutations:
    def test_simple_assignment(self):
        contract = _parse_contract(
            "contract f() -> Void\n"
            "  body:\n"
            "    x = 42\n"
        )
        fp = fingerprint_contract(contract)
        assert "x" in fp.mutations

    def test_field_assignment(self):
        contract = _parse_contract(
            "contract f(rec: Record) -> Void\n"
            "  body:\n"
            "    rec.value = 42\n"
        )
        fp = fingerprint_contract(contract)
        assert "rec.value" in fp.mutations

    def test_multiple_assignments(self):
        contract = _parse_contract(
            "contract f() -> Void\n"
            "  body:\n"
            "    a = 1\n"
            "    b = 2\n"
            "    c = 3\n"
        )
        fp = fingerprint_contract(contract)
        assert fp.mutations == {"a", "b", "c"}


class TestCalls:
    def test_function_call(self):
        contract = _parse_contract(
            "contract f() -> Result\n"
            "  body:\n"
            "    return compute(42)\n"
        )
        fp = fingerprint_contract(contract)
        assert "compute" in fp.calls

    def test_method_call(self):
        contract = _parse_contract(
            "contract f(buf: Buffer) -> Data\n"
            "  body:\n"
            "    return buf.transform(42)\n"
        )
        fp = fingerprint_contract(contract)
        assert "buf.transform" in fp.calls

    def test_chained_method_call(self):
        contract = _parse_contract(
            "contract f(ledger: Ledger, from: Account, amount: Currency) -> Void\n"
            "  body:\n"
            "    hold = ledger.escrow(from, amount)\n"
        )
        fp = fingerprint_contract(contract)
        assert "ledger.escrow" in fp.calls

    def test_constructor_call(self):
        contract = _parse_contract(
            "contract f() -> Result\n"
            "  body:\n"
            "    return Result()\n"
        )
        fp = fingerprint_contract(contract)
        assert "Result" in fp.calls


class TestEmittedEvents:
    def test_emit_statement(self):
        contract = _parse_contract(
            "contract f() -> Void\n"
            "  body:\n"
            "    emit TransferEvent()\n"
        )
        fp = fingerprint_contract(contract)
        assert "TransferEvent" in fp.emitted_events

    def test_emit_with_args(self):
        contract = _parse_contract(
            "contract f(a: Int, b: Int) -> Void\n"
            "  body:\n"
            "    emit ChangeEvent(a, b)\n"
        )
        fp = fingerprint_contract(contract)
        assert "ChangeEvent" in fp.emitted_events

    def test_no_emits(self):
        contract = _parse_contract(
            "contract f() -> Void\n"
            "  body:\n"
            "    x = 1\n"
        )
        fp = fingerprint_contract(contract)
        assert len(fp.emitted_events) == 0


class TestOldReferences:
    def test_old_in_body(self):
        """old() in the body is tracked by fingerprinting."""
        contract = _parse_contract(
            "contract f(x: Int) -> Int\n"
            "  body:\n"
            "    prev = old(x)\n"
            "    return x + 1\n"
        )
        fp = fingerprint_contract(contract)
        assert "x" in fp.old_references

    def test_old_field_access_in_body(self):
        contract = _parse_contract(
            "contract f(acc: Account) -> Currency\n"
            "  body:\n"
            "    prev = old(acc.balance)\n"
            "    acc.balance = acc.balance - 10\n"
            "    return acc.balance\n"
        )
        fp = fingerprint_contract(contract)
        assert "acc.balance" in fp.old_references

    def test_old_in_postcondition_not_in_body_fp(self):
        """old() in postcondition is NOT part of body fingerprint.
        The checker handles postcondition analysis separately."""
        contract = _parse_contract(
            "contract f(x: Int) -> Int\n"
            "  postcondition:\n"
            "    result == old(x) + 1\n"
            "  body:\n"
            "    return x + 1\n"
        )
        fp = fingerprint_contract(contract)
        # Body fingerprint doesn't include postcondition analysis
        assert "x" not in fp.old_references


class TestCapabilityChecks:
    def test_has_expression_in_body(self):
        """has expressions in body are tracked."""
        contract = _parse_contract(
            "contract f(user: User) -> Bool\n"
            "  body:\n"
            "    if user has admin_role:\n"
            "      return true\n"
            "    return false\n"
        )
        fp = fingerprint_contract(contract)
        assert "user has admin_role" in fp.capability_checks

    def test_has_in_precondition_not_in_body_fp(self):
        """has in precondition is NOT part of body fingerprint."""
        contract = _parse_contract(
            "contract f(user: User) -> Bool\n"
            "  precondition:\n"
            "    user has admin_role\n"
            "  body:\n"
            "    return true\n"
        )
        fp = fingerprint_contract(contract)
        assert len(fp.capability_checks) == 0


class TestControlFlow:
    def test_branching_detected(self):
        contract = _parse_contract(
            "contract f(x: Int) -> Int\n"
            "  body:\n"
            "    if x > 0:\n"
            "      return x\n"
            "    else:\n"
            "      return 0\n"
        )
        fp = fingerprint_contract(contract)
        assert fp.has_branching is True

    def test_no_branching(self):
        contract = _parse_contract(
            "contract f(x: Int) -> Int\n"
            "  body:\n"
            "    return x + 1\n"
        )
        fp = fingerprint_contract(contract)
        assert fp.has_branching is False

    def test_for_loop_detected(self):
        contract = _parse_contract(
            "contract f(items: List) -> Void\n"
            "  body:\n"
            "    for item in items:\n"
            "      process(item)\n"
        )
        fp = fingerprint_contract(contract)
        assert fp.has_looping is True

    def test_while_loop_detected(self):
        contract = _parse_contract(
            "contract f(x: Int) -> Void\n"
            "  body:\n"
            "    while x > 0:\n"
            "      x = x - 1\n"
        )
        fp = fingerprint_contract(contract)
        assert fp.has_looping is True

    def test_nesting_depth(self):
        contract = _parse_contract(
            "contract f(x: Int) -> Void\n"
            "  body:\n"
            "    if x > 0:\n"
            "      if x > 10:\n"
            "        y = x\n"
        )
        fp = fingerprint_contract(contract)
        assert fp.max_nesting_depth == 2

    def test_return_count(self):
        contract = _parse_contract(
            "contract f(x: Int) -> Int\n"
            "  body:\n"
            "    if x > 0:\n"
            "      return x\n"
            "    return 0\n"
        )
        fp = fingerprint_contract(contract)
        assert fp.return_count == 2

    def test_on_failure_counted(self):
        contract = _parse_contract(
            "contract f() -> Result\n"
            "  body:\n"
            "    return Result.ok()\n"
            "  on_failure:\n"
            "    return Result.error()\n"
        )
        fp = fingerprint_contract(contract)
        assert fp.return_count == 2


class TestCanonicalDict:
    def test_deterministic(self):
        contract = _parse_contract(
            "contract f(a: Int, b: Int) -> Int\n"
            "  body:\n"
            "    return a + b\n"
        )
        fp1 = fingerprint_contract(contract)
        fp2 = fingerprint_contract(contract)
        assert fp1.to_canonical_dict() == fp2.to_canonical_dict()

    def test_sorted_sets(self):
        contract = _parse_contract(
            "contract f() -> Void\n"
            "  body:\n"
            "    z = 1\n"
            "    a = 2\n"
            "    m = 3\n"
        )
        fp = fingerprint_contract(contract)
        d = fp.to_canonical_dict()
        assert d["mutations"] == ["a", "m", "z"]


class TestIntegrationFingerprint:
    def test_transfer_example(self):
        source = (
            'intent: "Transfer funds"\n'
            "scope: finance.transfers\n"
            "risk: high\n"
            "requires: [auth.verified, ledger.write_access]\n"
            "\n"
            "contract transfer(from: Account, to: Account, amount: Currency) -> TransferResult\n"
            "  precondition:\n"
            "    from.balance >= amount\n"
            "    from.owner has auth.current_session\n"
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
        tokens = Lexer(source, "test.cov").tokenize()
        program = Parser(tokens, "test.cov").parse()
        contract = program.contracts[0]
        fp = fingerprint_contract(contract)

        # Should detect mutations
        assert "hold" in fp.mutations

        # Should detect calls
        assert "ledger.escrow" in fp.calls
        assert "ledger.deposit" in fp.calls
        assert "ledger.rollback" in fp.calls

        # Should detect emitted events
        assert "TransferEvent" in fp.emitted_events

        # old() and has are only in preconditions/postconditions,
        # which are analyzed separately by the checker
        assert len(fp.old_references) == 0
        assert len(fp.capability_checks) == 0

        # Should have 2 returns (body + on_failure)
        assert fp.return_count == 2
