use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::rc::Rc;

pub struct AbortState {
    pub aborted: bool,
    pub reason: Value,
    pub next_token: u64,
    pub listeners: Vec<(u64, Value)>,
}

use yps_parser::ast::{Block, Expr, Param, Stmt};

use crate::environment::{EnvFrame, Environment};

#[derive(Debug, Clone)]
pub enum IteratorState {
    Array { values: Vec<Value>, index: usize },
    Chars { chars: Vec<char>, index: usize },
    MapEntries { entries: Vec<(Value, Value)>, index: usize },
    Map { inner: Box<IteratorState>, func: Value, index: usize },
    Filter { inner: Box<IteratorState>, func: Value, index: usize },
    Take { inner: Box<IteratorState>, remaining: usize },
    Drop { inner: Box<IteratorState>, count: usize, dropped: bool },
    Concat { iters: VecDeque<IteratorState> },
    RegexMatches { re: Rc<regex::Regex>, input: String, byte_pos: usize },
    Generator(Box<GenState>),
    Done,
}

#[derive(Clone)]
pub struct GenState {
    pub env: Environment,
    pub frames: Vec<GenFrame>,
    pub completed: bool,
    pub pending_bind: Option<BindTarget>,
}

#[derive(Clone)]
pub enum BindTarget {
    Variable { name: String, is_const: bool },
    Reassign(String),
}

#[derive(Clone)]
pub enum GenFrame {
    Block {
        stmts: Rc<[Stmt]>,
        idx: usize,
    },
    While {
        condition: Expr,
        body: Rc<Stmt>,
        phase: LoopPhase,
    },
    DoWhile {
        condition: Expr,
        body: Rc<Stmt>,
        phase: LoopPhase,
    },
    For {
        condition: Option<Expr>,
        update: Option<Expr>,
        body: Rc<Stmt>,
        phase: LoopPhase,
    },
    ForIter {
        var_name: String,
        iter: Rc<RefCell<IteratorState>>,
        body: Rc<Stmt>,
    },
    Delegate {
        inner: Rc<RefCell<IteratorState>>,
    },
    TryCatch {
        catch_param: Option<String>,
        catch_body: Option<Rc<[Stmt]>>,
        finally_body: Option<Rc<[Stmt]>>,
        state: TryState,
    },
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum LoopPhase {
    CheckCond,
    AfterBody,
}

#[derive(Clone)]
pub enum TryState {
    Trying,
    InCatch,
    FinallyNormal,
    FinallyAfterThrow(Value),
    FinallyAfterReturn(Value),
    FinallyAfterBreak,
    FinallyAfterContinue,
}

impl fmt::Debug for GenState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GenState").field("frames", &self.frames.len()).field("completed", &self.completed).finish()
    }
}

impl fmt::Debug for GenFrame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GenFrame::Block { idx, stmts } => write!(f, "Block(idx={idx}, len={})", stmts.len()),
            GenFrame::While { phase, .. } => write!(f, "While({phase:?})"),
            GenFrame::DoWhile { phase, .. } => write!(f, "DoWhile({phase:?})"),
            GenFrame::For { phase, .. } => write!(f, "For({phase:?})"),
            GenFrame::ForIter { var_name, .. } => write!(f, "ForIter({var_name})"),
            GenFrame::Delegate { .. } => write!(f, "Delegate"),
            GenFrame::TryCatch { .. } => write!(f, "TryCatch"),
        }
    }
}

impl fmt::Debug for LoopPhase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoopPhase::CheckCond => write!(f, "CheckCond"),
            LoopPhase::AfterBody => write!(f, "AfterBody"),
        }
    }
}

pub type MethodDef = (Rc<[Param]>, Rc<Block>, Rc<RefCell<EnvFrame>>);

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

#[derive(Clone, Copy, PartialEq, Eq)]
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
    pub parent: Option<Rc<ClassDef>>,
    pub instance_initializers: Vec<Value>,
}

