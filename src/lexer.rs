use crate::ast::Span;
use crate::error::{NiloError, Result};
use crate::token::{keyword_kind, Token, TokenKind, TokenLiteral};

pub struct Lexer<'a> {
    source: &'a str,
    chars: Vec<char>,
    byte_offsets: Vec<usize>,
    filename: String,
    index: usize,
    line: usize,
    column: usize,
}

impl<'a> Lexer<'a> {
    #[must_use]
    pub fn new(source: &'a str, filename: impl Into<String>) -> Self {
        let mut chars = Vec::new();
        let mut byte_offsets = Vec::new();
        for (offset, ch) in source.char_indices() {
            byte_offsets.push(offset);
            chars.push(ch);
        }
        byte_offsets.push(source.len());
        Self {
            source,
            chars,
            byte_offsets,
            filename: filename.into(),
            index: 0,
            line: 1,
            column: 1,
        }
    }

    pub fn tokenize(mut self) -> Result<Vec<Token>> {
        let mut tokens = Vec::new();
        while !self.at_end() {
            match self.peek() {
                ' ' | '\t' | '\r' => {
                    self.advance();
                }
                '\n' => self.advance_newline(),
                '/' if self.peek_next() == Some('/') => self.line_comment(),
                '/' if self.peek_next() == Some('*') => self.block_comment()?,
                '"' => tokens.push(self.string()?),
                ch if ch.is_ascii_digit() => tokens.push(self.number()?),
                ch if is_identifier_start(ch) => tokens.push(self.identifier()),
                _ => tokens.push(self.symbol()?),
            }
        }
        let offset = self.current_byte_offset();
        tokens.push(Token::new(
            TokenKind::Eof,
            "",
            None,
            Span::new(self.line, self.column, offset, 1),
        ));
        Ok(tokens)
    }

    fn symbol(&mut self) -> Result<Token> {
        let start_index = self.index;
        let line = self.line;
        let column = self.column;
        let first = self.advance();
        let pair_kind = match (first, self.peek_optional()) {
            ('-', Some('>')) => Some(TokenKind::Arrow),
            ('=', Some('=')) => Some(TokenKind::EqualEqual),
            ('!', Some('=')) => Some(TokenKind::BangEqual),
            ('<', Some('=')) => Some(TokenKind::LessEqual),
            ('>', Some('=')) => Some(TokenKind::GreaterEqual),
            ('&', Some('&')) => Some(TokenKind::AndAnd),
            ('|', Some('|')) => Some(TokenKind::OrOr),
            _ => None,
        };
        if let Some(kind) = pair_kind {
            self.advance();
            return Ok(self.make_token(kind, start_index, line, column, None));
        }

        let kind = match first {
            '=' => TokenKind::Assign,
            '!' => TokenKind::Bang,
            '<' => TokenKind::Less,
            '>' => TokenKind::Greater,
            '+' => TokenKind::Plus,
            '-' => TokenKind::Minus,
            '*' => TokenKind::Star,
            '/' => TokenKind::Slash,
            '%' => TokenKind::Percent,
            '(' => TokenKind::LeftParen,
            ')' => TokenKind::RightParen,
            '{' => TokenKind::LeftBrace,
            '}' => TokenKind::RightBrace,
            '[' => TokenKind::LeftBracket,
            ']' => TokenKind::RightBracket,
            ',' => TokenKind::Comma,
            '.' => TokenKind::Dot,
            ':' => TokenKind::Colon,
            ';' => TokenKind::Semicolon,
            '?' => TokenKind::Question,
            _ => {
                return Err(NiloError::lex(format!("unexpected character {first:?}")).at(
                    &self.filename,
                    Span::new(line, column, self.byte_offsets[start_index], first.len_utf8()),
                    Some(self.source),
                ));
            }
        };
        Ok(self.make_token(kind, start_index, line, column, None))
    }

    fn string(&mut self) -> Result<Token> {
        let start_index = self.index;
        let line = self.line;
        let column = self.column;
        self.advance();
        let mut value = String::new();
        while !self.at_end() && self.peek() != '"' {
            let ch = self.advance();
            if ch == '\n' {
                self.line += 1;
                self.column = 1;
                value.push('\n');
                continue;
            }
            if ch != '\\' {
                value.push(ch);
                continue;
            }
            if self.at_end() {
                break;
            }
            let escaped = self.advance();
            match escaped {
                'n' => value.push('\n'),
                'r' => value.push('\r'),
                't' => value.push('\t'),
                '0' => value.push('\0'),
                '"' => value.push('"'),
                '\\' => value.push('\\'),
                'u' if self.peek_optional() == Some('{') => {
                    self.advance();
                    let mut digits = String::new();
                    while !self.at_end() && self.peek() != '}' {
                        digits.push(self.advance());
                    }
                    if self.at_end() {
                        return Err(self.error_at(
                            "unterminated Unicode escape",
                            start_index,
                            line,
                            column,
                        ));
                    }
                    self.advance();
                    let code = u32::from_str_radix(&digits, 16).map_err(|_| {
                        self.error_at("invalid Unicode escape", start_index, line, column)
                    })?;
                    let decoded = char::from_u32(code).ok_or_else(|| {
                        self.error_at("invalid Unicode scalar value", start_index, line, column)
                    })?;
                    value.push(decoded);
                }
                other => {
                    return Err(self.error_at(
                        format!("unknown string escape \\{other}"),
                        start_index,
                        line,
                        column,
                    ));
                }
            }
        }
        if self.at_end() {
            return Err(self.error_at("unterminated string", start_index, line, column));
        }
        self.advance();
        Ok(self.make_token(
            TokenKind::String,
            start_index,
            line,
            column,
            Some(TokenLiteral::String(value)),
        ))
    }

