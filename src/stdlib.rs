use std::collections::BTreeMap;

use crate::env::{Binding, EnvRef};
use crate::value::{MapKey, NativeFunction, NativeId, Value};

pub fn install_globals(env: &EnvRef) {
    for (name, value) in [
        (
            "print",
            native(NativeFunction::variadic("print", NativeId::Print, 0)),
        ),
        (
            "len",
            native(NativeFunction::exact("len", NativeId::Len, 1)),
        ),
        (
            "push",
            native(NativeFunction::exact("push", NativeId::Push, 2)),
        ),
        (
            "pop",
            native(NativeFunction::exact("pop", NativeId::Pop, 1)),
        ),
        (
            "str",
            native(NativeFunction::exact("str", NativeId::Str, 1)),
        ),
        (
            "int",
            native(NativeFunction::exact("int", NativeId::Int, 1)),
        ),
        (
            "float",
            native(NativeFunction::exact("float", NativeId::Float, 1)),
        ),
        (
            "bool",
            native(NativeFunction::exact("bool", NativeId::Bool, 1)),
        ),
        (
            "range",
            native(NativeFunction::range("range", NativeId::Range, 1, 3)),
        ),
        (
            "assert",
            native(NativeFunction::range("assert", NativeId::Assert, 1, 2)),
        ),
        (
            "type_of",
            native(NativeFunction::exact("type_of", NativeId::TypeOf, 1)),
        ),
        (
            "keys",
            native(NativeFunction::exact("keys", NativeId::Keys, 1)),
        ),
        (
            "values",
            native(NativeFunction::exact("values", NativeId::Values, 1)),
        ),
        (
            "clock",
            native(NativeFunction::exact("clock", NativeId::Clock, 0)),
        ),
    ] {
        env.define_or_replace(name, Binding { value, ty: None });
    }
}

#[must_use]
pub fn module_exports(path: &str) -> Option<BTreeMap<String, Value>> {
    let exports = match path {
        "std/json" => pairs(&[
            (
                "parse",
                NativeFunction::exact("json.parse", NativeId::JsonParse, 1),
            ),
            (
                "stringify",
                NativeFunction::range("json.stringify", NativeId::JsonStringify, 1, 2),
            ),
        ]),
        "std/regex" => {
            let mut exports = pairs(&[
                (
                    "compile",
                    NativeFunction::range("regex.compile", NativeId::RegexCompile, 1, 2),
                ),
                (
                    "is_match",
                    NativeFunction::range("regex.is_match", NativeId::RegexIsMatch, 2, 3),
                ),
                (
                    "find",
                    NativeFunction::range("regex.find", NativeId::RegexFind, 2, 3),
                ),
                (
                    "find_all",
                    NativeFunction::range("regex.find_all", NativeId::RegexFindAll, 2, 3),
                ),
                (
                    "captures",
                    NativeFunction::range("regex.captures", NativeId::RegexCaptures, 2, 3),
                ),
                (
                    "replace",
                    NativeFunction::range("regex.replace", NativeId::RegexReplace, 3, 4),
                ),
                (
                    "split",
                    NativeFunction::range("regex.split", NativeId::RegexSplit, 2, 3),
                ),
                (
                    "escape",
                    NativeFunction::exact("regex.escape", NativeId::RegexEscape, 1),
                ),
            ]);
            let mut flags = BTreeMap::new();
            flags.insert(MapKey::String("ignore_case".to_owned()), Value::Int(1));
            flags.insert(MapKey::String("multiline".to_owned()), Value::Int(2));
            flags.insert(MapKey::String("dot_all".to_owned()), Value::Int(4));
            flags.insert(MapKey::String("verbose".to_owned()), Value::Int(8));
            flags.insert(MapKey::String("ascii".to_owned()), Value::Int(16));
            exports.insert("flags".to_owned(), Value::map(flags));
            exports
        }
        "std/fs" => pairs(&[
            (
                "read_text",
                NativeFunction::exact("fs.read_text", NativeId::FsReadText, 1),
            ),
            (
                "write_text",
                NativeFunction::exact("fs.write_text", NativeId::FsWriteText, 2),
            ),
            (
                "exists",
                NativeFunction::exact("fs.exists", NativeId::FsExists, 1),
            ),
            (
                "list_dir",
                NativeFunction::exact("fs.list_dir", NativeId::FsListDir, 1),
            ),
            (
                "remove",
                NativeFunction::exact("fs.remove", NativeId::FsRemove, 1),
            ),
        ]),
        "std/time" => pairs(&[
            (
                "now",
                NativeFunction::exact("time.now", NativeId::TimeNow, 0),
            ),
            (
                "sleep",
                NativeFunction::exact("time.sleep", NativeId::TimeSleep, 1),
            ),
        ]),
        "std/http" => pairs(&[
            (
                "get",
                NativeFunction::range("http.get", NativeId::HttpGet, 1, 2),
            ),
            (
                "post",
                NativeFunction::range("http.post", NativeId::HttpPost, 2, 3),
            ),
        ]),
        "std/list" => pairs(&[
            (
                "push",
                NativeFunction::exact("list.push", NativeId::ListPush, 2),
            ),
            (
                "pop",
                NativeFunction::exact("list.pop", NativeId::ListPop, 1),
            ),
            (
                "join",
                NativeFunction::exact("list.join", NativeId::ListJoin, 2),
            ),
            (
                "reverse",
                NativeFunction::exact("list.reverse", NativeId::ListReverse, 1),
            ),
            (
                "sort",
                NativeFunction::exact("list.sort", NativeId::ListSort, 1),
            ),
        ]),
        "std/string" => pairs(&[
            (
                "split",
                NativeFunction::exact("string.split", NativeId::StringSplit, 2),
            ),
            (
                "trim",
                NativeFunction::exact("string.trim", NativeId::StringTrim, 1),
            ),
            (
                "lower",
                NativeFunction::exact("string.lower", NativeId::StringLower, 1),
            ),
            (
                "upper",
                NativeFunction::exact("string.upper", NativeId::StringUpper, 1),
            ),
            (
                "contains",
                NativeFunction::exact("string.contains", NativeId::StringContains, 2),
            ),
            (
                "replace",
                NativeFunction::exact("string.replace", NativeId::StringReplace, 3),
            ),
        ]),
        "std/math" => pairs(&[
            (
                "abs",
                NativeFunction::exact("math.abs", NativeId::MathAbs, 1),
            ),
            (
                "min",
                NativeFunction::variadic("math.min", NativeId::MathMin, 1),
            ),
            (
                "max",
                NativeFunction::variadic("math.max", NativeId::MathMax, 1),
            ),
            (
                "round",
                NativeFunction::exact("math.round", NativeId::MathRound, 1),
            ),
            (
                "floor",
                NativeFunction::exact("math.floor", NativeId::MathFloor, 1),
            ),
            (
                "ceil",
                NativeFunction::exact("math.ceil", NativeId::MathCeil, 1),
            ),
            (
                "pow",
                NativeFunction::exact("math.pow", NativeId::MathPow, 2),
            ),
            (
                "sqrt",
                NativeFunction::exact("math.sqrt", NativeId::MathSqrt, 1),
            ),
        ]),
        _ => return None,
    };
    Some(exports)
}

fn native(function: NativeFunction) -> Value {
    Value::native(function)
}

fn pairs(pairs: &[(&str, NativeFunction)]) -> BTreeMap<String, Value> {
    pairs
        .iter()
        .map(|(name, function)| ((*name).to_owned(), native(*function)))
        .collect()
}
