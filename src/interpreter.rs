use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use regex::{Regex, RegexBuilder};

use crate::ast::{AssignTarget, Expr, ExprKind, Literal, Program, Span, Stmt, TypeRef};
use crate::env::{assign, binding_type, lookup, Binding, EnvRef, Environment};
use crate::error::{NiloError, Result, SourceContext};
use crate::runtime::parse_source;
use crate::stdlib;
use crate::value::{
    ListValue, MapKey, MapValue, ModuleValue, NativeFunction, NativeId, RecordValue, RegexValue,
    StructDef, UserFunction, Value,
};

#[derive(Clone)]
enum Flow {
    Normal,
    Return(Value),
    Break,
    Continue,
}

pub struct Interpreter<W: Write> {
    root: PathBuf,
    output: W,
    globals: EnvRef,
    session: EnvRef,
    modules: HashMap<PathBuf, Rc<ModuleValue>>,
    standard_modules: HashMap<String, Rc<ModuleValue>>,
}

impl<W: Write> Interpreter<W> {
    #[must_use]
    pub fn new(root: PathBuf, output: W) -> Self {
        let root = root.canonicalize().unwrap_or(root);
        let globals = Environment::root();
        stdlib::install_globals(&globals);
        let session = Environment::child(&globals);
        Self {
            root,
            output,
            globals,
            session,
            modules: HashMap::new(),
            standard_modules: HashMap::new(),
        }
    }

    #[must_use]
    pub fn into_output(self) -> W {
        self.output
    }

    pub fn reset_session(&mut self) {
        self.session = Environment::child(&self.globals);
    }

    pub fn run_file(&mut self, path: impl AsRef<Path>) -> Result<BTreeMap<String, Value>> {
        let supplied = path.as_ref();
        let path = supplied
            .canonicalize()
            .map_err(|error| NiloError::io(supplied, error))?;
        let source = fs::read_to_string(&path).map_err(|error| NiloError::io(&path, error))?;
        let filename = path.to_string_lossy().into_owned();
        let program = parse_source(&source, &filename)?;
        let context = SourceContext::new(
            filename,
            source,
            path.parent().unwrap_or(Path::new(".")).to_path_buf(),
        );
        let env = Environment::child(&self.globals);
        self.execute_program(&program, env, &context)
    }

    pub fn run_source(
        &mut self,
        source: impl Into<String>,
        filename: impl Into<String>,
    ) -> Result<BTreeMap<String, Value>> {
        let source = source.into();
        let filename = filename.into();
        let program = parse_source(&source, &filename)?;
        let context = SourceContext::new(filename, source, self.root.clone());
        self.execute_program(&program, self.session.clone(), &context)
    }

    fn execute_program(
        &mut self,
        program: &Program,
        env: EnvRef,
        context: &SourceContext,
    ) -> Result<BTreeMap<String, Value>> {
        let mut exports = BTreeMap::new();
        for statement in &program.statements {
            match self.execute(statement, env.clone(), context)? {
                Flow::Normal => {}
                Flow::Return(_) => {
                    return Err(
                        NiloError::runtime("return escaped the current function").at(
                            &context.filename,
                            statement.span(),
                            Some(context.source.as_str()),
                        ),
                    );
                }
                Flow::Break | Flow::Continue => {
                    return Err(
                        NiloError::runtime("loop control escaped the current loop").at(
                            &context.filename,
                            statement.span(),
                            Some(context.source.as_str()),
                        ),
                    );
                }
            }
            if let Some(name) = statement.exported_name() {
                let binding = lookup(&env, name).ok_or_else(|| {
                    NiloError::runtime(format!("exported name '{name}' was not defined"))
                })?;
                exports.insert(name.to_owned(), binding.value);
            }
        }
        Ok(exports)
    }

    fn execute(&mut self, statement: &Stmt, env: EnvRef, context: &SourceContext) -> Result<Flow> {
        self.execute_inner(statement, env, context)
            .map_err(|error| {
                error.at_if_missing(
                    &context.filename,
                    statement.span(),
                    Some(context.source.as_str()),
                )
            })
    }

    fn execute_inner(
        &mut self,
        statement: &Stmt,
        env: EnvRef,
        context: &SourceContext,
    ) -> Result<Flow> {
        match statement {
            Stmt::Let {
                name, ty, value, ..
            } => {
                let value = self.evaluate(value, env.clone(), context)?;
                if let Some(ty) = ty {
                    self.ensure_type(&value, ty, format!("variable '{name}'"))?;
                    self.apply_type_metadata(&value, ty)?;
                }
                if !env.define(
                    name,
                    Binding {
                        value,
                        ty: ty.clone(),
                    },
                ) {
                    return Err(NiloError::runtime(format!(
                        "name '{name}' is already defined in this scope"
                    )));
                }
                Ok(Flow::Normal)
            }
            Stmt::Assign { target, value, .. } => {
                let value = self.evaluate(value, env.clone(), context)?;
                self.assign_target(target, value, env, context)?;
                Ok(Flow::Normal)
            }
            Stmt::Function {
                name,
                params,
                return_type,
                body,
                ..
            } => {
                let function = Value::Function(Rc::new(UserFunction {
                    name: name.clone(),
                    params: params.clone(),
                    return_type: return_type.clone(),
                    body: body.clone(),
                    closure: env.clone(),
                    context: context.clone(),
                }));
                if !env.define(
                    name,
                    Binding {
                        value: function,
                        ty: None,
                    },
                ) {
                    return Err(NiloError::runtime(format!(
                        "name '{name}' is already defined in this scope"
                    )));
                }
                Ok(Flow::Normal)
            }
            Stmt::TypeDecl { name, fields, .. } => {
                let definition = Value::StructType(Rc::new(StructDef {
                    name: name.clone(),
                    fields: fields.clone(),
                }));
                if !env.define(
                    name,
                    Binding {
                        value: definition,
                        ty: None,
                    },
                ) {
                    return Err(NiloError::runtime(format!(
                        "name '{name}' is already defined in this scope"
                    )));
                }
                Ok(Flow::Normal)
            }
            Stmt::Import { path, alias, .. } => {
                let module = self.load_module(path, context)?;
                let name = alias.clone().unwrap_or_else(|| module_name(path));
                if !env.define(
                    &name,
                    Binding {
                        value: Value::Module(module),
                        ty: None,
                    },
                ) {
                    return Err(NiloError::runtime(format!(
                        "name '{name}' is already defined in this scope"
                    )));
                }
                Ok(Flow::Normal)
            }
            Stmt::FromImport { path, names, .. } => {
                let module = self.load_module(path, context)?;
                let exports = module.exports.borrow();
                for name in names {
                    let value = exports.get(name).cloned().ok_or_else(|| {
                        NiloError::module(format!("module '{path}' does not export '{name}'"))
                    })?;
                    if !env.define(name, Binding { value, ty: None }) {
                        return Err(NiloError::runtime(format!(
                            "name '{name}' is already defined in this scope"
                        )));
                    }
                }
                Ok(Flow::Normal)
            }
            Stmt::Return { value, .. } => {
                let value = if let Some(expression) = value {
                    self.evaluate(expression, env, context)?
                } else {
                    Value::Nil
                };
                Ok(Flow::Return(value))
            }
            Stmt::If {
                condition,
                then_block,
                else_block,
                ..
            } => {
                let condition = self.evaluate(condition, env.clone(), context)?;
                if self.truthy(&condition) {
                    self.execute_block(then_block, Environment::child(&env), context)
                } else if let Some(else_block) = else_block {
                    self.execute_block(else_block, Environment::child(&env), context)
                } else {
                    Ok(Flow::Normal)
                }
            }
            Stmt::While {
                condition, body, ..
            } => {
                loop {
                    let condition_value = self.evaluate(condition, env.clone(), context)?;
                    if !self.truthy(&condition_value) {
                        break;
                    }
                    match self.execute_block(body, Environment::child(&env), context)? {
                        Flow::Normal | Flow::Continue => {}
                        Flow::Break => break,
                        Flow::Return(value) => return Ok(Flow::Return(value)),
                    }
                }
                Ok(Flow::Normal)
            }
            Stmt::For {
                name,
                iterable,
                body,
                ..
            } => {
                let iterable = self.evaluate(iterable, env.clone(), context)?;
                let values = self.iterable_values(&iterable)?;
                for value in values {
                    let loop_env = Environment::child(&env);
                    loop_env.define(name, Binding { value, ty: None });
                    match self.execute_block(body, loop_env, context)? {
                        Flow::Normal | Flow::Continue => {}
                        Flow::Break => break,
                        Flow::Return(value) => return Ok(Flow::Return(value)),
                    }
                }
                Ok(Flow::Normal)
            }
            Stmt::Break { .. } => Ok(Flow::Break),
            Stmt::Continue { .. } => Ok(Flow::Continue),
            Stmt::Expression { expression, .. } => {
                self.evaluate(expression, env, context)?;
                Ok(Flow::Normal)
            }
        }
    }

