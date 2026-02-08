"""Immutable AST node definitions for Covenant.

Every node carries source location information and a metadata slot for
later compiler phases (intent verification, type checking, etc.) to
attach results. The AST is never mutated after construction — subsequent
phases produce annotated copies.
"""

from __future__ import annotations

import hashlib
from dataclasses import dataclass, field
from enum import Enum, auto
from typing import Any


# ---------------------------------------------------------------------------
# Source location
# ---------------------------------------------------------------------------

@dataclass(frozen=True, slots=True)
class SourceLocation:
    """Pinpoints a span in a source file."""

    file: str
    line: int
    column: int
    end_line: int | None = None
    end_column: int | None = None


# ---------------------------------------------------------------------------
# Base node
# ---------------------------------------------------------------------------

@dataclass(frozen=True)
class ASTNode:
    """Base class for all AST nodes.

    `source_hash` is the SHA-256 of the original source text that produced
    this node, used for tamper-evident audit trails.

    `metadata` is an open dict where later phases attach verification
    results, type information, capability labels, etc.
    """

    loc: SourceLocation
    source_hash: str = ""
    metadata: dict[str, Any] = field(default_factory=dict)

    @staticmethod
    def hash_source(text: str) -> str:
        return hashlib.sha256(text.encode("utf-8")).hexdigest()


# ---------------------------------------------------------------------------
# Top-level program
# ---------------------------------------------------------------------------

@dataclass(frozen=True)
class Program(ASTNode):
    header: FileHeader | None = None
    contracts: list[ContractDef] = field(default_factory=list)
    type_defs: list[TypeDef] = field(default_factory=list)
    shared_decls: list[SharedDecl] = field(default_factory=list)


# ---------------------------------------------------------------------------
# File header blocks
# ---------------------------------------------------------------------------

@dataclass(frozen=True)
class IntentBlock(ASTNode):
    """The intent declaration — compiler hashes this and binds it to
    the behavioral profile of the code."""

    text: str = ""


@dataclass(frozen=True)
class ScopeDecl(ASTNode):
    path: str = ""  # e.g. "finance.transfers"


class RiskLevel(Enum):
    LOW = auto()
    MEDIUM = auto()
    HIGH = auto()
    CRITICAL = auto()


@dataclass(frozen=True)
class RiskDecl(ASTNode):
    level: RiskLevel = RiskLevel.LOW


@dataclass(frozen=True)
class RequiresDecl(ASTNode):
    capabilities: list[str] = field(default_factory=list)


@dataclass(frozen=True)
class FileHeader(ASTNode):
    intent: IntentBlock | None = None
    scope: ScopeDecl | None = None
    risk: RiskDecl | None = None
    requires: RequiresDecl | None = None


# ---------------------------------------------------------------------------
# Type expressions
# ---------------------------------------------------------------------------

@dataclass(frozen=True)
class TypeExpr(ASTNode):
    """Base for all type expressions."""
    pass


@dataclass(frozen=True)
class SimpleType(TypeExpr):
    name: str = ""


@dataclass(frozen=True)
class GenericType(TypeExpr):
    name: str = ""
    params: list[TypeExpr] = field(default_factory=list)


@dataclass(frozen=True)
class ListType(TypeExpr):
    element_type: TypeExpr | None = None


@dataclass(frozen=True)
class AnnotatedType(TypeExpr):
    """A type with security/flow annotations, e.g. String [pii, no_log]."""

    base: TypeExpr | None = None
    annotations: list[str] = field(default_factory=list)


# ---------------------------------------------------------------------------
# Parameters
# ---------------------------------------------------------------------------

@dataclass(frozen=True)
class Param(ASTNode):
    name: str = ""
    type_expr: TypeExpr | None = None


# ---------------------------------------------------------------------------
# Expressions
# ---------------------------------------------------------------------------

@dataclass(frozen=True)
class Expr(ASTNode):
    """Base for all expressions."""
    pass


@dataclass(frozen=True)
class Identifier(Expr):
    name: str = ""


@dataclass(frozen=True)
class StringLiteral(Expr):
    value: str = ""


@dataclass(frozen=True)
class NumberLiteral(Expr):
    value: float | int = 0


@dataclass(frozen=True)
class BoolLiteral(Expr):
    value: bool = False


@dataclass(frozen=True)
class ListLiteral(Expr):
    elements: list[Expr] = field(default_factory=list)


@dataclass(frozen=True)
class BinaryOp(Expr):
    left: Expr | None = None
    op: str = ""
    right: Expr | None = None


@dataclass(frozen=True)
class UnaryOp(Expr):
    op: str = ""
    operand: Expr | None = None


@dataclass(frozen=True)
class FieldAccess(Expr):
    object: Expr | None = None
    field_name: str = ""


@dataclass(frozen=True)
class FunctionCall(Expr):
    function: Expr | None = None
    arguments: list[Expr] = field(default_factory=list)
    keyword_args: dict[str, Expr] = field(default_factory=dict)


@dataclass(frozen=True)
class MethodCall(Expr):
    object: Expr | None = None
    method: str = ""
    arguments: list[Expr] = field(default_factory=list)
    keyword_args: dict[str, Expr] = field(default_factory=dict)


