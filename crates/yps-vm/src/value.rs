use std::cell::RefCell;
use std::collections::HashSet;
use std::fmt;
use std::rc::Rc;

use crate::chunk::FnProto;

#[derive(Debug)]
pub enum UpvalueState {
    Open(usize),
    Closed(Value),
}

pub type Upvalue = Rc<RefCell<UpvalueState>>;

#[derive(Debug)]
pub struct Closure {
    pub proto: Rc<FnProto>,
    pub upvalues: Vec<Upvalue>,
}

#[derive(Debug, Default)]
pub struct ObjMap {
    entries: Vec<(String, Value)>,
}

impl ObjMap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        self.entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    pub fn insert(&mut self, key: String, value: Value) {
        if let Some(slot) = self.entries.iter_mut().find(|(k, _)| *k == key) {
            slot.1 = value;
        } else {
            self.entries.push((key, value));
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &Value)> {
        self.entries.iter().map(|(k, v)| (k, v))
    }
}

#[derive(Clone)]
pub enum Value {
    Number(f64),
    Str(Rc<str>),
    Bool(bool),
    Null,
    Undefined,
    Array(Rc<RefCell<Vec<Value>>>),
    Object(Rc<RefCell<ObjMap>>),
    Function(Rc<Closure>),
    Builtin(Rc<str>),
}

impl Value {
    pub fn string(s: impl Into<Rc<str>>) -> Self {
        Value::Str(s.into())
    }

    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Undefined | Value::Null => false,
            Value::Bool(b) => *b,
            Value::Number(n) => *n != 0.0,
            Value::Str(s) => !s.is_empty(),
            Value::Array(a) => !a.borrow().is_empty(),
            _ => true,
        }
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Number(_) => "число",
            Value::Str(_) => "строка",
            Value::Bool(_) => "булево",
            Value::Array(_) => "массив",
            Value::Object(_) => "объект",
            Value::Function(_) | Value::Builtin(_) => "функция",
            Value::Undefined => "неопределено",
            Value::Null => "нулл",
        }
    }

    pub fn typeof_str(&self) -> &'static str {
        match self {
            Value::Number(_) => "число",
            Value::Str(_) => "строка",
            Value::Bool(_) => "булево",
            Value::Undefined => "неопределено",
            Value::Null => "объект",
            Value::Function(_) | Value::Builtin(_) => "функция",
            Value::Array(_) | Value::Object(_) => "объект",
        }
    }

    pub fn to_number(&self) -> f64 {
        match self {
            Value::Number(n) => *n,
            Value::Bool(b) => {
                if *b {
                    1.0
                } else {
                    0.0
                }
            }
            Value::Null => 0.0,
            Value::Undefined => f64::NAN,
            Value::Str(s) => string_to_number(s),
            _ => f64::NAN,
        }
    }

    pub fn to_ecma_string(&self) -> String {
        match self {
            Value::Str(s) => s.to_string(),
            Value::Number(n) => number_to_string(*n),
            Value::Bool(b) => {
                if *b {
                    "true".to_string()
                } else {
                    "false".to_string()
                }
            }
            Value::Null => "null".to_string(),
            Value::Undefined => "undefined".to_string(),
            Value::Object(_) => "[object Object]".to_string(),
            Value::Array(elements) => {
                let snapshot = elements.borrow();
                let parts: Vec<String> = snapshot
                    .iter()
                    .map(|el| match el {
                        Value::Null | Value::Undefined => String::new(),
                        other => other.to_ecma_string(),
                    })
                    .collect();
                parts.join(",")
            }
            Value::Function(_) | Value::Builtin(_) => self.to_string(),
        }
    }

    fn fmt_with_seen(&self, f: &mut fmt::Formatter<'_>, seen: &mut HashSet<*const ()>) -> fmt::Result {
        match self {
            Value::Number(n) => {
                if *n == 0.0 && n.is_sign_negative() {
                    write!(f, "-0")
                } else if n.is_nan() {
                    write!(f, "NaN")
                } else if n.is_infinite() {
                    write!(f, "{}", if *n > 0.0 { "Infinity" } else { "-Infinity" })
                } else if n.fract() == 0.0 && n.abs() < 9.007_199_254_740_992e15 {
                    write!(f, "{}", *n as i64)
                } else {
                    write!(f, "{n}")
                }
            }
            Value::Str(s) => write!(f, "{s}"),
            Value::Bool(b) => write!(f, "{b}"),
            Value::Undefined => write!(f, "undefined"),
            Value::Null => write!(f, "null"),
            Value::Array(elements) => {
                let ptr = Rc::as_ptr(elements) as *const ();
                if !seen.insert(ptr) {
                    return write!(f, "[Циклично]");
                }
                let snapshot = elements.borrow();
                write!(f, "[")?;
                for (i, el) in snapshot.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    el.fmt_with_seen(f, seen)?;
                }
                seen.remove(&ptr);
                write!(f, "]")
            }
            Value::Object(map) => {
                let ptr = Rc::as_ptr(map) as *const ();
                if !seen.insert(ptr) {
                    return write!(f, "[Циклично]");
                }
                let snapshot = map.borrow();
                write!(f, "{{")?;
                for (i, (k, v)) in snapshot.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{k}: ")?;
                    v.fmt_with_seen(f, seen)?;
                }
                seen.remove(&ptr);
                write!(f, "}}")
            }
            Value::Function(c) if c.proto.name.is_empty() => write!(f, "[анонимная функция]"),
            Value::Function(c) => write!(f, "[функция {}]", c.proto.name),
            Value::Builtin(name) => write!(f, "[встроенная {name}]"),
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut seen: HashSet<*const ()> = HashSet::new();
        self.fmt_with_seen(f, &mut seen)
    }
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self}")
    }
}

