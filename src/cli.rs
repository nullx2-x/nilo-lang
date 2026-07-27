use std::env;
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::{NiloError, Result};
use crate::interpreter::Interpreter;
use crate::runtime::{parse_source, tokenize_source};

const HELP: &str = r#"Nilo — a small, readable programming language

USAGE:
    nilo                         Start the interactive REPL
    nilo <file.nilo>             Run a source file
    nilo run [file.nilo]         Run a source file or Nilo.toml entry
    nilo eval <source>           Evaluate source text
    nilo -e <source>             Evaluate source text
    nilo repl                    Start the interactive REPL
    nilo check <file.nilo>       Validate syntax without executing
    nilo test [path]             Run every *_test.nilo file
    nilo init [path] [--name N]  Create a new Nilo project
    nilo tokens <file.nilo>      Print lexer tokens as JSON
    nilo ast <file.nilo>         Print the AST as JSON
    nilo --version               Print the Nilo version
    nilo --help                  Print this help
"#;

#[derive(Debug, Deserialize)]
struct Manifest {
    package: ManifestPackage,
}

#[derive(Debug, Deserialize)]
struct ManifestPackage {
    name: Option<String>,
    entry: String,
}

pub fn run() -> i32 {
    match execute(env::args().skip(1).collect()) {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("{}", error.render());
            1
        }
    }
}

