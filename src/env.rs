use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::ast::TypeRef;
use crate::value::Value;

#[derive(Clone)]
pub struct Binding {
    pub value: Value,
    pub ty: Option<TypeRef>,
}

pub type EnvRef = Rc<Environment>;

pub struct Environment {
    parent: Option<EnvRef>,
    values: RefCell<HashMap<String, Binding>>,
}

impl Environment {
    #[must_use]
    pub fn root() -> EnvRef {
        Rc::new(Self {
            parent: None,
            values: RefCell::new(HashMap::new()),
        })
    }

    #[must_use]
    pub fn child(parent: &EnvRef) -> EnvRef {
        Rc::new(Self {
            parent: Some(parent.clone()),
            values: RefCell::new(HashMap::new()),
        })
    }

    pub fn define(&self, name: impl Into<String>, binding: Binding) -> bool {
        let name = name.into();
        let mut values = self.values.borrow_mut();
        if let std::collections::hash_map::Entry::Vacant(entry) = values.entry(name) {
            entry.insert(binding);
            true
        } else {
            false
        }
    }

    pub fn define_or_replace(&self, name: impl Into<String>, binding: Binding) {
        self.values.borrow_mut().insert(name.into(), binding);
    }

    #[must_use]
    pub fn local_names(&self) -> Vec<String> {
        self.values.borrow().keys().cloned().collect()
    }
}

#[must_use]
pub fn lookup(env: &EnvRef, name: &str) -> Option<Binding> {
    if let Some(binding) = env.values.borrow().get(name) {
        return Some(binding.clone());
    }
    env.parent.as_ref().and_then(|parent| lookup(parent, name))
}

pub fn assign(env: &EnvRef, name: &str, value: Value) -> bool {
    {
        let mut values = env.values.borrow_mut();
        if let Some(binding) = values.get_mut(name) {
            binding.value = value;
            return true;
        }
    }
    env.parent
        .as_ref()
        .is_some_and(|parent| assign(parent, name, value))
}

#[must_use]
pub fn binding_type(env: &EnvRef, name: &str) -> Option<Option<TypeRef>> {
    if let Some(binding) = env.values.borrow().get(name) {
        return Some(binding.ty.clone());
    }
    env.parent
        .as_ref()
        .and_then(|parent| binding_type(parent, name))
}
