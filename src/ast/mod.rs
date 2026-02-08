use std::fmt;

#[derive(Debug, Clone)]
pub struct SourceLocation {
    pub file: String,
    pub line: usize,
    pub column: usize,
}

impl SourceLocation {
    pub fn new(file: &str, line: usize, column: usize) -> Self {
        Self {
            file: file.to_string(),
            line,
            column,
        }
    }
}

// ── Risk level ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

// ── Type expressions ────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum TypeExpr {
    Simple {
        name: String,
        loc: SourceLocation,
    },
    Annotated {
        base: Box<TypeExpr>,
        annotations: Vec<String>,
        loc: SourceLocation,
    },
    Generic {
        name: String,
        params: Vec<TypeExpr>,
        loc: SourceLocation,
    },
    List {
        element_type: Box<TypeExpr>,
        loc: SourceLocation,
    },
}

impl TypeExpr {
    pub fn loc(&self) -> &SourceLocation {
        match self {
            TypeExpr::Simple { loc, .. } => loc,
            TypeExpr::Annotated { loc, .. } => loc,
            TypeExpr::Generic { loc, .. } => loc,
            TypeExpr::List { loc, .. } => loc,
        }
    }

    pub fn display_name(&self) -> String {
        match self {
            TypeExpr::Simple { name, .. } => name.clone(),
            TypeExpr::Annotated { base, annotations, .. } => {
                format!("{} [{}]", base.display_name(), annotations.join(", "))
            }
            TypeExpr::Generic { name, params, .. } => {
                let p: Vec<String> = params.iter().map(|t| t.display_name()).collect();
                format!("{}<{}>", name, p.join(", "))
            }
            TypeExpr::List { element_type, .. } => {
                format!("[{}]", element_type.display_name())
            }
        }
    }
}

// ── Parameters ──────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub type_expr: TypeExpr,
    pub loc: SourceLocation,
}

// ── Expressions ─────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum Expr {
    Identifier {
        name: String,
        loc: SourceLocation,
    },
    StringLiteral {
        value: String,
        loc: SourceLocation,
    },
    NumberLiteral {
        value: f64,
        is_int: bool,
        loc: SourceLocation,
    },
    BoolLiteral {
        value: bool,
        loc: SourceLocation,
    },
    ListLiteral {
        elements: Vec<Expr>,
        loc: SourceLocation,
    },
    BinaryOp {
        left: Box<Expr>,
        op: String,
        right: Box<Expr>,
        loc: SourceLocation,
    },
    UnaryOp {
        op: String,
        operand: Box<Expr>,
        loc: SourceLocation,
    },
    FieldAccess {
        object: Box<Expr>,
        field_name: String,
        loc: SourceLocation,
    },
    FunctionCall {
        function: Box<Expr>,
        arguments: Vec<Expr>,
        keyword_args: Vec<(String, Expr)>,
        loc: SourceLocation,
    },
    MethodCall {
        object: Box<Expr>,
        method: String,
        arguments: Vec<Expr>,
        keyword_args: Vec<(String, Expr)>,
        loc: SourceLocation,
    },
    OldExpr {
        inner: Box<Expr>,
        loc: SourceLocation,
    },
    HasExpr {
        subject: Box<Expr>,
        capability: Box<Expr>,
        loc: SourceLocation,
    },
}

impl Expr {
    pub fn loc(&self) -> &SourceLocation {
        match self {
            Expr::Identifier { loc, .. } => loc,
            Expr::StringLiteral { loc, .. } => loc,
            Expr::NumberLiteral { loc, .. } => loc,
            Expr::BoolLiteral { loc, .. } => loc,
            Expr::ListLiteral { loc, .. } => loc,
            Expr::BinaryOp { loc, .. } => loc,
            Expr::UnaryOp { loc, .. } => loc,
            Expr::FieldAccess { loc, .. } => loc,
            Expr::FunctionCall { loc, .. } => loc,
            Expr::MethodCall { loc, .. } => loc,
            Expr::OldExpr { loc, .. } => loc,
            Expr::HasExpr { loc, .. } => loc,
        }
    }
}

// ── Statements ──────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum Statement {
    Assignment {
        target: String,
        value: Expr,
        loc: SourceLocation,
    },
    Return {
        value: Expr,
        loc: SourceLocation,
    },
    Emit {
        event: Expr,
        loc: SourceLocation,
    },
    ExprStmt {
        expr: Expr,
        loc: SourceLocation,
    },
    If {
        condition: Expr,
        then_body: Vec<Statement>,
        else_body: Vec<Statement>,
        loc: SourceLocation,
    },
    For {
        var: String,
        iterable: Expr,
        body: Vec<Statement>,
        loc: SourceLocation,
    },
    While {
        condition: Expr,
        body: Vec<Statement>,
        loc: SourceLocation,
    },
}