    fn execute_block(
        &mut self,
        statements: &[Stmt],
        env: EnvRef,
        context: &SourceContext,
    ) -> Result<Flow> {
        for statement in statements {
            let flow = self.execute(statement, env.clone(), context)?;
            if !matches!(flow, Flow::Normal) {
                return Ok(flow);
            }
        }
        Ok(Flow::Normal)
    }

    fn evaluate(
        &mut self,
        expression: &Expr,
        env: EnvRef,
        context: &SourceContext,
    ) -> Result<Value> {
        self.evaluate_inner(expression, env, context)
            .map_err(|error| {
                error.at_if_missing(
                    &context.filename,
                    expression.span,
                    Some(context.source.as_str()),
                )
            })
    }

    fn evaluate_inner(
        &mut self,
        expression: &Expr,
        env: EnvRef,
        context: &SourceContext,
    ) -> Result<Value> {
        match &expression.kind {
            ExprKind::Literal { value } => Ok(match value {
                Literal::Nil => Value::Nil,
                Literal::Bool(value) => Value::Bool(*value),
                Literal::Int(value) => Value::Int(*value),
                Literal::Float(value) => Value::Float(*value),
                Literal::String(value) => Value::string(value),
            }),
            ExprKind::Variable { name } => lookup(&env, name)
                .map(|binding| binding.value)
                .ok_or_else(|| NiloError::runtime(format!("undefined name '{name}'"))),
            ExprKind::List { values } => {
                let values = values
                    .iter()
                    .map(|value| self.evaluate(value, env.clone(), context))
                    .collect::<Result<Vec<_>>>()?;
                Ok(Value::list(values))
            }
            ExprKind::Map { entries } => {
                let mut values = BTreeMap::new();
                for (key, value) in entries {
                    let key_value = self.evaluate(key, env.clone(), context)?;
                    let key = self.map_key(&key_value)?;
                    let value = self.evaluate(value, env.clone(), context)?;
                    values.insert(key, value);
                }
                Ok(Value::map(values))
            }
            ExprKind::Unary { operator, right } => {
                let right = self.evaluate(right, env, context)?;
                match operator.as_str() {
                    "!" => Ok(Value::Bool(!self.truthy(&right))),
                    "-" => match right {
                        Value::Int(value) => value
                            .checked_neg()
                            .map(Value::Int)
                            .ok_or_else(|| NiloError::runtime("integer negation overflow")),
                        Value::Float(value) => Ok(Value::Float(-value)),
                        other => Err(NiloError::type_error(format!(
                            "unary '-' expects a number, got {}",
                            other.type_name()
                        ))),
                    },
                    _ => Err(NiloError::runtime(format!(
                        "unknown unary operator '{operator}'"
                    ))),
                }
            }
            ExprKind::Binary {
                left,
                operator,
                right,
            } => {
                if operator == "&&" {
                    let left = self.evaluate(left, env.clone(), context)?;
                    if !self.truthy(&left) {
                        return Ok(Value::Bool(false));
                    }
                    let right = self.evaluate(right, env, context)?;
                    return Ok(Value::Bool(self.truthy(&right)));
                }
                if operator == "||" {
                    let left = self.evaluate(left, env.clone(), context)?;
                    if self.truthy(&left) {
                        return Ok(Value::Bool(true));
                    }
                    let right = self.evaluate(right, env, context)?;
                    return Ok(Value::Bool(self.truthy(&right)));
                }
                let left = self.evaluate(left, env.clone(), context)?;
                let right = self.evaluate(right, env, context)?;
                self.binary(operator, left, right)
            }
            ExprKind::Call { callee, args } => {
                let callee = self.evaluate(callee, env.clone(), context)?;
                let args = args
                    .iter()
                    .map(|argument| self.evaluate(argument, env.clone(), context))
                    .collect::<Result<Vec<_>>>()?;
                self.call_value(callee, args, expression.span, context)
            }
            ExprKind::Get { object, name } => {
                let object = self.evaluate(object, env, context)?;
                self.get_property(&object, name)
            }
            ExprKind::Index { object, index } => {
                let object = self.evaluate(object, env.clone(), context)?;
                let index = self.evaluate(index, env, context)?;
                self.get_index(&object, &index)
            }
        }
    }

    fn assign_target(
        &mut self,
        target: &AssignTarget,
        value: Value,
        env: EnvRef,
        context: &SourceContext,
    ) -> Result<()> {
        match target {
            AssignTarget::Variable { name, .. } => {
                let ty = binding_type(&env, name)
                    .ok_or_else(|| NiloError::runtime(format!("undefined name '{name}'")))?;
                if let Some(ty) = &ty {
                    self.ensure_type(&value, ty, format!("variable '{name}'"))?;
                    self.apply_type_metadata(&value, ty)?;
                }
                if !assign(&env, name, value) {
                    return Err(NiloError::runtime(format!("undefined name '{name}'")));
                }
                Ok(())
            }
            AssignTarget::Property { object, name, .. } => {
                let object = self.evaluate(object, env, context)?;
                match object {
                    Value::Record(record) => {
                        let field_type = {
                            let record_ref = record.borrow();
                            record_ref
                                .definition
                                .fields
                                .iter()
                                .find(|field| field.name == *name)
                                .map(|field| field.ty.clone())
                                .ok_or_else(|| {
                                    NiloError::runtime(format!(
                                        "type '{}' has no field '{name}'",
                                        record_ref.definition.name
                                    ))
                                })?
                        };
                        if let Some(ty) = &field_type {
                            self.ensure_type(&value, ty, format!("field '{name}'"))?;
                            self.apply_type_metadata(&value, ty)?;
                        }
                        record.borrow_mut().fields.insert(name.clone(), value);
                        Ok(())
                    }
                    Value::Map(map) => {
                        let value_type = map.borrow().value_type.clone();
                        if let Some(ty) = &value_type {
                            self.ensure_type(&value, ty, format!("map property '{name}'"))?;
                            self.apply_type_metadata(&value, ty)?;
                        }
                        map.borrow_mut()
                            .entries
                            .insert(MapKey::String(name.clone()), value);
                        Ok(())
                    }
                    other => Err(NiloError::type_error(format!(
                        "cannot assign property '{name}' on {}",
                        other.type_name()
                    ))),
                }
            }
            AssignTarget::Index { object, index, .. } => {
                let object = self.evaluate(object, env.clone(), context)?;
                let index = self.evaluate(index, env, context)?;
                match object {
                    Value::List(list) => {
                        let len = list.borrow().items.len();
                        let index = self.list_index(&index, len)?;
                        let element_type = list.borrow().element_type.clone();
                        if let Some(ty) = &element_type {
                            self.ensure_type(&value, ty, "list element")?;
                            self.apply_type_metadata(&value, ty)?;
                        }
                        list.borrow_mut().items[index] = value;
                        Ok(())
                    }
                    Value::Map(map) => {
                        let key = self.map_key(&index)?;
                        let (key_type, value_type) = {
                            let map = map.borrow();
                            (map.key_type.clone(), map.value_type.clone())
                        };
                        if let Some(ty) = &key_type {
                            self.ensure_type(&index, ty, "map key")?;
                        }
                        if let Some(ty) = &value_type {
                            self.ensure_type(&value, ty, "map value")?;
                            self.apply_type_metadata(&value, ty)?;
                        }
                        map.borrow_mut().entries.insert(key, value);
                        Ok(())
                    }
                    other => Err(NiloError::type_error(format!(
                        "cannot assign by index on {}",
                        other.type_name()
                    ))),
                }
            }
        }
    }

