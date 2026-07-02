use std::cell::RefCell;
use std::collections::HashSet;
use std::fmt;
use std::rc::Rc;

use indexmap::IndexMap;

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

pub const GETTER_PREFIX: &str = "__get_";
pub const SETTER_PREFIX: &str = "__set_";
const ACCESSOR_SUFFIX: &str = "__";

pub const CLASS_TAG: &str = "__class__";
pub const PROTO_KEY: &str = "__proto__";

#[must_use]
pub fn getter_key(prop: &str) -> String {
    format!("{GETTER_PREFIX}{prop}{ACCESSOR_SUFFIX}")
}

#[must_use]
pub fn setter_key(prop: &str) -> String {
    format!("{SETTER_PREFIX}{prop}{ACCESSOR_SUFFIX}")
}

#[must_use]
pub fn is_internal_key(k: &str) -> bool {
    k == CLASS_TAG || k == PROTO_KEY || k.starts_with(GETTER_PREFIX) || k.starts_with(SETTER_PREFIX)
}

pub type MethodDef = Rc<Closure>;

#[derive(Debug, Default)]
pub struct ClassMembers {
    pub methods: Vec<(String, MethodDef)>,
    pub getters: Vec<(String, MethodDef)>,
    pub setters: Vec<(String, MethodDef)>,
    pub static_methods: Vec<(String, MethodDef)>,
    pub static_getters: Vec<(String, MethodDef)>,
    pub static_setters: Vec<(String, MethodDef)>,
    pub field_inits: Vec<(String, Option<MethodDef>, Option<Value>)>,
}

#[derive(Debug)]
pub struct ClassDef {
    pub name: String,
    pub parent: Option<Rc<ClassDef>>,
    pub constructor: Option<MethodDef>,
    pub members: ClassMembers,
    pub static_fields: RefCell<ObjMap>,
}

impl ClassMembers {
    fn lookup<'a>(list: &'a [(String, MethodDef)], name: &str) -> Option<&'a MethodDef> {
        list.iter().find(|(k, _)| k == name).map(|(_, v)| v)
    }
}

impl ClassDef {
    pub fn find_method(self: &Rc<Self>, name: &str) -> Option<MethodDef> {
        let mut cur: Option<&Rc<ClassDef>> = Some(self);
        while let Some(c) = cur {
            if let Some(m) = ClassMembers::lookup(&c.members.methods, name) {
                return Some(Rc::clone(m));
            }
            cur = c.parent.as_ref();
        }
        None
    }

    pub fn find_method_owner(self: &Rc<Self>, name: &str) -> Option<Rc<ClassDef>> {
        let mut cur: Option<&Rc<ClassDef>> = Some(self);
        while let Some(c) = cur {
            if ClassMembers::lookup(&c.members.methods, name).is_some() {
                return Some(Rc::clone(c));
            }
            cur = c.parent.as_ref();
        }
        None
    }

    pub fn find_getter(self: &Rc<Self>, name: &str) -> Option<(MethodDef, Option<Rc<ClassDef>>)> {
        let mut cur: Option<&Rc<ClassDef>> = Some(self);
        while let Some(c) = cur {
            if let Some(m) = ClassMembers::lookup(&c.members.getters, name) {
                return Some((Rc::clone(m), Some(Rc::clone(c))));
            }
            cur = c.parent.as_ref();
        }
        None
    }

    pub fn find_setter(self: &Rc<Self>, name: &str) -> Option<(MethodDef, Option<Rc<ClassDef>>)> {
        let mut cur: Option<&Rc<ClassDef>> = Some(self);
        while let Some(c) = cur {
            if let Some(m) = ClassMembers::lookup(&c.members.setters, name) {
                return Some((Rc::clone(m), Some(Rc::clone(c))));
            }
            cur = c.parent.as_ref();
        }
        None
    }

    pub fn find_static_method(self: &Rc<Self>, name: &str) -> Option<(MethodDef, Option<Rc<ClassDef>>)> {
        let mut cur: Option<&Rc<ClassDef>> = Some(self);
        while let Some(c) = cur {
            if let Some(m) = ClassMembers::lookup(&c.members.static_methods, name) {
                return Some((Rc::clone(m), Some(Rc::clone(c))));
            }
            cur = c.parent.as_ref();
        }
        None
    }

