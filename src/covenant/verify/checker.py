"""Intent-behavior consistency checker for Covenant contracts.

Compares the behavioral fingerprint (what the code actually does) against
the declared intent, effects, preconditions, and postconditions. Produces
structured verification results with severity levels.

Checks performed:
  1. Effect Completeness — every mutation in body matches effects declaration
  2. Effect Soundness — every declared effect actually occurs in body
  3. Emit Completeness — every emitted event matches an emits declaration
  4. Emit Soundness — every declared emits actually occurs in body
  5. touches_nothing_else — no undeclared mutations or calls exist
  6. Precondition Relevance — preconditions reference state used in body
  7. Postcondition Achievability — postconditions reference only state
     the body could modify
  8. Intent Scope — code doesn't access capabilities beyond requires
  9. Structural completeness — body, precondition, postcondition, effects present
"""

from __future__ import annotations

from dataclasses import dataclass, field
from enum import Enum, auto

from covenant.ast.nodes import (
    ContractDef,
    Effects,
    EmitsEffect,
    ModifiesEffect,
    OldExpr,
    Precondition,
    Postcondition,
    Program,
    ReadsEffect,
    RiskLevel,
    TouchesNothingElse,
)
from covenant.verify.fingerprint import BehavioralFingerprint, fingerprint_contract


class Severity(Enum):
    """Severity of a verification finding."""

    INFO = auto()      # Informational, no action required
    WARNING = auto()   # Potential issue, review recommended
    ERROR = auto()     # Definite inconsistency, must fix
    CRITICAL = auto()  # Security-relevant inconsistency


@dataclass(frozen=True)
class VerificationResult:
    """A single finding from the intent verification engine."""

    severity: Severity
    code: str           # Machine-readable code, e.g. "E001"
    message: str        # Human-readable diagnostic
    contract_name: str
    file: str = ""
    line: int = 0

    def __str__(self) -> str:
        loc = f"{self.file}:{self.line}" if self.file else ""
        sev = self.severity.name
        return f"[{sev}] {self.code}: {loc}: contract '{self.contract_name}': {self.message}"


# ---------------------------------------------------------------------------
# Verification codes
# ---------------------------------------------------------------------------
# E = error, W = warning, I = info
#
# E001 — Undeclared mutation (effect completeness)
# E002 — touches_nothing_else violated by mutation
# E003 — touches_nothing_else violated by external call
# E004 — Missing body
# E005 — Undeclared event emission
# W001 — Declared effect not observed in body (effect soundness)
# W002 — Declared emits not observed in body (emit soundness)
# W003 — Missing precondition
# W004 — Missing postcondition
# W005 — Missing effects declaration
# W006 — Precondition references state not used in body
# W007 — Postcondition uses old() for state not modified
# W008 — Capability used beyond declared requires scope
# I001 — Contract has recursion
# I002 — Contract has high nesting depth