    fn call_value(
        &mut self,
        callee: Value,
        args: Vec<Value>,
        _span: Span,
        context: &SourceContext,
    ) -> Result<Value> {
        match callee {
            Value::Native(function) => {
                self.check_arity(
                    function.name,
                    args.len(),
                    function.min_arity,
                    function.max_arity,
                )?;
                self.call_native(function, args, context)
            }
            Value::Function(function) => {
                self.check_arity(
                    &function.name,
                    args.len(),
                    function.params.len(),
                    Some(function.params.len()),
                )?;
                let env = Environment::child(&function.closure);
                for (parameter, value) in function.params.iter().zip(args) {
                    if let Some(ty) = &parameter.ty {
                        self.ensure_type(
                            &value,
                            ty,
                            format!("argument '{}' to {}", parameter.name, function.name),
                        )?;
                        self.apply_type_metadata(&value, ty)?;
                    }
                    env.define(
                        &parameter.name,
                        Binding {
                            value,
                            ty: parameter.ty.clone(),
                        },
                    );
                }
                let value = match self.execute_block(&function.body, env, &function.context)? {
                    Flow::Return(value) => value,
                    Flow::Normal => Value::Nil,
                    Flow::Break | Flow::Continue => {
                        return Err(NiloError::runtime(
                            "loop control escaped a function body unexpectedly",
                        ));
                    }
                };
                if let Some(ty) = &function.return_type {
                    self.ensure_type(&value, ty, format!("return value of {}", function.name))?;
                    self.apply_type_metadata(&value, ty)?;
                }
                Ok(value)
            }
            Value::StructType(definition) => {
                self.check_arity(
                    &definition.name,
                    args.len(),
                    definition.fields.len(),
                    Some(definition.fields.len()),
                )?;
                let mut fields = BTreeMap::new();
                for (field, value) in definition.fields.iter().zip(args) {
                    if let Some(ty) = &field.ty {
                        self.ensure_type(
                            &value,
                            ty,
                            format!("field '{}.{}'", definition.name, field.name),
                        )?;
                        self.apply_type_metadata(&value, ty)?;
                    }
                    fields.insert(field.name.clone(), value);
                }
                Ok(Value::Record(Rc::new(RefCell::new(RecordValue {
                    definition,
                    fields,
                }))))
            }
            Value::Pattern(pattern) => {
                self.check_arity("compiled regex", args.len(), 1, Some(1))?;
                let text = self.expect_string(&args[0], "compiled regex argument")?;
                Ok(Value::Bool(pattern.compiled.is_match(text)))
            }
            other => Err(NiloError::type_error(format!(
                "{} is not callable",
                other.type_name()
            ))),
        }
    }

    fn get_property(&self, object: &Value, name: &str) -> Result<Value> {
        match object {
            Value::Record(record) => record
                .borrow()
                .fields
                .get(name)
                .cloned()
                .ok_or_else(|| NiloError::runtime(format!("record has no field '{name}'"))),
            Value::Map(map) => map
                .borrow()
                .entries
                .get(&MapKey::String(name.to_owned()))
                .cloned()
                .ok_or_else(|| NiloError::runtime(format!("map has no key '{name}'"))),
            Value::Module(module) => module.exports.borrow().get(name).cloned().ok_or_else(|| {
                NiloError::module(format!("module '{}' does not export '{name}'", module.name))
            }),
            Value::List(list) if name == "length" => Ok(Value::Int(
                i64::try_from(list.borrow().items.len()).unwrap_or(i64::MAX),
            )),
            Value::String(value) if name == "length" => Ok(Value::Int(
                i64::try_from(value.chars().count()).unwrap_or(i64::MAX),
            )),
            Value::Pattern(pattern) if name == "pattern" => Ok(Value::string(&pattern.pattern)),
            other => Err(NiloError::type_error(format!(
                "{} has no property '{name}'",
                other.type_name()
            ))),
        }
    }

    fn get_index(&self, object: &Value, index: &Value) -> Result<Value> {
        match object {
            Value::List(list) => {
                let list = list.borrow();
                let index = self.list_index(index, list.items.len())?;
                Ok(list.items[index].clone())
            }
            Value::String(value) => {
                let chars = value.chars().collect::<Vec<_>>();
                let index = self.list_index(index, chars.len())?;
                Ok(Value::string(chars[index].to_string()))
            }
            Value::Map(map) => {
                let key = self.map_key(index)?;
                map.borrow().entries.get(&key).cloned().ok_or_else(|| {
                    NiloError::runtime(format!("map key {:?} was not found", key.display()))
                })
            }
            other => Err(NiloError::type_error(format!(
                "{} cannot be indexed",
                other.type_name()
            ))),
        }
    }

    fn binary(&self, operator: &str, left: Value, right: Value) -> Result<Value> {
        match operator {
            "==" => Ok(Value::Bool(left == right)),
            "!=" => Ok(Value::Bool(left != right)),
            "<" | "<=" | ">" | ">=" => {
                let ordering = left.partial_cmp(&right).ok_or_else(|| {
                    NiloError::type_error(format!(
                        "cannot compare {} and {}",
                        left.type_name(),
                        right.type_name()
                    ))
                })?;
                Ok(Value::Bool(match operator {
                    "<" => ordering == Ordering::Less,
                    "<=" => ordering != Ordering::Greater,
                    ">" => ordering == Ordering::Greater,
                    ">=" => ordering != Ordering::Less,
                    _ => unreachable!(),
                }))
            }
            "+" => self.add(left, right),
            "-" => self.numeric_binary(left, right, "-"),
            "*" => self.multiply(left, right),
            "/" => self.numeric_binary(left, right, "/"),
            "%" => self.numeric_binary(left, right, "%"),
            _ => Err(NiloError::runtime(format!(
                "unknown binary operator '{operator}'"
            ))),
        }
    }

    fn add(&self, left: Value, right: Value) -> Result<Value> {
        match (left, right) {
            (Value::Int(left), Value::Int(right)) => left
                .checked_add(right)
                .map(Value::Int)
                .ok_or_else(|| NiloError::runtime("integer addition overflow")),
            (Value::Int(left), Value::Float(right)) => Ok(Value::Float(left as f64 + right)),
            (Value::Float(left), Value::Int(right)) => Ok(Value::Float(left + right as f64)),
            (Value::Float(left), Value::Float(right)) => Ok(Value::Float(left + right)),
            (Value::String(left), Value::String(right)) => {
                Ok(Value::string(format!("{left}{right}")))
            }
            (Value::List(left), Value::List(right)) => {
                let mut items = left.borrow().items.clone();
                items.extend(right.borrow().items.clone());
                Ok(Value::list(items))
            }
            (left, right) => Err(NiloError::type_error(format!(
                "operator '+' does not support {} and {}",
                left.type_name(),
                right.type_name()
            ))),
        }
    }

    fn multiply(&self, left: Value, right: Value) -> Result<Value> {
        match (left, right) {
            (Value::String(value), Value::Int(count))
            | (Value::Int(count), Value::String(value)) => {
                if count < 0 {
                    return Err(NiloError::runtime("string repeat count cannot be negative"));
                }
                let count = usize::try_from(count)
                    .map_err(|_| NiloError::runtime("string repeat count is too large"))?;
                Ok(Value::string(value.repeat(count)))
            }
            (Value::List(value), Value::Int(count)) | (Value::Int(count), Value::List(value)) => {
                if count < 0 {
                    return Err(NiloError::runtime("list repeat count cannot be negative"));
                }
                let count = usize::try_from(count)
                    .map_err(|_| NiloError::runtime("list repeat count is too large"))?;
                let original = value.borrow().items.clone();
                let mut items = Vec::with_capacity(original.len().saturating_mul(count));
                for _ in 0..count {
                    items.extend(original.iter().cloned());
                }
                Ok(Value::list(items))
            }
            (left, right) => self.numeric_binary(left, right, "*"),
        }
    }

