use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::{Deref, DerefMut};
use std::rc::{Rc, Weak};

use indexmap::{IndexMap, IndexSet};

pub struct AbortState {
    pub aborted: bool,
    pub reason: Value,
    pub next_token: u64,
    pub listeners: Vec<(u64, Value)>,
    pub promise: RefCell<Option<Value>>,
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
    RegexMatches { re: Rc<crate::stdlib::regexp::YopRegex>, input: String, byte_pos: usize },
    Generator(Box<GenState>),
    Done,
}

#[derive(Clone)]
pub struct GenState {
    pub name: Rc<str>,
    pub env: Environment,
    pub frames: Vec<GenFrame>,
    pub completed: bool,
    pub pending_bind: Option<BindTarget>,
    pub pending_send: Option<Value>,
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
        owns_scope: bool,
    },
    While {
        condition: Rc<Expr>,
        body: Rc<[Stmt]>,
        phase: LoopPhase,
    },
    DoWhile {
        condition: Rc<Expr>,
        body: Rc<[Stmt]>,
        phase: LoopPhase,
    },
    For {
        condition: Option<Rc<Expr>>,
        update: Option<Rc<Expr>>,
        body: Rc<[Stmt]>,
        phase: LoopPhase,
    },
    ForIter {
        var_name: String,
        iter: Rc<RefCell<IteratorState>>,
        body: Rc<[Stmt]>,
    },
    Delegate {
        inner: Rc<RefCell<IteratorState>>,
        bind: Option<BindTarget>,
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
            GenFrame::Block { idx, stmts, .. } => write!(f, "Block(idx={idx}, len={})", stmts.len()),
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

#[derive(Clone)]
pub struct MethodDef {
    pub params: Rc<[Param]>,
    pub body: Rc<Block>,
    pub env: Rc<RefCell<EnvFrame>>,
}

pub type SharedBuffer = Rc<RefCell<Vec<u8>>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypedArrayKind {
    U8,
    U8Clamped,
    I8,
    U16,
    I16,
    U32,
    I32,
    F32,
    F64,
}

impl TypedArrayKind {
    pub fn element_size(self) -> usize {
        match self {
            TypedArrayKind::U8 | TypedArrayKind::U8Clamped | TypedArrayKind::I8 => 1,
            TypedArrayKind::U16 | TypedArrayKind::I16 => 2,
            TypedArrayKind::U32 | TypedArrayKind::I32 | TypedArrayKind::F32 => 4,
            TypedArrayKind::F64 => 8,
        }
    }

    pub fn type_name(self) -> &'static str {
        match self {
            TypedArrayKind::U8 => "Ц8Массив",
            TypedArrayKind::U8Clamped => "Ц8ОграниченныйМассив",
            TypedArrayKind::I8 => "Ч8Массив",
            TypedArrayKind::U16 => "Ц16Массив",
            TypedArrayKind::I16 => "Ч16Массив",
            TypedArrayKind::U32 => "Ц32Массив",
            TypedArrayKind::I32 => "Ч32Массив",
            TypedArrayKind::F32 => "Др32Массив",
            TypedArrayKind::F64 => "Др64Массив",
        }
    }

    pub fn read_le(self, bytes: &[u8], byte_index: usize) -> f64 {
        let size = self.element_size();
        let slice = &bytes[byte_index..byte_index + size];
        match self {
            TypedArrayKind::U8 | TypedArrayKind::U8Clamped => slice[0] as f64,
            TypedArrayKind::I8 => slice[0] as i8 as f64,
            TypedArrayKind::U16 => u16::from_le_bytes([slice[0], slice[1]]) as f64,
            TypedArrayKind::I16 => i16::from_le_bytes([slice[0], slice[1]]) as f64,
            TypedArrayKind::U32 => u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]) as f64,
            TypedArrayKind::I32 => i32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]) as f64,
            TypedArrayKind::F32 => f32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]) as f64,
            TypedArrayKind::F64 => {
                f64::from_le_bytes([slice[0], slice[1], slice[2], slice[3], slice[4], slice[5], slice[6], slice[7]])
            }
        }
    }

    pub fn write_le(self, bytes: &mut [u8], byte_index: usize, num: f64) {
        let size = self.element_size();
        let dst = &mut bytes[byte_index..byte_index + size];
        match self {
            TypedArrayKind::U8 => dst[0] = to_uint_n(num, 8) as u8,
            TypedArrayKind::U8Clamped => dst[0] = clamp_u8(num),
            TypedArrayKind::I8 => dst[0] = (to_int_n(num, 8) as i8) as u8,
            TypedArrayKind::U16 => dst.copy_from_slice(&(to_uint_n(num, 16) as u16).to_le_bytes()),
            TypedArrayKind::I16 => dst.copy_from_slice(&(to_int_n(num, 16) as i16).to_le_bytes()),
            TypedArrayKind::U32 => dst.copy_from_slice(&(to_uint_n(num, 32) as u32).to_le_bytes()),
            TypedArrayKind::I32 => dst.copy_from_slice(&(to_int_n(num, 32) as i32).to_le_bytes()),
            TypedArrayKind::F32 => dst.copy_from_slice(&(num as f32).to_le_bytes()),
            TypedArrayKind::F64 => dst.copy_from_slice(&num.to_le_bytes()),
        }
    }
}

