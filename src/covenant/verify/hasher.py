"""Intent hashing for Covenant contracts.

Produces a cryptographic binding between a contract's intent declaration
and its behavioral fingerprint. If either the intent or the code changes,
the hash changes — enabling detection of semantic drift at compile time
and tamper detection at runtime.

The IntentHash is embedded in compiled artifacts and verified by the
audit runtime.
"""

from __future__ import annotations

import hashlib
import json
from dataclasses import dataclass

from covenant.ast.nodes import ContractDef
from covenant.verify.fingerprint import BehavioralFingerprint, fingerprint_contract


@dataclass(frozen=True)
class IntentHash:
    """Cryptographic binding of intent declaration to behavioral profile.

    Fields:
        intent_text: The original intent declaration text.
        intent_hash: SHA-256 of the intent text alone.
        fingerprint_hash: SHA-256 of the canonical behavioral fingerprint.
        combined_hash: SHA-256(intent_hash || fingerprint_hash) — the
            binding that connects what was intended to what was implemented.
        contract_name: Name of the contract this hash covers.
    """

    contract_name: str
    intent_text: str
    intent_hash: str
    fingerprint_hash: str
    combined_hash: str

    def to_dict(self) -> dict:
        return {
            "contract_name": self.contract_name,
            "intent_text": self.intent_text,
            "intent_hash": self.intent_hash,
            "fingerprint_hash": self.fingerprint_hash,
            "combined_hash": self.combined_hash,
        }

    def verify_against(self, other: IntentHash) -> IntentHashComparison:
        """Compare this hash against another (e.g., a previously stored one).

        Returns a structured comparison indicating what changed.
        """
        return IntentHashComparison(
            contract_name=self.contract_name,
            intent_changed=self.intent_hash != other.intent_hash,
            behavior_changed=self.fingerprint_hash != other.fingerprint_hash,
            combined_match=self.combined_hash == other.combined_hash,
            old_hash=other,
            new_hash=self,
        )


@dataclass(frozen=True)
class IntentHashComparison:
    """Result of comparing two IntentHash values for the same contract."""

    contract_name: str
    intent_changed: bool
    behavior_changed: bool
    combined_match: bool
    old_hash: IntentHash
    new_hash: IntentHash

    @property
    def is_drift(self) -> bool:
        """True if behavior changed without a corresponding intent update."""
        return self.behavior_changed and not self.intent_changed

    @property
    def is_consistent(self) -> bool:
        """True if intent and behavior changed together, or neither changed."""
        return self.combined_match or (self.intent_changed and self.behavior_changed)

    def describe(self) -> str:
        if self.combined_match:
            return f"contract '{self.contract_name}': no change"
        if self.is_drift:
            return (
                f"contract '{self.contract_name}': SEMANTIC DRIFT DETECTED — "
                f"behavior changed but intent declaration was not updated"
            )
        if self.intent_changed and not self.behavior_changed:
            return (
                f"contract '{self.contract_name}': intent updated but behavior "
                f"unchanged — verify intent still matches implementation"
            )
        return (
            f"contract '{self.contract_name}': both intent and behavior changed — "
            f"verify consistency"
        )


def compute_intent_hash(
    contract: ContractDef,
    intent_text: str = "",
    fingerprint: BehavioralFingerprint | None = None,
) -> IntentHash:
    """Compute the intent hash for a contract.

    Args:
        contract: The parsed contract definition.
        intent_text: The intent declaration text (from file header or
            contract-level intent). If empty, uses empty string.
        fingerprint: Pre-computed behavioral fingerprint, or None to
            compute one.

    Returns:
        An IntentHash binding the intent to the behavioral profile.
    """
    if fingerprint is None:
        fingerprint = fingerprint_contract(contract)

    # Hash the intent text
    intent_hash = hashlib.sha256(intent_text.encode("utf-8")).hexdigest()

    # Hash the behavioral fingerprint (canonical JSON for determinism)
    fp_json = json.dumps(fingerprint.to_canonical_dict(), sort_keys=True, separators=(",", ":"))
    fingerprint_hash = hashlib.sha256(fp_json.encode("utf-8")).hexdigest()

    # Combined hash: SHA-256(intent_hash || fingerprint_hash)
    combined = hashlib.sha256(
        (intent_hash + fingerprint_hash).encode("utf-8")
    ).hexdigest()

    return IntentHash(
        contract_name=contract.name,
        intent_text=intent_text,
        intent_hash=intent_hash,
        fingerprint_hash=fingerprint_hash,
        combined_hash=combined,
    )