pub fn strict_eq(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => x == y,
        (Value::Str(x), Value::Str(y)) => x == y,
        (Value::Bool(x), Value::Bool(y)) => x == y,
        (Value::Null, Value::Null) => true,
        (Value::Undefined, Value::Undefined) => true,
        (Value::Array(x), Value::Array(y)) => Rc::ptr_eq(x, y),
        (Value::Object(x), Value::Object(y)) => Rc::ptr_eq(x, y),
        (Value::Function(x), Value::Function(y)) => Rc::ptr_eq(x, y),
        (Value::Builtin(x), Value::Builtin(y)) => x == y,
        _ => false,
    }
}

pub fn abstract_eq(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Null | Value::Undefined, Value::Null | Value::Undefined) => true,
        (Value::Number(_), Value::Number(_)) | (Value::Str(_), Value::Str(_)) | (Value::Bool(_), Value::Bool(_)) => {
            strict_eq(a, b)
        }
        (Value::Number(x), Value::Str(y)) => *x == string_to_number(y),
        (Value::Str(x), Value::Number(y)) => string_to_number(x) == *y,
        (Value::Bool(_), _) => abstract_eq(&Value::Number(a.to_number()), b),
        (_, Value::Bool(_)) => abstract_eq(a, &Value::Number(b.to_number())),
        _ => strict_eq(a, b),
    }
}

pub fn number_to_string(n: f64) -> String {
    if n.is_nan() {
        return "NaN".to_string();
    }
    if n.is_infinite() {
        return if n > 0.0 { "Infinity".to_string() } else { "-Infinity".to_string() };
    }
    if n == 0.0 {
        return "0".to_string();
    }
    if n.fract() == 0.0 && n.abs() < 9.007_199_254_740_992e15 {
        return format!("{}", n as i64);
    }
    format!("{n}")
}

pub fn string_to_number(s: &str) -> f64 {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return 0.0;
    }
    match trimmed {
        "Infinity" | "+Infinity" => return f64::INFINITY,
        "-Infinity" => return f64::NEG_INFINITY,
        _ => {}
    }
    if let Some(hex) = trimmed.strip_prefix("0x").or_else(|| trimmed.strip_prefix("0X")) {
        return i64::from_str_radix(hex, 16).map(|v| v as f64).unwrap_or(f64::NAN);
    }
    if let Some(oct) = trimmed.strip_prefix("0o").or_else(|| trimmed.strip_prefix("0O")) {
        return i64::from_str_radix(oct, 8).map(|v| v as f64).unwrap_or(f64::NAN);
    }
    if let Some(bin) = trimmed.strip_prefix("0b").or_else(|| trimmed.strip_prefix("0B")) {
        return i64::from_str_radix(bin, 2).map(|v| v as f64).unwrap_or(f64::NAN);
    }
    trimmed.parse::<f64>().unwrap_or(f64::NAN)
}

pub fn to_int32(n: f64) -> i32 {
    if !n.is_finite() || n == 0.0 {
        return 0;
    }
    let m = n.trunc();
    let modulo = m.rem_euclid(4_294_967_296.0);
    if modulo >= 2_147_483_648.0 { (modulo - 4_294_967_296.0) as i64 as i32 } else { modulo as i64 as i32 }
}

pub fn to_uint32(n: f64) -> u32 {
    to_int32(n) as u32
}
