use serde::Serialize;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct Span {
    pub line: usize,
    pub column: usize,
    pub offset: usize,
    pub length: usize,
}

impl Span {
    #[must_use]
    pub const fn new(line: usize, column: usize, offset: usize, length: usize) -> Self {
        Self {
            line,
            column,
            offset,
            length,
        }
    }

    #[must_use]
    pub const fn merge(self, other: Self) -> Self {
        let end = other.offset + other.length;
        Self {
            line: self.line,
            column: self.column,
            offset: self.offset,
            length: end.saturating_sub(self.offset).max(1),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TypeRef {
    pub name: String,
    pub args: Vec<TypeRef>,
    pub nullable: bool,
    pub span: Span,
}

impl TypeRef {
    #[must_use]
    pub fn display_name(&self) -> String {
        let mut name = self.name.clone();
        if !self.args.is_empty() {
            name.push('<');
            name.push_str(
                &self
                    .args
                    .iter()
                    .map(Self::display_name)
                    .collect::<Vec<_>>()
                    .join(", "),
            );
            name.push('>');
        }
        if self.nullable {
            name.push('?');
        }
        name
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Program {
    pub statements: Vec<Stmt>,
}

pub type Block = Vec<Stmt>;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "node", rename_all = "snake_case")]
pub enum Stmt {
    Let {
        name: String,
        ty: Option<TypeRef>,
        value: Expr,
        exported: bool,
        span: Span,
    },
    Assign {
        target: AssignTarget,
        value: Expr,
        span: Span,
    },
    Function {
        name: String,
        params: Vec<Parameter>,
        return_type: Option<TypeRef>,
        body: Block,
        exported: bool,
        span: Span,
    },
    TypeDecl {
        name: String,
        fields: Vec<Field>,
        exported: bool,
        span: Span,
    },
    Import {
        path: String,
        alias: Option<String>,
        span: Span,
    },
    FromImport {
        path: String,
        names: Vec<String>,
        span: Span,
    },
    Return {
        value: Option<Expr>,
        span: Span,
    },
    If {
        condition: Expr,
        then_block: Block,
        else_block: Option<Block>,
        span: Span,
    },
    While {
        condition: Expr,
        body: Block,
        span: Span,
    },
    For {
        name: String,
        iterable: Expr,
        body: Block,
        span: Span,
    },
    Break {
        span: Span,
    },
    Continue {
        span: Span,
    },
    Expression {
        expression: Expr,
        span: Span,
    },
}

impl Stmt {
    #[must_use]
    pub const fn span(&self) -> Span {
        match self {
            Self::Let { span, .. }
            | Self::Assign { span, .. }
            | Self::Function { span, .. }
            | Self::TypeDecl { span, .. }
            | Self::Import { span, .. }
            | Self::FromImport { span, .. }
            | Self::Return { span, .. }
            | Self::If { span, .. }
            | Self::While { span, .. }
            | Self::For { span, .. }
            | Self::Break { span }
            | Self::Continue { span }
            | Self::Expression { span, .. } => *span,
        }
    }

    #[must_use]
    pub fn exported_name(&self) -> Option<&str> {
        match self {
            Self::Let {
                name,
                exported: true,
                ..
            }
            | Self::Function {
                name,
                exported: true,
                ..
            }
            | Self::TypeDecl {
                name,
                exported: true,
                ..
            } => Some(name),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Parameter {
    pub name: String,
    pub ty: Option<TypeRef>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct Field {
    pub name: String,
    pub ty: Option<TypeRef>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "target", rename_all = "snake_case")]
pub enum AssignTarget {
    Variable {
        name: String,
        span: Span,
    },
    Property {
        object: Box<Expr>,
        name: String,
        span: Span,
    },
    Index {
        object: Box<Expr>,
        index: Box<Expr>,
        span: Span,
    },
}

impl AssignTarget {
    #[must_use]
    pub const fn span(&self) -> Span {
        match self {
            Self::Variable { span, .. }
            | Self::Property { span, .. }
            | Self::Index { span, .. } => *span,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Expr {
    #[serde(flatten)]
    pub kind: ExprKind,
    pub span: Span,
}

impl Expr {
    #[must_use]
    pub const fn new(kind: ExprKind, span: Span) -> Self {
        Self { kind, span }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "node", rename_all = "snake_case")]
pub enum ExprKind {
    Literal {
        value: Literal,
    },
    Variable {
        name: String,
    },
    List {
        values: Vec<Expr>,
    },
    Map {
        entries: Vec<(Expr, Expr)>,
    },
    Unary {
        operator: String,
        right: Box<Expr>,
    },
    Binary {
        left: Box<Expr>,
        operator: String,
        right: Box<Expr>,
    },
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
    },
    Get {
        object: Box<Expr>,
        name: String,
    },
    Index {
        object: Box<Expr>,
        index: Box<Expr>,
    },
}

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum Literal {
    Nil,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
}
