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