    pub fn find_static_getter(self: &Rc<Self>, name: &str) -> Option<MethodDef> {
        let mut cur: Option<&Rc<ClassDef>> = Some(self);
        while let Some(c) = cur {
            if let Some(m) = ClassMembers::lookup(&c.members.static_getters, name) {
                return Some(Rc::clone(m));
            }
            cur = c.parent.as_ref();
        }
        None
    }

    pub fn find_static_setter(self: &Rc<Self>, name: &str) -> Option<MethodDef> {
        let mut cur: Option<&Rc<ClassDef>> = Some(self);
        while let Some(c) = cur {
            if let Some(m) = ClassMembers::lookup(&c.members.static_setters, name) {
                return Some(Rc::clone(m));
            }
            cur = c.parent.as_ref();
        }
        None
    }

    pub fn find_static_field(self: &Rc<Self>, name: &str) -> Option<Value> {
        let mut cur: Option<&Rc<ClassDef>> = Some(self);
        while let Some(c) = cur {
            if let Some(v) = c.static_fields.borrow().get(name) {
                return Some(v.clone());
            }
            cur = c.parent.as_ref();
        }
        None
    }

    pub fn find_static_field_owner(self: &Rc<Self>, name: &str) -> Option<Rc<ClassDef>> {
        let mut cur: Option<&Rc<ClassDef>> = Some(self);
        while let Some(c) = cur {
            if c.static_fields.borrow().contains_key(name) {
                return Some(Rc::clone(c));
            }
            cur = c.parent.as_ref();
        }
        None
    }

    pub fn is_subclass_of(self: &Rc<Self>, target: &Rc<ClassDef>) -> bool {
        let mut cur: Option<&Rc<ClassDef>> = Some(self);
        while let Some(c) = cur {
            if Rc::ptr_eq(c, target) {
                return true;
            }
            cur = c.parent.as_ref();
        }
        false
    }
}

#[derive(Debug, Default)]
pub struct ObjMap {
    entries: IndexMap<String, Value>,
}

impl ObjMap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        self.entries.get(key)
    }

    pub fn insert(&mut self, key: String, value: Value) {
        self.entries.insert(key, value);
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &Value)> {
        self.entries.iter()
    }

    pub fn remove(&mut self, key: &str) -> bool {
        self.entries.shift_remove(key).is_some()
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.entries.contains_key(key)
    }
}

#[derive(Clone)]
pub enum Value {
    Number(f64),
    BigInt(i128),
    Str(Rc<str>),
    Bool(bool),
    Null,
    Undefined,
    Array(Rc<RefCell<Vec<Value>>>),
    Object(Rc<RefCell<ObjMap>>),
    Function(Rc<Closure>),
    Builtin(Rc<str>),
    Class(Rc<ClassDef>),
    RegExp { pattern: Rc<str>, flags: Rc<str>, compiled: Rc<crate::regexp::YopRegex>, last_index: Rc<RefCell<usize>> },
    Generator(Rc<RefCell<GenState>>),
    ForIter(Rc<RefCell<ForIter>>),
    Promise { state: Rc<RefCell<PromiseState>> },
    PromiseCapability { state: Rc<RefCell<PromiseState>>, kind: CapKind },
    PromiseThenHandler { handler: Box<Value>, resolve: Box<Value>, reject: Box<Value>, is_fulfill: bool },
    PromiseFinallyHandler { cb: Box<Value>, cap: Box<Value> },
    PromiseAggregateHandler { state: Rc<RefCell<AggregateState>>, index: usize, role: AggregateRole },
    Host(yps_interpreter::value::Value),
}

#[derive(Clone)]
pub enum PromiseState {
    Pending { on_resolve: Vec<Value>, on_reject: Vec<Value> },
    Fulfilled(Value),
    Rejected(Value),
}