fn execute(args: Vec<String>) -> Result<()> {
    if args.is_empty() {
        return repl();
    }
    match args[0].as_str() {
        "-h" | "--help" | "help" => {
            print!("{HELP}");
            Ok(())
        }
        "-V" | "--version" | "version" => {
            println!("nilo {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        "run" => {
            ensure_arg_count(&args, 1, 2, "nilo run [file.nilo]")?;
            run_program(args.get(1).map(PathBuf::from))
        }
        "eval" | "-e" | "--eval" => {
            ensure_arg_count(&args, 2, 2, "nilo eval <source>")?;
            eval_source(&args[1])
        }
        "repl" => {
            ensure_arg_count(&args, 1, 1, "nilo repl")?;
            repl()
        }
        "check" => {
            ensure_arg_count(&args, 2, 2, "nilo check <file.nilo>")?;
            check_file(Path::new(&args[1]))
        }
        "test" => {
            ensure_arg_count(&args, 1, 2, "nilo test [path]")?;
            run_tests(args.get(1).map(PathBuf::from))
        }
        "init" => init_command(&args[1..]),
        "tokens" => {
            ensure_arg_count(&args, 2, 2, "nilo tokens <file.nilo>")?;
            print_tokens(Path::new(&args[1]))
        }
        "ast" => {
            ensure_arg_count(&args, 2, 2, "nilo ast <file.nilo>")?;
            print_ast(Path::new(&args[1]))
        }
        command if command.starts_with('-') => Err(NiloError::cli(format!(
            "unknown option '{command}'\n\n{HELP}"
        ))),
        file => {
            ensure_arg_count(&args, 1, 1, "nilo <file.nilo>")?;
            run_program(Some(PathBuf::from(file)))
        }
    }
}

fn ensure_arg_count(args: &[String], minimum: usize, maximum: usize, usage: &str) -> Result<()> {
    if (minimum..=maximum).contains(&args.len()) {
        Ok(())
    } else {
        Err(NiloError::cli(format!("usage: {usage}")))
    }
}

fn run_program(file: Option<PathBuf>) -> Result<()> {
    let file = match file {
        Some(path) if path.is_dir() => manifest_entry(&path)?,
        Some(path) => path,
        None => manifest_entry(&env::current_dir().map_err(|error| NiloError::io(".", error))?)?,
    };
    let file = file
        .canonicalize()
        .map_err(|error| NiloError::io(&file, error))?;
    let root = file.parent().unwrap_or(Path::new(".")).to_path_buf();
    let mut interpreter = Interpreter::new(root, io::stdout());
    interpreter.run_file(file)?;
    Ok(())
}

fn eval_source(source: &str) -> Result<()> {
    let root = env::current_dir().map_err(|error| NiloError::io(".", error))?;
    let mut interpreter = Interpreter::new(root, io::stdout());
    interpreter.run_source(normalize_repl_source(source), "<eval>")?;
    Ok(())
}

fn repl() -> Result<()> {
    let root = env::current_dir().map_err(|error| NiloError::io(".", error))?;
    let mut interpreter = Interpreter::new(root, io::stdout());
    let stdin = io::stdin();
    let mut input = stdin.lock();
    let mut buffer = String::new();

    println!(
        "Nilo {} — :help for commands, Ctrl-D to exit",
        env!("CARGO_PKG_VERSION")
    );
    loop {
        if buffer.is_empty() {
            print!("nilo> ");
        } else {
            print!("....> ");
        }
        io::stdout()
            .flush()
            .map_err(|error| NiloError::cli(format!("failed to flush prompt: {error}")))?;

        let mut line = String::new();
        let count = input
            .read_line(&mut line)
            .map_err(|error| NiloError::cli(format!("failed to read input: {error}")))?;
        if count == 0 {
            println!();
            return Ok(());
        }

        let trimmed = line.trim();
        if buffer.is_empty() && trimmed.starts_with(':') {
            match trimmed {
                ":quit" | ":q" | ":exit" => return Ok(()),
                ":help" => {
                    println!(":help        show this help");
                    println!(":reset       clear user-defined REPL state");
                    println!(":quit        exit the REPL");
                }
                ":reset" => {
                    interpreter.reset_session();
                    println!("state reset");
                }
                other => eprintln!("unknown REPL command: {other}"),
            }
            continue;
        }

        buffer.push_str(&line);
        if source_is_incomplete(&buffer) {
            continue;
        }

        let source = normalize_repl_source(&buffer);
        if let Err(error) = interpreter.run_source(source, "<repl>") {
            eprintln!("{}", error.render());
        }
        buffer.clear();
    }
}

fn check_file(path: &Path) -> Result<()> {
    let source = fs::read_to_string(path).map_err(|error| NiloError::io(path, error))?;
    parse_source(&source, &path.to_string_lossy())?;
    println!("ok {}", path.display());
    Ok(())
}

fn run_tests(path: Option<PathBuf>) -> Result<()> {
    let path = match path {
        Some(path) => path,
        None => {
            let cwd = env::current_dir().map_err(|error| NiloError::io(".", error))?;
            find_manifest(&cwd)
                .and_then(|manifest| manifest.parent().map(|parent| parent.join("tests")))
                .unwrap_or_else(|| cwd.join("tests"))
        }
    };

    let mut files = Vec::new();
    collect_test_files(&path, &mut files)?;
    files.sort();
    if files.is_empty() {
        return Err(NiloError::cli(format!(
            "no *_test.nilo files found under {}",
            path.display()
        )));
    }

    let mut passed = 0usize;
    let mut failed = 0usize;
    for file in &files {
        let root = file.parent().unwrap_or(Path::new(".")).to_path_buf();
        let mut interpreter = Interpreter::new(root, io::stdout());
        match interpreter.run_file(file) {
            Ok(_) => {
                passed += 1;
                println!("ok   {}", file.display());
            }
            Err(error) => {
                failed += 1;
                eprintln!("fail {}\n{}", file.display(), error.render());
            }
        }
    }
    println!("{passed} passed, {failed} failed");
    if failed == 0 {
        Ok(())
    } else {
        Err(NiloError::cli(format!("{failed} Nilo test file(s) failed")))
    }
}

fn collect_test_files(path: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    if path.is_file() {
        if is_test_file(path) {
            files.push(path.to_path_buf());
        }
        return Ok(());
    }
    let entries = fs::read_dir(path).map_err(|error| NiloError::io(path, error))?;
    for entry in entries {
        let entry = entry.map_err(|error| NiloError::io(path, error))?;
        let entry_path = entry.path();
        if entry_path.is_dir() {
            collect_test_files(&entry_path, files)?;
        } else if is_test_file(&entry_path) {
            files.push(entry_path);
        }
    }
    Ok(())
}

fn is_test_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with("_test.nilo"))
}

fn init_command(args: &[String]) -> Result<()> {
    let mut path = PathBuf::from(".");
    let mut name = None;
    let mut index = 0usize;
    while index < args.len() {
        match args[index].as_str() {
            "--name" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| NiloError::cli("--name requires a project name"))?;
                name = Some(value.clone());
            }
            option if option.starts_with('-') => {
                return Err(NiloError::cli(format!("unknown init option '{option}'")));
            }
            value if path == PathBuf::from(".") => path = PathBuf::from(value),
            _ => return Err(NiloError::cli("usage: nilo init [path] [--name NAME]")),
        }
        index += 1;
    }
    init_project(&path, name.as_deref())
}