pub fn to_uint_n(num: f64, bits: u32) -> u64 {
    if !num.is_finite() {
        return 0;
    }
    let truncated = num.trunc();
    let modulus = 2f64.powi(bits as i32);
    let wrapped = truncated.rem_euclid(modulus);
    wrapped as u64
}

pub fn to_int_n(num: f64, bits: u32) -> i64 {
    let unsigned = to_uint_n(num, bits);
    let half = 1u64 << (bits - 1);
    if unsigned >= half { (unsigned as i64) - (1i64 << bits) } else { unsigned as i64 }
}

fn clamp_u8(num: f64) -> u8 {
    if num.is_nan() {
        return 0;
    }
    if num <= 0.0 {
        return 0;
    }
    if num >= 255.0 {
        return 255;
    }
    let floor = num.floor();
    let diff = num - floor;
    let rounded = if diff < 0.5 {
        floor
    } else if diff > 0.5 {
        floor + 1.0
    } else if (floor as i64) % 2 == 0 {
        floor
    } else {
        floor + 1.0
    };
    rounded as u8
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
    pub static_fields: RefCell<HashMap<String, Value>>,
    pub field_inits: Vec<(String, Option<Rc<Block>>, Option<Value>)>,
    pub getters: HashMap<String, MethodDef>,
    pub setters: HashMap<String, MethodDef>,
    pub static_getters: HashMap<String, MethodDef>,
    pub static_setters: HashMap<String, MethodDef>,
    pub parent: Option<Rc<ClassDef>>,
    pub instance_initializers: Vec<Value>,
    pub prototype_cache: std::cell::OnceCell<Value>,
}

#[derive(Clone, Default, Debug)]
pub struct ArrayStore(pub Vec<Value>);

#[derive(Clone, Default, Debug)]
pub struct ObjectStore {
    pub map: IndexMap<String, Value>,
    pub frozen: bool,
}

impl ObjectStore {
    pub fn new(map: IndexMap<String, Value>) -> Self {
        ObjectStore { map, frozen: false }
    }
}

#[derive(Clone, Default)]
pub struct MapStore(pub IndexMap<MapKey, Value>);

#[derive(Clone, Default)]
pub struct SetStore(pub IndexSet<MapKey>);

