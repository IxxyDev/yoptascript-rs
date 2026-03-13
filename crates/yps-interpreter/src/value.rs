use std::collections::HashMap;
use std::fmt;

use yps_parser::ast::Block;

#[derive(Debug, Clone)]
pub enum Value {
    Number(f64),
    String(String),
    Boolean(bool),
    Array(Vec<Value>),
    Object(HashMap<String, Value>),
    Function { name: String, params: Vec<String>, body: Block },
    BuiltinFunction(String),
    Undefined,
    Null,
}

impl Value {
    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Undefined | Value::Null => false,
            Value::Boolean(b) => *b,
            Value::Number(n) => *n != 0.0,
            Value::String(s) => !s.is_empty(),
            Value::Array(a) => !a.is_empty(),
            _ => true,
        }
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Number(_) => "число",
            Value::String(_) => "строка",
            Value::Boolean(_) => "булево",
            Value::Array(_) => "массив",
            Value::Object(_) => "объект",
            Value::Function { .. } | Value::BuiltinFunction(_) => "функция",
            Value::Undefined => "неопределено",
            Value::Null => "нулл",
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Number(n) => {
                if n.fract() == 0.0 && n.is_finite() {
                    write!(f, "{}", *n as i64)
                } else {
                    write!(f, "{n}")
                }
            }
            Value::String(s) => write!(f, "{s}"),
            Value::Boolean(b) => write!(f, "{b}"),
            Value::Undefined => write!(f, "undefined"),
            Value::Null => write!(f, "null"),
            Value::Array(elements) => {
                write!(f, "[")?;
                for (i, el) in elements.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{el}")?;
                }
                write!(f, "]")
            }
            Value::Object(map) => {
                write!(f, "{{")?;
                for (i, (k, v)) in map.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{k}: {v}")?;
                }
                write!(f, "}}")
            }
            Value::Function { name, .. } if name.is_empty() => write!(f, "[анонимная функция]"),
            Value::Function { name, .. } => write!(f, "[функция {name}]"),
            Value::BuiltinFunction(name) => write!(f, "[встроенная {name}]"),
        }
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Number(a), Value::Number(b)) => a == b,
            (Value::String(a), Value::String(b)) => a == b,
            (Value::Boolean(a), Value::Boolean(b)) => a == b,
            (Value::Undefined, Value::Undefined) => true,
            (Value::Null, Value::Null) => true,
            _ => false,
        }
    }
}
