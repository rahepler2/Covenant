"""Covenant Intent Verification Engine (IVE).

Phase 2 of the Covenant compiler pipeline. Verifies that the code inside
a contract body is consistent with the intent declaration and contract
specification. This is not full formal verification — it is a lighter-weight
analysis that catches semantic drift without requiring theorem-proving
infrastructure.

Components:
    - fingerprint: Behavioral fingerprinting (what the code actually does)
    - checker: Intent-behavior consistency checks
    - hasher: Intent hashing (SHA-256 binding intent to behavioral profile)
"""

from covenant.verify.fingerprint import BehavioralFingerprint, fingerprint_contract
from covenant.verify.checker import VerificationResult, Severity, verify_contract, verify_program
from covenant.verify.hasher import IntentHash, compute_intent_hash

__all__ = [
    "BehavioralFingerprint",
    "fingerprint_contract",
    "VerificationResult",
    "Severity",
    "verify_contract",
    "verify_program",
    "IntentHash",
    "compute_intent_hash",
]