#[derive(Clone)]
pub enum Value {
    Number(f64),
    BigInt(i128),
    String(String),
    Boolean(bool),
    Array(Vec<Value>),
    Object(HashMap<String, Value>),
    Map(Vec<(Value, Value)>),
    Set(Vec<Value>),
    Function {
        name: Rc<str>,
        params: Rc<[Param]>,
        body: Rc<Block>,
        env: Rc<RefCell<EnvFrame>>,
        is_generator: bool,
        is_async: bool,
    },
    BuiltinFunction(String),
    Class(Rc<ClassDef>),
    Symbol {
        description: Option<String>,
        id: u64,
    },
    Promise {
        state: Rc<RefCell<PromiseState>>,
    },
    PromiseCapability {
        state: Rc<RefCell<PromiseState>>,
        kind: CapKind,
    },
    PromiseThenHandler {
        handler: Box<Value>,
        resolve: Box<Value>,
        reject: Box<Value>,
        is_fulfill: bool,
    },
    PromiseFinallyHandler {
        cb: Box<Value>,
        cap: Box<Value>,
    },
    PromiseAggregateHandler {
        state: Rc<RefCell<AggregateState>>,
        index: usize,
        role: AggregateRole,
    },
    Iterator(Rc<RefCell<IteratorState>>),
    RegExp {
        pattern: String,
        flags: String,
        compiled: Rc<regex::Regex>,
        last_index: Rc<RefCell<usize>>,
    },
    AbortController {
        state: Rc<RefCell<AbortState>>,
    },
    AbortSignal {
        state: Rc<RefCell<AbortState>>,
    },
    AbortListener {
        target: Rc<RefCell<AbortState>>,
    },
    AbortUnsubscribe {
        state: Rc<RefCell<AbortState>>,
        token: u64,
    },
    AbortCancelTimer {
        timer_id: u64,
    },
    Undefined,
    Null,
}