impl Deref for ArrayStore {
    type Target = Vec<Value>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for ArrayStore {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Deref for ObjectStore {
    type Target = IndexMap<String, Value>;
    fn deref(&self) -> &Self::Target {
        &self.map
    }
}

impl DerefMut for ObjectStore {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.map
    }
}

impl Drop for ArrayStore {
    fn drop(&mut self) {
        let mut stack = std::mem::take(&mut self.0);
        drain_value_tree(&mut stack);
    }
}

impl Drop for ObjectStore {
    fn drop(&mut self) {
        let mut stack: Vec<Value> = std::mem::take(&mut self.map).into_values().collect();
        drain_value_tree(&mut stack);
    }
}

impl Deref for MapStore {
    type Target = IndexMap<MapKey, Value>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for MapStore {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Deref for SetStore {
    type Target = IndexSet<MapKey>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for SetStore {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Drop for MapStore {
    fn drop(&mut self) {
        let mut stack: Vec<Value> = Vec::with_capacity(self.0.len() * 2);
        for (k, v) in std::mem::take(&mut self.0) {
            stack.push(k.0);
            stack.push(v);
        }
        drain_value_tree(&mut stack);
    }
}

impl Drop for SetStore {
    fn drop(&mut self) {
        let mut stack: Vec<Value> = std::mem::take(&mut self.0).into_iter().map(|k| k.0).collect();
        drain_value_tree(&mut stack);
    }
}

fn drain_value_tree(stack: &mut Vec<Value>) {
    while let Some(value) = stack.pop() {
        match value {
            Value::Array(rc) => {
                if Rc::strong_count(&rc) == 1
                    && let Ok(mut inner) = rc.try_borrow_mut()
                {
                    stack.append(&mut inner.0);
                }
            }
            Value::Object(rc) => {
                if Rc::strong_count(&rc) == 1
                    && let Ok(mut inner) = rc.try_borrow_mut()
                {
                    let map = std::mem::take(&mut inner.map);
                    stack.extend(map.into_values());
                }
            }
            Value::Map(rc) => {
                if Rc::strong_count(&rc) == 1
                    && let Ok(mut inner) = rc.try_borrow_mut()
                {
                    for (k, v) in std::mem::take(&mut inner.0) {
                        stack.push(k.0);
                        stack.push(v);
                    }
                }
            }
            Value::Set(rc) => {
                if Rc::strong_count(&rc) == 1
                    && let Ok(mut inner) = rc.try_borrow_mut()
                {
                    stack.extend(std::mem::take(&mut inner.0).into_iter().map(|k| k.0));
                }
            }
            _ => {}
        }
    }
}

#[derive(Clone)]
pub enum Value {
    Number(f64),
    BigInt(i128),
    String(String),
    Boolean(bool),
    Array(Rc<RefCell<ArrayStore>>),
    Object(Rc<RefCell<ObjectStore>>),
    Map(Rc<RefCell<MapStore>>),
    Set(Rc<RefCell<SetStore>>),
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
    WeakClass(Weak<ClassDef>),
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
        compiled: Rc<crate::stdlib::regexp::YopRegex>,
        last_index: Rc<RefCell<usize>>,
    },
    Date(Rc<Cell<f64>>),
    AbortController {
        state: Rc<RefCell<AbortState>>,
    },
    AbortSignal {
        state: Rc<RefCell<AbortState>>,
    },
    AbortListener {
        target: Weak<RefCell<AbortState>>,
    },
    AbortUnsubscribe {
        state: Rc<RefCell<AbortState>>,
        token: u64,
    },
    AbortCancelTimer {
        timer_id: u64,
    },
    AbortRejectPromise {
        reject_cap: Box<Value>,
        reason_from_signal: bool,
    },
    ArrayBuffer(SharedBuffer),
    TypedArray {
        buffer: SharedBuffer,
        offset: usize,
        length: usize,
        kind: TypedArrayKind,
    },
    DataView {
        buffer: SharedBuffer,
        offset: usize,
        length: usize,
    },
    Proxy {
        target: Rc<Value>,
        handler: Rc<Value>,
    },
    WeakMap(WeakMapStore),
    WeakSet(WeakSetStore),
    WeakRef(Rc<WeakKey>),
    FinalizationRegistry(Rc<RefCell<FinRegState>>),
    Undefined,
    Null,
}

pub type WeakMapStore = Rc<RefCell<HashMap<usize, (WeakKey, Value)>>>;
pub type WeakSetStore = Rc<RefCell<HashMap<usize, WeakKey>>>;

#[derive(Clone)]
pub enum WeakKey {
    Object(Weak<RefCell<ObjectStore>>),
    Array(Weak<RefCell<ArrayStore>>),
    Map(Weak<RefCell<MapStore>>),
    Set(Weak<RefCell<SetStore>>),
}

impl WeakKey {
    pub fn try_from_value(value: &Value) -> Option<WeakKey> {
        match value {
            Value::Object(rc) => Some(WeakKey::Object(Rc::downgrade(rc))),
            Value::Array(rc) => Some(WeakKey::Array(Rc::downgrade(rc))),
            Value::Map(rc) => Some(WeakKey::Map(Rc::downgrade(rc))),
            Value::Set(rc) => Some(WeakKey::Set(Rc::downgrade(rc))),
            _ => None,
        }
    }

    pub fn ptr(&self) -> usize {
        match self {
            WeakKey::Object(w) => w.as_ptr() as *const () as usize,
            WeakKey::Array(w) => w.as_ptr() as *const () as usize,
            WeakKey::Map(w) => w.as_ptr() as *const () as usize,
            WeakKey::Set(w) => w.as_ptr() as *const () as usize,
        }
    }

    pub fn upgrade(&self) -> Option<Value> {
        match self {
            WeakKey::Object(w) => w.upgrade().map(Value::Object),
            WeakKey::Array(w) => w.upgrade().map(Value::Array),
            WeakKey::Map(w) => w.upgrade().map(Value::Map),
            WeakKey::Set(w) => w.upgrade().map(Value::Set),
        }
    }

    pub fn is_alive(&self) -> bool {
        match self {
            WeakKey::Object(w) => w.strong_count() > 0,
            WeakKey::Array(w) => w.strong_count() > 0,
            WeakKey::Map(w) => w.strong_count() > 0,
            WeakKey::Set(w) => w.strong_count() > 0,
        }
    }
}

pub struct FinRegState {
    pub callback: Value,
    pub entries: Vec<FinRegEntry>,
}

pub struct FinRegEntry {
    pub target: WeakKey,
    pub held: Value,
    pub token: Option<WeakKey>,
}

impl Value {
    pub fn array(items: Vec<Value>) -> Value {
        Value::Array(Rc::new(RefCell::new(ArrayStore(items))))
    }

    pub fn object(map: IndexMap<String, Value>) -> Value {
        Value::Object(Rc::new(RefCell::new(ObjectStore::new(map))))
    }

    pub fn map(entries: IndexMap<MapKey, Value>) -> Value {
        Value::Map(Rc::new(RefCell::new(MapStore(entries))))
    }

    pub fn set(items: IndexSet<MapKey>) -> Value {
        Value::Set(Rc::new(RefCell::new(SetStore(items))))
    }

    pub(crate) fn proxy_parts(&self) -> Option<(Rc<Value>, Rc<Value>)> {
        match self {
            Value::Proxy { target, handler } => Some((Rc::clone(target), Rc::clone(handler))),
            _ => None,
        }
    }

    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Undefined | Value::Null => false,
            Value::Boolean(b) => *b,
            Value::Number(n) => *n != 0.0,
            Value::BigInt(n) => *n != 0,
            Value::String(s) => !s.is_empty(),
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
            | Value::WeakClass(_)
            | Value::Map(_)
            | Value::Set(_)
            | Value::Promise { .. }
            | Value::Iterator(_)
            | Value::RegExp { .. }
            | Value::Date(_)
            | Value::ArrayBuffer(_)
            | Value::TypedArray { .. }
            | Value::DataView { .. }
            | Value::WeakMap(_)
            | Value::WeakSet(_)
            | Value::WeakRef(_)
            | Value::FinalizationRegistry(_) => "объект",
            Value::Symbol { .. } => "символ",
            Value::AbortController { .. } => "контроллёрОтмены",
            Value::AbortSignal { .. } => "сигналОтмены",
            Value::AbortListener { .. }
            | Value::AbortUnsubscribe { .. }
            | Value::AbortCancelTimer { .. }
            | Value::AbortRejectPromise { .. } => "функция",
            Value::Proxy { target, .. } => target.typeof_str(),
        }
    }

    pub fn is_callable(&self) -> bool {
        matches!(
            self,
            Value::Function { .. }
                | Value::BuiltinFunction(_)
                | Value::PromiseCapability { .. }
                | Value::PromiseThenHandler { .. }
                | Value::PromiseFinallyHandler { .. }
                | Value::PromiseAggregateHandler { .. }
                | Value::AbortUnsubscribe { .. }
                | Value::AbortListener { .. }
                | Value::Proxy { .. }
        )
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
            Value::Class(_) | Value::WeakClass(_) => "класс",
            Value::Symbol { .. } => "символ",
            Value::Promise { .. } => "обещание",
            Value::Iterator(_) => "итератор",
            Value::RegExp { .. } => "регэксп",
            Value::Date(_) => "дата",
            Value::ArrayBuffer(_) => "ОбластьБайтов",
            Value::TypedArray { kind, .. } => kind.type_name(),
            Value::DataView { .. } => "ОбзорБайтов",
            Value::AbortController { .. } => "контроллёрОтмены",
            Value::AbortSignal { .. } => "сигналОтмены",
            Value::AbortListener { .. }
            | Value::AbortUnsubscribe { .. }
            | Value::AbortCancelTimer { .. }
            | Value::AbortRejectPromise { .. } => "функция",
            Value::Proxy { target, .. } => target.type_name(),
            Value::WeakMap(_) => "слабаяКарта",
            Value::WeakSet(_) => "слабыйНабор",
            Value::WeakRef(_) => "слабаяСсылка",
            Value::FinalizationRegistry(_) => "реестрФинализации",
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
            Value::Array(a) => f.debug_tuple("Array").field(&*a.borrow()).finish(),
            Value::Object(o) => f.debug_tuple("Object").field(&*o.borrow()).finish(),
            Value::Map(m) => {
                let entries: Vec<(Value, Value)> = m.borrow().iter().map(|(k, v)| (k.0.clone(), v.clone())).collect();
                f.debug_tuple("Map").field(&entries).finish()
            }
            Value::Set(s) => {
                let items: Vec<Value> = s.borrow().iter().map(|k| k.0.clone()).collect();
                f.debug_tuple("Set").field(&items).finish()
            }
            Value::Function { name, params, .. } => {
                let param_names: Vec<&str> = params.iter().map(|p| p.name.name.as_str()).collect();
                write!(f, "Function {{ name: {name:?}, params: {param_names:?}, .. }}")
            }
            Value::BuiltinFunction(name) => write!(f, "BuiltinFunction({name:?})"),
            Value::Class(cls) => write!(f, "Class({})", cls.name),
            Value::WeakClass(w) => match w.upgrade() {
                Some(cls) => write!(f, "WeakClass({})", cls.name),
                None => write!(f, "WeakClass(dropped)"),
            },
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
            Value::Date(cell) => write!(f, "Date({})", crate::stdlib::date::format_iso(cell.get())),
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
            Value::AbortRejectPromise { reason_from_signal, .. } => {
                write!(f, "AbortRejectPromise(from_signal={reason_from_signal})")
            }
            Value::ArrayBuffer(buf) => write!(f, "ArrayBuffer({})", buf.borrow().len()),
            Value::TypedArray { offset, length, kind, .. } => {
                write!(f, "TypedArray({}, offset={offset}, length={length})", kind.type_name())
            }
            Value::DataView { offset, length, .. } => write!(f, "DataView(offset={offset}, length={length})"),
            Value::Proxy { target, handler } => {
                f.debug_struct("Proxy").field("target", target).field("handler", handler).finish()
            }
            Value::WeakMap(store) => write!(f, "WeakMap(entries={})", store.borrow().len()),
            Value::WeakSet(store) => write!(f, "WeakSet(entries={})", store.borrow().len()),
            Value::WeakRef(key) => write!(f, "WeakRef(alive={})", key.is_alive()),
            Value::FinalizationRegistry(state) => {
                write!(f, "FinalizationRegistry(entries={})", state.borrow().entries.len())
            }
            Value::Undefined => write!(f, "Undefined"),
            Value::Null => write!(f, "Null"),
        }
    }
}

const VALUE_STACK_RED_ZONE: usize = 256 * 1024;
const VALUE_STACK_GROW_SIZE: usize = 8 * 1024 * 1024;

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut seen: std::collections::HashSet<*const ()> = std::collections::HashSet::new();
        self.fmt_with_seen(f, &mut seen)
    }
}

