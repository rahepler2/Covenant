"""Token types and Token dataclass for the Covenant lexer."""

from __future__ import annotations

from dataclasses import dataclass
from enum import Enum, auto


class TokenType(Enum):
    """Every distinct token the Covenant lexer can produce."""

    # Structure
    INDENT = auto()
    DEDENT = auto()
    NEWLINE = auto()
    EOF = auto()

    # Literals
    STRING = auto()
    INTEGER = auto()
    FLOAT = auto()
    TRUE = auto()
    FALSE = auto()

    # Identifiers & operators
    IDENTIFIER = auto()
    DOT = auto()
    COMMA = auto()
    COLON = auto()
    ARROW = auto()          # ->
    LPAREN = auto()
    RPAREN = auto()
    LBRACKET = auto()
    RBRACKET = auto()

    # Comparison / arithmetic
    EQUALS = auto()         # ==
    NOT_EQUALS = auto()     # !=
    LESS_THAN = auto()
    LESS_EQUAL = auto()
    GREATER_THAN = auto()
    GREATER_EQUAL = auto()
    PLUS = auto()
    MINUS = auto()
    STAR = auto()
    SLASH = auto()
    ASSIGN = auto()         # =

    # Keywords — language structure
    INTENT = auto()
    SCOPE = auto()
    RISK = auto()
    REQUIRES = auto()
    CONTRACT = auto()
    PRECONDITION = auto()
    POSTCONDITION = auto()
    EFFECTS = auto()
    BODY = auto()
    ON_FAILURE = auto()

    # Keywords — effects
    MODIFIES = auto()
    READS = auto()
    EMITS = auto()
    TOUCHES_NOTHING_ELSE = auto()

    # Keywords — control flow / expressions
    RETURN = auto()
    EMIT = auto()
    IF = auto()
    ELSE = auto()
    FOR = auto()
    IN = auto()
    WHILE = auto()
    AND = auto()
    OR = auto()
    NOT = auto()
    HAS = auto()

    # Keywords — type system
    TYPE = auto()
    FIELDS = auto()
    FLOW_CONSTRAINTS = auto()
    NEVER_FLOWS_TO = auto()
    REQUIRES_CONTEXT = auto()
    SHARED = auto()
    ACCESS = auto()
    ISOLATION = auto()
    AUDIT = auto()

    # Keywords — permissions
    PERMISSIONS = auto()
    GRANTS = auto()
    DENIES = auto()
    ESCALATION = auto()

    # Keywords — risk levels
    LOW = auto()
    MEDIUM = auto()
    HIGH = auto()
    CRITICAL = auto()

    # Special
    OLD = auto()            # old() — pre-execution state reference
    COMMENT = auto()        # -- comment

    # Keywords — audit
    AUDIT_QUERY = auto()
    SHOW = auto()
    ALL = auto()
    WHERE = auto()
    SINCE = auto()


# Map keyword strings to token types
KEYWORDS: dict[str, TokenType] = {
    "intent": TokenType.INTENT,
    "scope": TokenType.SCOPE,
    "risk": TokenType.RISK,
    "requires": TokenType.REQUIRES,
    "contract": TokenType.CONTRACT,
    "precondition": TokenType.PRECONDITION,
    "postcondition": TokenType.POSTCONDITION,
    "effects": TokenType.EFFECTS,
    "body": TokenType.BODY,
    "on_failure": TokenType.ON_FAILURE,
    "modifies": TokenType.MODIFIES,
    "reads": TokenType.READS,
    "emits": TokenType.EMITS,
    "touches_nothing_else": TokenType.TOUCHES_NOTHING_ELSE,
    "return": TokenType.RETURN,
    "emit": TokenType.EMIT,
    "if": TokenType.IF,
    "else": TokenType.ELSE,
    "for": TokenType.FOR,
    "in": TokenType.IN,
    "while": TokenType.WHILE,
    "and": TokenType.AND,
    "or": TokenType.OR,
    "not": TokenType.NOT,
    "has": TokenType.HAS,
    "type": TokenType.TYPE,
    "fields": TokenType.FIELDS,
    "flow_constraints": TokenType.FLOW_CONSTRAINTS,
    "never_flows_to": TokenType.NEVER_FLOWS_TO,
    "requires_context": TokenType.REQUIRES_CONTEXT,
    "shared": TokenType.SHARED,
    "access": TokenType.ACCESS,
    "isolation": TokenType.ISOLATION,
    "audit": TokenType.AUDIT,
    "permissions": TokenType.PERMISSIONS,
    "grants": TokenType.GRANTS,
    "denies": TokenType.DENIES,
    "escalation": TokenType.ESCALATION,
    "low": TokenType.LOW,
    "medium": TokenType.MEDIUM,
    "high": TokenType.HIGH,
    "critical": TokenType.CRITICAL,
    "old": TokenType.OLD,
    "true": TokenType.TRUE,
    "false": TokenType.FALSE,
    "show": TokenType.SHOW,
    "all": TokenType.ALL,
    "where": TokenType.WHERE,
    "since": TokenType.SINCE,
}


@dataclass(frozen=True, slots=True)
class Token:
    """A single token produced by the lexer.

    Tokens are immutable and carry full source location information
    for diagnostics and audit provenance.
    """

    type: TokenType
    value: str
    line: int
    column: int
    file: str = "<unknown>"

    def __repr__(self) -> str:
        if self.type in (TokenType.INDENT, TokenType.DEDENT, TokenType.NEWLINE, TokenType.EOF):
            return f"Token({self.type.name}, {self.line}:{self.column})"
        return f"Token({self.type.name}, {self.value!r}, {self.line}:{self.column})"