def verify_contract(
    contract: ContractDef,
    fingerprint: BehavioralFingerprint | None = None,
    file: str = "",
    declared_capabilities: list[str] | None = None,
    risk_level: RiskLevel = RiskLevel.LOW,
) -> list[VerificationResult]:
    """Run all consistency checks on a single contract.

    Args:
        contract: The parsed contract AST node.
        fingerprint: Pre-computed fingerprint, or None to compute it.
        file: Source filename for diagnostics.
        declared_capabilities: Capabilities from the file header requires.
        risk_level: Risk level from the file header.

    Returns:
        List of verification results (may be empty if all checks pass).
    """
    if fingerprint is None:
        fingerprint = fingerprint_contract(contract)

    results: list[VerificationResult] = []
    line = contract.loc.line
    name = contract.name

    def _add(severity: Severity, code: str, message: str) -> None:
        results.append(VerificationResult(
            severity=severity, code=code, message=message,
            contract_name=name, file=file, line=line,
        ))

    # -- Structural completeness ----------------------------------------

    if contract.body is None:
        _add(Severity.ERROR, "E004", "contract has no body")
        return results  # Can't check further without a body

    if contract.precondition is None:
        sev = Severity.ERROR if risk_level in (RiskLevel.HIGH, RiskLevel.CRITICAL) else Severity.WARNING
        _add(sev, "W003",
             "no precondition — every contract should declare what must be true before execution")

    if contract.postcondition is None:
        sev = Severity.ERROR if risk_level in (RiskLevel.HIGH, RiskLevel.CRITICAL) else Severity.WARNING
        _add(sev, "W004",
             "no postcondition — every contract should declare what will be true after execution")

    if contract.effects is None:
        sev = Severity.ERROR if risk_level in (RiskLevel.HIGH, RiskLevel.CRITICAL) else Severity.WARNING
        _add(sev, "W005",
             "no effects declaration — every contract must declare its side effects")

    # -- Effect Completeness (E001) -------------------------------------
    # Every mutation in the body must be covered by a declared modifies effect.

    declared_modifies = _extract_declared_modifies(contract.effects)
    declared_reads = _extract_declared_reads(contract.effects)
    declared_emits = _extract_declared_emits(contract.effects)
    has_touches_nothing = _has_touches_nothing_else(contract.effects)

    for mutation in fingerprint.mutations:
        if not _is_covered_by(mutation, declared_modifies):
            sev = Severity.ERROR if has_touches_nothing else Severity.WARNING
            if has_touches_nothing:
                _add(Severity.ERROR, "E002",
                     f"touches_nothing_else violated: body mutates '{mutation}' "
                     f"which is not in the modifies declaration")
            else:
                _add(Severity.WARNING, "E001",
                     f"body mutates '{mutation}' but it is not listed in "
                     f"the effects modifies declaration")

    # -- Effect Soundness (W001) ----------------------------------------
    # Every declared modifies target should actually be mutated in the body.

    for declared in declared_modifies:
        if not _is_observed_in(declared, fingerprint.mutations):
            _add(Severity.WARNING, "W001",
                 f"effects declares modifies '{declared}' but the body "
                 f"does not appear to mutate it")

    # -- Emit Completeness (E005) ---------------------------------------
    # Every emitted event must be declared in effects.

    for event in fingerprint.emitted_events:
        if event not in declared_emits:
            sev = Severity.ERROR if has_touches_nothing else Severity.WARNING
            _add(sev, "E005",
                 f"body emits '{event}' but it is not declared in the effects block")

    # -- Emit Soundness (W002) ------------------------------------------
    # Every declared emits should actually appear in the body.

    for declared_event in declared_emits:
        if declared_event not in fingerprint.emitted_events:
            _add(Severity.WARNING, "W002",
                 f"effects declares emits '{declared_event}' but the body "
                 f"does not emit it")

    # -- touches_nothing_else (E003) ------------------------------------
    # If declared, verify no external calls beyond known-safe patterns.

    if has_touches_nothing:
        # Calls to methods on declared modifies/reads targets are OK.
        # Calls to standalone functions that aren't constructors or known
        # utilities get flagged.
        allowed_call_prefixes = set()
        for m in declared_modifies:
            allowed_call_prefixes.add(m.split(".")[0])
        for r in declared_reads:
            allowed_call_prefixes.add(r.split(".")[0])
        # Parameters are always allowed call targets
        for param in contract.params:
            allowed_call_prefixes.add(param.name)
        # Required capabilities from file header imply allowed access
        if declared_capabilities:
            for cap in declared_capabilities:
                allowed_call_prefixes.add(cap.split(".")[0])

        for call in fingerprint.calls:
            root = call.split(".")[0]
            # Allow constructor-like calls (capitalized names) and calls
            # on allowed targets
            if root in allowed_call_prefixes:
                continue
            if root and root[0].isupper():
                continue  # Constructor/type call
            # Allow calls to functions that are locally assigned
            if root in fingerprint.mutations:
                continue
            # Allow reads of local variables
            if root in {s.split(".")[0] for s in fingerprint.reads if "." not in s}:
                # Local variable — check if it was assigned in body
                if root in fingerprint.mutations:
                    continue
            _add(Severity.ERROR, "E003",
                 f"touches_nothing_else violated: body calls '{call}' "
                 f"which is not covered by declared effects or parameters")

    # -- Precondition Relevance (W006) ----------------------------------
    # Precondition expressions should reference parameters or state the
    # body actually uses.

    if contract.precondition:
        param_names = {p.name for p in contract.params}
        precond_fp = _fingerprint_expressions(contract.precondition.conditions)
        # Collect all identifier roots used in the body
        body_roots = set()
        for r in fingerprint.reads:
            body_roots.add(r.split(".")[0])
        for m in fingerprint.mutations:
            body_roots.add(m.split(".")[0])

        for read in precond_fp.reads:
            root = read.split(".")[0]
            # Skip capitalized names — these are type/constructor references
            if root and root[0].isupper():
                continue
            if root not in param_names and root not in body_roots:
                _add(Severity.WARNING, "W006",
                     f"precondition references '{read}' which is not a parameter "
                     f"and not used in the body")

    # -- Postcondition Achievability (W007) -----------------------------
    # old() references in postconditions should reference state that the
    # body actually modifies.

    if contract.postcondition:
        postcond_fp = _fingerprint_expressions(contract.postcondition.conditions)
        for old_ref in postcond_fp.old_references:
            if not _is_mutation_covered(old_ref, fingerprint.mutations):
                _add(Severity.WARNING, "W007",
                     f"postcondition uses old({old_ref}) but the body does not "
                     f"appear to modify '{old_ref}'")

    # -- Intent Scope (W008) -------------------------------------------
    # If the file header declares required capabilities, check that the
    # body doesn't access capability namespaces beyond those.

    if declared_capabilities:
        cap_roots = set()
        for cap in declared_capabilities:
            cap_roots.add(cap.split(".")[0])

        for check in fingerprint.capability_checks:
            # "subject has capability" — extract capability part
            parts = check.split(" has ")
            if len(parts) == 2:
                cap_path = parts[1]
                cap_root = cap_path.split(".")[0]
                if cap_root not in cap_roots and cap_root not in {p.name for p in contract.params}:
                    _add(Severity.WARNING, "W008",
                         f"body checks capability '{cap_path}' but the file header "
                         f"only requires: {declared_capabilities}")

    # -- Informational --------------------------------------------------

    if fingerprint.has_recursion:
        _add(Severity.INFO, "I001", "contract contains recursive self-calls")

    if fingerprint.max_nesting_depth >= 4:
        _add(Severity.INFO, "I002",
             f"contract has nesting depth {fingerprint.max_nesting_depth} — "
             f"consider simplifying for auditability")

    return results


