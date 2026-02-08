"""Behavioral fingerprinting for Covenant contracts.

Computes a deterministic behavioral fingerprint for each contract by
walking the AST and extracting:
  - What data the code reads (identifiers, field accesses)
  - What data the code mutates (assignment targets)
  - What external calls it makes (function/method calls)
  - What events it emits (emit statements)
  - What old() references it uses (pre-execution state)
  - Control flow structure (branching, looping)

The fingerprint is computed entirely from the AST — no execution needed.
When a contract is modified, the fingerprint changes if and only if the
behavioral profile changes.
"""

from __future__ import annotations

from dataclasses import dataclass, field

from covenant.ast.nodes import (
    ASTNode,
    Assignment,
    BinaryOp,
    Body,
    BoolLiteral,
    ContractDef,
    EmitStmt,
    Expr,
    ExprStmt,
    FieldAccess,
    ForStmt,
    FunctionCall,
    HasExpr,
    Identifier,
    IfStmt,
    ListLiteral,
    MethodCall,
    NumberLiteral,
    OldExpr,
    OnFailure,
    ReturnStmt,
    Statement,
    StringLiteral,
    UnaryOp,
    WhileStmt,
)


@dataclass
class BehavioralFingerprint:
    """Captures the abstract behavior of a contract body.

    All sets are sorted for deterministic hashing. This fingerprint
    does NOT include the contract's declared intent/effects — it
    captures what the code *actually does*, which the checker then
    compares against declarations.
    """

    # What state the body reads (identifiers and dotted paths)
    reads: set[str] = field(default_factory=set)

    # What state the body mutates (assignment targets)
    mutations: set[str] = field(default_factory=set)

    # Function/method calls made (as "object.method" or "function")
    calls: set[str] = field(default_factory=set)

    # Events emitted via emit statements
    emitted_events: set[str] = field(default_factory=set)

    # Pre-execution state references: old(expr) paths
    old_references: set[str] = field(default_factory=set)

    # Capability checks (subject has capability)
    capability_checks: set[str] = field(default_factory=set)

    # Operators used (for detecting semantic changes like + vs *)
    operators: list[str] = field(default_factory=list)

    # Literal values used (for detecting value changes like 1 vs 2)
    literals: list[str] = field(default_factory=list)

    # Control flow: does the body contain branching/looping?
    has_branching: bool = False
    has_looping: bool = False
    has_recursion: bool = False  # set if contract calls itself

    # Return paths
    return_count: int = 0

    # Structural complexity (depth of deepest nested scope)
    max_nesting_depth: int = 0

    def to_canonical_dict(self) -> dict:
        """Produce a deterministic dict for hashing."""
        return {
            "reads": sorted(self.reads),
            "mutations": sorted(self.mutations),
            "calls": sorted(self.calls),
            "emitted_events": sorted(self.emitted_events),
            "old_references": sorted(self.old_references),
            "capability_checks": sorted(self.capability_checks),
            "operators": sorted(self.operators),
            "literals": sorted(self.literals),
            "has_branching": self.has_branching,
            "has_looping": self.has_looping,
            "has_recursion": self.has_recursion,
            "return_count": self.return_count,
            "max_nesting_depth": self.max_nesting_depth,
        }


def fingerprint_contract(contract: ContractDef) -> BehavioralFingerprint:
    """Compute the behavioral fingerprint for a contract.

    Only fingerprints the body and on_failure sections — these represent
    what the code *actually does*. Preconditions and postconditions are
    analyzed separately by the checker since they represent declarations,
    not behavior.
    """
    fp = BehavioralFingerprint()
    walker = _ASTWalker(fp, contract.name)

    if contract.body:
        walker.walk_statements(contract.body.statements, depth=0)

    if contract.on_failure:
        walker.walk_statements(contract.on_failure.statements, depth=0)

    return fp


