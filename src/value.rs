use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;
use std::rc::Rc;

use regex::Regex;

use crate::ast::{Block, Field, Parameter, TypeRef};
use crate::env::EnvRef;
use crate::error::SourceContext;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum MapKey {
    String(String),
    Int(i64),
    Bool(bool),
}

impl MapKey {
    #[must_use]
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::String(_) => "str",
            Self::Int(_) => "int",
            Self::Bool(_) => "bool",
        }
    }

    #[must_use]
    pub fn display(&self) -> String {
        match self {
            Self::String(value) => value.clone(),
            Self::Int(value) => value.to_string(),
            Self::Bool(value) => value.to_string(),
        }
    }
}

#[derive(Clone)]
pub struct ListValue {
    pub items: Vec<Value>,
    pub element_type: Option<TypeRef>,
}

#[derive(Clone)]
pub struct MapValue {
    pub entries: BTreeMap<MapKey, Value>,
    pub key_type: Option<TypeRef>,
    pub value_type: Option<TypeRef>,
}

#[derive(Clone)]
pub struct UserFunction {
    pub name: String,
    pub params: Vec<Parameter>,
    pub return_type: Option<TypeRef>,
    pub body: Block,
    pub closure: EnvRef,
    pub context: SourceContext,
}

#[derive(Clone)]
pub struct StructDef {
    pub name: String,
    pub fields: Vec<Field>,
}

#[derive(Clone)]
pub struct RecordValue {
    pub definition: Rc<StructDef>,
    pub fields: BTreeMap<String, Value>,
}

#[derive(Clone)]
pub struct ModuleValue {
    pub name: String,
    pub path: Option<PathBuf>,
    pub exports: RefCell<BTreeMap<String, Value>>,
}

#[derive(Clone)]
pub struct RegexValue {
    pub pattern: String,
    pub flags: i64,
    pub compiled: Regex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeId {
    Print,
    Len,
    Push,
    Pop,
    Str,
    Int,
    Float,
    Bool,
    Range,
    Assert,
    TypeOf,
    Keys,
    Values,
    Clock,
    JsonParse,
    JsonStringify,
    RegexCompile,
    RegexIsMatch,
    RegexFind,
    RegexFindAll,
    RegexCaptures,
    RegexReplace,
    RegexSplit,
    RegexEscape,
    FsReadText,
    FsWriteText,
    FsExists,
    FsListDir,
    FsRemove,
    TimeNow,
    TimeSleep,
    HttpGet,
    HttpPost,
    ListPush,
    ListPop,
    ListJoin,
    ListReverse,
    ListSort,
    StringSplit,
    StringTrim,
    StringLower,
    StringUpper,
    StringContains,
    StringReplace,
    MathAbs,
    MathMin,
    MathMax,
    MathRound,
    MathFloor,
    MathCeil,
    MathPow,
    MathSqrt,
}

#[derive(Debug, Clone, Copy)]
pub struct NativeFunction {
    pub name: &'static str,
    pub id: NativeId,
    pub min_arity: usize,
    pub max_arity: Option<usize>,
}

impl NativeFunction {
    #[must_use]
    pub const fn exact(name: &'static str, id: NativeId, arity: usize) -> Self {
        Self {
            name,
            id,
            min_arity: arity,
            max_arity: Some(arity),
        }
    }

    #[must_use]
    pub const fn range(
        name: &'static str,
        id: NativeId,
        min_arity: usize,
        max_arity: usize,
    ) -> Self {
        Self {
            name,
            id,
            min_arity,
            max_arity: Some(max_arity),
        }
    }

    #[must_use]
    pub const fn variadic(name: &'static str, id: NativeId, min_arity: usize) -> Self {
        Self {
            name,
            id,
            min_arity,
            max_arity: None,
        }
    }
}

#[derive(Clone)]
pub enum Value {
    Nil,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(Rc<String>),
    List(Rc<RefCell<ListValue>>),
    Map(Rc<RefCell<MapValue>>),
    Function(Rc<UserFunction>),
    Native(NativeFunction),
    StructType(Rc<StructDef>),
    Record(Rc<RefCell<RecordValue>>),
    Module(Rc<ModuleValue>),
    Pattern(Rc<RegexValue>),
}

impl Value {
    #[must_use]
    pub fn string(value: impl Into<String>) -> Self {
        Self::String(Rc::new(value.into()))
    }

    #[must_use]
    pub fn list(items: Vec<Self>) -> Self {
        Self::List(Rc::new(RefCell::new(ListValue {
            items,
            element_type: None,
        })))
    }

    #[must_use]
    pub fn map(entries: BTreeMap<MapKey, Self>) -> Self {
        Self::Map(Rc::new(RefCell::new(MapValue {
            entries,
            key_type: None,
            value_type: None,
        })))
    }

    #[must_use]
    pub fn empty_map() -> Self {
        Self::map(BTreeMap::new())
    }

    #[must_use]
    pub fn native(function: NativeFunction) -> Self {
        Self::Native(function)
    }