def verify_program(program: Program, file: str = "") -> list[VerificationResult]:
    """Run verification on an entire program (all contracts)."""
    results: list[VerificationResult] = []

    risk_level = RiskLevel.LOW
    declared_capabilities: list[str] | None = None

    if program.header:
        if program.header.risk:
            risk_level = program.header.risk.level
        if program.header.requires:
            declared_capabilities = program.header.requires.capabilities

    for contract in program.contracts:
        fp = fingerprint_contract(contract)
        results.extend(verify_contract(
            contract,
            fingerprint=fp,
            file=file,
            declared_capabilities=declared_capabilities,
            risk_level=risk_level,
        ))

    return results


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def _extract_declared_modifies(effects: Effects | None) -> set[str]:
    if effects is None:
        return set()
    result: set[str] = set()
    for decl in effects.declarations:
        if isinstance(decl, ModifiesEffect):
            result.update(decl.targets)
    return result


def _extract_declared_reads(effects: Effects | None) -> set[str]:
    if effects is None:
        return set()
    result: set[str] = set()
    for decl in effects.declarations:
        if isinstance(decl, ReadsEffect):
            result.update(decl.targets)
    return result


def _extract_declared_emits(effects: Effects | None) -> set[str]:
    if effects is None:
        return set()
    result: set[str] = set()
    for decl in effects.declarations:
        if isinstance(decl, EmitsEffect):
            result.add(decl.event_type)
    return result


def _has_touches_nothing_else(effects: Effects | None) -> bool:
    if effects is None:
        return False
    return any(isinstance(d, TouchesNothingElse) for d in effects.declarations)


def _is_covered_by(actual: str, declared: set[str]) -> bool:
    """Check if an actual mutation/read path is covered by declared paths.

    'from.balance' is covered by 'from.balance' (exact match) or by
    'from' (parent covers children). Local variables (no dots) that
    aren't in declared are flagged only if they shadow external state.
    """
    if actual in declared:
        return True

    # Check if any declared path is a prefix (parent covers children)
    for d in declared:
        if actual.startswith(d + "."):
            return True

    # Local variable assignments (no dots) are generally OK — they're
    # local temporaries, not external state mutations
    if "." not in actual:
        return True

    return False


def _is_mutation_covered(ref: str, mutations: set[str]) -> bool:
    """Check if an old() reference path is covered by actual mutations.

    Unlike _is_covered_by, this does NOT auto-allow dotless names —
    old() references specifically assert that state was modified, so
    every referenced path must match an actual mutation.
    """
    if ref in mutations:
        return True
    # Check parent/child relationships
    for m in mutations:
        if ref.startswith(m + ".") or m.startswith(ref + "."):
            return True
    return False


def _is_observed_in(declared: str, actual: set[str]) -> bool:
    """Check if a declared effect path is observed in actual mutations.

    'from.balance' is observed if 'from.balance' is in actual or if
    'from' (a parent) is mutated.
    """
    if declared in actual:
        return True

    # Check if any actual mutation is within the declared scope
    for a in actual:
        if a.startswith(declared + "."):
            return True
        # Also check if the declared is a sub-path of an actual
        if declared.startswith(a + "."):
            return True

    return False


def _fingerprint_expressions(exprs: list) -> BehavioralFingerprint:
    """Create a mini-fingerprint from a list of expressions.

    Used to analyze precondition/postcondition expressions separately
    from the body.
    """
    from covenant.verify.fingerprint import _ASTWalker

    fp = BehavioralFingerprint()
    walker = _ASTWalker(fp, "")
    for expr in exprs:
        walker.walk_expr(expr)
    return fp