impl Value {
    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Undefined | Value::Null => false,
            Value::Boolean(b) => *b,
            Value::Number(n) => *n != 0.0,
            Value::BigInt(n) => *n != 0,
            Value::String(s) => !s.is_empty(),
            Value::Array(a) => !a.is_empty(),
            _ => true,
        }
    }

    pub fn typeof_str(&self) -> &'static str {
        match self {
            Value::Number(_) => "число",
            Value::BigInt(_) => "бигцелое",
            Value::String(_) => "строка",
            Value::Boolean(_) => "булево",
            Value::Undefined => "неопределено",
            Value::Null => "объект",
            Value::Function { .. }
            | Value::BuiltinFunction(_)
            | Value::PromiseCapability { .. }
            | Value::PromiseThenHandler { .. }
            | Value::PromiseFinallyHandler { .. }
            | Value::PromiseAggregateHandler { .. } => "функция",
            Value::Array(_)
            | Value::Object(_)
            | Value::Class(_)
            | Value::Map(_)
            | Value::Set(_)
            | Value::Promise { .. }
            | Value::Iterator(_)
            | Value::RegExp { .. } => "объект",
            Value::Symbol { .. } => "символ",
            Value::AbortController { .. } => "контроллёрОтмены",
            Value::AbortSignal { .. } => "сигналОтмены",
            Value::AbortListener { .. } | Value::AbortUnsubscribe { .. } | Value::AbortCancelTimer { .. } => "функция",
        }
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Number(_) => "число",
            Value::BigInt(_) => "бигцелое",
            Value::String(_) => "строка",
            Value::Boolean(_) => "булево",
            Value::Array(_) => "массив",
            Value::Object(_) => "объект",
            Value::Map(_) => "карта",
            Value::Set(_) => "набор",
            Value::Function { .. }
            | Value::BuiltinFunction(_)
            | Value::PromiseCapability { .. }
            | Value::PromiseThenHandler { .. }
            | Value::PromiseFinallyHandler { .. }
            | Value::PromiseAggregateHandler { .. } => "функция",
            Value::Class(_) => "класс",
            Value::Symbol { .. } => "символ",
            Value::Promise { .. } => "обещание",
            Value::Iterator(_) => "итератор",
            Value::RegExp { .. } => "регэксп",
            Value::AbortController { .. } => "контроллёрОтмены",
            Value::AbortSignal { .. } => "сигналОтмены",
            Value::AbortListener { .. } | Value::AbortUnsubscribe { .. } | Value::AbortCancelTimer { .. } => "функция",
            Value::Undefined => "неопределено",
            Value::Null => "нулл",
        }
    }
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Number(n) => write!(f, "Number({n})"),
            Value::BigInt(n) => write!(f, "BigInt({n})"),
            Value::String(s) => write!(f, "String({s:?})"),
            Value::Boolean(b) => write!(f, "Boolean({b})"),
            Value::Array(a) => f.debug_tuple("Array").field(a).finish(),
            Value::Object(o) => f.debug_tuple("Object").field(o).finish(),
            Value::Map(m) => f.debug_tuple("Map").field(m).finish(),
            Value::Set(s) => f.debug_tuple("Set").field(s).finish(),
            Value::Function { name, params, .. } => {
                let param_names: Vec<&str> = params.iter().map(|p| p.name.name.as_str()).collect();
                write!(f, "Function {{ name: {name:?}, params: {param_names:?}, .. }}")
            }
            Value::BuiltinFunction(name) => write!(f, "BuiltinFunction({name:?})"),
            Value::Class(cls) => write!(f, "Class({})", cls.name),
            Value::Symbol { description, id } => write!(f, "Symbol({description:?}, id={id})"),
            Value::Promise { state } => match &*state.borrow() {
                PromiseState::Pending { .. } => write!(f, "Promise(Pending)"),
                PromiseState::Fulfilled(v) => write!(f, "Promise(Fulfilled({v:?}))"),
                PromiseState::Rejected(v) => write!(f, "Promise(Rejected({v:?}))"),
            },
            Value::PromiseCapability { kind, .. } => match kind {
                CapKind::Resolve => write!(f, "PromiseCapability(Resolve)"),
                CapKind::Reject => write!(f, "PromiseCapability(Reject)"),
            },
            Value::PromiseThenHandler { .. } => write!(f, "PromiseThenHandler"),
            Value::PromiseFinallyHandler { .. } => write!(f, "PromiseFinallyHandler"),
            Value::PromiseAggregateHandler { .. } => write!(f, "PromiseAggregateHandler"),
            Value::Iterator(state) => f.debug_tuple("Iterator").field(&*state.borrow()).finish(),
            Value::RegExp { pattern, flags, .. } => write!(f, "RegExp(/{pattern}/{flags})"),
            Value::AbortController { state } => {
                if state.borrow().aborted {
                    write!(f, "AbortController(aborted)")
                } else {
                    write!(f, "AbortController(active)")
                }
            }
            Value::AbortSignal { state } => {
                if state.borrow().aborted {
                    write!(f, "AbortSignal(aborted)")
                } else {
                    write!(f, "AbortSignal(active)")
                }
            }
            Value::AbortListener { .. } => write!(f, "AbortListener"),
            Value::AbortUnsubscribe { token, .. } => write!(f, "AbortUnsubscribe(token={token})"),
            Value::AbortCancelTimer { timer_id } => write!(f, "AbortCancelTimer(timer_id={timer_id})"),
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
            Value::BigInt(n) => write!(f, "{n}n"),
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
            Value::Set(items) => {
                write!(f, "Набор(")?;
                for (i, v) in items.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{v}")?;
                }
                write!(f, ")")
            }
            Value::Function { name, .. } if name.is_empty() => write!(f, "[анонимная функция]"),
            Value::Function { name, .. } => write!(f, "[функция {name}]"),
            Value::BuiltinFunction(name) => write!(f, "[встроенная {name}]"),
            Value::Class(cls) => write!(f, "[класс {}]", cls.name),
            Value::Symbol { description: Some(d), .. } => write!(f, "Симбол({d})"),
            Value::Symbol { description: None, .. } => write!(f, "Симбол()"),
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
            Value::Iterator(_) => write!(f, "[итератор]"),
            Value::RegExp { pattern, flags, .. } => write!(f, "/{pattern}/{flags}"),
            Value::AbortController { state } => {
                if state.borrow().aborted {
                    write!(f, "[контроллёрОтмены отменён]")
                } else {
                    write!(f, "[контроллёрОтмены активен]")
                }
            }
            Value::AbortSignal { state } => {
                if state.borrow().aborted {
                    write!(f, "[сигналОтмены отменён]")
                } else {
                    write!(f, "[сигналОтмены активен]")
                }
            }
            Value::AbortListener { .. } | Value::AbortUnsubscribe { .. } | Value::AbortCancelTimer { .. } => {
                write!(f, "[отписка]")
            }
        }
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Number(a), Value::Number(b)) => a == b,
            (Value::BigInt(a), Value::BigInt(b)) => a == b,
            (Value::String(a), Value::String(b)) => a == b,
            (Value::Boolean(a), Value::Boolean(b)) => a == b,
            (Value::Array(a), Value::Array(b)) => a == b,
            (Value::Map(a), Value::Map(b)) => a == b,
            (Value::Set(a), Value::Set(b)) => a == b,
            (Value::Class(a), Value::Class(b)) => Rc::ptr_eq(a, b),
            (Value::Symbol { id: a, .. }, Value::Symbol { id: b, .. }) => a == b,
            (Value::Promise { state: a }, Value::Promise { state: b }) => Rc::ptr_eq(a, b),
            (Value::Iterator(a), Value::Iterator(b)) => Rc::ptr_eq(a, b),
            (Value::RegExp { pattern: pa, flags: fa, .. }, Value::RegExp { pattern: pb, flags: fb, .. }) => {
                pa == pb && fa == fb
            }
            (Value::AbortController { state: a }, Value::AbortController { state: b }) => Rc::ptr_eq(a, b),
            (Value::AbortSignal { state: a }, Value::AbortSignal { state: b }) => Rc::ptr_eq(a, b),
            (Value::AbortListener { target: a }, Value::AbortListener { target: b }) => Rc::ptr_eq(a, b),
            (Value::AbortUnsubscribe { state: a, token: ta }, Value::AbortUnsubscribe { state: b, token: tb }) => {
                Rc::ptr_eq(a, b) && ta == tb
            }
            (Value::Undefined, Value::Undefined) => true,
            (Value::Null, Value::Null) => true,
            _ => false,
        }
    }
}