#[derive(Clone, Copy)]
pub enum CapKind {
    Resolve,
    Reject,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AggregateKind {
    All,
    AllSettled,
    Any,
    Race,
}

#[derive(Clone, Copy)]
pub enum AggregateRole {
    Fulfill,
    Reject,
}

pub struct AggregateState {
    pub kind: AggregateKind,
    pub remaining: usize,
    pub results: Vec<Value>,
    pub resolve: Value,
    pub reject: Value,
    pub settled: bool,
}

pub enum ForIter {
    Values { values: Vec<Value>, index: usize },
    Generator(Rc<RefCell<GenState>>),
}

pub struct GenState {
    pub closure: Rc<Closure>,
    pub owner: Option<Rc<ClassDef>>,
    pub started: bool,
    pub completed: bool,
    pub stack: Vec<Value>,
    pub frames: Vec<crate::vm::CallFrame>,
    pub handlers: Vec<crate::vm::Handler>,
    pub open_upvalues: Vec<Upvalue>,
    pub this: Value,
    pub args: Vec<Value>,
    pub delegate: Option<Delegate>,
}

pub enum Delegate {
    Generator(Rc<RefCell<GenState>>),
    Values { values: Vec<Value>, index: usize },
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
            Value::BigInt(n) => *n != 0,
            Value::Str(s) => !s.is_empty(),
            Value::Host(iv) => iv.is_truthy(),
            _ => true,
        }
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Number(_) => "число",
            Value::BigInt(_) => "бигцелое",
            Value::Str(_) => "строка",
            Value::Bool(_) => "булево",
            Value::Array(_) => "массив",
            Value::Object(_) => "объект",
            Value::Function(_) | Value::Builtin(_) => "функция",
            Value::Class(_) => "класс",
            Value::RegExp { .. } => "регэксп",
            Value::Generator(_) | Value::ForIter(_) => "итератор",
            Value::Promise { .. } => "обещание",
            Value::PromiseCapability { .. }
            | Value::PromiseThenHandler { .. }
            | Value::PromiseFinallyHandler { .. }
            | Value::PromiseAggregateHandler { .. } => "функция",
            Value::Host(iv) => iv.type_name(),
            Value::Undefined => "неопределено",
            Value::Null => "нулл",
        }
    }

    pub fn typeof_str(&self) -> &'static str {
        match self {
            Value::Number(_) => "число",
            Value::BigInt(_) => "бигцелое",
            Value::Str(_) => "строка",
            Value::Bool(_) => "булево",
            Value::Undefined => "неопределено",
            Value::Null => "объект",
            Value::Function(_) | Value::Builtin(_) | Value::Class(_) => "функция",
            Value::PromiseCapability { .. }
            | Value::PromiseThenHandler { .. }
            | Value::PromiseFinallyHandler { .. }
            | Value::PromiseAggregateHandler { .. } => "функция",
            Value::Array(_)
            | Value::Object(_)
            | Value::RegExp { .. }
            | Value::Generator(_)
            | Value::ForIter(_)
            | Value::Promise { .. } => "объект",
            Value::Host(iv) => iv.typeof_str(),
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
            Value::BigInt(n) => n.to_string(),
            Value::Bool(b) => {
                if *b {
                    "true".to_string()
                } else {
                    "false".to_string()
                }
            }
            Value::Null => "null".to_string(),
            Value::Undefined => "undefined".to_string(),
            Value::Object(_) | Value::RegExp { .. } => "[object Object]".to_string(),
            Value::Generator(_) | Value::ForIter(_) => "[итератор]".to_string(),
            Value::Promise { .. }
            | Value::PromiseCapability { .. }
            | Value::PromiseThenHandler { .. }
            | Value::PromiseFinallyHandler { .. }
            | Value::PromiseAggregateHandler { .. } => self.to_string(),
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
            Value::Function(_) | Value::Builtin(_) | Value::Class(_) => self.to_string(),
            Value::Host(iv) => iv.to_string(),
        }
    }

    fn fmt_with_seen(&self, f: &mut fmt::Formatter<'_>, seen: &mut HashSet<*const ()>) -> fmt::Result {
        match self {
            Value::Number(n) => {
                if *n == 0.0 && n.is_sign_negative() {
                    write!(f, "-0")
                } else {
                    write!(f, "{}", number_to_string(*n))
                }
            }
            Value::BigInt(n) => write!(f, "{n}n"),
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
                let mut first = true;
                for (k, v) in snapshot.iter() {
                    if is_internal_key(k) {
                        continue;
                    }
                    if !first {
                        write!(f, ", ")?;
                    }
                    first = false;
                    write!(f, "{k}: ")?;
                    v.fmt_with_seen(f, seen)?;
                }
                seen.remove(&ptr);
                write!(f, "}}")
            }
            Value::Function(c) if c.proto.name.is_empty() => write!(f, "[анонимная функция]"),
            Value::Function(c) => write!(f, "[функция {}]", c.proto.name),
            Value::Builtin(name) => write!(f, "[встроенная {name}]"),
            Value::Class(cls) => write!(f, "[класс {}]", cls.name),
            Value::RegExp { pattern, flags, .. } => write!(f, "/{pattern}/{flags}"),
            Value::Generator(_) | Value::ForIter(_) => write!(f, "[итератор]"),
            Value::Promise { state } => match &*state.borrow() {
                PromiseState::Pending { .. } => write!(f, "[обещание ждёт]"),
                PromiseState::Fulfilled(v) => write!(f, "[обещание решено: {v}]"),
                PromiseState::Rejected(v) => write!(f, "[обещание отвергнуто: {v}]"),
            },
            Value::PromiseCapability { kind, .. } => match kind {
                CapKind::Resolve => write!(f, "[капабилити решить]"),
                CapKind::Reject => write!(f, "[капабилити отвергнуть]"),
            },
            Value::PromiseThenHandler { .. } => write!(f, "[обработчик потом]"),
            Value::PromiseFinallyHandler { .. } => write!(f, "[обработчик наконец]"),
            Value::PromiseAggregateHandler { .. } => write!(f, "[обработчик агрегата]"),
            Value::Host(iv) => write!(f, "{iv}"),
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
        (Value::BigInt(x), Value::BigInt(y)) => x == y,
        (Value::Str(x), Value::Str(y)) => x == y,
        (Value::Bool(x), Value::Bool(y)) => x == y,
        (Value::Null, Value::Null) => true,
        (Value::Undefined, Value::Undefined) => true,
        (Value::Array(x), Value::Array(y)) => Rc::ptr_eq(x, y),
        (Value::Object(x), Value::Object(y)) => Rc::ptr_eq(x, y),
        (Value::Function(x), Value::Function(y)) => Rc::ptr_eq(x, y),
        (Value::Builtin(x), Value::Builtin(y)) => x == y,
        (Value::Class(x), Value::Class(y)) => Rc::ptr_eq(x, y),
        (Value::RegExp { pattern: pa, flags: fa, .. }, Value::RegExp { pattern: pb, flags: fb, .. }) => {
            pa == pb && fa == fb
        }
        (Value::Generator(x), Value::Generator(y)) => Rc::ptr_eq(x, y),
        (Value::ForIter(x), Value::ForIter(y)) => Rc::ptr_eq(x, y),
        (Value::Promise { state: x }, Value::Promise { state: y }) => Rc::ptr_eq(x, y),
        (Value::Host(x), Value::Host(y)) => x == y,
        _ => false,
    }
}

