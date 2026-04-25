use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

use yps_parser::ast::{Block, Param};

use crate::environment::EnvFrame;

pub type MethodDef = (Vec<Param>, Rc<Block>, Rc<RefCell<EnvFrame>>);

#[derive(Clone)]
pub struct ClassDef {
    pub name: String,
    pub constructor: Option<MethodDef>,
    pub methods: HashMap<String, MethodDef>,
    pub static_methods: HashMap<String, MethodDef>,
    pub static_fields: HashMap<String, Value>,
    pub field_inits: Vec<(String, Option<Rc<Block>>, Option<Value>)>,
    pub getters: HashMap<String, MethodDef>,
    pub setters: HashMap<String, MethodDef>,
    pub static_getters: HashMap<String, MethodDef>,
    pub static_setters: HashMap<String, MethodDef>,
    pub parent: Option<Box<ClassDef>>,
    pub instance_initializers: Vec<Value>,
}

#[derive(Clone)]
pub enum Value {
    Number(f64),
    String(String),
    Boolean(bool),
    Array(Vec<Value>),
    Object(HashMap<String, Value>),
    Map(Vec<(Value, Value)>),
    Function { name: String, params: Vec<Param>, body: Rc<Block>, env: Rc<RefCell<EnvFrame>> },
    BuiltinFunction(String),
    Class(Rc<ClassDef>),
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

    pub fn typeof_str(&self) -> &'static str {
        match self {
            Value::Number(_) => "число",
            Value::String(_) => "строка",
            Value::Boolean(_) => "булево",
            Value::Undefined => "неопределено",
            Value::Null => "объект",
            Value::Function { .. } | Value::BuiltinFunction(_) => "функция",
            Value::Array(_) | Value::Object(_) | Value::Class(_) | Value::Map(_) => "объект",
        }
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Number(_) => "число",
            Value::String(_) => "строка",
            Value::Boolean(_) => "булево",
            Value::Array(_) => "массив",
            Value::Object(_) => "объект",
            Value::Map(_) => "карта",
            Value::Function { .. } | Value::BuiltinFunction(_) => "функция",
            Value::Class(_) => "класс",
            Value::Undefined => "неопределено",
            Value::Null => "нулл",
        }
    }
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Number(n) => write!(f, "Number({n})"),
            Value::String(s) => write!(f, "String({s:?})"),
            Value::Boolean(b) => write!(f, "Boolean({b})"),
            Value::Array(a) => f.debug_tuple("Array").field(a).finish(),
            Value::Object(o) => f.debug_tuple("Object").field(o).finish(),
            Value::Map(m) => f.debug_tuple("Map").field(m).finish(),
            Value::Function { name, params, .. } => {
                let param_names: Vec<&str> = params.iter().map(|p| p.name.name.as_str()).collect();
                write!(f, "Function {{ name: {name:?}, params: {param_names:?}, .. }}")
            }
            Value::BuiltinFunction(name) => write!(f, "BuiltinFunction({name:?})"),
            Value::Class(cls) => write!(f, "Class({})", cls.name),
            Value::Undefined => write!(f, "Undefined"),
            Value::Null => write!(f, "Null"),
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
            Value::Map(entries) => {
                write!(f, "Карта(")?;
                for (i, (k, v)) in entries.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{k} => {v}")?;
                }
                write!(f, ")")
            }
            Value::Function { name, .. } if name.is_empty() => write!(f, "[анонимная функция]"),
            Value::Function { name, .. } => write!(f, "[функция {name}]"),
            Value::BuiltinFunction(name) => write!(f, "[встроенная {name}]"),
            Value::Class(cls) => write!(f, "[класс {}]", cls.name),
        }
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Number(a), Value::Number(b)) => a == b,
            (Value::String(a), Value::String(b)) => a == b,
            (Value::Boolean(a), Value::Boolean(b)) => a == b,
            (Value::Array(a), Value::Array(b)) => a == b,
            (Value::Map(a), Value::Map(b)) => a == b,
            (Value::Class(a), Value::Class(b)) => Rc::ptr_eq(a, b),
            (Value::Undefined, Value::Undefined) => true,
            (Value::Null, Value::Null) => true,
            _ => false,
        }
    }
}
