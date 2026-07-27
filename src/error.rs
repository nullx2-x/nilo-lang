use std::fmt;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crate::ast::Span;

pub type Result<T> = std::result::Result<T, NiloError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    Io,
    Lex,
    Parse,
    Type,
    Runtime,
    Module,
    Cli,
}

impl ErrorKind {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Io => "I/O error",
            Self::Lex => "lex error",
            Self::Parse => "parse error",
            Self::Type => "type error",
            Self::Runtime => "runtime error",
            Self::Module => "module error",
            Self::Cli => "command error",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    pub filename: String,
    pub line: usize,
    pub column: usize,
    pub length: usize,
}

#[derive(Debug, Clone)]
pub struct NiloError {
    pub kind: ErrorKind,
    pub message: String,
    pub location: Option<Location>,
    pub source_line: Option<String>,
    pub notes: Vec<String>,
}

impl NiloError {
    #[must_use]
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            location: None,
            source_line: None,
            notes: Vec::new(),
        }
    }

    #[must_use]
    pub fn lex(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Lex, message)
    }

    #[must_use]
    pub fn parse(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Parse, message)
    }

    #[must_use]
    pub fn type_error(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Type, message)
    }

    #[must_use]
    pub fn runtime(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Runtime, message)
    }

    #[must_use]
    pub fn module(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Module, message)
    }

    #[must_use]
    pub fn cli(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Cli, message)
    }

    #[must_use]
    pub fn io(path: impl AsRef<Path>, error: impl fmt::Display) -> Self {
        Self::new(
            ErrorKind::Io,
            format!("{}: {error}", path.as_ref().display()),
        )
    }

    #[must_use]
    pub fn at(mut self, filename: impl Into<String>, span: Span, source: Option<&str>) -> Self {
        self.location = Some(Location {
            filename: filename.into(),
            line: span.line,
            column: span.column,
            length: span.length.max(1),
        });
        if let Some(source) = source {
            self.source_line = source
                .lines()
                .nth(span.line.saturating_sub(1))
                .map(str::to_owned);
        }
        self
    }

    #[must_use]
    pub fn at_if_missing(
        self,
        filename: impl Into<String>,
        span: Span,
        source: Option<&str>,
    ) -> Self {
        if self.location.is_some() {
            self
        } else {
            self.at(filename, span, source)
        }
    }

    #[must_use]
    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }

    #[must_use]
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str("nilo: ");
        out.push_str(self.kind.label());
        if let Some(location) = &self.location {
            out.push_str(&format!(
                " at {}:{}:{}",
                location.filename, location.line, location.column
            ));
        }
        out.push_str(": ");
        out.push_str(&self.message);

        if let (Some(location), Some(source_line)) = (&self.location, &self.source_line) {
            out.push('\n');
            out.push_str(&format!("{:>4} | {source_line}", location.line));
            out.push('\n');
            out.push_str("     | ");
            out.push_str(&" ".repeat(location.column.saturating_sub(1)));
            let available = source_line
                .chars()
                .count()
                .saturating_sub(location.column.saturating_sub(1))
                .max(1);
            out.push_str(&"^".repeat(location.length.min(available)));
        }
        for note in &self.notes {
            out.push_str("\n  note: ");
            out.push_str(note);
        }
        out
    }
}

impl fmt::Display for NiloError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.render())
    }
}

impl std::error::Error for NiloError {}

#[derive(Clone)]
pub struct SourceContext {
    pub filename: String,
    pub source: Rc<String>,
    pub directory: PathBuf,
}

impl SourceContext {
    #[must_use]
    pub fn new(filename: impl Into<String>, source: impl Into<String>, directory: PathBuf) -> Self {
        Self {
            filename: filename.into(),
            source: Rc::new(source.into()),
            directory,
        }
    }

    #[must_use]
    pub fn synthetic(filename: impl Into<String>, directory: PathBuf) -> Self {
        Self::new(filename, "", directory)
    }
}