fn init_project(path: &Path, requested_name: Option<&str>) -> Result<()> {
    fs::create_dir_all(path.join("src")).map_err(|error| NiloError::io(path, error))?;
    fs::create_dir_all(path.join("tests")).map_err(|error| NiloError::io(path, error))?;
    let default_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty() && *name != ".")
        .unwrap_or("my-nilo-app");
    let name = requested_name.unwrap_or(default_name);

    write_if_missing(
        &path.join("Nilo.toml"),
        &format!(
            "[package]\nname = {}\nversion = \"0.1.0\"\nentry = \"src/main.nilo\"\n\n[exports]\nmain = \"src/main.nilo\"\n",
            toml_string(name)
        ),
    )?;
    write_if_missing(
        &path.join("src/main.nilo"),
        "func greet(name: str) -> str {\n    return \"Hello, \" + name + \"!\";\n}\n\nprint(greet(\"Nilo\"));\n",
    )?;
    write_if_missing(
        &path.join("tests/main_test.nilo"),
        "assert(1 + 1 == 2, \"arithmetic should work\");\nprint(\"test ok\");\n",
    )?;
    write_if_missing(&path.join(".gitignore"), "/.nilo-cache\n")?;
    println!("initialized Nilo project at {}", path.display());
    Ok(())
}

fn write_if_missing(path: &Path, content: &str) -> Result<()> {
    if path.exists() {
        return Ok(());
    }
    fs::write(path, content).map_err(|error| NiloError::io(path, error))
}

fn print_tokens(path: &Path) -> Result<()> {
    let source = fs::read_to_string(path).map_err(|error| NiloError::io(path, error))?;
    let tokens = tokenize_source(&source, &path.to_string_lossy())?;
    let json = serde_json::to_string_pretty(&tokens)
        .map_err(|error| NiloError::cli(format!("failed to encode tokens: {error}")))?;
    println!("{json}");
    Ok(())
}

fn print_ast(path: &Path) -> Result<()> {
    let source = fs::read_to_string(path).map_err(|error| NiloError::io(path, error))?;
    let program = parse_source(&source, &path.to_string_lossy())?;
    let json = serde_json::to_string_pretty(&program)
        .map_err(|error| NiloError::cli(format!("failed to encode AST: {error}")))?;
    println!("{json}");
    Ok(())
}

fn manifest_entry(start: &Path) -> Result<PathBuf> {
    let manifest_path = find_manifest(start).ok_or_else(|| {
        NiloError::cli(format!(
            "no Nilo.toml found from {} upward; pass a .nilo file explicitly",
            start.display()
        ))
    })?;
    let text =
        fs::read_to_string(&manifest_path).map_err(|error| NiloError::io(&manifest_path, error))?;
    let manifest: Manifest = toml::from_str(&text)
        .map_err(|error| NiloError::cli(format!("invalid {}: {error}", manifest_path.display())))?;
    let root = manifest_path.parent().unwrap_or(Path::new("."));
    let entry = root.join(&manifest.package.entry);
    if !entry.is_file() {
        let package = manifest.package.name.as_deref().unwrap_or("package");
        return Err(NiloError::cli(format!(
            "entry file for {package:?} does not exist: {}",
            entry.display()
        )));
    }
    Ok(entry)
}

fn find_manifest(start: &Path) -> Option<PathBuf> {
    let mut current = if start.is_file() {
        start.parent()?.to_path_buf()
    } else {
        start.to_path_buf()
    };
    loop {
        let candidate = current.join("Nilo.toml");
        if candidate.is_file() {
            return Some(candidate);
        }
        if !current.pop() {
            return None;
        }
    }
}

fn source_is_incomplete(source: &str) -> bool {
    let mut braces = 0i64;
    let mut brackets = 0i64;
    let mut parentheses = 0i64;
    let mut in_string = false;
    let mut escaped = false;
    let mut chars = source.chars().peekable();
    while let Some(character) = chars.next() {
        if in_string {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' {
                in_string = false;
            }
            continue;
        }
        if character == '"' {
            in_string = true;
            continue;
        }
        if character == '/' && chars.peek() == Some(&'/') {
            for character in chars.by_ref() {
                if character == '\n' {
                    break;
                }
            }
            continue;
        }
        match character {
            '{' => braces += 1,
            '}' => braces -= 1,
            '[' => brackets += 1,
            ']' => brackets -= 1,
            '(' => parentheses += 1,
            ')' => parentheses -= 1,
            _ => {}
        }
    }
    in_string || braces > 0 || brackets > 0 || parentheses > 0
}

fn normalize_repl_source(source: &str) -> String {
    let trimmed = source.trim_end();
    if trimmed.is_empty() || trimmed.ends_with(';') || trimmed.ends_with('}') {
        source.to_owned()
    } else {
        format!("{source};")
    }
}

fn toml_string(value: &str) -> String {
    format!(
        "\"{}\"",
        value
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
    )
}

#[cfg(test)]
mod tests {
    use super::{normalize_repl_source, source_is_incomplete};

    #[test]
    fn tracks_multiline_repl_input() {
        assert!(source_is_incomplete("if (true) {\n"));
        assert!(!source_is_incomplete("if (true) { print(1); }\n"));
        assert_eq!(normalize_repl_source("print(1)"), "print(1);");
    }
}