impl Value {
    fn fmt_with_seen(
        &self,
        f: &mut fmt::Formatter<'_>,
        seen: &mut std::collections::HashSet<*const ()>,
    ) -> fmt::Result {
        stacker::maybe_grow(VALUE_STACK_RED_ZONE, VALUE_STACK_GROW_SIZE, || self.fmt_with_seen_inner(f, seen))
    }

    fn fmt_with_seen_inner(
        &self,
        f: &mut fmt::Formatter<'_>,
        seen: &mut std::collections::HashSet<*const ()>,
    ) -> fmt::Result {
        match self {
            Value::Number(n) => {
                if *n == 0.0 && n.is_sign_negative() {
                    write!(f, "-0")
                } else {
                    write!(f, "{}", crate::interpreter::coercion::number_to_string(*n))
                }
            }
            Value::BigInt(n) => write!(f, "{n}n"),
            Value::String(s) => write!(f, "{s}"),
            Value::Boolean(b) => write!(f, "{b}"),
            Value::Undefined => write!(f, "undefined"),
            Value::Null => write!(f, "null"),
            Value::Array(elements) => {
                let ptr = Rc::as_ptr(elements) as *const ();
                if !seen.insert(ptr) {
                    return write!(f, "[Циклично]");
                }
                let snapshot = elements.borrow().clone();
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
                let snapshot: Vec<(String, Value)> = map
                    .borrow()
                    .iter()
                    .filter(|(k, _)| !crate::symbols::is_internal_key(k))
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();
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
            Value::Map(entries) => {
                let ptr = Rc::as_ptr(entries) as *const ();
                if !seen.insert(ptr) {
                    return write!(f, "[Циклично]");
                }
                let snapshot: Vec<(Value, Value)> =
                    entries.borrow().iter().map(|(k, v)| (k.0.clone(), v.clone())).collect();
                write!(f, "Карта(")?;
                for (i, (k, v)) in snapshot.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    k.fmt_with_seen(f, seen)?;
                    write!(f, " => ")?;
                    v.fmt_with_seen(f, seen)?;
                }
                seen.remove(&ptr);
                write!(f, ")")
            }
            Value::Set(items) => {
                let ptr = Rc::as_ptr(items) as *const ();
                if !seen.insert(ptr) {
                    return write!(f, "[Циклично]");
                }
                let snapshot: Vec<Value> = items.borrow().iter().map(|k| k.0.clone()).collect();
                write!(f, "Набор(")?;
                for (i, v) in snapshot.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    v.fmt_with_seen(f, seen)?;
                }
                seen.remove(&ptr);
                write!(f, ")")
            }
            Value::Function { name, .. } if name.is_empty() => write!(f, "[анонимная функция]"),
            Value::Function { name, .. } => write!(f, "[функция {name}]"),
            Value::BuiltinFunction(name) => write!(f, "[встроенная {name}]"),
            Value::Class(cls) => write!(f, "[класс {}]", cls.name),
            Value::WeakClass(w) => match w.upgrade() {
                Some(cls) => write!(f, "[класс {}]", cls.name),
                None => write!(f, "[класс]"),
            },
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
            Value::Date(cell) => write!(f, "{}", crate::stdlib::date::format_iso(cell.get())),
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
            Value::ArrayBuffer(buf) => write!(f, "ОбластьБайтов({})", buf.borrow().len()),
            Value::TypedArray { buffer, offset, length, kind } => {
                write!(f, "{}[", kind.type_name())?;
                let bytes = buffer.borrow();
                let size = kind.element_size();
                for i in 0..*length {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    let num = kind.read_le(&bytes, offset + i * size);
                    if num.fract() == 0.0 && num.is_finite() {
                        write!(f, "{}", num as i64)?;
                    } else {
                        write!(f, "{num}")?;
                    }
                }
                write!(f, "]")
            }
            Value::DataView { offset, length, .. } => write!(f, "ОбзорБайтов({offset}, {length})"),
            Value::AbortListener { .. }
            | Value::AbortUnsubscribe { .. }
            | Value::AbortCancelTimer { .. }
            | Value::AbortRejectPromise { .. } => write!(f, "[отписка]"),
            Value::Proxy { .. } => write!(f, "[посредник]"),
            Value::WeakMap(_) => write!(f, "[слабаяКарта]"),
            Value::WeakSet(_) => write!(f, "[слабыйНабор]"),
            Value::WeakRef(_) => write!(f, "[слабаяСсылка]"),
            Value::FinalizationRegistry(_) => write!(f, "[реестрФинализации]"),
        }
    }
}