    #[must_use]
    pub fn type_name(&self) -> String {
        match self {
            Self::Nil => "nil".to_owned(),
            Self::Bool(_) => "bool".to_owned(),
            Self::Int(_) => "int".to_owned(),
            Self::Float(_) => "float".to_owned(),
            Self::String(_) => "str".to_owned(),
            Self::List(_) => "list".to_owned(),
            Self::Map(_) => "map".to_owned(),
            Self::Function(_) | Self::Native(_) | Self::Pattern(_) => "func".to_owned(),
            Self::StructType(_) => "type".to_owned(),
            Self::Record(record) => record.borrow().definition.name.clone(),
            Self::Module(_) => "module".to_owned(),
        }
    }

    #[must_use]
    pub fn is_callable(&self) -> bool {
        matches!(
            self,
            Self::Function(_) | Self::Native(_) | Self::StructType(_) | Self::Pattern(_)
        )
    }

    #[must_use]
    pub fn as_string(&self) -> Option<&str> {
        match self {
            Self::String(value) => Some(value.as_str()),
            _ => None,
        }
    }

    #[must_use]
    pub fn repr(&self) -> String {
        format_value(self, 0, true)
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&format_value(self, 0, false))
    }
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.repr())
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Nil, Self::Nil) => true,
            (Self::Bool(left), Self::Bool(right)) => left == right,
            (Self::Int(left), Self::Int(right)) => left == right,
            (Self::Float(left), Self::Float(right)) => left == right,
            (Self::Int(left), Self::Float(right)) => (*left as f64) == *right,
            (Self::Float(left), Self::Int(right)) => *left == (*right as f64),
            (Self::String(left), Self::String(right)) => left == right,
            (Self::List(left), Self::List(right)) => {
                if Rc::ptr_eq(left, right) {
                    true
                } else {
                    left.borrow().items == right.borrow().items
                }
            }
            (Self::Map(left), Self::Map(right)) => {
                if Rc::ptr_eq(left, right) {
                    true
                } else {
                    left.borrow().entries == right.borrow().entries
                }
            }
            (Self::Function(left), Self::Function(right)) => Rc::ptr_eq(left, right),
            (Self::Native(left), Self::Native(right)) => left.id == right.id,
            (Self::StructType(left), Self::StructType(right)) => Rc::ptr_eq(left, right),
            (Self::Record(left), Self::Record(right)) => {
                if Rc::ptr_eq(left, right) {
                    true
                } else {
                    let left = left.borrow();
                    let right = right.borrow();
                    left.definition.name == right.definition.name && left.fields == right.fields
                }
            }
            (Self::Module(left), Self::Module(right)) => Rc::ptr_eq(left, right),
            (Self::Pattern(left), Self::Pattern(right)) => {
                left.pattern == right.pattern && left.flags == right.flags
            }
            _ => false,
        }
    }
}

impl PartialOrd for Value {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match (self, other) {
            (Self::Int(left), Self::Int(right)) => left.partial_cmp(right),
            (Self::Float(left), Self::Float(right)) => left.partial_cmp(right),
            (Self::Int(left), Self::Float(right)) => (*left as f64).partial_cmp(right),
            (Self::Float(left), Self::Int(right)) => left.partial_cmp(&(*right as f64)),
            (Self::String(left), Self::String(right)) => left.partial_cmp(right),
            (Self::Bool(left), Self::Bool(right)) => left.partial_cmp(right),
            _ => None,
        }
    }
}

fn format_value(value: &Value, depth: usize, quoted_strings: bool) -> String {
    if depth > 5 {
        return "…".to_owned();
    }
    match value {
        Value::Nil => "nil".to_owned(),
        Value::Bool(value) => value.to_string(),
        Value::Int(value) => value.to_string(),
        Value::Float(value) => {
            if value.fract() == 0.0 {
                format!("{value:.1}")
            } else {
                value.to_string()
            }
        }
        Value::String(value) if quoted_strings => format!("{value:?}"),
        Value::String(value) => value.as_str().to_owned(),
        Value::List(list) => {
            let list = list.borrow();
            format!(
                "[{}]",
                list.items
                    .iter()
                    .map(|item| format_value(item, depth + 1, true))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
        Value::Map(map) => {
            let map = map.borrow();
            format!(
                "{{{}}}",
                map.entries
                    .iter()
                    .map(|(key, value)| format!(
                        "{}: {}",
                        match key {
                            MapKey::String(value) => format!("{value:?}"),
                            _ => key.display(),
                        },
                        format_value(value, depth + 1, true)
                    ))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
        Value::Function(function) => format!("<func {}>", function.name),
        Value::Native(function) => format!("<native {}>", function.name),
        Value::StructType(definition) => format!("<type {}>", definition.name),
        Value::Record(record) => {
            let record = record.borrow();
            format!(
                "{}({})",
                record.definition.name,
                record
                    .fields
                    .iter()
                    .map(|(name, value)| format!(
                        "{name}: {}",
                        format_value(value, depth + 1, true)
                    ))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
        Value::Module(module) => format!("<module {}>", module.name),
        Value::Pattern(pattern) => format!("<regex {:?}>", pattern.pattern),
    }
}