    fn numeric_binary(&self, left: Value, right: Value, operator: &str) -> Result<Value> {
        match (left, right) {
            (Value::Int(left), Value::Int(right)) if operator != "/" => match operator {
                "-" => left
                    .checked_sub(right)
                    .map(Value::Int)
                    .ok_or_else(|| NiloError::runtime("integer subtraction overflow")),
                "*" => left
                    .checked_mul(right)
                    .map(Value::Int)
                    .ok_or_else(|| NiloError::runtime("integer multiplication overflow")),
                "%" => {
                    if right == 0 {
                        Err(NiloError::runtime("division by zero"))
                    } else {
                        left.checked_rem(right)
                            .map(Value::Int)
                            .ok_or_else(|| NiloError::runtime("integer remainder overflow"))
                    }
                }
                _ => unreachable!(),
            },
            (left, right) => {
                let left = self.number(&left, "left operand")?;
                let right = self.number(&right, "right operand")?;
                if matches!(operator, "/" | "%") && right == 0.0 {
                    return Err(NiloError::runtime("division by zero"));
                }
                Ok(Value::Float(match operator {
                    "-" => left - right,
                    "*" => left * right,
                    "/" => left / right,
                    "%" => left % right,
                    _ => unreachable!(),
                }))
            }
        }
    }

    fn iterable_values(&self, value: &Value) -> Result<Vec<Value>> {
        match value {
            Value::List(list) => Ok(list.borrow().items.clone()),
            Value::Map(map) => Ok(map.borrow().entries.keys().map(map_key_value).collect()),
            Value::String(value) => Ok(value
                .chars()
                .map(|character| Value::string(character.to_string()))
                .collect()),
            other => Err(NiloError::type_error(format!(
                "{} is not iterable",
                other.type_name()
            ))),
        }
    }

    fn load_module(&mut self, path_text: &str, context: &SourceContext) -> Result<Rc<ModuleValue>> {
        if path_text.starts_with("std/") {
            return self.load_standard_module(path_text);
        }

        let mut path = PathBuf::from(path_text);
        if !path.is_absolute() {
            path = context.directory.join(path);
        }
        if path.extension().is_none() {
            path.set_extension("nilo");
        }
        let path = path.canonicalize().map_err(|error| {
            NiloError::module(format!("module '{path_text}' was not found: {error}"))
        })?;
        if let Some(module) = self.modules.get(&path) {
            return Ok(module.clone());
        }

        let source = fs::read_to_string(&path).map_err(|error| NiloError::io(&path, error))?;
        let filename = path.to_string_lossy().into_owned();
        let program = parse_source(&source, &filename)?;
        let module = Rc::new(ModuleValue {
            name: module_name(path_text),
            path: Some(path.clone()),
            exports: RefCell::new(BTreeMap::new()),
        });
        self.modules.insert(path.clone(), module.clone());
        let module_context = SourceContext::new(
            filename,
            source,
            path.parent().unwrap_or(Path::new(".")).to_path_buf(),
        );
        let env = Environment::child(&self.globals);
        match self.execute_program(&program, env, &module_context) {
            Ok(exports) => {
                *module.exports.borrow_mut() = exports;
                Ok(module)
            }
            Err(error) => {
                self.modules.remove(&path);
                Err(error.with_note(format!("while loading module '{path_text}'")))
            }
        }
    }

    fn load_standard_module(&mut self, path: &str) -> Result<Rc<ModuleValue>> {
        if let Some(module) = self.standard_modules.get(path) {
            return Ok(module.clone());
        }
        let exports = stdlib::module_exports(path)
            .ok_or_else(|| NiloError::module(format!("standard module '{path}' does not exist")))?;
        let module = Rc::new(ModuleValue {
            name: module_name(path),
            path: None,
            exports: RefCell::new(exports),
        });
        self.standard_modules
            .insert(path.to_owned(), module.clone());
        Ok(module)
    }

    fn check_arity(
        &self,
        name: &str,
        actual: usize,
        minimum: usize,
        maximum: Option<usize>,
    ) -> Result<()> {
        let valid = actual >= minimum && maximum.is_none_or(|maximum| actual <= maximum);
        if valid {
            return Ok(());
        }
        let expected = match maximum {
            Some(maximum) if minimum == maximum => minimum.to_string(),
            Some(maximum) => format!("{minimum}..={maximum}"),
            None => format!("at least {minimum}"),
        };
        Err(NiloError::runtime(format!(
            "{name} expected {expected} argument(s), got {actual}"
        )))
    }

    fn truthy(&self, value: &Value) -> bool {
        match value {
            Value::Nil => false,
            Value::Bool(value) => *value,
            Value::Int(value) => *value != 0,
            Value::Float(value) => *value != 0.0 && !value.is_nan(),
            Value::String(value) => !value.is_empty(),
            Value::List(value) => !value.borrow().items.is_empty(),
            Value::Map(value) => !value.borrow().entries.is_empty(),
            _ => true,
        }
    }

    fn map_key(&self, value: &Value) -> Result<MapKey> {
        match value {
            Value::String(value) => Ok(MapKey::String(value.as_str().to_owned())),
            Value::Int(value) => Ok(MapKey::Int(*value)),
            Value::Bool(value) => Ok(MapKey::Bool(*value)),
            other => Err(NiloError::type_error(format!(
                "map keys must be str, int, or bool; got {}",
                other.type_name()
            ))),
        }
    }

    fn list_index(&self, value: &Value, length: usize) -> Result<usize> {
        let Value::Int(index) = value else {
            return Err(NiloError::type_error(format!(
                "list and string indices must be int, got {}",
                value.type_name()
            )));
        };
        let length_i64 = i64::try_from(length).unwrap_or(i64::MAX);
        let normalized = if *index < 0 {
            length_i64.saturating_add(*index)
        } else {
            *index
        };
        if normalized < 0 || normalized >= length_i64 {
            return Err(NiloError::runtime(format!(
                "index {index} is out of range for length {length}"
            )));
        }
        usize::try_from(normalized).map_err(|_| NiloError::runtime("index is too large"))
    }