impl Statement {
    pub fn loc(&self) -> &SourceLocation {
        match self {
            Statement::Assignment { loc, .. } => loc,
            Statement::Return { loc, .. } => loc,
            Statement::Emit { loc, .. } => loc,
            Statement::ExprStmt { loc, .. } => loc,
            Statement::If { loc, .. } => loc,
            Statement::For { loc, .. } => loc,
            Statement::While { loc, .. } => loc,
        }
    }
}

// ── Effect declarations ─────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum EffectDecl {
    Modifies {
        targets: Vec<String>,
        loc: SourceLocation,
    },
    Reads {
        targets: Vec<String>,
        loc: SourceLocation,
    },
    Emits {
        event_type: String,
        loc: SourceLocation,
    },
    TouchesNothingElse {
        loc: SourceLocation,
    },
}

// ── Flow constraints ────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum FlowConstraint {
    NeverFlowsTo {
        destinations: Vec<String>,
        loc: SourceLocation,
    },
    RequiresContext {
        context: String,
        loc: SourceLocation,
    },
}

// ── Contract sections ───────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Precondition {
    pub conditions: Vec<Expr>,
    pub loc: SourceLocation,
}

#[derive(Debug, Clone)]
pub struct Postcondition {
    pub conditions: Vec<Expr>,
    pub loc: SourceLocation,
}

#[derive(Debug, Clone)]
pub struct Effects {
    pub declarations: Vec<EffectDecl>,
    pub loc: SourceLocation,
}

#[derive(Debug, Clone)]
pub struct Body {
    pub statements: Vec<Statement>,
    pub loc: SourceLocation,
}

#[derive(Debug, Clone)]
pub struct OnFailure {
    pub statements: Vec<Statement>,
    pub loc: SourceLocation,
}

// ── Permissions ─────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct GrantsPermission {
    pub permissions: Vec<String>,
    pub loc: SourceLocation,
}

#[derive(Debug, Clone)]
pub struct DeniesPermission {
    pub permissions: Vec<String>,
    pub loc: SourceLocation,
}

#[derive(Debug, Clone)]
pub struct EscalationPolicy {
    pub policy: String,
    pub loc: SourceLocation,
}

#[derive(Debug, Clone)]
pub struct PermissionsBlock {
    pub grants: Option<GrantsPermission>,
    pub denies: Option<DeniesPermission>,
    pub escalation: Option<EscalationPolicy>,
    pub loc: SourceLocation,
}

// ── Contract definition ─────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ContractDef {
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: Option<TypeExpr>,
    pub precondition: Option<Precondition>,
    pub postcondition: Option<Postcondition>,
    pub effects: Option<Effects>,
    pub permissions: Option<PermissionsBlock>,
    pub body: Option<Body>,
    pub on_failure: Option<OnFailure>,
    pub loc: SourceLocation,
}

// ── Type definitions ────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct FieldDef {
    pub name: String,
    pub type_expr: TypeExpr,
    pub loc: SourceLocation,
}

#[derive(Debug, Clone)]
pub struct TypeDef {
    pub name: String,
    pub base_type: String,
    pub fields: Vec<FieldDef>,
    pub flow_constraints: Vec<FlowConstraint>,
    pub loc: SourceLocation,
}

// ── Shared state declarations ───────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct SharedDecl {
    pub name: String,
    pub type_name: String,
    pub access: String,
    pub isolation: String,
    pub audit: String,
    pub loc: SourceLocation,
}

// ── File header ─────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct IntentBlock {
    pub text: String,
    pub loc: SourceLocation,
}

#[derive(Debug, Clone)]
pub struct ScopeDecl {
    pub path: String,
    pub loc: SourceLocation,
}

#[derive(Debug, Clone)]
pub struct RiskDecl {
    pub level: RiskLevel,
    pub loc: SourceLocation,
}

#[derive(Debug, Clone)]
pub struct RequiresDecl {
    pub capabilities: Vec<String>,
    pub loc: SourceLocation,
}

#[derive(Debug, Clone)]
pub struct FileHeader {
    pub intent: Option<IntentBlock>,
    pub scope: Option<ScopeDecl>,
    pub risk: Option<RiskDecl>,
    pub requires: Option<RequiresDecl>,
    pub loc: SourceLocation,
}

// ── Use declarations ────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct UseDecl {
    pub name: String,
    pub alias: Option<String>,
    pub loc: SourceLocation,
}

// ── Top-level program ───────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Program {
    pub header: Option<FileHeader>,
    pub uses: Vec<UseDecl>,
    pub contracts: Vec<ContractDef>,
    pub type_defs: Vec<TypeDef>,
    pub shared_decls: Vec<SharedDecl>,
    pub loc: SourceLocation,
}

impl fmt::Display for RiskLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RiskLevel::Low => write!(f, "low"),
            RiskLevel::Medium => write!(f, "medium"),
            RiskLevel::High => write!(f, "high"),
            RiskLevel::Critical => write!(f, "critical"),
        }
    }
}