class _ASTWalker:
    """Walks the AST to populate a BehavioralFingerprint."""

    def __init__(self, fp: BehavioralFingerprint, contract_name: str) -> None:
        self.fp = fp
        self.contract_name = contract_name

    def walk_statements(self, stmts: list[Statement], depth: int) -> None:
        self.fp.max_nesting_depth = max(self.fp.max_nesting_depth, depth)
        for stmt in stmts:
            self.walk_statement(stmt, depth)

    def walk_statement(self, stmt: Statement, depth: int) -> None:
        if isinstance(stmt, Assignment):
            self.fp.mutations.add(stmt.target)
            if stmt.value:
                self.walk_expr(stmt.value)

        elif isinstance(stmt, ReturnStmt):
            self.fp.return_count += 1
            if stmt.value:
                self.walk_expr(stmt.value)

        elif isinstance(stmt, EmitStmt):
            if stmt.event:
                event_name = self._extract_event_name(stmt.event)
                if event_name:
                    self.fp.emitted_events.add(event_name)
                self.walk_expr(stmt.event)

        elif isinstance(stmt, ExprStmt):
            if stmt.expr:
                self.walk_expr(stmt.expr)

        elif isinstance(stmt, IfStmt):
            self.fp.has_branching = True
            if stmt.condition:
                self.walk_expr(stmt.condition)
            self.walk_statements(stmt.then_body, depth + 1)
            if stmt.else_body:
                self.walk_statements(stmt.else_body, depth + 1)

        elif isinstance(stmt, ForStmt):
            self.fp.has_looping = True
            if stmt.iterable:
                self.walk_expr(stmt.iterable)
            self.walk_statements(stmt.loop_body, depth + 1)

        elif isinstance(stmt, WhileStmt):
            self.fp.has_looping = True
            if stmt.condition:
                self.walk_expr(stmt.condition)
            self.walk_statements(stmt.loop_body, depth + 1)

    def walk_expr(self, expr: Expr) -> None:
        if isinstance(expr, Identifier):
            self.fp.reads.add(expr.name)

        elif isinstance(expr, FieldAccess):
            path = self._extract_dotted_path(expr)
            self.fp.reads.add(path)

        elif isinstance(expr, FunctionCall):
            call_name = self._extract_call_name(expr.function)
            if call_name:
                self.fp.calls.add(call_name)
                if call_name == self.contract_name:
                    self.fp.has_recursion = True
            if expr.function:
                self.walk_expr(expr.function)
            for arg in expr.arguments:
                self.walk_expr(arg)
            for arg in expr.keyword_args.values():
                self.walk_expr(arg)

        elif isinstance(expr, MethodCall):
            obj_path = self._extract_call_name(expr.object) if expr.object else ""
            call_name = f"{obj_path}.{expr.method}" if obj_path else expr.method
            self.fp.calls.add(call_name)
            if expr.object:
                self.walk_expr(expr.object)
            for arg in expr.arguments:
                self.walk_expr(arg)
            for arg in expr.keyword_args.values():
                self.walk_expr(arg)

        elif isinstance(expr, BinaryOp):
            if expr.op:
                self.fp.operators.append(expr.op)
            if expr.left:
                self.walk_expr(expr.left)
            if expr.right:
                self.walk_expr(expr.right)

        elif isinstance(expr, UnaryOp):
            if expr.op:
                self.fp.operators.append(expr.op)
            if expr.operand:
                self.walk_expr(expr.operand)

        elif isinstance(expr, OldExpr):
            if expr.inner:
                path = self._extract_dotted_path_from_expr(expr.inner)
                self.fp.old_references.add(path)
                self.walk_expr(expr.inner)

        elif isinstance(expr, HasExpr):
            if expr.subject and expr.capability:
                subj = self._extract_dotted_path_from_expr(expr.subject)
                cap = self._extract_dotted_path_from_expr(expr.capability)
                self.fp.capability_checks.add(f"{subj} has {cap}")

        elif isinstance(expr, ListLiteral):
            for elem in expr.elements:
                self.walk_expr(elem)

        elif isinstance(expr, NumberLiteral):
            self.fp.literals.append(str(expr.value))

        elif isinstance(expr, StringLiteral):
            self.fp.literals.append(repr(expr.value))

        elif isinstance(expr, BoolLiteral):
            self.fp.literals.append(str(expr.value))

    def _extract_dotted_path(self, expr: FieldAccess) -> str:
        """Convert a chain of FieldAccess nodes to a dotted string."""
        parts: list[str] = []
        current: Expr | None = expr
        while isinstance(current, FieldAccess):
            parts.append(current.field_name)
            current = current.object
        if isinstance(current, Identifier):
            parts.append(current.name)
        return ".".join(reversed(parts))

    def _extract_dotted_path_from_expr(self, expr: Expr) -> str:
        """Extract a dotted path from any expression."""
        if isinstance(expr, Identifier):
            return expr.name
        if isinstance(expr, FieldAccess):
            return self._extract_dotted_path(expr)
        if isinstance(expr, MethodCall):
            obj = self._extract_call_name(expr.object) if expr.object else ""
            return f"{obj}.{expr.method}()" if obj else f"{expr.method}()"
        if isinstance(expr, FunctionCall):
            name = self._extract_call_name(expr.function) if expr.function else ""
            return f"{name}()"
        return "<complex>"

    def _extract_call_name(self, expr: Expr | None) -> str:
        """Extract the name of a function/method being called."""
        if expr is None:
            return ""
        if isinstance(expr, Identifier):
            return expr.name
        if isinstance(expr, FieldAccess):
            return self._extract_dotted_path(expr)
        return "<indirect>"

    def _extract_event_name(self, expr: Expr) -> str | None:
        """Extract the event type name from an emit expression."""
        if isinstance(expr, FunctionCall):
            return self._extract_call_name(expr.function)
        if isinstance(expr, Identifier):
            return expr.name
        return None
