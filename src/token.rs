use serde::Serialize;

use crate::ast::Span;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TokenKind {
    Eof,
    Identifier,
    Int,
    Float,
    String,
    True,
    False,
    Nil,
    Let,
    Func,
    Return,
    Type,
    Import,
    From,
    As,
    Export,
    If,
    Else,
    While,
    For,
    In,
    Break,
    Continue,
    Arrow,
    EqualEqual,
    BangEqual,
    LessEqual,
    GreaterEqual,
    AndAnd,
    OrOr,
    Assign,
    Bang,
    Less,
    Greater,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    LeftParen,
    RightParen,
    LeftBrace,
    RightBrace,
    LeftBracket,
    RightBracket,
    Comma,
    Dot,
    Colon,
    Semicolon,
    Question,
}

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum TokenLiteral {
    Int(i64),
    Float(f64),
    String(String),
}

#[derive(Debug, Clone, Serialize)]
pub struct Token {
    pub kind: TokenKind,
    pub lexeme: String,
    pub literal: Option<TokenLiteral>,
    pub span: Span,
}

impl Token {
    #[must_use]
    pub fn new(
        kind: TokenKind,
        lexeme: impl Into<String>,
        literal: Option<TokenLiteral>,
        span: Span,
    ) -> Self {
        Self {
            kind,
            lexeme: lexeme.into(),
            literal,
            span,
        }
    }
}

#[must_use]
pub fn keyword_kind(identifier: &str) -> Option<TokenKind> {
    Some(match identifier {
        "let" => TokenKind::Let,
        "func" => TokenKind::Func,
        "return" => TokenKind::Return,
        "type" => TokenKind::Type,
        "import" => TokenKind::Import,
        "from" => TokenKind::From,
        "as" => TokenKind::As,
        "export" => TokenKind::Export,
        "if" => TokenKind::If,
        "else" => TokenKind::Else,
        "while" => TokenKind::While,
        "for" => TokenKind::For,
        "in" => TokenKind::In,
        "break" => TokenKind::Break,
        "continue" => TokenKind::Continue,
        "true" => TokenKind::True,
        "false" => TokenKind::False,
        "nil" | "null" => TokenKind::Nil,
        _ => return None,
    })
}
