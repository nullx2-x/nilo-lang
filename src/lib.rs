//! Nilo language implementation.
//!
//! The public API exposes the lexer, parser, runtime values, and interpreter so
//! Nilo can be embedded as well as executed through the `nilo` command.

pub mod ast;
pub mod cli;
pub mod env;
pub mod error;
pub mod interpreter;
pub mod lexer;
pub mod parser;
pub mod runtime;
pub mod stdlib;
pub mod token;
pub mod value;

pub use error::{ErrorKind, NiloError, Result, SourceContext};
pub use interpreter::Interpreter;
pub use runtime::{parse_source, tokenize_source};