@dataclass(frozen=True)
class OldExpr(Expr):
    """References pre-execution state: old(expr)."""

    inner: Expr | None = None


@dataclass(frozen=True)
class HasExpr(Expr):
    """Capability check: subject has capability."""

    subject: Expr | None = None
    capability: Expr | None = None


# ---------------------------------------------------------------------------
# Statements
# ---------------------------------------------------------------------------

@dataclass(frozen=True)
class Statement(ASTNode):
    """Base for all statements."""
    pass


@dataclass(frozen=True)
class Assignment(Statement):
    target: str = ""
    value: Expr | None = None


@dataclass(frozen=True)
class ReturnStmt(Statement):
    value: Expr | None = None


@dataclass(frozen=True)
class EmitStmt(Statement):
    event: Expr | None = None


@dataclass(frozen=True)
class ExprStmt(Statement):
    expr: Expr | None = None


@dataclass(frozen=True)
class IfStmt(Statement):
    condition: Expr | None = None
    then_body: list[Statement] = field(default_factory=list)
    else_body: list[Statement] = field(default_factory=list)


@dataclass(frozen=True)
class ForStmt(Statement):
    var: str = ""
    iterable: Expr | None = None
    loop_body: list[Statement] = field(default_factory=list)


@dataclass(frozen=True)
class WhileStmt(Statement):
    condition: Expr | None = None
    loop_body: list[Statement] = field(default_factory=list)


# ---------------------------------------------------------------------------
# Contract sections
# ---------------------------------------------------------------------------

@dataclass(frozen=True)
class Precondition(ASTNode):
    conditions: list[Expr] = field(default_factory=list)


@dataclass(frozen=True)
class Postcondition(ASTNode):
    conditions: list[Expr] = field(default_factory=list)


# Effects
@dataclass(frozen=True)
class EffectDecl(ASTNode):
    """Base for effect declarations."""
    pass


@dataclass(frozen=True)
class ModifiesEffect(EffectDecl):
    targets: list[str] = field(default_factory=list)


@dataclass(frozen=True)
class ReadsEffect(EffectDecl):
    targets: list[str] = field(default_factory=list)


@dataclass(frozen=True)
class EmitsEffect(EffectDecl):
    event_type: str = ""


@dataclass(frozen=True)
class TouchesNothingElse(EffectDecl):
    pass


@dataclass(frozen=True)
class Effects(ASTNode):
    declarations: list[EffectDecl] = field(default_factory=list)


@dataclass(frozen=True)
class Body(ASTNode):
    statements: list[Statement] = field(default_factory=list)


@dataclass(frozen=True)
class OnFailure(ASTNode):
    statements: list[Statement] = field(default_factory=list)


# ---------------------------------------------------------------------------
# Permissions block
# ---------------------------------------------------------------------------

@dataclass(frozen=True)
class GrantsPermission(ASTNode):
    permissions: list[str] = field(default_factory=list)


@dataclass(frozen=True)
class DeniesPermission(ASTNode):
    permissions: list[str] = field(default_factory=list)


@dataclass(frozen=True)
class EscalationPolicy(ASTNode):
    policy: str = ""


@dataclass(frozen=True)
class PermissionsBlock(ASTNode):
    grants: GrantsPermission | None = None
    denies: DeniesPermission | None = None
    escalation: EscalationPolicy | None = None


# ---------------------------------------------------------------------------
# Contract definition
# ---------------------------------------------------------------------------

@dataclass(frozen=True)
class ContractDef(ASTNode):
    name: str = ""
    params: list[Param] = field(default_factory=list)
    return_type: TypeExpr | None = None
    precondition: Precondition | None = None
    postcondition: Postcondition | None = None
    effects: Effects | None = None
    permissions: PermissionsBlock | None = None
    body: Body | None = None
    on_failure: OnFailure | None = None


# ---------------------------------------------------------------------------
# Type definitions
# ---------------------------------------------------------------------------

@dataclass(frozen=True)
class FieldDef(ASTNode):
    name: str = ""
    type_expr: TypeExpr | None = None


@dataclass(frozen=True)
class FlowConstraint(ASTNode):
    """Base for flow constraints."""
    pass


@dataclass(frozen=True)
class NeverFlowsTo(FlowConstraint):
    destinations: list[str] = field(default_factory=list)


@dataclass(frozen=True)
class RequiresContext(FlowConstraint):
    context: str = ""


@dataclass(frozen=True)
class TypeDef(ASTNode):
    name: str = ""
    base_type: str = ""
    fields: list[FieldDef] = field(default_factory=list)
    flow_constraints: list[FlowConstraint] = field(default_factory=list)


# ---------------------------------------------------------------------------
# Shared state declarations
# ---------------------------------------------------------------------------

@dataclass(frozen=True)
class SharedDecl(ASTNode):
    name: str = ""
    type_name: str = ""
    access: str = ""        # e.g. "transactional"
    isolation: str = ""     # e.g. "serializable"
    audit: str = ""         # e.g. "full_history"