pub fn same_value_zero(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => x == y || (x.is_nan() && y.is_nan()),
        _ => a == b,
    }
}

#[derive(Clone)]
pub struct MapKey(pub Value);

impl MapKey {
    pub fn new(value: Value) -> Self {
        if let Value::Number(n) = value
            && n == 0.0
        {
            return MapKey(Value::Number(0.0));
        }
        MapKey(value)
    }

    pub fn into_value(self) -> Value {
        self.0
    }

    pub fn as_value(&self) -> &Value {
        &self.0
    }
}

impl PartialEq for MapKey {
    fn eq(&self, other: &Self) -> bool {
        same_value_zero(&self.0, &other.0)
    }
}

impl Eq for MapKey {}

fn hash_rc_ptr<T, H: Hasher>(rc: &Rc<T>, state: &mut H) {
    (Rc::as_ptr(rc) as *const () as usize).hash(state);
}

impl Hash for MapKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match &self.0 {
            Value::Number(n) => {
                let canonical = if n.is_nan() {
                    f64::NAN.to_bits()
                } else if *n == 0.0 {
                    0.0f64.to_bits()
                } else {
                    n.to_bits()
                };
                0u8.hash(state);
                canonical.hash(state);
            }
            Value::BigInt(n) => {
                1u8.hash(state);
                n.hash(state);
            }
            Value::String(s) => {
                2u8.hash(state);
                s.hash(state);
            }
            Value::Boolean(b) => {
                3u8.hash(state);
                b.hash(state);
            }
            Value::Symbol { id, .. } => {
                4u8.hash(state);
                id.hash(state);
            }
            Value::RegExp { pattern, flags, .. } => {
                5u8.hash(state);
                pattern.hash(state);
                flags.hash(state);
            }
            Value::Array(rc) => {
                6u8.hash(state);
                hash_rc_ptr(rc, state);
            }
            Value::Object(rc) => {
                7u8.hash(state);
                hash_rc_ptr(rc, state);
            }
            Value::Map(rc) => {
                8u8.hash(state);
                hash_rc_ptr(rc, state);
            }
            Value::Set(rc) => {
                9u8.hash(state);
                hash_rc_ptr(rc, state);
            }
            Value::Class(rc) => {
                10u8.hash(state);
                hash_rc_ptr(rc, state);
            }
            Value::Promise { state: rc } => {
                11u8.hash(state);
                hash_rc_ptr(rc, state);
            }
            Value::Iterator(rc) => {
                12u8.hash(state);
                hash_rc_ptr(rc, state);
            }
            Value::Date(rc) => {
                13u8.hash(state);
                (Rc::as_ptr(rc) as *const () as usize).hash(state);
            }
            Value::ArrayBuffer(rc) => {
                14u8.hash(state);
                hash_rc_ptr(rc, state);
            }
            Value::TypedArray { buffer, offset, length, kind } => {
                15u8.hash(state);
                hash_rc_ptr(buffer, state);
                offset.hash(state);
                length.hash(state);
                (*kind as u8).hash(state);
            }
            Value::DataView { buffer, offset, length } => {
                16u8.hash(state);
                hash_rc_ptr(buffer, state);
                offset.hash(state);
                length.hash(state);
            }
            Value::AbortController { state: rc } => {
                17u8.hash(state);
                hash_rc_ptr(rc, state);
            }
            Value::AbortSignal { state: rc } => {
                18u8.hash(state);
                hash_rc_ptr(rc, state);
            }
            Value::AbortListener { target } => {
                19u8.hash(state);
                (target.as_ptr() as *const () as usize).hash(state);
            }
            Value::AbortUnsubscribe { state: rc, token } => {
                20u8.hash(state);
                hash_rc_ptr(rc, state);
                token.hash(state);
            }
            Value::Proxy { target, handler } => {
                21u8.hash(state);
                hash_rc_ptr(target, state);
                hash_rc_ptr(handler, state);
            }
            Value::WeakMap(rc) => {
                27u8.hash(state);
                hash_rc_ptr(rc, state);
            }
            Value::WeakSet(rc) => {
                28u8.hash(state);
                hash_rc_ptr(rc, state);
            }
            Value::WeakRef(rc) => {
                29u8.hash(state);
                hash_rc_ptr(rc, state);
            }
            Value::FinalizationRegistry(rc) => {
                30u8.hash(state);
                hash_rc_ptr(rc, state);
            }
            Value::Undefined => 22u8.hash(state),
            Value::Null => 23u8.hash(state),
            Value::Function { env, .. } => {
                24u8.hash(state);
                hash_rc_ptr(env, state);
            }
            Value::BuiltinFunction(name) => {
                25u8.hash(state);
                name.hash(state);
            }
            other => {
                26u8.hash(state);
                std::mem::discriminant(other).hash(state);
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
            (Value::Array(a), Value::Array(b)) => Rc::ptr_eq(a, b),
            (Value::Object(a), Value::Object(b)) => Rc::ptr_eq(a, b),
            (Value::Map(a), Value::Map(b)) => Rc::ptr_eq(a, b),
            (Value::Set(a), Value::Set(b)) => Rc::ptr_eq(a, b),
            (Value::Class(a), Value::Class(b)) => Rc::ptr_eq(a, b),
            (Value::WeakClass(a), Value::WeakClass(b)) => Weak::ptr_eq(a, b),
            (Value::Symbol { id: a, .. }, Value::Symbol { id: b, .. }) => a == b,
            (Value::Promise { state: a }, Value::Promise { state: b }) => Rc::ptr_eq(a, b),
            (Value::Iterator(a), Value::Iterator(b)) => Rc::ptr_eq(a, b),
            (Value::Date(a), Value::Date(b)) => Rc::ptr_eq(a, b),
            (Value::RegExp { pattern: pa, flags: fa, .. }, Value::RegExp { pattern: pb, flags: fb, .. }) => {
                pa == pb && fa == fb
            }
            (Value::AbortController { state: a }, Value::AbortController { state: b }) => Rc::ptr_eq(a, b),
            (Value::AbortSignal { state: a }, Value::AbortSignal { state: b }) => Rc::ptr_eq(a, b),
            (Value::AbortListener { target: a }, Value::AbortListener { target: b }) => Weak::ptr_eq(a, b),
            (Value::AbortUnsubscribe { state: a, token: ta }, Value::AbortUnsubscribe { state: b, token: tb }) => {
                Rc::ptr_eq(a, b) && ta == tb
            }
            (Value::ArrayBuffer(a), Value::ArrayBuffer(b)) => Rc::ptr_eq(a, b),
            (
                Value::TypedArray { buffer: ba, offset: oa, length: la, kind: ka },
                Value::TypedArray { buffer: bb, offset: ob, length: lb, kind: kb },
            ) => Rc::ptr_eq(ba, bb) && oa == ob && la == lb && ka == kb,
            (
                Value::DataView { buffer: ba, offset: oa, length: la },
                Value::DataView { buffer: bb, offset: ob, length: lb },
            ) => Rc::ptr_eq(ba, bb) && oa == ob && la == lb,
            (Value::Proxy { target: ta, handler: ha }, Value::Proxy { target: tb, handler: hb }) => {
                Rc::ptr_eq(ta, tb) && Rc::ptr_eq(ha, hb)
            }
            (Value::WeakMap(a), Value::WeakMap(b)) => Rc::ptr_eq(a, b),
            (Value::WeakSet(a), Value::WeakSet(b)) => Rc::ptr_eq(a, b),
            (Value::WeakRef(a), Value::WeakRef(b)) => Rc::ptr_eq(a, b),
            (Value::FinalizationRegistry(a), Value::FinalizationRegistry(b)) => Rc::ptr_eq(a, b),
            (Value::Undefined, Value::Undefined) => true,
            (Value::Null, Value::Null) => true,
            _ => false,
        }
    }
}