pub fn abstract_eq(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Null | Value::Undefined, Value::Null | Value::Undefined) => true,
        (Value::Number(_), Value::Number(_))
        | (Value::BigInt(_), Value::BigInt(_))
        | (Value::Str(_), Value::Str(_))
        | (Value::Bool(_), Value::Bool(_)) => strict_eq(a, b),
        (Value::Number(x), Value::Str(y)) => *x == string_to_number(y),
        (Value::Str(x), Value::Number(y)) => string_to_number(x) == *y,
        (Value::BigInt(x), Value::Str(y)) => bigint_eq_str(*x, y),
        (Value::Str(x), Value::BigInt(y)) => bigint_eq_str(*y, x),
        (Value::BigInt(x), Value::Number(y)) => bigint_eq_number(*x, *y),
        (Value::Number(x), Value::BigInt(y)) => bigint_eq_number(*y, *x),
        (Value::Bool(_), _) => abstract_eq(&Value::Number(a.to_number()), b),
        (_, Value::Bool(_)) => abstract_eq(a, &Value::Number(b.to_number())),
        _ => strict_eq(a, b),
    }
}

fn bigint_eq_number(a: i128, b: f64) -> bool {
    if !b.is_finite() || b.fract() != 0.0 {
        return false;
    }
    (a as f64) == b && (b as i128) == a
}

fn bigint_eq_str(a: i128, s: &str) -> bool {
    match s.trim().parse::<i128>() {
        Ok(n) => n == a,
        Err(_) => false,
    }
}

pub use yps_interpreter::interpreter::coercion::{number_to_string, string_to_number};

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
