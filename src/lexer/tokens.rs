use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TokenType {
    // Structure
    Indent,
    Dedent,
    Newline,
    Eof,

    // Literals
    StringLit,
    Integer,
    Float,
    True,
    False,

    // Identifiers & operators
    Identifier,
    Dot,
    Comma,
    Colon,
    Arrow,
    LParen,
    RParen,
    LBracket,
    RBracket,

    // Comparison / arithmetic
    Equals,
    NotEquals,
    LessThan,
    LessEqual,
    GreaterThan,
    GreaterEqual,
    Plus,
    Minus,
    Star,
    Slash,
    Assign,

    // Keywords — language structure
    Intent,
    Scope,
    Risk,
    Requires,
    Contract,
    Precondition,
    Postcondition,
    Effects,
    Body,
    OnFailure,

    // Keywords — effects
    Modifies,
    Reads,
    Emits,
    TouchesNothingElse,

    // Keywords — control flow / expressions
    Return,
    Emit,
    If,
    Else,
    For,
    In,
    While,
    And,
    Or,
    Not,
    Has,

    // Keywords — type system
    Type,
    Fields,
    FlowConstraints,
    NeverFlowsTo,
    RequiresContext,
    Shared,
    Access,
    Isolation,
    Audit,

    // Keywords — permissions
    Permissions,
    Grants,
    Denies,
    Escalation,

    // Keywords — risk levels
    Low,
    Medium,
    High,
    Critical,

    // Special
    Old,
    Use,
    As,
    Pure,

    // Keywords — audit
    Show,
    All,
    Where,
    Since,
}

impl TokenType {
    /// Whether this keyword token can appear in identifier position
    /// (dotted names, field access, etc.).
    pub fn can_be_identifier(&self) -> bool {
        matches!(
            self,
            TokenType::Access
                | TokenType::Audit
                | TokenType::Grants
                | TokenType::Denies
                | TokenType::Escalation
                | TokenType::Isolation
                | TokenType::Scope
                | TokenType::Risk
                | TokenType::Low
                | TokenType::Medium
                | TokenType::High
                | TokenType::Critical
                | TokenType::Fields
                | TokenType::Show
                | TokenType::All
                | TokenType::Where
                | TokenType::Since
                | TokenType::Reads
                | TokenType::Emits
                | TokenType::Modifies
                | TokenType::Shared
                | TokenType::Type
                | TokenType::Requires
                | TokenType::Intent
                | TokenType::Old
                | TokenType::Body
                | TokenType::Effects
                | TokenType::Precondition
                | TokenType::Postcondition
                | TokenType::Permissions
                | TokenType::Has
                | TokenType::As
                | TokenType::Pure
        )
    }
}

/// Look up a keyword string and return its TokenType, or None if it's a plain identifier.
pub fn keyword_type(word: &str) -> Option<TokenType> {
    match word {
        "intent" => Some(TokenType::Intent),
        "scope" => Some(TokenType::Scope),
        "risk" => Some(TokenType::Risk),
        "requires" => Some(TokenType::Requires),
        "contract" => Some(TokenType::Contract),
        "precondition" => Some(TokenType::Precondition),
        "postcondition" => Some(TokenType::Postcondition),
        "effects" => Some(TokenType::Effects),
        "body" => Some(TokenType::Body),
        "on_failure" => Some(TokenType::OnFailure),
        "modifies" => Some(TokenType::Modifies),
        "reads" => Some(TokenType::Reads),
        "emits" => Some(TokenType::Emits),
        "touches_nothing_else" => Some(TokenType::TouchesNothingElse),
        "return" => Some(TokenType::Return),
        "emit" => Some(TokenType::Emit),
        "if" => Some(TokenType::If),
        "else" => Some(TokenType::Else),
        "for" => Some(TokenType::For),
        "in" => Some(TokenType::In),
        "while" => Some(TokenType::While),
        "and" => Some(TokenType::And),
        "or" => Some(TokenType::Or),
        "not" => Some(TokenType::Not),
        "has" => Some(TokenType::Has),
        "type" => Some(TokenType::Type),
        "fields" => Some(TokenType::Fields),
        "flow_constraints" => Some(TokenType::FlowConstraints),
        "never_flows_to" => Some(TokenType::NeverFlowsTo),
        "requires_context" => Some(TokenType::RequiresContext),
        "shared" => Some(TokenType::Shared),
        "access" => Some(TokenType::Access),
        "isolation" => Some(TokenType::Isolation),
        "audit" => Some(TokenType::Audit),
        "permissions" => Some(TokenType::Permissions),
        "grants" => Some(TokenType::Grants),
        "denies" => Some(TokenType::Denies),
        "escalation" => Some(TokenType::Escalation),
        "low" => Some(TokenType::Low),
        "medium" => Some(TokenType::Medium),
        "high" => Some(TokenType::High),
        "critical" => Some(TokenType::Critical),
        "old" => Some(TokenType::Old),
        "use" => Some(TokenType::Use),
        "as" => Some(TokenType::As),
        "true" => Some(TokenType::True),
        "false" => Some(TokenType::False),
        "show" => Some(TokenType::Show),
        "all" => Some(TokenType::All),
        "where" => Some(TokenType::Where),
        "since" => Some(TokenType::Since),
        "pure" => Some(TokenType::Pure),
        _ => None,
    }
}

#[derive(Debug, Clone)]
pub struct Token {
    pub token_type: TokenType,
    pub value: String,
    pub line: usize,
    pub column: usize,
    pub file: String,
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.token_type {
            TokenType::Indent | TokenType::Dedent | TokenType::Newline | TokenType::Eof => {
                write!(f, "Token({:?}, {}:{})", self.token_type, self.line, self.column)
            }
            _ => {
                write!(
                    f,
                    "Token({:?}, {:?}, {}:{})",
                    self.token_type, self.value, self.line, self.column
                )
            }
        }
    }
}
