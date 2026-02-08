"""Tests for intent hashing."""

from covenant.lexer.lexer import Lexer
from covenant.parser.parser import Parser
from covenant.verify.fingerprint import fingerprint_contract
from covenant.verify.hasher import compute_intent_hash


def _parse_contract(source: str):
    """Parse source and return the first contract."""
    tokens = Lexer(source, "test.cov").tokenize()
    program = Parser(tokens, "test.cov").parse()
    return program.contracts[0]


class TestIntentHash:
    def test_deterministic(self):
        source = (
            "contract f(x: Int) -> Int\n"
            "  body:\n"
            "    return x + 1\n"
        )
        contract = _parse_contract(source)
        h1 = compute_intent_hash(contract, intent_text="increment x")
        h2 = compute_intent_hash(contract, intent_text="increment x")
        assert h1.combined_hash == h2.combined_hash

    def test_different_intent_different_hash(self):
        source = (
            "contract f(x: Int) -> Int\n"
            "  body:\n"
            "    return x + 1\n"
        )
        contract = _parse_contract(source)
        h1 = compute_intent_hash(contract, intent_text="increment x")
        h2 = compute_intent_hash(contract, intent_text="add one to x")
        assert h1.intent_hash != h2.intent_hash
        assert h1.combined_hash != h2.combined_hash
        # Fingerprint should be the same since code is identical
        assert h1.fingerprint_hash == h2.fingerprint_hash

    def test_different_code_different_hash(self):
        source1 = (
            "contract f(x: Int) -> Int\n"
            "  body:\n"
            "    return x + 1\n"
        )
        source2 = (
            "contract f(x: Int) -> Int\n"
            "  body:\n"
            "    return x + 2\n"
        )
        c1 = _parse_contract(source1)
        c2 = _parse_contract(source2)
        h1 = compute_intent_hash(c1, intent_text="increment x")
        h2 = compute_intent_hash(c2, intent_text="increment x")
        assert h1.fingerprint_hash != h2.fingerprint_hash
        assert h1.combined_hash != h2.combined_hash
        # Intent should be the same
        assert h1.intent_hash == h2.intent_hash

    def test_empty_intent(self):
        source = (
            "contract f() -> Void\n"
            "  body:\n"
            "    return Void()\n"
        )
        contract = _parse_contract(source)
        h = compute_intent_hash(contract, intent_text="")
        assert h.intent_hash != ""
        assert h.fingerprint_hash != ""
        assert h.combined_hash != ""

    def test_hash_fields(self):
        source = (
            "contract f(x: Int) -> Int\n"
            "  body:\n"
            "    return x\n"
        )
        contract = _parse_contract(source)
        h = compute_intent_hash(contract, intent_text="identity")
        assert h.contract_name == "f"
        assert h.intent_text == "identity"
        assert len(h.intent_hash) == 64  # SHA-256 hex
        assert len(h.fingerprint_hash) == 64
        assert len(h.combined_hash) == 64

    def test_to_dict(self):
        source = (
            "contract f() -> Void\n"
            "  body:\n"
            "    return Void()\n"
        )
        contract = _parse_contract(source)
        h = compute_intent_hash(contract, intent_text="noop")
        d = h.to_dict()
        assert d["contract_name"] == "f"
        assert d["intent_text"] == "noop"
        assert "intent_hash" in d
        assert "fingerprint_hash" in d
        assert "combined_hash" in d


class TestIntentHashComparison:
    def test_no_change(self):
        source = (
            "contract f(x: Int) -> Int\n"
            "  body:\n"
            "    return x + 1\n"
        )
        contract = _parse_contract(source)
        h1 = compute_intent_hash(contract, intent_text="increment")
        h2 = compute_intent_hash(contract, intent_text="increment")
        cmp = h1.verify_against(h2)
        assert cmp.combined_match is True
        assert cmp.is_drift is False
        assert cmp.is_consistent is True

    def test_semantic_drift_detected(self):
        """Behavior changed but intent stayed the same -> drift."""
        source1 = (
            "contract f(x: Int) -> Int\n"
            "  body:\n"
            "    return x + 1\n"
        )
        source2 = (
            "contract f(x: Int) -> Int\n"
            "  body:\n"
            "    return x * 2\n"
        )
        c1 = _parse_contract(source1)
        c2 = _parse_contract(source2)
        h1 = compute_intent_hash(c1, intent_text="increment")
        h2 = compute_intent_hash(c2, intent_text="increment")
        cmp = h2.verify_against(h1)
        assert cmp.is_drift is True
        assert "SEMANTIC DRIFT" in cmp.describe()

    def test_consistent_change(self):
        """Both intent and behavior changed -> consistent."""
        source1 = (
            "contract f(x: Int) -> Int\n"
            "  body:\n"
            "    return x + 1\n"
        )
        source2 = (
            "contract f(x: Int) -> Int\n"
            "  body:\n"
            "    return x * 2\n"
        )
        c1 = _parse_contract(source1)
        c2 = _parse_contract(source2)
        h1 = compute_intent_hash(c1, intent_text="increment")
        h2 = compute_intent_hash(c2, intent_text="double")
        cmp = h2.verify_against(h1)
        assert cmp.is_drift is False
        assert cmp.is_consistent is True

    def test_intent_only_change(self):
        """Intent changed but behavior didn't."""
        source = (
            "contract f(x: Int) -> Int\n"
            "  body:\n"
            "    return x + 1\n"
        )
        contract = _parse_contract(source)
        h1 = compute_intent_hash(contract, intent_text="increment")
        h2 = compute_intent_hash(contract, intent_text="add one")
        cmp = h2.verify_against(h1)
        assert cmp.intent_changed is True
        assert cmp.behavior_changed is False
        assert "intent updated" in cmp.describe()
