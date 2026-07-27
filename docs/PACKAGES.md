# Nilo Packages

A Nilo package is a directory containing `Nilo.toml`. The current format is intentionally small and stable enough for local projects and Git repositories.

## Manifest

```toml
[package]
name = "my-package"
version = "0.1.0"
description = "Reusable Nilo utilities"
entry = "src/main.nilo"

[exports]
math = "src/math.nilo"
strings = "src/strings.nilo"
```

`nilo run` searches for `Nilo.toml` from the current directory upward and executes `package.entry`. The `[exports]` table documents public modules for tooling and a future package manager; file imports work directly today.

## Recommended layout

```text
my-package/
├── Nilo.toml
├── README.md
├── src/
│   ├── main.nilo
│   └── math.nilo
└── tests/
    └── math_test.nilo
```

## Commands

```bash
nilo init my-package
cd my-package
nilo run
nilo test
```

The test runner discovers files ending in `_test.nilo` recursively. Tests should use the built-in `assert` function; an assertion failure produces a non-zero process exit.

## Distribution

Until a registry and lockfile format are defined, distribute Nilo packages as versioned Git repositories or directories. Tag public releases using semantic versions and keep exported modules listed in `Nilo.toml`.