    fn expect_string<'a>(&self, value: &'a Value, label: &str) -> Result<&'a str> {
        value.as_string().ok_or_else(|| {
            NiloError::type_error(format!("{label} must be str, got {}", value.type_name()))
        })
    }

    fn expect_list<'a>(&self, value: &'a Value, label: &str) -> Result<&'a Rc<RefCell<ListValue>>> {
        match value {
            Value::List(list) => Ok(list),
            other => Err(NiloError::type_error(format!(
                "{label} must be list, got {}",
                other.type_name()
            ))),
        }
    }

    fn expect_map<'a>(&self, value: &'a Value, label: &str) -> Result<&'a Rc<RefCell<MapValue>>> {
        match value {
            Value::Map(map) => Ok(map),
            other => Err(NiloError::type_error(format!(
                "{label} must be map, got {}",
                other.type_name()
            ))),
        }
    }

    fn number(&self, value: &Value, label: &str) -> Result<f64> {
        match value {
            Value::Int(value) => Ok(*value as f64),
            Value::Float(value) => Ok(*value),
            other => Err(NiloError::type_error(format!(
                "{label} must be numeric, got {}",
                other.type_name()
            ))),
        }
    }

    fn integer(&self, value: &Value, label: &str) -> Result<i64> {
        match value {
            Value::Int(value) => Ok(*value),
            other => Err(NiloError::type_error(format!(
                "{label} must be int, got {}",
                other.type_name()
            ))),
        }
    }

    fn ensure_type(&self, value: &Value, ty: &TypeRef, label: impl AsRef<str>) -> Result<()> {
        if self.value_matches_type(value, ty) {
            Ok(())
        } else {
            Err(NiloError::type_error(format!(
                "{} expected {}, got {}",
                label.as_ref(),
                ty.display_name(),
                value.type_name()
            )))
        }
    }

    fn value_matches_type(&self, value: &Value, ty: &TypeRef) -> bool {
        if ty.nullable && matches!(value, Value::Nil) {
            return true;
        }
        let builtin = ty.name.to_ascii_lowercase();
        match builtin.as_str() {
            "any" => true,
            "nil" | "null" => matches!(value, Value::Nil),
            "bool" => matches!(value, Value::Bool(_)),
            "int" => matches!(value, Value::Int(_)),
            "float" => matches!(value, Value::Float(_)),
            "num" | "number" => matches!(value, Value::Int(_) | Value::Float(_)),
            "str" | "string" => matches!(value, Value::String(_)),
            "func" | "function" => value.is_callable(),
            "type" => matches!(value, Value::StructType(_)),
            "module" => matches!(value, Value::Module(_)),
            "list" => match value {
                Value::List(list) if ty.args.is_empty() => list.borrow().element_type.is_none(),
                Value::List(list) if ty.args.len() == 1 => {
                    let list = list.borrow();
                    if let Some(existing) = &list.element_type {
                        if existing != &ty.args[0] {
                            return false;
                        }
                    }
                    list.items
                        .iter()
                        .all(|item| self.value_matches_type(item, &ty.args[0]))
                }
                _ => false,
            },
            "map" => match value {
                Value::Map(map) if ty.args.is_empty() => {
                    let map = map.borrow();
                    map.key_type.is_none() && map.value_type.is_none()
                }
                Value::Map(map) if ty.args.len() == 2 => {
                    let map = map.borrow();
                    if map
                        .key_type
                        .as_ref()
                        .is_some_and(|existing| existing != &ty.args[0])
                        || map
                            .value_type
                            .as_ref()
                            .is_some_and(|existing| existing != &ty.args[1])
                    {
                        return false;
                    }
                    map.entries.iter().all(|(key, value)| {
                        self.map_key_matches_type(key, &ty.args[0])
                            && self.value_matches_type(value, &ty.args[1])
                    })
                }
                _ => false,
            },
            _ => match value {
                Value::Record(record) => record.borrow().definition.name == ty.name,
                _ => false,
            },
        }
    }

    fn map_key_matches_type(&self, key: &MapKey, ty: &TypeRef) -> bool {
        if ty.nullable {
            return false;
        }
        match ty.name.to_ascii_lowercase().as_str() {
            "any" => true,
            "str" | "string" => matches!(key, MapKey::String(_)),
            "int" => matches!(key, MapKey::Int(_)),
            "bool" => matches!(key, MapKey::Bool(_)),
            _ => false,
        }
    }

    fn apply_type_metadata(&self, value: &Value, ty: &TypeRef) -> Result<()> {
        if matches!(value, Value::Nil) && ty.nullable {
            return Ok(());
        }
        match (ty.name.to_ascii_lowercase().as_str(), value) {
            ("list", Value::List(list)) if ty.args.len() == 1 => {
                let element_type = ty.args[0].clone();
                {
                    let mut list = list.borrow_mut();
                    if let Some(existing) = &list.element_type {
                        if existing != &element_type {
                            return Err(NiloError::type_error(format!(
                                "list is already typed as list<{}>, not list<{}>",
                                existing.display_name(),
                                element_type.display_name()
                            )));
                        }
                    } else {
                        list.element_type = Some(element_type.clone());
                    }
                }
                let items = list.borrow().items.clone();
                for item in items {
                    self.apply_type_metadata(&item, &element_type)?;
                }
            }
            ("map", Value::Map(map)) if ty.args.len() == 2 => {
                let key_type = ty.args[0].clone();
                let value_type = ty.args[1].clone();
                {
                    let mut map = map.borrow_mut();
                    if map
                        .key_type
                        .as_ref()
                        .is_some_and(|existing| existing != &key_type)
                        || map
                            .value_type
                            .as_ref()
                            .is_some_and(|existing| existing != &value_type)
                    {
                        return Err(NiloError::type_error(
                            "map already has an incompatible type annotation",
                        ));
                    }
                    map.key_type = Some(key_type.clone());
                    map.value_type = Some(value_type.clone());
                }
                let values = map.borrow().entries.values().cloned().collect::<Vec<_>>();
                for item in values {
                    self.apply_type_metadata(&item, &value_type)?;
                }
            }
            (_, Value::Record(record)) => {
                let record = record.borrow();
                for field in &record.definition.fields {
                    if let (Some(field_type), Some(field_value)) =
                        (&field.ty, record.fields.get(&field.name))
                    {
                        self.apply_type_metadata(field_value, field_type)?;
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn call_native(
        &mut self,
        function: NativeFunction,
        args: Vec<Value>,
        context: &SourceContext,
    ) -> Result<Value> {
        match function.id {
            NativeId::Print => {
                let text = args
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(" ");
                writeln!(self.output, "{text}").map_err(|error| {
                    NiloError::runtime(format!("failed to write output: {error}"))
                })?;
                Ok(Value::Nil)
            }
            NativeId::Len => self.native_len(&args[0]),
            NativeId::Push | NativeId::ListPush => self.native_push(&args[0], args[1].clone()),
            NativeId::Pop | NativeId::ListPop => self.native_pop(&args[0]),
            NativeId::Str => Ok(Value::string(args[0].to_string())),
            NativeId::Int => self.native_int(&args[0]),
            NativeId::Float => self.native_float(&args[0]),
            NativeId::Bool => Ok(Value::Bool(self.truthy(&args[0]))),
            NativeId::Range => self.native_range(&args),
            NativeId::Assert => {
                if self.truthy(&args[0]) {
                    Ok(Value::Nil)
                } else {
                    let message = args
                        .get(1)
                        .map(ToString::to_string)
                        .unwrap_or_else(|| "assertion failed".to_owned());
                    Err(NiloError::runtime(message))
                }
            }
            NativeId::TypeOf => Ok(Value::string(args[0].type_name())),
            NativeId::Keys => self.native_keys(&args[0]),
            NativeId::Values => self.native_values(&args[0]),
            NativeId::Clock | NativeId::TimeNow => Ok(Value::Float(unix_time_seconds()?)),
            NativeId::JsonParse => self.native_json_parse(&args[0]),
            NativeId::JsonStringify => self.native_json_stringify(&args),
            NativeId::RegexCompile => self.native_regex_compile(&args),
            NativeId::RegexIsMatch => self.native_regex_is_match(&args),
            NativeId::RegexFind => self.native_regex_find(&args),
            NativeId::RegexFindAll => self.native_regex_find_all(&args),
            NativeId::RegexCaptures => self.native_regex_find(&args),
            NativeId::RegexReplace => self.native_regex_replace(&args),
            NativeId::RegexSplit => self.native_regex_split(&args),
            NativeId::RegexEscape => {
                let value = self.expect_string(&args[0], "regex.escape argument")?;
                Ok(Value::string(regex::escape(value)))
            }
            NativeId::FsReadText => {
                let path = self.native_path(&args[0], context, "fs.read_text path")?;
                let text =
                    fs::read_to_string(&path).map_err(|error| NiloError::io(&path, error))?;
                Ok(Value::string(text))
            }
            NativeId::FsWriteText => {
                let path = self.native_path(&args[0], context, "fs.write_text path")?;
                let text = self.expect_string(&args[1], "fs.write_text content")?;
                fs::write(&path, text).map_err(|error| NiloError::io(&path, error))?;
                Ok(Value::Nil)
            }
            NativeId::FsExists => {
                let path = self.native_path(&args[0], context, "fs.exists path")?;
                Ok(Value::Bool(path.exists()))
            }
            NativeId::FsListDir => self.native_fs_list_dir(&args[0], context),
            NativeId::FsRemove => self.native_fs_remove(&args[0], context),
            NativeId::TimeSleep => {
                let seconds = self.number(&args[0], "time.sleep seconds")?;
                if !seconds.is_finite() || seconds < 0.0 {
                    return Err(NiloError::runtime(
                        "time.sleep seconds must be a finite non-negative number",
                    ));
                }
                thread::sleep(Duration::from_secs_f64(seconds));
                Ok(Value::Nil)
            }
            NativeId::HttpGet => self.native_http("GET", &args),
            NativeId::HttpPost => self.native_http("POST", &args),
            NativeId::ListJoin => self.native_list_join(&args[0], &args[1]),
            NativeId::ListReverse => {
                let list = self.expect_list(&args[0], "list.reverse argument")?;
                list.borrow_mut().items.reverse();
                Ok(args[0].clone())
            }
            NativeId::ListSort => self.native_list_sort(&args[0]),
            NativeId::StringSplit => {
                let value = self.expect_string(&args[0], "string.split text")?;
                let separator = self.expect_string(&args[1], "string.split separator")?;
                if separator.is_empty() {
                    Ok(Value::list(
                        value
                            .chars()
                            .map(|character| Value::string(character.to_string()))
                            .collect(),
                    ))
                } else {
                    Ok(Value::list(
                        value.split(separator).map(Value::string).collect(),
                    ))
                }
            }
            NativeId::StringTrim => Ok(Value::string(
                self.expect_string(&args[0], "string.trim argument")?.trim(),
            )),
            NativeId::StringLower => Ok(Value::string(
                self.expect_string(&args[0], "string.lower argument")?
                    .to_lowercase(),
            )),
            NativeId::StringUpper => Ok(Value::string(
                self.expect_string(&args[0], "string.upper argument")?
                    .to_uppercase(),
            )),
            NativeId::StringContains => {
                let text = self.expect_string(&args[0], "string.contains text")?;
                let needle = self.expect_string(&args[1], "string.contains needle")?;
                Ok(Value::Bool(text.contains(needle)))
            }
            NativeId::StringReplace => {
                let text = self.expect_string(&args[0], "string.replace text")?;
                let from = self.expect_string(&args[1], "string.replace from")?;
                let to = self.expect_string(&args[2], "string.replace to")?;
                Ok(Value::string(text.replace(from, to)))
            }
            NativeId::MathAbs => self.native_math_abs(&args[0]),
            NativeId::MathMin => self.native_extreme(&args, Ordering::Less),
            NativeId::MathMax => self.native_extreme(&args, Ordering::Greater),
            NativeId::MathRound => Ok(Value::Float(
                self.number(&args[0], "math.round argument")?.round(),
            )),
            NativeId::MathFloor => Ok(Value::Float(
                self.number(&args[0], "math.floor argument")?.floor(),
            )),
            NativeId::MathCeil => Ok(Value::Float(
                self.number(&args[0], "math.ceil argument")?.ceil(),
            )),
            NativeId::MathPow => Ok(Value::Float(
                self.number(&args[0], "math.pow base")?
                    .powf(self.number(&args[1], "math.pow exponent")?),
            )),
            NativeId::MathSqrt => {
                let value = self.number(&args[0], "math.sqrt argument")?;
                if value < 0.0 {
                    Err(NiloError::runtime("math.sqrt argument cannot be negative"))
                } else {
                    Ok(Value::Float(value.sqrt()))
                }
            }
        }
    }

    fn native_len(&self, value: &Value) -> Result<Value> {
        let length = match value {
            Value::String(value) => value.chars().count(),
            Value::List(value) => value.borrow().items.len(),
            Value::Map(value) => value.borrow().entries.len(),
            Value::Record(value) => value.borrow().fields.len(),
            Value::Module(value) => value.exports.borrow().len(),
            other => {
                return Err(NiloError::type_error(format!(
                    "len expects str, list, map, record, or module; got {}",
                    other.type_name()
                )))
            }
        };
        Ok(Value::Int(i64::try_from(length).unwrap_or(i64::MAX)))
    }

    fn native_push(&self, list_value: &Value, value: Value) -> Result<Value> {
        let list = self.expect_list(list_value, "push first argument")?;
        let element_type = list.borrow().element_type.clone();
        if let Some(ty) = &element_type {
            self.ensure_type(&value, ty, "pushed list element")?;
            self.apply_type_metadata(&value, ty)?;
        }
        list.borrow_mut().items.push(value);
        Ok(list_value.clone())
    }

    fn native_pop(&self, list_value: &Value) -> Result<Value> {
        let list = self.expect_list(list_value, "pop argument")?;
        let value = list.borrow_mut().items.pop().unwrap_or(Value::Nil);
        Ok(value)
    }

    fn native_int(&self, value: &Value) -> Result<Value> {
        match value {
            Value::Int(value) => Ok(Value::Int(*value)),
            Value::Bool(value) => Ok(Value::Int(i64::from(*value))),
            Value::Float(value) => {
                if !value.is_finite() || *value < i64::MIN as f64 || *value > i64::MAX as f64 {
                    return Err(NiloError::runtime("float cannot be represented as int"));
                }
                Ok(Value::Int(value.trunc() as i64))
            }
            Value::String(value) => value
                .trim()
                .parse::<i64>()
                .map(Value::Int)
                .map_err(|_| NiloError::runtime(format!("cannot parse {value:?} as int"))),
            other => Err(NiloError::type_error(format!(
                "int conversion does not support {}",
                other.type_name()
            ))),
        }
    }

    fn native_float(&self, value: &Value) -> Result<Value> {
        match value {
            Value::Int(value) => Ok(Value::Float(*value as f64)),
            Value::Float(value) => Ok(Value::Float(*value)),
            Value::Bool(value) => Ok(Value::Float(if *value { 1.0 } else { 0.0 })),
            Value::String(value) => value
                .trim()
                .parse::<f64>()
                .map(Value::Float)
                .map_err(|_| NiloError::runtime(format!("cannot parse {value:?} as float"))),
            other => Err(NiloError::type_error(format!(
                "float conversion does not support {}",
                other.type_name()
            ))),
        }
    }

    fn native_range(&self, args: &[Value]) -> Result<Value> {
        let (start, end, step) = match args {
            [end] => (0, self.integer(end, "range end")?, 1),
            [start, end] => (
                self.integer(start, "range start")?,
                self.integer(end, "range end")?,
                1,
            ),
            [start, end, step] => (
                self.integer(start, "range start")?,
                self.integer(end, "range end")?,
                self.integer(step, "range step")?,
            ),
            _ => unreachable!("arity was checked"),
        };
        if step == 0 {
            return Err(NiloError::runtime("range step cannot be zero"));
        }
        let mut values = Vec::new();
        let mut current = start;
        while if step > 0 {
            current < end
        } else {
            current > end
        } {
            if values.len() >= 1_000_000 {
                return Err(NiloError::runtime(
                    "range would contain more than 1,000,000 values",
                ));
            }
            values.push(Value::Int(current));
            current = current
                .checked_add(step)
                .ok_or_else(|| NiloError::runtime("range overflow"))?;
        }
        Ok(Value::list(values))
    }

    fn native_keys(&self, value: &Value) -> Result<Value> {
        match value {
            Value::Map(map) => Ok(Value::list(
                map.borrow().entries.keys().map(map_key_value).collect(),
            )),
            Value::Record(record) => Ok(Value::list(
                record.borrow().fields.keys().map(Value::string).collect(),
            )),
            Value::Module(module) => Ok(Value::list(
                module.exports.borrow().keys().map(Value::string).collect(),
            )),
            other => Err(NiloError::type_error(format!(
                "keys expects map, record, or module; got {}",
                other.type_name()
            ))),
        }
    }

    fn native_values(&self, value: &Value) -> Result<Value> {
        match value {
            Value::Map(map) => Ok(Value::list(
                map.borrow().entries.values().cloned().collect(),
            )),
            Value::Record(record) => Ok(Value::list(
                record.borrow().fields.values().cloned().collect(),
            )),
            Value::Module(module) => Ok(Value::list(
                module.exports.borrow().values().cloned().collect(),
            )),
            other => Err(NiloError::type_error(format!(
                "values expects map, record, or module; got {}",
                other.type_name()
            ))),
        }
    }

    fn native_json_parse(&self, value: &Value) -> Result<Value> {
        let text = self.expect_string(value, "json.parse argument")?;
        let json = serde_json::from_str::<serde_json::Value>(text)
            .map_err(|error| NiloError::runtime(format!("invalid JSON: {error}")))?;
        self.value_from_json(json)
    }

    fn native_json_stringify(&self, args: &[Value]) -> Result<Value> {
        let json = self.value_to_json(&args[0])?;
        let pretty = args.get(1).is_some_and(|value| self.truthy(value));
        let text = if pretty {
            serde_json::to_string_pretty(&json)
        } else {
            serde_json::to_string(&json)
        }
        .map_err(|error| NiloError::runtime(format!("failed to encode JSON: {error}")))?;
        Ok(Value::string(text))
    }

    fn value_from_json(&self, value: serde_json::Value) -> Result<Value> {
        Ok(match value {
            serde_json::Value::Null => Value::Nil,
            serde_json::Value::Bool(value) => Value::Bool(value),
            serde_json::Value::Number(value) => {
                if let Some(value) = value.as_i64() {
                    Value::Int(value)
                } else {
                    Value::Float(
                        value.as_f64().ok_or_else(|| {
                            NiloError::runtime("JSON number cannot be represented")
                        })?,
                    )
                }
            }
            serde_json::Value::String(value) => Value::string(value),
            serde_json::Value::Array(values) => Value::list(
                values
                    .into_iter()
                    .map(|value| self.value_from_json(value))
                    .collect::<Result<Vec<_>>>()?,
            ),
            serde_json::Value::Object(values) => {
                let entries = values
                    .into_iter()
                    .map(|(key, value)| {
                        self.value_from_json(value)
                            .map(|value| (MapKey::String(key), value))
                    })
                    .collect::<Result<BTreeMap<_, _>>>()?;
                Value::map(entries)
            }
        })
    }

    fn value_to_json(&self, value: &Value) -> Result<serde_json::Value> {
        Ok(match value {
            Value::Nil => serde_json::Value::Null,
            Value::Bool(value) => serde_json::Value::Bool(*value),
            Value::Int(value) => serde_json::Value::Number((*value).into()),
            Value::Float(value) => serde_json::Number::from_f64(*value)
                .map(serde_json::Value::Number)
                .ok_or_else(|| NiloError::runtime("NaN and infinity cannot be encoded as JSON"))?,
            Value::String(value) => serde_json::Value::String(value.as_str().to_owned()),
            Value::List(list) => serde_json::Value::Array(
                list.borrow()
                    .items
                    .iter()
                    .map(|value| self.value_to_json(value))
                    .collect::<Result<Vec<_>>>()?,
            ),
            Value::Map(map) => {
                let mut object = serde_json::Map::new();
                for (key, value) in &map.borrow().entries {
                    object.insert(key.display(), self.value_to_json(value)?);
                }
                serde_json::Value::Object(object)
            }
            Value::Record(record) => {
                let record = record.borrow();
                let mut object = serde_json::Map::new();
                object.insert(
                    "__type__".to_owned(),
                    serde_json::Value::String(record.definition.name.clone()),
                );
                for (key, value) in &record.fields {
                    object.insert(key.clone(), self.value_to_json(value)?);
                }
                serde_json::Value::Object(object)
            }
            Value::Module(_)
            | Value::Function(_)
            | Value::Native(_)
            | Value::StructType(_)
            | Value::Pattern(_) => {
                return Err(NiloError::type_error(format!(
                    "{} cannot be encoded as JSON",
                    value.type_name()
                )))
            }
        })
    }

    fn native_regex_compile(&self, args: &[Value]) -> Result<Value> {
        let pattern = self.expect_string(&args[0], "regex.compile pattern")?;
        let flags = args
            .get(1)
            .map(|value| self.regex_flags(value))
            .transpose()?
            .unwrap_or(0);
        Ok(Value::Pattern(Rc::new(self.build_regex(pattern, flags)?)))
    }

    fn native_regex_is_match(&self, args: &[Value]) -> Result<Value> {
        let pattern = self.regex_value(&args[0], args.get(2))?;
        let text = self.expect_string(&args[1], "regex.is_match text")?;
        Ok(Value::Bool(pattern.compiled.is_match(text)))
    }

    fn native_regex_find(&self, args: &[Value]) -> Result<Value> {
        let pattern = self.regex_value(&args[0], args.get(2))?;
        let text = self.expect_string(&args[1], "regex.find text")?;
        let Some(captures) = pattern.compiled.captures(text) else {
            return Ok(Value::Nil);
        };
        Ok(regex_match_value(&pattern.compiled, &captures))
    }

    fn native_regex_find_all(&self, args: &[Value]) -> Result<Value> {
        let pattern = self.regex_value(&args[0], args.get(2))?;
        let text = self.expect_string(&args[1], "regex.find_all text")?;
        Ok(Value::list(
            pattern
                .compiled
                .captures_iter(text)
                .map(|captures| regex_match_value(&pattern.compiled, &captures))
                .collect(),
        ))
    }

    fn native_regex_replace(&self, args: &[Value]) -> Result<Value> {
        let pattern = self.regex_value(&args[0], args.get(3))?;
        let text = self.expect_string(&args[1], "regex.replace text")?;
        let replacement = self.expect_string(&args[2], "regex.replace replacement")?;
        Ok(Value::string(
            pattern.compiled.replace_all(text, replacement).into_owned(),
        ))
    }

    fn native_regex_split(&self, args: &[Value]) -> Result<Value> {
        let pattern = self.regex_value(&args[0], args.get(2))?;
        let text = self.expect_string(&args[1], "regex.split text")?;
        Ok(Value::list(
            pattern.compiled.split(text).map(Value::string).collect(),
        ))
    }

    fn regex_value(&self, value: &Value, flags: Option<&Value>) -> Result<Rc<RegexValue>> {
        match value {
            Value::Pattern(pattern) if flags.is_none() => Ok(pattern.clone()),
            Value::Pattern(_) => Err(NiloError::runtime(
                "flags cannot be supplied with an already compiled regex",
            )),
            Value::String(pattern) => {
                let flags = flags
                    .map(|value| self.regex_flags(value))
                    .transpose()?
                    .unwrap_or(0);
                Ok(Rc::new(self.build_regex(pattern, flags)?))
            }
            other => Err(NiloError::type_error(format!(
                "regex pattern must be str or compiled regex, got {}",
                other.type_name()
            ))),
        }
    }

    fn regex_flags(&self, value: &Value) -> Result<i64> {
        match value {
            Value::Nil => Ok(0),
            Value::Int(value) => Ok(*value),
            Value::List(list) => list.borrow().items.iter().try_fold(0_i64, |flags, value| {
                self.integer(value, "regex flag").map(|value| flags | value)
            }),
            other => Err(NiloError::type_error(format!(
                "regex flags must be int or list<int>, got {}",
                other.type_name()
            ))),
        }
    }

    fn build_regex(&self, pattern: &str, flags: i64) -> Result<RegexValue> {
        let mut builder = RegexBuilder::new(pattern);
        builder
            .case_insensitive(flags & 1 != 0)
            .multi_line(flags & 2 != 0)
            .dot_matches_new_line(flags & 4 != 0)
            .ignore_whitespace(flags & 8 != 0)
            .unicode(flags & 16 == 0);
        let compiled = builder
            .build()
            .map_err(|error| NiloError::runtime(format!("invalid regex: {error}")))?;
        Ok(RegexValue {
            pattern: pattern.to_owned(),
            flags,
            compiled,
        })
    }

    fn native_path(&self, value: &Value, context: &SourceContext, label: &str) -> Result<PathBuf> {
        let path = PathBuf::from(self.expect_string(value, label)?);
        Ok(if path.is_absolute() {
            path
        } else {
            context.directory.join(path)
        })
    }

    fn native_fs_list_dir(&self, value: &Value, context: &SourceContext) -> Result<Value> {
        let path = self.native_path(value, context, "fs.list_dir path")?;
        let mut entries = fs::read_dir(&path)
            .map_err(|error| NiloError::io(&path, error))?
            .map(|entry| {
                entry
                    .map_err(|error| NiloError::io(&path, error))
                    .map(|entry| Value::string(entry.file_name().to_string_lossy()))
            })
            .collect::<Result<Vec<_>>>()?;
        entries.sort_by_key(ToString::to_string);
        Ok(Value::list(entries))
    }

    fn native_fs_remove(&self, value: &Value, context: &SourceContext) -> Result<Value> {
        let path = self.native_path(value, context, "fs.remove path")?;
        if !path.exists() {
            return Ok(Value::Bool(false));
        }
        if path.is_dir() {
            fs::remove_dir_all(&path).map_err(|error| NiloError::io(&path, error))?;
        } else {
            fs::remove_file(&path).map_err(|error| NiloError::io(&path, error))?;
        }
        Ok(Value::Bool(true))
    }

    fn native_http(&self, method: &str, args: &[Value]) -> Result<Value> {
        let url = self.expect_string(&args[0], "HTTP URL")?;
        let (body, headers) = if method == "GET" {
            (None, args.get(1))
        } else {
            (
                Some(self.expect_string(&args[1], "HTTP request body")?),
                args.get(2),
            )
        };
        let headers = headers
            .map(|value| self.http_headers(value))
            .transpose()?
            .unwrap_or_default();

        let result = if method == "GET" {
            let mut request = ureq::get(url);
            for (name, value) in &headers {
                request = request.set(name, value);
            }
            request.call()
        } else {
            let mut request = ureq::post(url);
            for (name, value) in &headers {
                request = request.set(name, value);
            }
            request.send_string(body.unwrap_or_default())
        };

        match result {
            Ok(response) => self.http_response(response),
            Err(ureq::Error::Status(_, response)) => self.http_response(response),
            Err(ureq::Error::Transport(error)) => {
                Err(NiloError::runtime(format!("HTTP request failed: {error}")))
            }
        }
    }

    fn http_headers(&self, value: &Value) -> Result<BTreeMap<String, String>> {
        let map = self.expect_map(value, "HTTP headers")?;
        let mut headers = BTreeMap::new();
        for (key, value) in &map.borrow().entries {
            let MapKey::String(key) = key else {
                return Err(NiloError::type_error("HTTP header names must be strings"));
            };
            let value = self.expect_string(value, "HTTP header value")?;
            headers.insert(key.clone(), value.to_owned());
        }
        Ok(headers)
    }

    fn http_response(&self, response: ureq::Response) -> Result<Value> {
        let status = response.status();
        let mut headers = BTreeMap::new();
        for name in response.headers_names() {
            if let Some(value) = response.header(&name) {
                headers.insert(MapKey::String(name), Value::string(value));
            }
        }
        let body = response.into_string().map_err(|error| {
            NiloError::runtime(format!("failed to read HTTP response: {error}"))
        })?;
        let mut result = BTreeMap::new();
        result.insert(
            MapKey::String("status".to_owned()),
            Value::Int(i64::from(status)),
        );
        result.insert(MapKey::String("headers".to_owned()), Value::map(headers));
        result.insert(MapKey::String("body".to_owned()), Value::string(body));
        Ok(Value::map(result))
    }

    fn native_list_join(&self, list_value: &Value, separator: &Value) -> Result<Value> {
        let list = self.expect_list(list_value, "list.join first argument")?;
        let separator = self.expect_string(separator, "list.join separator")?;
        Ok(Value::string(
            list.borrow()
                .items
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(separator),
        ))
    }

    fn native_list_sort(&self, list_value: &Value) -> Result<Value> {
        let list = self.expect_list(list_value, "list.sort argument")?;
        {
            let list_ref = list.borrow();
            for pair in list_ref.items.windows(2) {
                if pair[0].partial_cmp(&pair[1]).is_none() {
                    return Err(NiloError::type_error(format!(
                        "list.sort cannot compare {} and {}",
                        pair[0].type_name(),
                        pair[1].type_name()
                    )));
                }
            }
        }
        list.borrow_mut()
            .items
            .sort_by(|left, right| left.partial_cmp(right).unwrap_or(Ordering::Equal));
        Ok(list_value.clone())
    }

    fn native_math_abs(&self, value: &Value) -> Result<Value> {
        match value {
            Value::Int(value) => value
                .checked_abs()
                .map(Value::Int)
                .ok_or_else(|| NiloError::runtime("integer absolute value overflow")),
            Value::Float(value) => Ok(Value::Float(value.abs())),
            other => Err(NiloError::type_error(format!(
                "math.abs expects a number, got {}",
                other.type_name()
            ))),
        }
    }

    fn native_extreme(&self, args: &[Value], desired: Ordering) -> Result<Value> {
        let mut best = args[0].clone();
        for value in &args[1..] {
            let ordering = value.partial_cmp(&best).ok_or_else(|| {
                NiloError::type_error(format!(
                    "cannot compare {} and {}",
                    value.type_name(),
                    best.type_name()
                ))
            })?;
            if ordering == desired {
                best = value.clone();
            }
        }
        Ok(best)
    }
}

fn map_key_value(key: &MapKey) -> Value {
    match key {
        MapKey::String(value) => Value::string(value),
        MapKey::Int(value) => Value::Int(*value),
        MapKey::Bool(value) => Value::Bool(*value),
    }
}

fn module_name(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("module")
        .replace('-', "_")
}

fn unix_time_seconds() -> Result<f64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs_f64())
        .map_err(|error| NiloError::runtime(format!("system clock error: {error}")))
}

fn regex_match_value(regex: &Regex, captures: &regex::Captures<'_>) -> Value {
    let whole = captures.get(0);
    let mut result = BTreeMap::new();
    result.insert(
        MapKey::String("text".to_owned()),
        whole
            .map(|value| Value::string(value.as_str()))
            .unwrap_or(Value::Nil),
    );
    result.insert(
        MapKey::String("match".to_owned()),
        whole
            .map(|value| Value::string(value.as_str()))
            .unwrap_or(Value::Nil),
    );
    result.insert(
        MapKey::String("start".to_owned()),
        whole
            .and_then(|value| i64::try_from(value.start()).ok())
            .map(Value::Int)
            .unwrap_or(Value::Nil),
    );
    result.insert(
        MapKey::String("end".to_owned()),
        whole
            .and_then(|value| i64::try_from(value.end()).ok())
            .map(Value::Int)
            .unwrap_or(Value::Nil),
    );
    result.insert(
        MapKey::String("groups".to_owned()),
        Value::list(
            captures
                .iter()
                .skip(1)
                .map(|capture| {
                    capture
                        .map(|value| Value::string(value.as_str()))
                        .unwrap_or(Value::Nil)
                })
                .collect(),
        ),
    );
    let mut named = BTreeMap::new();
    for name in regex.capture_names().flatten() {
        named.insert(
            MapKey::String(name.to_owned()),
            captures
                .name(name)
                .map(|value| Value::string(value.as_str()))
                .unwrap_or(Value::Nil),
        );
    }
    result.insert(MapKey::String("named".to_owned()), Value::map(named));
    Value::map(result)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::Interpreter;

    fn run(source: &str) -> crate::error::Result<String> {
        let mut interpreter = Interpreter::new(PathBuf::from("."), Vec::<u8>::new());
        interpreter.run_source(source, "<test>")?;
        String::from_utf8(interpreter.into_output())
            .map_err(|error| crate::error::NiloError::runtime(error.to_string()))
    }

    #[test]
    fn executes_functions_records_and_typed_lists() {
        let output = run(r#"
            type Point { x: int; y: int; }
            func sum(point: Point) -> int { return point.x + point.y; }
            let values: list<int> = [1, 2];
            push(values, 3);
            let point: Point = Point(values[0], values[2]);
            print(sum(point));
            "#)
        .expect("program should run");
        assert_eq!(output, "4\n");
    }

    #[test]
    fn rejects_typed_list_mutation() {
        let error = run(r#"
            let values: list<int> = [1];
            push(values, "bad");
            "#)
        .expect_err("type mismatch should fail");
        assert!(error.render().contains("expected int"));
    }

    #[test]
    fn supports_break_and_continue() {
        let output = run(r#"
            let total: int = 0;
            for value in range(0, 10) {
                if (value == 2) { continue; }
                if (value == 5) { break; }
                total = total + value;
            }
            print(total);
            "#)
        .expect("program should run");
        assert_eq!(output, "8\n");
    }
}
