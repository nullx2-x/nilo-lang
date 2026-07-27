from __future__ import annotations

from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    file = Path(path)
    text = file.read_text(encoding="utf-8")
    if old in text:
        file.write_text(text.replace(old, new, 1), encoding="utf-8")
        print(f"patched {path}")
        return
    if new in text:
        print(f"already patched {path}")
        return
    raise SystemExit(f"expected text was not found in {path}: {old!r}")


# Compiler compatibility fixes found by the first Rust build.
replace_once(
    "src/interpreter.rs",
    ".map(Duration::as_secs_f64)",
    ".map(|duration| duration.as_secs_f64())",
)
replace_once(
    "src/ast.rs",
    "pub const fn merge(self, other: Self) -> Self",
    "pub fn merge(self, other: Self) -> Self",
)

# Nilo errors deliberately retain rich source spans, snippets, and notes. Boxing
# them would make every public Result harder to use for little practical gain.
replace_once(
    "src/lib.rs",
    "//! Nilo language implementation.",
    "#![allow(clippy::result_large_err)]\n\n//! Nilo language implementation.",
)

# Apply the actionable Clippy suggestions without weakening the lint policy.
replace_once(
    "src/cli.rs",
    'value if path == PathBuf::from(".") => path = PathBuf::from(value),',
    'value if path == Path::new(".") => path = PathBuf::from(value),',
)
replace_once(
    "src/env.rs",
    """        if values.contains_key(&name) {
            false
        } else {
            values.insert(name, binding);
            true
        }
""",
    """        if let std::collections::hash_map::Entry::Vacant(entry) = values.entry(name) {
            entry.insert(binding);
            true
        } else {
            false
        }
""",
)
replace_once(
    "src/interpreter.rs",
    """                        return Err(NiloError::type_error(format!(
                            "map already has an incompatible type annotation"
                        )));
""",
    """                        return Err(NiloError::type_error(
                            "map already has an incompatible type annotation",
                        ));
""",
)
replace_once(
    "src/parser.rs",
    "return self.from_import_statement(self.previous().span);",
    "return self.parse_from_import_statement(self.previous().span);",
)
replace_once(
    "src/parser.rs",
    "fn from_import_statement(&mut self, start: Span) -> Result<Stmt>",
    "fn parse_from_import_statement(&mut self, start: Span) -> Result<Stmt>",
)
replace_once(
    "src/parser.rs",
    """            if !self.matches(TokenKind::Comma) && !self.matches(TokenKind::Semicolon) {
                if !self.check(TokenKind::RightBrace) {
                    return Err(self.error(self.current(), "expected ',', ';', or '}' after field"));
                }
            }
""",
    """            if !self.matches(TokenKind::Comma)
                && !self.matches(TokenKind::Semicolon)
                && !self.check(TokenKind::RightBrace)
            {
                return Err(self.error(self.current(), "expected ',', ';', or '}' after field"));
            }
""",
)