    fn number(&mut self) -> Result<Token> {
        let start_index = self.index;
        let line = self.line;
        let column = self.column;
        while !self.at_end() && self.peek().is_ascii_digit() {
            self.advance();
        }

        let mut kind = TokenKind::Int;
        if !self.at_end()
            && self.peek() == '.'
            && self.peek_next().is_some_and(|ch| ch.is_ascii_digit())
        {
            kind = TokenKind::Float;
            self.advance();
            while !self.at_end() && self.peek().is_ascii_digit() {
                self.advance();
            }
        }

        if !self.at_end() && matches!(self.peek(), 'e' | 'E') {
            let exponent_start = self.index;
            self.advance();
            if !self.at_end() && matches!(self.peek(), '+' | '-') {
                self.advance();
            }
            if self.at_end() || !self.peek().is_ascii_digit() {
                return Err(self.error_at(
                    "expected digits after numeric exponent",
                    exponent_start,
                    line,
                    column,
                ));
            }
            kind = TokenKind::Float;
            while !self.at_end() && self.peek().is_ascii_digit() {
                self.advance();
            }
        }

        let text = self.slice(start_index, self.index);
        let literal = match kind {
            TokenKind::Int => TokenLiteral::Int(text.parse::<i64>().map_err(|_| {
                self.error_at("integer literal is out of range", start_index, line, column)
            })?),
            TokenKind::Float => TokenLiteral::Float(text.parse::<f64>().map_err(|_| {
                self.error_at("invalid floating-point literal", start_index, line, column)
            })?),
            _ => unreachable!(),
        };
        Ok(self.make_token(kind, start_index, line, column, Some(literal)))
    }

    fn identifier(&mut self) -> Token {
        let start_index = self.index;
        let line = self.line;
        let column = self.column;
        while !self.at_end() && is_identifier_continue(self.peek()) {
            self.advance();
        }
        let text = self.slice(start_index, self.index).to_owned();
        let kind = keyword_kind(&text).unwrap_or(TokenKind::Identifier);
        self.make_token(kind, start_index, line, column, None)
    }

    fn line_comment(&mut self) {
        while !self.at_end() && self.peek() != '\n' {
            self.advance();
        }
    }

    fn block_comment(&mut self) -> Result<()> {
        let start_index = self.index;
        let line = self.line;
        let column = self.column;
        self.advance();
        self.advance();
        let mut depth = 1usize;
        while !self.at_end() && depth > 0 {
            if self.peek() == '/' && self.peek_next() == Some('*') {
                self.advance();
                self.advance();
                depth += 1;
            } else if self.peek() == '*' && self.peek_next() == Some('/') {
                self.advance();
                self.advance();
                depth -= 1;
            } else if self.peek() == '\n' {
                self.advance_newline();
            } else {
                self.advance();
            }
        }
        if depth != 0 {
            return Err(self.error_at("unterminated block comment", start_index, line, column));
        }
        Ok(())
    }

    fn make_token(
        &self,
        kind: TokenKind,
        start_index: usize,
        line: usize,
        column: usize,
        literal: Option<TokenLiteral>,
    ) -> Token {
        let start = self.byte_offsets[start_index];
        let end = self.current_byte_offset();
        Token::new(
            kind,
            self.source[start..end].to_owned(),
            literal,
            Span::new(line, column, start, end.saturating_sub(start).max(1)),
        )
    }

    fn error_at(
        &self,
        message: impl Into<String>,
        start_index: usize,
        line: usize,
        column: usize,
    ) -> NiloError {
        let start = self.byte_offsets[start_index.min(self.chars.len())];
        let end = self.current_byte_offset();
        NiloError::lex(message).at(
            &self.filename,
            Span::new(line, column, start, end.saturating_sub(start).max(1)),
            Some(self.source),
        )
    }

    fn slice(&self, start_index: usize, end_index: usize) -> &str {
        &self.source[self.byte_offsets[start_index]..self.byte_offsets[end_index]]
    }

    fn peek(&self) -> char {
        self.chars[self.index]
    }

    fn peek_optional(&self) -> Option<char> {
        self.chars.get(self.index).copied()
    }

    fn peek_next(&self) -> Option<char> {
        self.chars.get(self.index + 1).copied()
    }

    fn advance(&mut self) -> char {
        let ch = self.chars[self.index];
        self.index += 1;
        self.column += 1;
        ch
    }

    fn advance_newline(&mut self) {
        self.index += 1;
        self.line += 1;
        self.column = 1;
    }

    fn current_byte_offset(&self) -> usize {
        self.byte_offsets[self.index]
    }

    fn at_end(&self) -> bool {
        self.index >= self.chars.len()
    }
}

fn is_identifier_start(ch: char) -> bool {
    ch == '_' || ch.is_alphabetic()
}

fn is_identifier_continue(ch: char) -> bool {
    ch == '_' || ch.is_alphanumeric()
}

#[cfg(test)]
mod tests {
    use super::Lexer;
    use crate::token::{TokenKind, TokenLiteral};

    #[test]
    fn tokenizes_unicode_comments_and_numbers() {
        let tokens = Lexer::new(
            "let 日本語 = 12.5e2; /* nested /* ok */ done */",
            "<test>",
        )
        .tokenize()
        .expect("source should tokenize");
        assert_eq!(tokens[0].kind, TokenKind::Let);
        assert_eq!(tokens[1].lexeme, "日本語");
        assert!(matches!(
            tokens[3].literal,
            Some(TokenLiteral::Float(value)) if value == 1250.0
        ));
    }

    #[test]
    fn reports_unknown_escape() {
        let error = Lexer::new(r#""bad\q""#, "sample.nilo")
            .tokenize()
            .expect_err("escape should fail");
        assert!(error.render().contains("unknown string escape"));
    }
}
