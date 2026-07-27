//! Front-door helpers for tokenizing and parsing Nilo source.

use crate::ast::Program;
use crate::error::Result;
use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::token::Token;

pub fn tokenize_source(source: &str, filename: &str) -> Result<Vec<Token>> {
    Lexer::new(source, filename).tokenize()
}

pub fn parse_source(source: &str, filename: &str) -> Result<Program> {
    let tokens = tokenize_source(source, filename)?;
    Parser::new(tokens, filename, source).parse()
}
