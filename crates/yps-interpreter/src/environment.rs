use std::collections::{HashMap, HashSet};

use crate::value::Value;

#[derive(Debug, Clone, Default)]
pub struct Environment {
    scopes: Vec<HashMap<String, Value>>,
    constants: HashSet<String>,
}

impl Environment {
    pub fn new() -> Self {
        Self { scopes: vec![HashMap::new()], constants: HashSet::new() }
    }

    pub fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    pub fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    pub fn define(&mut self, name: String, value: Value, is_const: bool) {
        if is_const {
            self.constants.insert(name.clone());
        }
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name, value);
        }
    }

    pub fn is_const(&self, name: &str) -> bool {
        self.constants.contains(name)
    }

    pub fn get(&self, name: &str) -> Option<&Value> {
        for scope in self.scopes.iter().rev() {
            if let Some(value) = scope.get(name) {
                return Some(value);
            }
        }
        None
    }

    pub fn set(&mut self, name: &str, value: Value) -> bool {
        for scope in self.scopes.iter_mut().rev() {
            if scope.contains_key(name) {
                scope.insert(name.to_string(), value);
                return true;
            }
        }
        false
    }
}
