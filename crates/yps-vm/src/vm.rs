use std::cell::RefCell;
use std::cmp::Ordering;
use std::io::{self, Write};
use std::rc::Rc;

use yps_lexer::Span;

use crate::builtins;
use crate::chunk::{ClassBlueprint, Constant, FnProto, MemberKind, Op};
use crate::error::VmError;
use crate::promise::{MacrotaskQueue, Microtask};
use crate::value::{
    CapKind, ClassDef, ClassMembers, Closure, GenState, MethodDef, ObjMap, Upvalue, UpvalueState, Value, abstract_eq,
    strict_eq, to_int32, to_uint32,
};

const MAX_CALL_DEPTH: usize = 1000;
const ERROR_NAME: &str = "Косяк";
const DISPOSE_METHOD: &str = "расход";

type ModuleExports = std::collections::HashMap<String, Value>;
type ModuleCache = Rc<RefCell<std::collections::HashMap<std::path::PathBuf, Rc<ModuleExports>>>>;
type ModuleLoading = Rc<RefCell<std::collections::HashSet<std::path::PathBuf>>>;

enum Step {
    Continue,
    Done,
    Throw(Value),
    Yield(Value),
    YieldDelegate(Value),
}

pub(crate) enum GenInput {
    Send(Value),
    Return(Value),
    Throw(Value),
}

enum GenOutcome {
    Yielded(Value),
    Done(Value),
}

enum GenRun {
    Yielded(Value),
    Done(Value),
    Threw(Value),
    Delegate(Value),
}

enum PendingInject {
    None,
    Return(Value),
    Throw(Value),
}

#[derive(Clone, Copy)]
enum DelegateKind {
    Send,
    Return,
    Throw,
}

const GEN_RETURN_TAG: &str = "\0gen_return";

pub struct CallFrame {
    closure: Rc<Closure>,
    ip: usize,
    base: usize,
    owner: Option<Rc<ClassDef>>,
}

pub struct Handler {
    target: usize,
    frame_len: usize,
    stack_len: usize,
    is_finally: bool,
}

pub struct Vm {
    stack: Vec<Value>,
    frames: Vec<CallFrame>,
    globals: Vec<(String, Value, bool)>,
    open_upvalues: Vec<Upvalue>,
    handlers: Vec<Handler>,
    region_floor: usize,
    gen_yield: Option<Value>,
    disposables: Vec<Value>,
    base_path: Option<std::path::PathBuf>,
    module_cache: ModuleCache,
    module_loading: ModuleLoading,
    exports: ModuleExports,
    pub(crate) microtasks: std::collections::VecDeque<Microtask>,
    pub(crate) macrotasks: MacrotaskQueue,
    out: Box<dyn Write>,
}

impl Default for Vm {
    fn default() -> Self {
        Self::new()
    }
}

impl Vm {
    pub fn new() -> Self {
        Self::with_writer(Box::new(io::stdout()))
    }

    pub fn with_writer(out: Box<dyn Write>) -> Self {
        Vm {
            stack: Vec::new(),
            frames: Vec::new(),
            globals: Vec::new(),
            open_upvalues: Vec::new(),
            handlers: Vec::new(),
            region_floor: 0,
            gen_yield: None,
            disposables: Vec::new(),
            base_path: None,
            module_cache: Rc::new(RefCell::new(std::collections::HashMap::new())),
            module_loading: Rc::new(RefCell::new(std::collections::HashSet::new())),
            exports: std::collections::HashMap::new(),
            microtasks: std::collections::VecDeque::new(),
            macrotasks: MacrotaskQueue::new(),
            out,
        }
    }

    pub fn set_base_path(&mut self, path: std::path::PathBuf) {
        self.base_path = Some(path);
    }

    pub fn run(&mut self, proto: Rc<FnProto>) -> Result<(), VmError> {
        let closure = Rc::new(Closure { proto, upvalues: Vec::new() });
        self.stack.push(Value::Function(Rc::clone(&closure)));
        self.frames.push(CallFrame { closure, ip: 0, base: 0, owner: None });
        self.run_loop()?;
        self.drive_event_loop(Span { start: 0, end: 0 })
    }

    fn global_get(&self, name: &str) -> Option<&Value> {
        self.globals.iter().rev().find(|(n, _, _)| n == name).map(|(_, v, _)| v)
    }

    fn run_loop(&mut self) -> Result<(), VmError> {
        self.run_to_depth(0)
    }

    fn run_to_depth(&mut self, min_depth: usize) -> Result<(), VmError> {
        let saved_floor = self.region_floor;
        self.region_floor = min_depth;
        let result = self.run_region(min_depth);
        self.region_floor = saved_floor;
        result
    }

    fn run_region(&mut self, min_depth: usize) -> Result<(), VmError> {
        loop {
            if self.frames.len() <= min_depth {
                return Ok(());
            }
            let frame_idx = self.frames.len() - 1;
            let closure = Rc::clone(&self.frames[frame_idx].closure);
            let chunk = &closure.proto.chunk;
            let ip = self.frames[frame_idx].ip;
            let op = chunk.code[ip];
            let span = chunk.spans[ip];
            self.frames[frame_idx].ip = ip + 1;
            let base = self.frames[frame_idx].base;

            match self.exec_op(op, span, frame_idx, base, &closure, chunk) {
                Ok(Step::Continue) => {}
                Ok(Step::Done) => return Ok(()),
                Err(err) => {
                    let value = match &err.thrown {
                        Some(v) => (**v).clone(),
                        None => self.error_to_value(err.clone()),
                    };
                    if !self.unwind_to_handler_above(value, min_depth) {
                        return Err(err);
                    }
                }
                Ok(Step::Throw(value)) => {
                    if !self.unwind_to_handler_above(value.clone(), min_depth) {
                        return Err(
                            VmError::new(uncaught_message(&value), Span { start: 0, end: 0 }).with_thrown(value)
                        );
                    }
                }
                Ok(Step::Yield(_) | Step::YieldDelegate(_)) => {
                    return Err(VmError::new("'поебалу' вне генератора", span));
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn exec_op(
        &mut self,
        op: Op,
        span: Span,
        frame_idx: usize,
        base: usize,
        closure: &Rc<Closure>,
        chunk: &crate::chunk::Chunk,
    ) -> Result<Step, VmError> {
        {
            match op {
                Op::Constant(idx) => match &chunk.constants[idx as usize] {
                    Constant::Number(n) => self.stack.push(Value::Number(*n)),
                    Constant::BigInt(n) => self.stack.push(Value::BigInt(*n)),
                    Constant::Str(s) => self.stack.push(Value::Str(Rc::clone(s))),
                    Constant::Proto(_)
                    | Constant::Class(_)
                    | Constant::Template(_)
                    | Constant::RegExp { .. }
                    | Constant::Import(_) => {
                        return Err(VmError::new("нечисловая константа загружена как значение", span));
                    }
                },
                Op::MakeRegex(idx) => {
                    let (pattern, flags) = match &chunk.constants[idx as usize] {
                        Constant::RegExp { pattern, flags } => (Rc::clone(pattern), Rc::clone(flags)),
                        _ => return Err(VmError::new("MakeRegex ожидает константу regex", span)),
                    };
                    let compiled = crate::regexp::compile(&pattern, &flags, span)?;
                    self.stack.push(Value::RegExp { pattern, flags, compiled, last_index: Rc::new(RefCell::new(0)) });
                }
                Op::Null => self.stack.push(Value::Null),
                Op::Undefined => self.stack.push(Value::Undefined),
                Op::True => self.stack.push(Value::Bool(true)),
                Op::False => self.stack.push(Value::Bool(false)),
                Op::Pop => {
                    self.pop();
                }
                Op::Dup => {
                    let v = self.peek(0).clone();
                    self.stack.push(v);
                }
                Op::Dup2 => {
                    let a = self.peek(1).clone();
                    let b = self.peek(0).clone();
                    self.stack.push(a);
                    self.stack.push(b);
                }
                Op::Neg => {
                    let a = self.pop();
                    if let Value::BigInt(n) = a {
                        let neg = n.checked_neg().ok_or_else(|| VmError::new("Переполнение бигцелого", span))?;
                        self.stack.push(Value::BigInt(neg));
                    } else {
                        self.stack.push(Value::Number(-a.to_number()));
                    }
                }
                Op::Pos => {
                    let a = self.pop();
                    if matches!(a, Value::BigInt(_)) {
                        return Err(VmError::new("Нельзя применить унарный '+' к бигцелому", span));
                    }
                    self.stack.push(Value::Number(a.to_number()));
                }
                Op::Not => {
                    let a = self.pop();
                    self.stack.push(Value::Bool(!a.is_truthy()));
                }
                Op::BitNot => {
                    let a = self.pop();
                    self.stack.push(Value::Number(f64::from(!to_int32(a.to_number()))));
                }
                Op::Typeof => {
                    let a = self.pop();
                    self.stack.push(Value::string(a.typeof_str()));
                }
                Op::Add => {
                    if !self.try_bigint_binop(BigOp::Add, span)? {
                        let b = self.pop();
                        let a = self.pop();
                        self.stack.push(add_values(&a, &b));
                    }
                }
                Op::Sub => {
                    if !self.try_bigint_binop(BigOp::Sub, span)? {
                        self.numeric_bin(span, |a, b| a - b)?;
                    }
                }
                Op::Mul => {
                    if !self.try_bigint_binop(BigOp::Mul, span)? {
                        self.numeric_bin(span, |a, b| a * b)?;
                    }
                }
                Op::Div => {
                    if !self.try_bigint_binop(BigOp::Div, span)? {
                        self.numeric_bin(span, |a, b| a / b)?;
                    }
                }
                Op::Mod => {
                    if !self.try_bigint_binop(BigOp::Mod, span)? {
                        self.numeric_bin(span, |a, b| a % b)?;
                    }
                }
                Op::Pow => {
                    if !self.try_bigint_binop(BigOp::Pow, span)? {
                        self.numeric_bin(span, |a, b| a.powf(b))?;
                    }
                }
                Op::BitAnd => self.int_bin(|a, b| a & b),
                Op::BitOr => self.int_bin(|a, b| a | b),
                Op::BitXor => self.int_bin(|a, b| a ^ b),
                Op::Shl => self.shift_bin(|a, s| a.wrapping_shl(s)),
                Op::Shr => self.shift_bin(|a, s| a.wrapping_shr(s)),
                Op::UShr => {
                    let b = self.pop();
                    let a = self.pop();
                    let av = to_uint32(a.to_number());
                    let s = to_uint32(b.to_number()) & 0x1f;
                    self.stack.push(Value::Number(f64::from(av.wrapping_shr(s))));
                }
                Op::Eq => {
                    let b = self.pop();
                    let a = self.pop();
                    self.stack.push(Value::Bool(abstract_eq(&a, &b)));
                }
                Op::Ne => {
                    let b = self.pop();
                    let a = self.pop();
                    self.stack.push(Value::Bool(!abstract_eq(&a, &b)));
                }
                Op::StrictEq => {
                    let b = self.pop();
                    let a = self.pop();
                    self.stack.push(Value::Bool(strict_eq(&a, &b)));
                }
                Op::StrictNe => {
                    let b = self.pop();
                    let a = self.pop();
                    self.stack.push(Value::Bool(!strict_eq(&a, &b)));
                }
                Op::Lt => {
                    if !self.try_bigint_binop(BigOp::Lt, span)? {
                        self.compare(span, |o| o == Ordering::Less)?;
                    }
                }
                Op::Gt => {
                    if !self.try_bigint_binop(BigOp::Gt, span)? {
                        self.compare(span, |o| o == Ordering::Greater)?;
                    }
                }
                Op::Le => {
                    if !self.try_bigint_binop(BigOp::Le, span)? {
                        self.compare(span, |o| o != Ordering::Greater)?;
                    }
                }
                Op::Ge => {
                    if !self.try_bigint_binop(BigOp::Ge, span)? {
                        self.compare(span, |o| o != Ordering::Less)?;
                    }
                }
                Op::In => {
                    let container = self.pop();
                    let key = self.pop();
                    let result = match container {
                        Value::Host(iv) => crate::bridge::host_in(self, &key, &iv, span)?,
                        Value::Object(map) => map.borrow().contains_key(&key.to_ecma_string()),
                        Value::Array(arr) => {
                            let len = arr.borrow().len();
                            match &key {
                                Value::Number(n) => n.fract() == 0.0 && *n >= 0.0 && (*n as usize) < len,
                                other => match other.to_ecma_string().parse::<usize>() {
                                    Ok(idx) => idx < len,
                                    Err(_) => false,
                                },
                            }
                        }
                        other => {
                            return Err(VmError::new(
                                format!(
                                    "Правая сторона 'из' должна быть объектом или массивом, получено '{}'",
                                    other.type_name()
                                ),
                                span,
                            ));
                        }
                    };
                    self.stack.push(Value::Bool(result));
                }

                Op::DefineGlobal(idx, is_const) => {
                    let name = self.const_str(chunk, idx);
                    let value = self.pop();
                    self.globals.retain(|(n, _, _)| *n != name);
                    self.globals.push((name, value, is_const));
                }
                Op::GetGlobal(idx) => {
                    let name = self.const_str(chunk, idx);
                    if let Some(v) = self.global_get(&name) {
                        let v = v.clone();
                        self.stack.push(v);
                    } else if builtins::is_builtin(&name) {
                        self.stack.push(Value::Builtin(Rc::from(name.as_str())));
                    } else if let Some(ns) = crate::bridge::namespace_value(&name) {
                        self.stack.push(ns);
                    } else {
                        return Err(VmError::new(format!("переменная не определена: '{name}'"), span));
                    }
                }
                Op::SetGlobal(idx) => {
                    let name = self.const_str(chunk, idx);
                    let value = self.peek(0).clone();
                    match self.globals.iter_mut().rev().find(|(n, _, _)| *n == name) {
                        Some((_, slot, is_const)) => {
                            if *is_const {
                                return Err(VmError::new(format!("нельзя менять константу '{name}'"), span));
                            }
                            *slot = value;
                        }
                        None => {
                            return Err(VmError::new(format!("переменная не определена: '{name}'"), span));
                        }
                    }
                }
                Op::GetLocal(slot) => {
                    let v = self.stack[base + slot as usize].clone();
                    self.stack.push(v);
                }
                Op::SetLocal(slot) => {
                    let v = self.peek(0).clone();
                    self.stack[base + slot as usize] = v;
                }
                Op::GetUpvalue(slot) => {
                    let uv = Rc::clone(&closure.upvalues[slot as usize]);
                    let v = match &*uv.borrow() {
                        UpvalueState::Open(i) => self.stack.get(*i).cloned().unwrap_or(Value::Undefined),
                        UpvalueState::Closed(v) => v.clone(),
                    };
                    self.stack.push(v);
                }
                Op::SetUpvalue(slot) => {
                    let v = self.peek(0).clone();
                    let uv = Rc::clone(&closure.upvalues[slot as usize]);
                    let target = match &*uv.borrow() {
                        UpvalueState::Open(i) => Some(*i),
                        UpvalueState::Closed(_) => None,
                    };
                    match target {
                        Some(i) if i < self.stack.len() => self.stack[i] = v,
                        Some(_) => {}
                        None => *uv.borrow_mut() = UpvalueState::Closed(v),
                    }
                }
                Op::CloseUpvalue => {
                    let top = self.stack.len() - 1;
                    self.close_upvalues(top);
                    self.pop();
                }
                Op::CloseUpvalueTo(slot) => {
                    self.close_upvalues(base + slot as usize);
                }

                Op::Jump(t) => self.frames[frame_idx].ip = t,
                Op::JumpIfFalse(t) => {
                    let c = self.pop();
                    if !c.is_truthy() {
                        self.frames[frame_idx].ip = t;
                    }
                }
                Op::JumpIfFalsePeek(t) => {
                    if !self.peek(0).is_truthy() {
                        self.frames[frame_idx].ip = t;
                    }
                }
                Op::JumpIfTruePeek(t) => {
                    if self.peek(0).is_truthy() {
                        self.frames[frame_idx].ip = t;
                    }
                }
                Op::JumpIfNullishPeek(t) => {
                    if matches!(self.peek(0), Value::Null | Value::Undefined) {
                        self.frames[frame_idx].ip = t;
                    }
                }
                Op::JumpIfNotNullishPeek(t) => {
                    if !matches!(self.peek(0), Value::Null | Value::Undefined) {
                        self.frames[frame_idx].ip = t;
                    }
                }

                Op::Throw => {
                    let value = self.pop();
                    return Ok(Step::Throw(value));
                }
                Op::PushHandler(target, is_finally) => {
                    self.handlers.push(Handler {
                        target,
                        frame_len: self.frames.len(),
                        stack_len: self.stack.len(),
                        is_finally,
                    });
                }
                Op::PopHandler => {
                    self.handlers.pop();
                }
                Op::ForInKeys => {
                    let src = self.pop();
                    let keys = if let Value::Host(iv) = &src {
                        let iv = iv.clone();
                        crate::bridge::host_for_in_keys(self, &iv, span)?
                    } else {
                        for_in_keys(&src, span)?
                    };
                    self.stack.push(Value::Array(Rc::new(RefCell::new(keys))));
                }
                Op::ForIterInit => {
                    let src = self.pop();
                    let handle = match src {
                        Value::Generator(genrc) => crate::value::ForIter::Generator(genrc),
                        other => crate::value::ForIter::Values { values: self.iterate_values(&other, span)?, index: 0 },
                    };
                    self.stack.push(Value::ForIter(Rc::new(RefCell::new(handle))));
                }
                Op::ForIterNext(target) => {
                    let handle = self.pop();
                    let Value::ForIter(rc) = handle else {
                        return Err(VmError::new("ForIterNext ожидает итератор", span));
                    };
                    let advanced = self.for_iter_next(&rc, span)?;
                    match advanced {
                        Some(value) => self.stack.push(value),
                        None => self.frames[frame_idx].ip = target,
                    }
                }
                Op::ForIterClose => {
                    let handle = self.pop();
                    if let Value::ForIter(rc) = handle {
                        let genrc = match &*rc.borrow() {
                            crate::value::ForIter::Generator(genrc) => Some(Rc::clone(genrc)),
                            crate::value::ForIter::Values { .. } => None,
                        };
                        if let Some(genrc) = genrc
                            && !genrc.borrow().completed
                        {
                            self.gen_return(&genrc, Value::Undefined, span)?;
                        }
                    }
                }
                Op::ArrayLen => {
                    let len = match self.peek(0) {
                        Value::Array(a) => a.borrow().len(),
                        _ => return Err(VmError::new("ArrayLen ожидает массив", span)),
                    };
                    self.pop();
                    self.stack.push(Value::Number(len as f64));
                }

                Op::Call(argc) => self.do_call(argc as usize, span)?,
                Op::CallSpread => {
                    let args_arr = self.pop();
                    let argv: Vec<Value> = match args_arr {
                        Value::Array(a) => a.borrow().clone(),
                        _ => return Err(VmError::new("CallSpread ожидает массив аргументов", span)),
                    };
                    let argc = argv.len();
                    self.stack.extend(argv);
                    self.do_call(argc, span)?;
                }
                Op::Closure(idx) => self.do_closure(chunk, idx, base),
                Op::Return => {
                    let result = self.pop();
                    let frame = self.frames.pop().unwrap();
                    self.close_upvalues(frame.base);
                    self.stack.truncate(frame.base);
                    self.stack.push(result);
                    if self.frames.len() <= self.region_floor {
                        return Ok(Step::Done);
                    }
                }
                Op::Yield => {
                    let value = self.pop();
                    return Ok(Step::Yield(value));
                }
                Op::YieldDelegate => {
                    let iterable = self.pop();
                    return Ok(Step::YieldDelegate(iterable));
                }
                Op::Await => {
                    let value = self.pop();
                    match self.do_await(value, span) {
                        Ok(v) => self.stack.push(v),
                        Err(e) => {
                            let thrown = match e.thrown {
                                Some(v) => *v,
                                None => self.error_to_value(VmError::new(e.message, span)),
                            };
                            return Ok(Step::Throw(thrown));
                        }
                    }
                }
                Op::DynamicImport => {
                    let source = self.pop();
                    let promise = self.dynamic_import(source, span)?;
                    self.stack.push(promise);
                }

                Op::NewArray(n) => {
                    let n = n as usize;
                    let at = self.stack.len() - n;
                    let elems: Vec<Value> = self.stack.split_off(at);
                    self.stack.push(Value::Array(Rc::new(RefCell::new(elems))));
                }
                Op::ArrPush => {
                    let v = self.pop();
                    match self.peek(0) {
                        Value::Array(a) => a.borrow_mut().push(v),
                        _ => return Err(VmError::new("ArrPush ожидает массив", span)),
                    }
                }
                Op::AppendSpread => {
                    let src = self.pop();
                    let items = if let Value::Generator(genrc) = &src {
                        self.drain_generator(&Rc::clone(genrc), span)?
                    } else if let Value::Host(iv) = &src {
                        let iv = iv.clone();
                        crate::bridge::host_iterate(self, &iv, span)?
                    } else {
                        spread_into_values(&src, span)?
                    };
                    match self.peek(0) {
                        Value::Array(a) => a.borrow_mut().extend(items),
                        _ => return Err(VmError::new("AppendSpread ожидает массив", span)),
                    }
                }
                Op::ArrayRest(start) => {
                    let src = self.pop();
                    let start = start as usize;
                    let rest: Vec<Value> = match src {
                        Value::Array(a) => {
                            let arr = a.borrow();
                            if start < arr.len() { arr[start..].to_vec() } else { Vec::new() }
                        }
                        _ => {
                            return Err(VmError::new(
                                format!("Невозможно деструктурировать {} как массив", src.type_name()),
                                span,
                            ));
                        }
                    };
                    self.stack.push(Value::Array(Rc::new(RefCell::new(rest))));
                }
                Op::ObjectRest(key_count) => {
                    let key_count = key_count as usize;
                    let keys: Vec<String> =
                        self.stack.split_off(self.stack.len() - key_count).iter().map(|k| k.to_ecma_string()).collect();
                    let src = self.pop();
                    let mut map = ObjMap::new();
                    match src {
                        Value::Object(m) => {
                            for (k, v) in m.borrow().iter() {
                                if !keys.contains(k) && !crate::value::is_internal_key(k) {
                                    map.insert(k.clone(), v.clone());
                                }
                            }
                        }
                        _ => {
                            return Err(VmError::new(
                                format!("Невозможно деструктурировать {} как объект", src.type_name()),
                                span,
                            ));
                        }
                    }
                    self.stack.push(Value::Object(Rc::new(RefCell::new(map))));
                }
                Op::NewObject(n) => {
                    let n = n as usize;
                    let at = self.stack.len() - n * 2;
                    let flat: Vec<Value> = self.stack.split_off(at);
                    let mut map = ObjMap::new();
                    let mut it = flat.into_iter();
                    while let (Some(k), Some(v)) = (it.next(), it.next()) {
                        map.insert(k.to_ecma_string(), v);
                    }
                    self.stack.push(Value::Object(Rc::new(RefCell::new(map))));
                }
                Op::ObjSet => {
                    let v = self.pop();
                    let k = self.pop();
                    match self.peek(0) {
                        Value::Object(m) => {
                            m.borrow_mut().insert(k.to_ecma_string(), v);
                        }
                        _ => return Err(VmError::new("ObjSet ожидает объект", span)),
                    }
                }
                Op::SpreadObject => {
                    let src = self.pop();
                    match (&src, self.peek(0)) {
                        (Value::Object(s), Value::Object(dst)) => {
                            let pairs: Vec<(String, Value)> = s
                                .borrow()
                                .iter()
                                .filter(|(k, _)| !crate::value::is_internal_key(k))
                                .map(|(k, v)| (k.clone(), v.clone()))
                                .collect();
                            let mut d = dst.borrow_mut();
                            for (k, v) in pairs {
                                d.insert(k, v);
                            }
                        }
                        _ => {
                            return Err(VmError::new(
                                format!("Нельзя развернуть тип '{}' в объект", src.type_name()),
                                span,
                            ));
                        }
                    }
                }
                Op::DefineGetter => {
                    let closure = self.pop();
                    let key = self.pop();
                    match self.peek(0) {
                        Value::Object(m) => {
                            m.borrow_mut().insert(crate::value::getter_key(&key.to_ecma_string()), closure);
                        }
                        _ => return Err(VmError::new("DefineGetter ожидает объект", span)),
                    }
                }
                Op::DefineSetter => {
                    let closure = self.pop();
                    let key = self.pop();
                    match self.peek(0) {
                        Value::Object(m) => {
                            m.borrow_mut().insert(crate::value::setter_key(&key.to_ecma_string()), closure);
                        }
                        _ => return Err(VmError::new("DefineSetter ожидает объект", span)),
                    }
                }
                Op::GetIndex => {
                    let index = self.pop();
                    let obj = self.pop();
                    let result = if let Value::Host(iv) = &obj {
                        let iv = iv.clone();
                        crate::bridge::host_index_get(self, &iv, &index, span)?
                    } else {
                        get_index(&obj, &index, span)?
                    };
                    self.stack.push(result);
                }
                Op::SetIndex => {
                    let value = self.pop();
                    let index = self.pop();
                    let obj = self.pop();
                    if let Value::Host(iv) = &obj {
                        let iv = iv.clone();
                        crate::bridge::host_index_set(self, &iv, &index, &value, span)?;
                    } else {
                        set_index(&obj, &index, value.clone(), span)?;
                    }
                    self.stack.push(value);
                }
                Op::GetProp(idx) => {
                    let name = self.const_str(chunk, idx);
                    let obj = self.pop();
                    let val = self.get_property(&obj, &name, span)?;
                    self.stack.push(val);
                }
                Op::SetProp(idx) => {
                    let name = self.const_str(chunk, idx);
                    let value = self.pop();
                    let obj = self.pop();
                    self.set_property(&obj, &name, value, span)?;
                }
                Op::DeleteProp(idx) => {
                    let name = self.const_str(chunk, idx);
                    let obj = self.pop();
                    if let Value::Object(map) = &obj {
                        map.borrow_mut().remove(&name);
                    }
                    self.stack.push(Value::Bool(true));
                }
                Op::DeleteIndex => {
                    let index = self.pop();
                    let obj = self.pop();
                    match &obj {
                        Value::Object(map) => {
                            map.borrow_mut().remove(&index.to_ecma_string());
                        }
                        Value::Array(a) => {
                            let n = index.to_number();
                            if n.is_finite() && n >= 0.0 && n.fract() == 0.0 {
                                let i = n as usize;
                                let mut arr = a.borrow_mut();
                                if i < arr.len() {
                                    arr[i] = Value::Undefined;
                                }
                            }
                        }
                        Value::Str(_) => {
                            return Err(VmError::new("Нельзя 'ёбнуть' символ строки — строки неизменяемы", span));
                        }
                        _ => {}
                    }
                    self.stack.push(Value::Bool(true));
                }

                Op::RegisterDisposable => {
                    let value = self.peek(0).clone();
                    if !matches!(value, Value::Null | Value::Undefined) && !self.has_dispose_method(&value) {
                        return Err(VmError::new("Ресурс 'юзай' должен иметь метод 'расход'", span));
                    }
                    self.disposables.push(value);
                }
                Op::DisposeScope(count) => {
                    self.dispose_scope(count as usize, span)?;
                }
                Op::Import(idx) => {
                    let request = match &chunk.constants[idx as usize] {
                        Constant::Import(req) => Rc::clone(req),
                        _ => return Err(VmError::new("Import ожидает дескриптор импорта", span)),
                    };
                    self.do_import(&request, span)?;
                }
                Op::RecordExport(idx) => {
                    let name = self.const_str(chunk, idx);
                    if let Some(value) = self.global_get(&name).cloned() {
                        self.exports.insert(name, value);
                    }
                }

                Op::BuildClass(idx) => self.build_class(chunk, idx, span)?,
                Op::New(argc) => {
                    let argc = argc as usize;
                    let args: Vec<Value> = self.stack.split_off(self.stack.len() - argc);
                    let callee = self.pop();
                    let instance = self.instantiate(callee, args, span)?;
                    self.stack.push(instance);
                }
                Op::NewSpread => {
                    let args_arr = self.pop();
                    let args: Vec<Value> = match args_arr {
                        Value::Array(a) => a.borrow().clone(),
                        _ => return Err(VmError::new("NewSpread ожидает массив аргументов", span)),
                    };
                    let callee = self.pop();
                    let instance = self.instantiate(callee, args, span)?;
                    self.stack.push(instance);
                }
                Op::Invoke(idx, argc) => {
                    let name = self.const_str(chunk, idx);
                    self.do_invoke(&name, argc as usize, span)?;
                }
                Op::Instanceof => {
                    let target = self.pop();
                    let value = self.pop();
                    let result = match target {
                        Value::Class(cls) => self.instance_of(&value, &cls),
                        _ => {
                            return Err(VmError::new(
                                format!(
                                    "Правая сторона 'шкура' должна быть классом, получено '{}'",
                                    target.type_name()
                                ),
                                span,
                            ));
                        }
                    };
                    self.stack.push(Value::Bool(result));
                }
                Op::SuperCall(argc) => {
                    let argc = argc as usize;
                    let args: Vec<Value> = self.stack.split_off(self.stack.len() - argc);
                    let this = self.stack[base].clone();
                    let owner = self.frames[frame_idx].owner.clone();
                    self.super_call(owner, this, args, span)?;
                    self.stack.push(Value::Undefined);
                }
                Op::SuperCallSpread => {
                    let args = self.pop_spread_args(span)?;
                    let this = self.stack[base].clone();
                    let owner = self.frames[frame_idx].owner.clone();
                    self.super_call(owner, this, args, span)?;
                    self.stack.push(Value::Undefined);
                }
                Op::SuperGet(idx) => {
                    let name = self.const_str(chunk, idx);
                    let this = self.stack[base].clone();
                    let owner = self.frames[frame_idx].owner.clone();
                    let val = self.super_get(owner, this, &name, span)?;
                    self.stack.push(val);
                }
                Op::SuperInvoke(idx, argc) => {
                    let name = self.const_str(chunk, idx);
                    let argc = argc as usize;
                    let args: Vec<Value> = self.stack.split_off(self.stack.len() - argc);
                    let this = self.stack[base].clone();
                    let owner = self.frames[frame_idx].owner.clone();
                    let result = self.super_invoke(owner, this, &name, args, span)?;
                    self.stack.push(result);
                }
                Op::SuperInvokeSpread(idx) => {
                    let name = self.const_str(chunk, idx);
                    let args = self.pop_spread_args(span)?;
                    let this = self.stack[base].clone();
                    let owner = self.frames[frame_idx].owner.clone();
                    let result = self.super_invoke(owner, this, &name, args, span)?;
                    self.stack.push(result);
                }
                Op::TaggedTemplate(idx) => {
                    let strings = match &chunk.constants[idx as usize] {
                        Constant::Template(t) => Rc::clone(t),
                        _ => return Err(VmError::new("TaggedTemplate ожидает шаблон", span)),
                    };
                    let obj = build_template_strings(&strings);
                    self.stack.push(obj);
                }
            }
        }
        Ok(Step::Continue)
    }

    fn unwind_to_handler_above(&mut self, value: Value, min_depth: usize) -> bool {
        while let Some(handler) = self.handlers.last() {
            if handler.frame_len <= min_depth {
                break;
            }
            let handler = self.handlers.pop().unwrap();
            if handler.frame_len > self.frames.len() {
                continue;
            }
            self.frames.truncate(handler.frame_len);
            if handler.stack_len <= self.stack.len() {
                self.close_upvalues(handler.stack_len);
                self.stack.truncate(handler.stack_len);
            }
            self.stack.push(value);
            let top = self.frames.len() - 1;
            self.frames[top].ip = handler.target;
            return true;
        }
        false
    }

    fn error_to_value(&self, err: VmError) -> Value {
        let mut map = ObjMap::new();
        map.insert("name".to_string(), Value::string(ERROR_NAME));
        map.insert("message".to_string(), Value::string(err.message));
        Value::Object(Rc::new(RefCell::new(map)))
    }

    fn const_str(&self, chunk: &crate::chunk::Chunk, idx: u32) -> String {
        match &chunk.constants[idx as usize] {
            Constant::Str(s) => s.to_string(),
            _ => String::new(),
        }
    }

    fn do_closure(&mut self, chunk: &crate::chunk::Chunk, idx: u32, base: usize) {
        let Constant::Proto(proto) = &chunk.constants[idx as usize] else {
            return;
        };
        let proto = Rc::clone(proto);
        let mut upvalues = Vec::with_capacity(proto.upvalues.len());
        let parent = Rc::clone(&self.frames.last().unwrap().closure);
        for desc in &proto.upvalues {
            if desc.from_parent_local {
                upvalues.push(self.capture_upvalue(base + desc.index));
            } else {
                upvalues.push(Rc::clone(&parent.upvalues[desc.index]));
            }
        }
        self.stack.push(Value::Function(Rc::new(Closure { proto, upvalues })));
    }

    fn do_call(&mut self, argc: usize, span: Span) -> Result<(), VmError> {
        let callee_idx = self.stack.len() - 1 - argc;
        let callee = self.stack[callee_idx].clone();
        match callee {
            Value::Builtin(name) => {
                let args: Vec<Value> = self.stack.split_off(self.stack.len() - argc);
                self.pop();
                let result = self.call_builtin_value(&name, args, span)?;
                self.stack.push(result);
                Ok(())
            }
            Value::Function(closure) => {
                if closure.proto.is_generator {
                    let args: Vec<Value> = self.stack.split_off(self.stack.len() - argc);
                    self.pop();
                    let genrc = self.make_generator(closure, Value::Undefined, None, args);
                    self.stack.push(genrc);
                    return Ok(());
                }
                if closure.proto.is_async {
                    let args: Vec<Value> = self.stack.split_off(self.stack.len() - argc);
                    self.pop();
                    let promise = self.spawn_async(closure, None, None, args);
                    self.stack.push(promise);
                    return Ok(());
                }
                if self.frames.len() >= MAX_CALL_DEPTH {
                    return Err(VmError::new("переполнение стека вызовов", span));
                }
                let arity = closure.proto.arity;
                let has_rest = closure.proto.has_rest;
                let fixed = if has_rest { arity.saturating_sub(1) } else { arity };
                let args: Vec<Value> = self.stack.split_off(self.stack.len() - argc);
                let base = self.stack.len() - 1;
                for i in 0..fixed {
                    self.stack.push(args.get(i).cloned().unwrap_or(Value::Undefined));
                }
                if has_rest {
                    let rest: Vec<Value> = if args.len() > fixed { args[fixed..].to_vec() } else { Vec::new() };
                    self.stack.push(Value::Array(Rc::new(RefCell::new(rest))));
                }
                self.frames.push(CallFrame { closure, ip: 0, base, owner: None });
                Ok(())
            }
            Value::Class(_) => {
                let args: Vec<Value> = self.stack.split_off(self.stack.len() - argc);
                self.pop();
                let instance = self.instantiate(callee, args, span)?;
                self.stack.push(instance);
                Ok(())
            }
            Value::PromiseCapability { .. }
            | Value::PromiseThenHandler { .. }
            | Value::PromiseFinallyHandler { .. }
            | Value::PromiseAggregateHandler { .. } => {
                let args: Vec<Value> = self.stack.split_off(self.stack.len() - argc);
                self.pop();
                let result = self.call_value(callee, None, &args, span)?;
                self.stack.push(result);
                Ok(())
            }
            Value::Host(iv) => {
                let args: Vec<Value> = self.stack.split_off(self.stack.len() - argc);
                self.pop();
                let result = crate::bridge::host_call(self, &iv, &args, span)?;
                self.stack.push(result);
                Ok(())
            }
            other => Err(VmError::new(format!("значение типа '{}' не является функцией", other.type_name()), span)),
        }
    }

    fn push_call_frame(
        &mut self,
        closure: Rc<Closure>,
        this: Option<Value>,
        owner: Option<Rc<ClassDef>>,
        args: &[Value],
        span: Span,
    ) -> Result<(), VmError> {
        if self.frames.len() >= MAX_CALL_DEPTH {
            return Err(VmError::new("переполнение стека вызовов", span));
        }
        let arity = closure.proto.arity;
        let has_rest = closure.proto.has_rest;
        let fixed = if has_rest { arity.saturating_sub(1) } else { arity };
        let base = self.stack.len();
        self.stack.push(this.unwrap_or(Value::Undefined));
        for i in 0..fixed {
            self.stack.push(args.get(i).cloned().unwrap_or(Value::Undefined));
        }
        if has_rest {
            let rest: Vec<Value> = if args.len() > fixed { args[fixed..].to_vec() } else { Vec::new() };
            self.stack.push(Value::Array(Rc::new(RefCell::new(rest))));
        }
        self.frames.push(CallFrame { closure, ip: 0, base, owner });
        Ok(())
    }

    fn call_closure_sync(
        &mut self,
        closure: Rc<Closure>,
        this: Option<Value>,
        owner: Option<Rc<ClassDef>>,
        args: &[Value],
        span: Span,
    ) -> Result<Value, VmError> {
        let target_depth = self.frames.len();
        self.push_call_frame(closure, this, owner, args, span)?;
        self.run_to_depth(target_depth)?;
        Ok(self.pop())
    }

    pub(crate) fn spawn_async(
        &mut self,
        closure: Rc<Closure>,
        this: Option<Value>,
        owner: Option<Rc<ClassDef>>,
        args: Vec<Value>,
    ) -> Value {
        let (promise, _resolve, _reject) = crate::promise::make_pending_promise();
        let outer_state = match &promise {
            Value::Promise { state } => Rc::clone(state),
            _ => unreachable!(),
        };
        self.enqueue_microtask(Box::new(move |vm, sp| {
            let (kind, value) = match vm.call_closure_sync(closure, this, owner, &args, sp) {
                Ok(val) => (CapKind::Resolve, val),
                Err(e) => match e.thrown {
                    Some(val) => (CapKind::Reject, *val),
                    None => (CapKind::Reject, vm.error_to_value(VmError::new(e.message, sp))),
                },
            };
            Vm::settle_promise(&outer_state, kind, value, vm, sp)
        }));
        promise
    }

    pub(crate) fn error_object(&self, message: String) -> Value {
        self.error_to_value(VmError::new(message, Span { start: 0, end: 0 }))
    }

    pub(crate) fn call_builtin_value(&mut self, name: &str, args: Vec<Value>, span: Span) -> Result<Value, VmError> {
        if name == "СловоПацана" {
            return self.promise_construct(args, span);
        }
        if let Some(method) = name.strip_prefix("СловоПацана.") {
            return crate::promise::call_promise_static(self, method, args, span);
        }
        if let Some(res) = self.try_call_timer_builtin(name, &args, span) {
            return res;
        }
        if crate::bridge::is_host_callback(name) {
            return crate::bridge::call_host_callback(self, name, args, span);
        }
        if crate::bridge::is_bridged_call(name) {
            return crate::bridge::call_bridged(self, name, args, span);
        }
        builtins::call_builtin(&mut *self.out, name, args, span)
    }

    fn promise_construct(&mut self, args: Vec<Value>, span: Span) -> Result<Value, VmError> {
        let executor = args.into_iter().next().unwrap_or(Value::Undefined);
        if !matches!(executor, Value::Function(_) | Value::Builtin(_)) {
            return Err(VmError::new("'СловоПацана' ожидает функцию-исполнитель", span));
        }
        let (promise, resolve, reject) = crate::promise::make_pending_promise();
        if let Err(e) = self.call_value(executor, None, &[resolve, reject.clone()], span) {
            match e.thrown {
                Some(val) => {
                    self.call_value(reject, None, &[*val], span)?;
                }
                None => return Err(e),
            }
        }
        Ok(promise)
    }

    fn try_call_timer_builtin(&mut self, name: &str, args: &[Value], span: Span) -> Option<Result<Value, VmError>> {
        match name {
            "чутка" => Some(self.builtin_schedule_timeout(args, span)),
            "отменаЧутки" | "отменаИнтервала" => Some(self.builtin_cancel_timer(args, span)),
            "интервал" => Some(self.builtin_schedule_interval(args, span)),
            "сразу" => Some(self.builtin_queue_microtask(args, span, false)),
            "наСледующемТике" => Some(self.builtin_queue_microtask(args, span, true)),
            "подождать" => Some(self.builtin_wait_promise(args, span)),
            "сОчередить" => Some(self.builtin_queue_microtask_promise(args, span)),
            _ => None,
        }
    }

    fn builtin_schedule_timeout(&mut self, args: &[Value], span: Span) -> Result<Value, VmError> {
        let cb = timer_callback(args.first().cloned(), "чутка", span)?;
        let ms = parse_delay_ms(args.get(1).cloned(), "чутка", span)?;
        let id = self.macrotasks.schedule(
            std::time::Duration::from_millis(ms),
            Box::new(move |vm, sp| {
                if let Err(e) = vm.call_value(cb, None, &[], sp) {
                    report_async_error("чутка", &e);
                }
                Ok(())
            }),
        );
        Ok(Value::Number(id as f64))
    }

    fn builtin_cancel_timer(&mut self, args: &[Value], span: Span) -> Result<Value, VmError> {
        match args.first() {
            Some(Value::Number(n)) if n.is_finite() && *n >= 0.0 => self.macrotasks.cancel(*n as u64),
            Some(Value::Undefined) | None => return Ok(Value::Undefined),
            Some(other) => {
                return Err(VmError::new(
                    format!("Идентификатор таймера должен быть числом, получено '{}'", other.type_name()),
                    span,
                ));
            }
        }
        Ok(Value::Undefined)
    }

    fn builtin_schedule_interval(&mut self, args: &[Value], span: Span) -> Result<Value, VmError> {
        let cb = timer_callback(args.first().cloned(), "интервал", span)?;
        let ms = parse_delay_ms(args.get(1).cloned(), "интервал", span)?;
        let id = self.macrotasks.allocate_id();
        self.schedule_interval_tick(id, ms, cb);
        Ok(Value::Number(id as f64))
    }

    fn schedule_interval_tick(&mut self, id: u64, ms: u64, cb: Value) {
        let cb_clone = cb.clone();
        self.macrotasks.schedule_with_id(
            id,
            std::time::Duration::from_millis(ms),
            Box::new(move |vm, sp| {
                if let Err(e) = vm.call_value(cb_clone, None, &[], sp) {
                    report_async_error("интервал", &e);
                }
                if !vm.macrotasks.cancelled.contains(&id) {
                    vm.schedule_interval_tick(id, ms, cb);
                }
                Ok(())
            }),
        );
    }

    fn builtin_queue_microtask(&mut self, args: &[Value], span: Span, front: bool) -> Result<Value, VmError> {
        let name = if front { "наСледующемТике" } else { "сразу" };
        let cb = timer_callback(args.first().cloned(), name, span)?;
        let task: Microtask = Box::new(move |vm, sp| vm.call_value(cb, None, &[], sp).map(|_| ()));
        if front {
            self.microtasks.push_front(task);
        } else {
            self.microtasks.push_back(task);
        }
        Ok(Value::Undefined)
    }

    fn builtin_queue_microtask_promise(&mut self, args: &[Value], span: Span) -> Result<Value, VmError> {
        let cb = timer_callback(args.first().cloned(), "сОчередить", span)?;
        self.enqueue_microtask(Box::new(move |vm, sp| {
            if let Err(e) = vm.call_value(cb, None, &[], sp) {
                report_async_error("сОчередить", &e);
            }
            Ok(())
        }));
        Ok(Value::Undefined)
    }

    fn builtin_wait_promise(&mut self, args: &[Value], span: Span) -> Result<Value, VmError> {
        let ms = parse_delay_ms(args.first().cloned(), "подождать", span)?;
        let (promise, resolve_cap, _reject) = crate::promise::make_pending_promise();
        self.macrotasks.schedule(
            std::time::Duration::from_millis(ms),
            Box::new(move |vm, sp| {
                if let Value::PromiseCapability { state, kind } = resolve_cap {
                    let _ = Vm::settle_promise(&state, kind, Value::Undefined, vm, sp);
                }
                Ok(())
            }),
        );
        Ok(promise)
    }

    pub(crate) fn load_module_exports(&mut self, source: &str, span: Span) -> Result<Rc<ModuleExports>, VmError> {
        self.load_module(source, span)
    }

    fn make_generator(
        &self,
        closure: Rc<Closure>,
        this: Value,
        owner: Option<Rc<ClassDef>>,
        args: Vec<Value>,
    ) -> Value {
        let genrc = GenState {
            closure,
            owner,
            started: false,
            completed: false,
            stack: Vec::new(),
            frames: Vec::new(),
            handlers: Vec::new(),
            open_upvalues: Vec::new(),
            this,
            args,
            delegate: None,
        };
        Value::Generator(Rc::new(RefCell::new(genrc)))
    }

    fn for_iter_next(&mut self, rc: &Rc<RefCell<crate::value::ForIter>>, span: Span) -> Result<Option<Value>, VmError> {
        let genrc = match &mut *rc.borrow_mut() {
            crate::value::ForIter::Values { values, index } => {
                if *index < values.len() {
                    let v = values[*index].clone();
                    *index += 1;
                    return Ok(Some(v));
                }
                return Ok(None);
            }
            crate::value::ForIter::Generator(genrc) => Rc::clone(genrc),
        };
        if genrc.borrow().completed {
            return Ok(None);
        }
        let result = self.gen_next(&genrc, Value::Undefined, span)?;
        match result {
            Value::Object(map) => {
                let done = matches!(map.borrow().get("готово"), Some(Value::Bool(true)));
                if done {
                    Ok(None)
                } else {
                    Ok(Some(map.borrow().get("значение").cloned().unwrap_or(Value::Undefined)))
                }
            }
            _ => Ok(None),
        }
    }

    fn drain_generator(&mut self, genrc: &Rc<RefCell<GenState>>, span: Span) -> Result<Vec<Value>, VmError> {
        let mut out = Vec::new();
        loop {
            if genrc.borrow().completed {
                break;
            }
            match self.step_generator(genrc, GenInput::Send(Value::Undefined), span)? {
                GenOutcome::Yielded(v) => out.push(v),
                GenOutcome::Done(_) => break,
            }
        }
        Ok(out)
    }

    fn gen_iter_result(&self, value: Value, done: bool) -> Value {
        let mut map = ObjMap::new();
        map.insert("значение".to_string(), value.clone());
        map.insert("value".to_string(), value);
        map.insert("готово".to_string(), Value::Bool(done));
        map.insert("done".to_string(), Value::Bool(done));
        Value::Object(Rc::new(RefCell::new(map)))
    }

    fn gen_next(&mut self, genrc: &Rc<RefCell<GenState>>, sent: Value, span: Span) -> Result<Value, VmError> {
        if genrc.borrow().completed {
            return Ok(self.gen_iter_result(Value::Undefined, true));
        }
        match self.step_generator(genrc, GenInput::Send(sent), span)? {
            GenOutcome::Yielded(v) => Ok(self.gen_iter_result(v, false)),
            GenOutcome::Done(v) => Ok(self.gen_iter_result(v, true)),
        }
    }

    fn gen_return(&mut self, genrc: &Rc<RefCell<GenState>>, value: Value, span: Span) -> Result<Value, VmError> {
        if genrc.borrow().completed {
            return Ok(self.gen_iter_result(value, true));
        }
        match self.step_generator(genrc, GenInput::Return(value), span)? {
            GenOutcome::Yielded(v) => Ok(self.gen_iter_result(v, false)),
            GenOutcome::Done(v) => Ok(self.gen_iter_result(v, true)),
        }
    }

    fn gen_throw(&mut self, genrc: &Rc<RefCell<GenState>>, value: Value, span: Span) -> Result<Value, VmError> {
        if genrc.borrow().completed {
            return Err(VmError::new(uncaught_message(&value), span).with_thrown(value));
        }
        match self.step_generator(genrc, GenInput::Throw(value), span)? {
            GenOutcome::Yielded(v) => Ok(self.gen_iter_result(v, false)),
            GenOutcome::Done(v) => Ok(self.gen_iter_result(v, true)),
        }
    }

    fn step_generator(
        &mut self,
        genrc: &Rc<RefCell<GenState>>,
        input: GenInput,
        span: Span,
    ) -> Result<GenOutcome, VmError> {
        if genrc.borrow().delegate.is_some() {
            return self.step_delegate(genrc, input, span);
        }

        let saved_stack = std::mem::take(&mut self.stack);
        let saved_frames = std::mem::take(&mut self.frames);
        let saved_handlers = std::mem::take(&mut self.handlers);
        let saved_upvalues = std::mem::take(&mut self.open_upvalues);
        let saved_floor = self.region_floor;
        let saved_yield = self.gen_yield.take();

        let started = genrc.borrow().started;
        {
            let mut g = genrc.borrow_mut();
            self.stack = std::mem::take(&mut g.stack);
            self.frames = std::mem::take(&mut g.frames);
            self.handlers = std::mem::take(&mut g.handlers);
            self.open_upvalues = std::mem::take(&mut g.open_upvalues);
        }

        let mut pending = PendingInject::None;
        if !started {
            let (closure, this, owner, args) = {
                let g = genrc.borrow();
                (Rc::clone(&g.closure), g.this.clone(), g.owner.clone(), g.args.clone())
            };
            genrc.borrow_mut().started = true;
            let r = self.push_call_frame(closure, Some(this), owner, &args, span);
            if let Err(e) = r {
                self.restore_main(saved_stack, saved_frames, saved_handlers, saved_upvalues, saved_floor, saved_yield);
                return Err(e);
            }
            match input {
                GenInput::Send(_) => {}
                GenInput::Return(v) => pending = PendingInject::Return(v),
                GenInput::Throw(v) => pending = PendingInject::Throw(v),
            }
        } else {
            match input {
                GenInput::Send(v) => self.stack.push(v),
                GenInput::Return(v) => pending = PendingInject::Return(v),
                GenInput::Throw(v) => pending = PendingInject::Throw(v),
            }
        }

        self.region_floor = 0;
        let outcome = self.run_generator(&mut pending, span);

        {
            let mut g = genrc.borrow_mut();
            g.stack = std::mem::take(&mut self.stack);
            g.frames = std::mem::take(&mut self.frames);
            g.handlers = std::mem::take(&mut self.handlers);
            g.open_upvalues = std::mem::take(&mut self.open_upvalues);
        }
        self.restore_main(saved_stack, saved_frames, saved_handlers, saved_upvalues, saved_floor, saved_yield);

        match outcome {
            Ok(GenRun::Yielded(v)) => Ok(GenOutcome::Yielded(v)),
            Ok(GenRun::Done(v)) => {
                genrc.borrow_mut().completed = true;
                Ok(GenOutcome::Done(v))
            }
            Ok(GenRun::Threw(v)) => {
                genrc.borrow_mut().completed = true;
                Err(VmError::new(uncaught_message(&v), span).with_thrown(v))
            }
            Ok(GenRun::Delegate(iterable)) => {
                genrc.borrow_mut().completed = false;
                self.begin_delegate(genrc, iterable, span)?;
                self.step_delegate(genrc, GenInput::Send(Value::Undefined), span)
            }
            Err(e) => {
                genrc.borrow_mut().completed = true;
                Err(e)
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn restore_main(
        &mut self,
        stack: Vec<Value>,
        frames: Vec<CallFrame>,
        handlers: Vec<Handler>,
        upvalues: Vec<Upvalue>,
        floor: usize,
        yield_slot: Option<Value>,
    ) {
        self.stack = stack;
        self.frames = frames;
        self.handlers = handlers;
        self.open_upvalues = upvalues;
        self.region_floor = floor;
        self.gen_yield = yield_slot;
    }

    fn run_generator(&mut self, pending: &mut PendingInject, span: Span) -> Result<GenRun, VmError> {
        if let Some(step) = self.apply_pending(pending, span)? {
            return Ok(step);
        }
        loop {
            if self.frames.is_empty() {
                let result = self.stack.pop().unwrap_or(Value::Undefined);
                return Ok(GenRun::Done(result));
            }
            let frame_idx = self.frames.len() - 1;
            let closure = Rc::clone(&self.frames[frame_idx].closure);
            let chunk = &closure.proto.chunk;
            let ip = self.frames[frame_idx].ip;
            let op = chunk.code[ip];
            let op_span = chunk.spans[ip];

            if matches!(op, Op::Return) && frame_idx == 0 {
                let result = self.stack.pop().unwrap_or(Value::Undefined);
                let frame = self.frames.pop().unwrap();
                self.close_upvalues(frame.base);
                self.stack.truncate(frame.base);
                return Ok(GenRun::Done(result));
            }

            self.frames[frame_idx].ip = ip + 1;
            let base = self.frames[frame_idx].base;

            match self.exec_op(op, op_span, frame_idx, base, &closure, chunk) {
                Ok(Step::Continue) => {}
                Ok(Step::Done) => {
                    let result = self.stack.pop().unwrap_or(Value::Undefined);
                    return Ok(GenRun::Done(result));
                }
                Ok(Step::Yield(v)) => return Ok(GenRun::Yielded(v)),
                Ok(Step::YieldDelegate(it)) => return Ok(GenRun::Delegate(it)),
                Ok(Step::Throw(value)) => {
                    if let Some(ret) = as_return_token(&value) {
                        self.frames.clear();
                        self.stack.clear();
                        self.handlers.clear();
                        return Ok(GenRun::Done(ret));
                    } else if let Some(step) = self.gen_unwind_throw(value, span)? {
                        return Ok(step);
                    }
                }
                Err(err) => {
                    let value = match &err.thrown {
                        Some(v) => (**v).clone(),
                        None => self.error_to_value(err.clone()),
                    };
                    if let Some(step) = self.gen_unwind_throw(value, span)? {
                        return Ok(step);
                    }
                }
            }
        }
    }

    fn apply_pending(&mut self, pending: &mut PendingInject, span: Span) -> Result<Option<GenRun>, VmError> {
        match std::mem::replace(pending, PendingInject::None) {
            PendingInject::None => Ok(None),
            PendingInject::Throw(v) => Ok(self.gen_unwind_throw(v, span)?),
            PendingInject::Return(v) => Ok(self.gen_unwind_return(v, span)?),
        }
    }

    fn gen_unwind_throw(&mut self, value: Value, _span: Span) -> Result<Option<GenRun>, VmError> {
        while let Some(handler) = self.handlers.pop() {
            if handler.frame_len > self.frames.len() {
                continue;
            }
            self.frames.truncate(handler.frame_len);
            if handler.stack_len <= self.stack.len() {
                self.close_upvalues(handler.stack_len);
                self.stack.truncate(handler.stack_len);
            }
            self.stack.push(value);
            let top = self.frames.len() - 1;
            self.frames[top].ip = handler.target;
            return Ok(None);
        }
        Ok(Some(GenRun::Threw(value)))
    }

    fn gen_unwind_return(&mut self, value: Value, _span: Span) -> Result<Option<GenRun>, VmError> {
        while let Some(handler) = self.handlers.last() {
            if !handler.is_finally {
                self.handlers.pop();
                continue;
            }
            let handler = self.handlers.pop().unwrap();
            if handler.frame_len > self.frames.len() {
                continue;
            }
            self.frames.truncate(handler.frame_len);
            if handler.stack_len <= self.stack.len() {
                self.close_upvalues(handler.stack_len);
                self.stack.truncate(handler.stack_len);
            }
            let token = self.make_return_token(value);
            self.stack.push(token);
            let top = self.frames.len() - 1;
            self.frames[top].ip = handler.target;
            return Ok(None);
        }
        Ok(Some(GenRun::Done(value)))
    }

    fn make_return_token(&self, value: Value) -> Value {
        let mut map = ObjMap::new();
        map.insert(GEN_RETURN_TAG.to_string(), value);
        Value::Object(Rc::new(RefCell::new(map)))
    }

    fn iterate_values(&mut self, src: &Value, span: Span) -> Result<Vec<Value>, VmError> {
        if let Value::Host(iv) = src {
            let iv = iv.clone();
            return crate::bridge::host_iterate(self, &iv, span);
        }
        for_of_values(src, span)
    }

    fn begin_delegate(&mut self, genrc: &Rc<RefCell<GenState>>, iterable: Value, span: Span) -> Result<(), VmError> {
        let delegate = match iterable {
            Value::Generator(inner) => crate::value::Delegate::Generator(inner),
            other => {
                let values = self.iterate_values(&other, span)?;
                crate::value::Delegate::Values { values, index: 0 }
            }
        };
        genrc.borrow_mut().delegate = Some(delegate);
        Ok(())
    }

    fn step_delegate(
        &mut self,
        genrc: &Rc<RefCell<GenState>>,
        input: GenInput,
        span: Span,
    ) -> Result<GenOutcome, VmError> {
        let inner = match genrc.borrow_mut().delegate.take() {
            Some(d) => d,
            None => return Err(VmError::new("делегирование без итератора", span)),
        };
        match inner {
            crate::value::Delegate::Generator(inner_gen) => {
                let (kind, arg) = match input {
                    GenInput::Send(v) => (DelegateKind::Send, v),
                    GenInput::Return(v) => (DelegateKind::Return, v),
                    GenInput::Throw(v) => (DelegateKind::Throw, v),
                };
                let result = match kind {
                    DelegateKind::Send => self.step_generator(&inner_gen, GenInput::Send(arg.clone()), span),
                    DelegateKind::Return => self.step_generator(&inner_gen, GenInput::Return(arg.clone()), span),
                    DelegateKind::Throw => self.step_generator(&inner_gen, GenInput::Throw(arg.clone()), span),
                };
                match result {
                    Ok(GenOutcome::Yielded(v)) => {
                        genrc.borrow_mut().delegate = Some(crate::value::Delegate::Generator(inner_gen));
                        Ok(GenOutcome::Yielded(v))
                    }
                    Ok(GenOutcome::Done(ret)) => {
                        genrc.borrow_mut().delegate = None;
                        match kind {
                            DelegateKind::Send => self.step_generator(genrc, GenInput::Send(ret), span),
                            DelegateKind::Return => self.step_generator(genrc, GenInput::Return(arg), span),
                            DelegateKind::Throw => self.step_generator(genrc, GenInput::Throw(arg), span),
                        }
                    }
                    Err(e) => {
                        genrc.borrow_mut().delegate = None;
                        Err(e)
                    }
                }
            }
            crate::value::Delegate::Values { values, mut index } => match input {
                GenInput::Send(_) => {
                    if index < values.len() {
                        let v = values[index].clone();
                        index += 1;
                        genrc.borrow_mut().delegate = Some(crate::value::Delegate::Values { values, index });
                        Ok(GenOutcome::Yielded(v))
                    } else {
                        genrc.borrow_mut().delegate = None;
                        self.step_generator(genrc, GenInput::Send(Value::Undefined), span)
                    }
                }
                GenInput::Return(v) => {
                    genrc.borrow_mut().delegate = None;
                    self.step_generator(genrc, GenInput::Return(v), span)
                }
                GenInput::Throw(v) => {
                    genrc.borrow_mut().delegate = None;
                    self.step_generator(genrc, GenInput::Throw(v), span)
                }
            },
        }
    }

    fn build_class(&mut self, chunk: &crate::chunk::Chunk, idx: u32, span: Span) -> Result<(), VmError> {
        let blueprint: Rc<ClassBlueprint> = match &chunk.constants[idx as usize] {
            Constant::Class(b) => Rc::clone(b),
            _ => return Err(VmError::new("BuildClass ожидает чертёж класса", span)),
        };

        let class_decorators = self.pop_decorators(blueprint.class_decorator_count as usize);
        let mut member_decorators: Vec<Vec<Value>> = Vec::with_capacity(blueprint.members.len());
        for desc in blueprint.members.iter().rev() {
            member_decorators.push(self.pop_decorators(desc.decorator_count as usize));
        }
        member_decorators.reverse();

        let value_count =
            blueprint.members.iter().filter(|m| m.has_value).count() + usize::from(blueprint.has_constructor);
        let mut values: Vec<Value> = self.stack.split_off(self.stack.len() - value_count);
        let mut it = values.drain(..);

        let parent = if blueprint.has_parent {
            match self.pop() {
                Value::Class(c) => Some(c),
                other => {
                    return Err(VmError::new(
                        format!("Родительский класс должен быть классом, получено '{}'", other.type_name()),
                        span,
                    ));
                }
            }
        } else {
            None
        };

        let constructor = if blueprint.has_constructor {
            match it.next() {
                Some(Value::Function(c)) => Some(c),
                _ => None,
            }
        } else {
            None
        };

        let raw_values: Vec<Option<Value>> =
            blueprint.members.iter().map(|d| if d.has_value { it.next() } else { None }).collect();
        drop(it);
        drop(values);

        let mut members = ClassMembers::default();
        let mut static_field_inits: Vec<(String, Option<Value>, Option<Value>)> = Vec::new();
        for (i, desc) in blueprint.members.iter().enumerate() {
            let decs = &member_decorators[i];
            let raw = raw_values[i].clone();
            let kind_label = decorator_kind_label(desc.kind);
            match desc.kind {
                MemberKind::Method
                | MemberKind::Getter
                | MemberKind::Setter
                | MemberKind::StaticMethod
                | MemberKind::StaticGetter
                | MemberKind::StaticSetter => {
                    let mut current = raw.unwrap();
                    if !decs.is_empty() {
                        current = self.apply_member_decorators(current, decs, kind_label, desc, span)?;
                    }
                    let closure = match current {
                        Value::Function(c) => c,
                        other => {
                            return Err(VmError::new(
                                format!(
                                    "Декоратор '{}' должен вернуть функцию, получено '{}'",
                                    desc.name,
                                    other.type_name()
                                ),
                                span,
                            ));
                        }
                    };
                    match desc.kind {
                        MemberKind::Method => members.methods.push((desc.name.clone(), closure)),
                        MemberKind::Getter => members.getters.push((desc.name.clone(), closure)),
                        MemberKind::Setter => members.setters.push((desc.name.clone(), closure)),
                        MemberKind::StaticMethod => members.static_methods.push((desc.name.clone(), closure)),
                        MemberKind::StaticGetter => members.static_getters.push((desc.name.clone(), closure)),
                        MemberKind::StaticSetter => members.static_setters.push((desc.name.clone(), closure)),
                        _ => unreachable!(),
                    }
                }
                MemberKind::Field | MemberKind::StaticField => {
                    let init_closure = match raw {
                        Some(Value::Function(c)) => Some(c),
                        _ => None,
                    };
                    let transform = if decs.is_empty() {
                        None
                    } else {
                        let result = self.apply_member_decorators(Value::Undefined, decs, kind_label, desc, span)?;
                        if matches!(result, Value::Undefined) { None } else { Some(result) }
                    };
                    if matches!(desc.kind, MemberKind::StaticField) {
                        static_field_inits.push((desc.name.clone(), init_closure.map(Value::Function), transform));
                    } else {
                        members.field_inits.push((desc.name.clone(), init_closure, transform));
                    }
                }
            }
        }

        let class_def = Rc::new(ClassDef {
            name: blueprint.name.clone(),
            parent,
            constructor,
            members,
            static_fields: RefCell::new(ObjMap::new()),
        });

        for (name, init, transform) in static_field_inits {
            let base = match init {
                Some(Value::Function(closure)) => {
                    self.call_closure_sync(closure, None, Some(Rc::clone(&class_def)), &[], span)?
                }
                _ => Value::Undefined,
            };
            let val = match transform {
                Some(tf) => self.call_value(tf, None, &[base], span)?,
                None => base,
            };
            class_def.static_fields.borrow_mut().insert(name, val);
        }

        let mut class_value = Value::Class(class_def);
        if !class_decorators.is_empty() {
            class_value = self.apply_class_decorators(class_value, &class_decorators, &blueprint.name, span)?;
        }
        self.stack.push(class_value);
        Ok(())
    }

    fn pop_decorators(&mut self, count: usize) -> Vec<Value> {
        if count == 0 {
            return Vec::new();
        }
        let at = self.stack.len() - count;
        self.stack.split_off(at)
    }

    fn build_decorator_context(&self, kind: &str, name: &str, is_static: bool, is_private: bool) -> Value {
        let mut ctx = ObjMap::new();
        ctx.insert("вид".to_string(), Value::string(kind));
        ctx.insert("имя".to_string(), Value::string(name));
        ctx.insert("статичное".to_string(), Value::Bool(is_static));
        ctx.insert("приватное".to_string(), Value::Bool(is_private));
        Value::Object(Rc::new(RefCell::new(ctx)))
    }

    fn apply_member_decorators(
        &mut self,
        value: Value,
        decorators: &[Value],
        kind: &str,
        desc: &crate::chunk::ClassMemberDesc,
        span: Span,
    ) -> Result<Value, VmError> {
        let mut current = value;
        for decorator in decorators.iter().rev() {
            let context = self.build_decorator_context(kind, &desc.name, desc.is_static, desc.is_private);
            let result = self.call_value(decorator.clone(), None, &[current.clone(), context], span)?;
            if !matches!(result, Value::Undefined) {
                current = result;
            }
        }
        Ok(current)
    }

    fn apply_class_decorators(
        &mut self,
        value: Value,
        decorators: &[Value],
        name: &str,
        span: Span,
    ) -> Result<Value, VmError> {
        let mut current = value;
        for decorator in decorators.iter().rev() {
            let context = self.build_decorator_context("класс", name, false, false);
            let result = self.call_value(decorator.clone(), None, &[current.clone(), context], span)?;
            if !matches!(result, Value::Undefined) {
                current = result;
            }
        }
        Ok(current)
    }

    pub(crate) fn call_value(
        &mut self,
        callee: Value,
        this: Option<Value>,
        args: &[Value],
        span: Span,
    ) -> Result<Value, VmError> {
        match callee {
            Value::Function(closure) => {
                if closure.proto.is_async {
                    Ok(self.spawn_async(closure, this, None, args.to_vec()))
                } else {
                    self.call_closure_sync(closure, this, None, args, span)
                }
            }
            Value::Builtin(name) => self.call_builtin_value(&name, args.to_vec(), span),
            Value::Class(_) => self.instantiate(callee, args.to_vec(), span),
            Value::PromiseCapability { state, kind } => {
                let val = args.first().cloned().unwrap_or(Value::Undefined);
                Vm::settle_promise(&state, kind, val, self, span)?;
                Ok(Value::Undefined)
            }
            Value::PromiseThenHandler { handler, resolve, reject, is_fulfill } => {
                let val = args.first().cloned().unwrap_or(Value::Undefined);
                crate::promise::invoke_handler(self, *handler, val, *resolve, *reject, is_fulfill, span)?;
                Ok(Value::Undefined)
            }
            Value::PromiseFinallyHandler { cb, cap } => {
                let val = args.first().cloned().unwrap_or(Value::Undefined);
                self.call_value(*cb, None, &[], span)?;
                self.call_value(*cap, None, &[val], span)?;
                Ok(Value::Undefined)
            }
            Value::PromiseAggregateHandler { state, index, role } => {
                let val = args.first().cloned().unwrap_or(Value::Undefined);
                crate::promise::apply_aggregate(self, state, index, role, val, span)?;
                Ok(Value::Undefined)
            }
            Value::Host(iv) => crate::bridge::host_call(self, &iv, args, span),
            other => Err(VmError::new(format!("значение типа '{}' не является функцией", other.type_name()), span)),
        }
    }

    fn instantiate(&mut self, callee: Value, args: Vec<Value>, span: Span) -> Result<Value, VmError> {
        let class_def = match callee {
            Value::Class(cls) => cls,
            Value::Builtin(name) => return self.call_builtin_value(&name, args, span),
            other => {
                return Err(VmError::new(format!("'{}' не является классом", other.type_name()), span));
            }
        };
        let mut seed = ObjMap::new();
        seed.insert(crate::value::CLASS_TAG.to_string(), Value::Class(Rc::clone(&class_def)));
        let instance = Value::Object(Rc::new(RefCell::new(seed)));

        self.init_instance_fields(&class_def, &instance, span)?;
        self.run_constructor(&class_def, &instance, args, span)?;
        Ok(instance)
    }

    fn init_instance_fields(&mut self, class_def: &Rc<ClassDef>, instance: &Value, span: Span) -> Result<(), VmError> {
        if let Some(parent) = &class_def.parent {
            self.init_instance_fields(parent, instance, span)?;
        }
        let map = match instance {
            Value::Object(m) => Rc::clone(m),
            _ => return Ok(()),
        };
        let field_inits: Vec<(String, Option<MethodDef>, Option<Value>)> =
            class_def.members.field_inits.iter().map(|(n, c, t)| (n.clone(), c.clone(), t.clone())).collect();
        for (name, init, transform) in &field_inits {
            let base = match init {
                Some(closure) => self.call_closure_sync(
                    Rc::clone(closure),
                    Some(instance.clone()),
                    Some(Rc::clone(class_def)),
                    &[],
                    span,
                )?,
                None => Value::Undefined,
            };
            let val = match transform {
                Some(tf) => self.call_value(tf.clone(), Some(instance.clone()), &[base], span)?,
                None => base,
            };
            map.borrow_mut().insert(name.clone(), val);
        }
        Ok(())
    }

    fn run_constructor(
        &mut self,
        class_def: &Rc<ClassDef>,
        instance: &Value,
        args: Vec<Value>,
        span: Span,
    ) -> Result<(), VmError> {
        if let Some(ctor) = &class_def.constructor {
            self.call_closure_sync(Rc::clone(ctor), Some(instance.clone()), Some(Rc::clone(class_def)), &args, span)?;
        } else if let Some(parent) = &class_def.parent {
            self.run_constructor(parent, instance, args, span)?;
        }
        Ok(())
    }

    fn super_call(
        &mut self,
        owner: Option<Rc<ClassDef>>,
        this: Value,
        args: Vec<Value>,
        span: Span,
    ) -> Result<(), VmError> {
        let owner = owner.ok_or_else(|| VmError::new("'яга' (super) используется вне класса-наследника", span))?;
        let parent =
            owner.parent.clone().ok_or_else(|| VmError::new("Родительский класс не имеет конструктора", span))?;
        self.run_constructor(&parent, &this, args, span)
    }

    fn super_get(
        &mut self,
        owner: Option<Rc<ClassDef>>,
        this: Value,
        name: &str,
        span: Span,
    ) -> Result<Value, VmError> {
        let owner = owner.ok_or_else(|| VmError::new("'яга' (super) используется вне класса-наследника", span))?;
        let parent = match &owner.parent {
            Some(p) => p,
            None => return Ok(Value::Undefined),
        };
        if let Some((getter, gp)) = parent.find_getter(name) {
            return self.call_closure_sync(getter, Some(this), gp, &[], span);
        }
        if let Some(method) = parent.find_method(name) {
            return Ok(Value::Function(method));
        }
        Ok(Value::Undefined)
    }

    fn super_invoke(
        &mut self,
        owner: Option<Rc<ClassDef>>,
        this: Value,
        name: &str,
        args: Vec<Value>,
        span: Span,
    ) -> Result<Value, VmError> {
        let owner = owner.ok_or_else(|| VmError::new("'яга' (super) используется вне класса-наследника", span))?;
        let parent = owner
            .parent
            .clone()
            .ok_or_else(|| VmError::new("'яга' (super) используется вне класса-наследника", span))?;
        match parent.find_method(name) {
            Some(method) => {
                let mowner = parent.find_method_owner(name);
                self.call_closure_sync(method, Some(this), mowner, &args, span)
            }
            None => Err(VmError::new(format!("'{name}' не является методом родительского класса"), span)),
        }
    }

    fn resolve_module_path(&self, source: &str, span: Span) -> Result<std::path::PathBuf, VmError> {
        let base = self.base_path.clone().unwrap_or_else(|| std::path::PathBuf::from("."));
        let mut candidate = base.join(source);
        if candidate.extension().is_none() {
            candidate.set_extension("yopta");
        }
        candidate
            .canonicalize()
            .map_err(|e| VmError::new(format!("Не удалось разрешить путь модуля '{source}': {e}"), span))
    }

    fn load_module(&mut self, source: &str, span: Span) -> Result<Rc<ModuleExports>, VmError> {
        let resolved = self.resolve_module_path(source, span)?;
        if let Some(cached) = self.module_cache.borrow().get(&resolved) {
            return Ok(Rc::clone(cached));
        }
        if self.module_loading.borrow().contains(&resolved) {
            return Ok(Rc::new(std::collections::HashMap::new()));
        }
        self.module_loading.borrow_mut().insert(resolved.clone());

        let result = self.load_module_inner(&resolved, span);
        self.module_loading.borrow_mut().remove(&resolved);
        let exports = result?;
        self.module_cache.borrow_mut().insert(resolved, Rc::clone(&exports));
        Ok(exports)
    }

    fn load_module_inner(&mut self, resolved: &std::path::Path, span: Span) -> Result<Rc<ModuleExports>, VmError> {
        let code = std::fs::read_to_string(resolved)
            .map_err(|e| VmError::new(format!("Не удалось прочитать модуль '{}': {e}", resolved.display()), span))?;
        let source_file = yps_lexer::SourceFile::new(resolved.display().to_string(), code);
        let lexer = yps_lexer::Lexer::new(&source_file);
        let (tokens, lex_diags) = lexer.tokenize();
        if !lex_diags.is_empty() {
            return Err(VmError::new(format!("Ошибки лексера в модуле '{}'", resolved.display()), span));
        }
        let parser = yps_parser::Parser::new(&tokens, &source_file);
        let (program, parse_diags) = parser.parse_program();
        if !parse_diags.is_empty() {
            return Err(VmError::new(format!("Ошибки парсера в модуле '{}'", resolved.display()), span));
        }
        let proto = crate::compiler::compile_program(&program).map_err(|e| VmError::new(e.message, span))?;

        let mut sub = Vm::with_writer(Box::new(io::stdout()));
        sub.module_cache = Rc::clone(&self.module_cache);
        sub.module_loading = Rc::clone(&self.module_loading);
        sub.base_path = resolved.parent().map(std::path::Path::to_path_buf);
        sub.run_uninstrumented(proto)?;
        Ok(Rc::new(std::mem::take(&mut sub.exports)))
    }

    fn load_json_module(&mut self, source: &str, span: Span) -> Result<Rc<ModuleExports>, VmError> {
        let base = self.base_path.clone().unwrap_or_else(|| std::path::PathBuf::from("."));
        let resolved = base
            .join(source)
            .canonicalize()
            .map_err(|e| VmError::new(format!("Не удалось разрешить путь модуля '{source}': {e}"), span))?;

        if let Some(cached) = self.module_cache.borrow().get(&resolved) {
            return Ok(Rc::clone(cached));
        }

        let code = std::fs::read_to_string(&resolved).map_err(|e| {
            VmError::new(format!("Не удалось прочитать JSON модуль '{}': {e}", resolved.display()), span)
        })?;
        let parsed = yps_interpreter::stdlib::json::parse_str(&code, span).map_err(|e| {
            VmError::new(format!("Ошибка разбора JSON модуля '{}': {}", resolved.display(), e.message), span)
        })?;
        let value = crate::bridge::interp_to_vm(&parsed)
            .map_err(|m| VmError::new(format!("Ошибка конвертации JSON модуля '{}': {m}", resolved.display()), span))?;
        let mut exports = std::collections::HashMap::new();
        exports.insert("default".to_string(), value);
        let exports = Rc::new(exports);
        self.module_cache.borrow_mut().insert(resolved, Rc::clone(&exports));
        Ok(exports)
    }

    fn run_uninstrumented(&mut self, proto: Rc<FnProto>) -> Result<(), VmError> {
        let closure = Rc::new(Closure { proto, upvalues: Vec::new() });
        self.stack.push(Value::Function(Rc::clone(&closure)));
        self.frames.push(CallFrame { closure, ip: 0, base: 0, owner: None });
        self.run_loop()
    }

    fn do_import(&mut self, request: &crate::chunk::ImportRequest, span: Span) -> Result<(), VmError> {
        let exports = if request.is_json {
            self.load_json_module(&request.source, span)?
        } else {
            self.load_module(&request.source, span)?
        };
        for binding in &request.specifiers {
            match binding {
                crate::chunk::ImportBinding::Default { local } => {
                    let val = exports.get("default").cloned().unwrap_or(Value::Undefined);
                    self.define_module_global(local.clone(), val);
                }
                crate::chunk::ImportBinding::Named { imported, local } => {
                    let val = exports.get(imported).cloned().ok_or_else(|| {
                        VmError::new(format!("Модуль '{}' не экспортирует '{imported}'", request.source), span)
                    })?;
                    self.define_module_global(local.clone(), val);
                }
                crate::chunk::ImportBinding::Namespace { local } => {
                    let mut map = ObjMap::new();
                    for (k, v) in exports.iter() {
                        map.insert(k.clone(), v.clone());
                    }
                    self.define_module_global(local.clone(), Value::Object(Rc::new(RefCell::new(map))));
                }
            }
        }
        Ok(())
    }

    fn define_module_global(&mut self, name: String, value: Value) {
        self.globals.retain(|(n, _, _)| *n != name);
        self.globals.push((name, value, true));
    }

    fn has_dispose_method(&self, value: &Value) -> bool {
        if let Value::Object(map) = value {
            if matches!(map.borrow().get(DISPOSE_METHOD), Some(Value::Function(_))) {
                return true;
            }
            if let Some(cls) = Self::resolve_class(map)
                && cls.find_method(DISPOSE_METHOD).is_some()
            {
                return true;
            }
        }
        false
    }

    fn dispose_scope(&mut self, count: usize, span: Span) -> Result<(), VmError> {
        let mut first_err: Option<VmError> = None;
        for _ in 0..count {
            let Some(resource) = self.disposables.pop() else {
                break;
            };
            if matches!(resource, Value::Null | Value::Undefined) {
                continue;
            }
            if let Err(e) = self.invoke_dispose(resource, span)
                && first_err.is_none()
            {
                first_err = Some(e);
            }
        }
        match first_err {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }

    fn invoke_dispose(&mut self, resource: Value, span: Span) -> Result<(), VmError> {
        if let Value::Object(map) = &resource {
            let direct = map.borrow().get(DISPOSE_METHOD).cloned();
            if let Some(Value::Function(closure)) = direct {
                self.call_closure_sync(closure, Some(resource.clone()), None, &[], span)?;
                return Ok(());
            }
            if let Some(cls) = Self::resolve_class(map)
                && let Some(method) = cls.find_method(DISPOSE_METHOD)
            {
                let owner = cls.find_method_owner(DISPOSE_METHOD);
                self.call_closure_sync(method, Some(resource.clone()), owner, &[], span)?;
                return Ok(());
            }
        }
        Err(VmError::new("Ресурс 'юзай' должен иметь метод 'расход'", span))
    }

    fn do_invoke(&mut self, name: &str, argc: usize, span: Span) -> Result<(), VmError> {
        let recv_idx = self.stack.len() - 1 - argc;
        let receiver = self.stack[recv_idx].clone();

        if let Value::RegExp { .. } = &receiver {
            let args: Vec<Value> = self.stack.split_off(self.stack.len() - argc);
            self.pop();
            let result = crate::regexp::call(&receiver, name, &args, span)?;
            self.stack.push(result);
            return Ok(());
        }

        if let Value::Generator(genrc) = &receiver {
            let genrc = Rc::clone(genrc);
            let args: Vec<Value> = self.stack.split_off(self.stack.len() - argc);
            self.pop();
            let arg = args.into_iter().next().unwrap_or(Value::Undefined);
            let result = match name {
                "следующий" | "next" => self.gen_next(&genrc, arg, span)?,
                "вернуть" | "return" => self.gen_return(&genrc, arg, span)?,
                "кинуть" | "throw" => self.gen_throw(&genrc, arg, span)?,
                other => {
                    return Err(VmError::new(format!("у генератора нет метода '{other}'"), span));
                }
            };
            self.stack.push(result);
            return Ok(());
        }

        if let Value::Promise { state } = &receiver {
            let state = Rc::clone(state);
            let args: Vec<Value> = self.stack.split_off(self.stack.len() - argc);
            self.pop();
            let result = crate::promise::call_promise_method(self, &state, name, args, span)?;
            self.stack.push(result);
            return Ok(());
        }

        if let Value::Host(iv) = &receiver {
            let iv = iv.clone();
            let args: Vec<Value> = self.stack.split_off(self.stack.len() - argc);
            self.pop();
            let result = crate::bridge::call_host_method(self, &iv, name, args, span)?;
            self.stack.push(result);
            return Ok(());
        }

        if let Value::Object(map) = &receiver
            && let Some(cls) = Self::resolve_class(map)
            && map.borrow().get(name).is_none()
            && let Some(method) = cls.find_method(name)
        {
            let owner = cls.find_method_owner(name);
            let args: Vec<Value> = self.stack.split_off(self.stack.len() - argc);
            self.pop();
            let result = self.call_closure_sync(method, Some(receiver), owner, &args, span)?;
            self.stack.push(result);
            return Ok(());
        }

        if let Value::Class(cls) = &receiver
            && let Some((method, owner)) = cls.find_static_method(name)
        {
            let args: Vec<Value> = self.stack.split_off(self.stack.len() - argc);
            self.pop();
            let result = self.call_closure_sync(method, Some(receiver.clone()), owner, &args, span)?;
            self.stack.push(result);
            return Ok(());
        }

        if let Value::Array(arr) = &receiver
            && matches!(name, "push" | "добавить" | "втолкнуть")
        {
            let args: Vec<Value> = self.stack.split_off(self.stack.len() - argc);
            self.pop();
            let len = {
                let mut guard = arr.borrow_mut();
                for a in args {
                    guard.push(a);
                }
                guard.len() as f64
            };
            self.stack.push(Value::Number(len));
            return Ok(());
        }

        let prop = self.get_property(&receiver, name, span)?;
        self.stack[recv_idx] = prop;
        if let Value::Object(_) = &receiver {
            self.do_call_with_this(argc, receiver, span)
        } else {
            self.do_call(argc, span)
        }
    }

    fn do_call_with_this(&mut self, argc: usize, this: Value, span: Span) -> Result<(), VmError> {
        let callee_idx = self.stack.len() - 1 - argc;
        let callee = self.stack[callee_idx].clone();
        if let Value::Function(closure) = callee {
            let args: Vec<Value> = self.stack.split_off(self.stack.len() - argc);
            self.pop();
            if closure.proto.is_generator {
                let genrc = self.make_generator(closure, this, None, args);
                self.stack.push(genrc);
                return Ok(());
            }
            if closure.proto.is_async {
                let promise = self.spawn_async(closure, Some(this), None, args);
                self.stack.push(promise);
                return Ok(());
            }
            self.push_call_frame(closure, Some(this), None, &args, span)?;
            Ok(())
        } else {
            self.do_call(argc, span)
        }
    }

    fn get_property(&mut self, obj: &Value, name: &str, span: Span) -> Result<Value, VmError> {
        match obj {
            Value::Object(map) => {
                let getter = map.borrow().get(&crate::value::getter_key(name)).cloned();
                if let Some(Value::Function(getter)) = getter {
                    return self.call_closure_sync(getter, Some(obj.clone()), None, &[], span);
                }
                if let Some(val) = map.borrow().get(name).cloned() {
                    return Ok(val);
                }
                if let Some(cls) = Self::resolve_class(map) {
                    if let Some((getter, gp)) = cls.find_getter(name) {
                        return self.call_closure_sync(getter, Some(obj.clone()), gp, &[], span);
                    }
                    if name == "конструктор" || name == "constructor" {
                        return Ok(Value::Class(cls));
                    }
                    if let Some(method) = cls.find_method(name) {
                        return Ok(Value::Function(method));
                    }
                }
                Ok(Value::Undefined)
            }
            Value::Class(cls) => {
                if let Some(getter) = cls.find_static_getter(name) {
                    return self.call_closure_sync(getter, Some(obj.clone()), cls.parent.clone(), &[], span);
                }
                if let Some(val) = cls.find_static_field(name) {
                    return Ok(val);
                }
                if let Some((method, _)) = cls.find_static_method(name) {
                    return Ok(Value::Function(method));
                }
                Ok(Value::Undefined)
            }
            Value::Host(iv) => {
                let iv = iv.clone();
                crate::bridge::host_member_get(self, &iv, name, span)
            }
            _ => get_prop(obj, name, span),
        }
    }

    fn set_property(&mut self, obj: &Value, name: &str, value: Value, span: Span) -> Result<(), VmError> {
        match obj {
            Value::Object(map) => {
                let setter = map.borrow().get(&crate::value::setter_key(name)).cloned();
                if let Some(Value::Function(setter)) = setter {
                    self.call_closure_sync(setter, Some(obj.clone()), None, std::slice::from_ref(&value), span)?;
                    self.stack.push(value);
                    return Ok(());
                }
                if let Some(cls) = Self::resolve_class(map)
                    && let Some((setter, sp)) = cls.find_setter(name)
                {
                    self.call_closure_sync(setter, Some(obj.clone()), sp, std::slice::from_ref(&value), span)?;
                    self.stack.push(value);
                    return Ok(());
                }
                map.borrow_mut().insert(name.to_string(), value.clone());
                self.stack.push(value);
                Ok(())
            }
            Value::Class(cls) => {
                if let Some((setter, sp)) = cls.find_static_setter(name).map(|s| (s, cls.parent.clone())) {
                    self.call_closure_sync(setter, Some(obj.clone()), sp, std::slice::from_ref(&value), span)?;
                    self.stack.push(value);
                    return Ok(());
                }
                let owner = cls.find_static_field_owner(name);
                match owner {
                    Some(owner) => owner.static_fields.borrow_mut().insert(name.to_string(), value.clone()),
                    None => cls.static_fields.borrow_mut().insert(name.to_string(), value.clone()),
                }
                self.stack.push(value);
                Ok(())
            }
            Value::Host(iv) => {
                let iv = iv.clone();
                crate::bridge::host_member_set(self, &iv, name, &value, span)?;
                self.stack.push(value);
                Ok(())
            }
            other => Err(VmError::new(format!("нельзя задать свойство '{name}' у типа '{}'", other.type_name()), span)),
        }
    }

    fn instance_of(&self, value: &Value, target: &Rc<ClassDef>) -> bool {
        if let Value::Object(map) = value
            && let Some(cls) = Self::resolve_class(map)
        {
            return cls.is_subclass_of(target);
        }
        false
    }

    fn resolve_class(map: &Rc<RefCell<ObjMap>>) -> Option<Rc<ClassDef>> {
        match map.borrow().get(crate::value::CLASS_TAG) {
            Some(Value::Class(cls)) => Some(Rc::clone(cls)),
            _ => None,
        }
    }

    fn capture_upvalue(&mut self, idx: usize) -> Upvalue {
        for uv in &self.open_upvalues {
            if let UpvalueState::Open(i) = &*uv.borrow()
                && *i == idx
            {
                return Rc::clone(uv);
            }
        }
        let uv = Rc::new(RefCell::new(UpvalueState::Open(idx)));
        self.open_upvalues.push(Rc::clone(&uv));
        uv
    }

    fn close_upvalues(&mut self, from: usize) {
        let mut i = 0;
        while i < self.open_upvalues.len() {
            let idx = match &*self.open_upvalues[i].borrow() {
                UpvalueState::Open(idx) if *idx >= from => Some(*idx),
                _ => None,
            };
            match idx {
                Some(idx) => {
                    let val = self.stack[idx].clone();
                    *self.open_upvalues[i].borrow_mut() = UpvalueState::Closed(val);
                    self.open_upvalues.remove(i);
                }
                None => i += 1,
            }
        }
    }

    fn numeric_bin(&mut self, _span: Span, f: impl Fn(f64, f64) -> f64) -> Result<(), VmError> {
        let b = self.pop();
        let a = self.pop();
        self.stack.push(Value::Number(f(a.to_number(), b.to_number())));
        Ok(())
    }

    fn try_bigint_binop(&mut self, op: BigOp, span: Span) -> Result<bool, VmError> {
        let left_big = matches!(self.peek(1), Value::BigInt(_));
        let right_big = matches!(self.peek(0), Value::BigInt(_));
        if !left_big && !right_big {
            return Ok(false);
        }
        if left_big ^ right_big {
            let b = self.pop();
            let a = self.pop();
            return Err(VmError::new(
                format!("Нельзя смешивать '{}' и '{}' в одной операции", a.type_name(), b.type_name()),
                span,
            ));
        }
        let b = self.pop();
        let a = self.pop();
        let (Value::BigInt(a), Value::BigInt(b)) = (&a, &b) else {
            unreachable!("оба операнда бигцелые");
        };
        let result = bigint_op(op, *a, *b, span)?;
        self.stack.push(result);
        Ok(true)
    }

    fn int_bin(&mut self, f: impl Fn(i32, i32) -> i32) {
        let b = self.pop();
        let a = self.pop();
        self.stack.push(Value::Number(f64::from(f(to_int32(a.to_number()), to_int32(b.to_number())))));
    }

    fn shift_bin(&mut self, f: impl Fn(i32, u32) -> i32) {
        let b = self.pop();
        let a = self.pop();
        let s = to_uint32(b.to_number()) & 0x1f;
        self.stack.push(Value::Number(f64::from(f(to_int32(a.to_number()), s))));
    }

    fn compare(&mut self, _span: Span, pred: impl Fn(Ordering) -> bool) -> Result<(), VmError> {
        let b = self.pop();
        let a = self.pop();
        let result = if let (Value::Str(x), Value::Str(y)) = (&a, &b) {
            pred(x.as_bytes().cmp(y.as_bytes()))
        } else {
            let x = a.to_number();
            let y = b.to_number();
            x.partial_cmp(&y).is_some_and(&pred)
        };
        self.stack.push(Value::Bool(result));
        Ok(())
    }

    fn pop_spread_args(&mut self, span: Span) -> Result<Vec<Value>, VmError> {
        match self.pop() {
            Value::Array(a) => Ok(a.borrow().clone()),
            _ => Err(VmError::new("ожидался массив аргументов spread", span)),
        }
    }

    fn pop(&mut self) -> Value {
        self.stack.pop().expect("stack underflow")
    }

    fn peek(&self, depth: usize) -> &Value {
        &self.stack[self.stack.len() - 1 - depth]
    }
}

fn uncaught_message(value: &Value) -> String {
    format!("Необработанное исключение: {value}")
}

fn report_async_error(source: &str, err: &VmError) {
    eprintln!("необработанное исключение в '{source}': {}", err.message);
}

fn timer_callback(value: Option<Value>, fn_name: &str, span: Span) -> Result<Value, VmError> {
    match value {
        Some(v) if matches!(v, Value::Function(_) | Value::Builtin(_)) => Ok(v),
        Some(other) => {
            Err(VmError::new(format!("'{fn_name}' ожидает функцию, получено '{}'", other.type_name()), span))
        }
        None => Err(VmError::new(format!("'{fn_name}' ожидает функцию"), span)),
    }
}

fn parse_delay_ms(value: Option<Value>, fn_name: &str, span: Span) -> Result<u64, VmError> {
    match value {
        Some(Value::Number(n)) if n.is_finite() && n >= 0.0 => Ok(n as u64),
        Some(Value::Undefined) | None => Ok(0),
        Some(other) => Err(VmError::new(
            format!("'{fn_name}' ожидает миллисекунды числом, получено '{}'", other.type_name()),
            span,
        )),
    }
}

fn as_return_token(value: &Value) -> Option<Value> {
    if let Value::Object(map) = value {
        let m = map.borrow();
        if m.contains_key(GEN_RETURN_TAG) {
            return m.get(GEN_RETURN_TAG).cloned();
        }
    }
    None
}

fn decorator_kind_label(kind: MemberKind) -> &'static str {
    match kind {
        MemberKind::Method | MemberKind::StaticMethod => "метод",
        MemberKind::Getter | MemberKind::StaticGetter => "геттер",
        MemberKind::Setter | MemberKind::StaticSetter => "сеттер",
        MemberKind::Field | MemberKind::StaticField => "поле",
    }
}

fn build_template_strings(strings: &crate::chunk::TemplateStrings) -> Value {
    let mut map = ObjMap::new();
    for (i, cooked) in strings.cooked.iter().enumerate() {
        map.insert(i.to_string(), Value::string(cooked.as_str()));
    }
    let len = Value::Number(strings.cooked.len() as f64);
    map.insert("длина".to_string(), len.clone());
    map.insert("length".to_string(), len);
    let raw: Vec<Value> = strings.raw.iter().map(|r| Value::string(r.as_str())).collect();
    let raw_arr = Value::Array(Rc::new(RefCell::new(raw)));
    map.insert("сырьё".to_string(), raw_arr.clone());
    map.insert("raw".to_string(), raw_arr);
    Value::Object(Rc::new(RefCell::new(map)))
}

fn for_in_keys(src: &Value, span: Span) -> Result<Vec<Value>, VmError> {
    match src {
        Value::Array(a) => Ok(a.borrow().clone()),
        Value::Object(map) => Ok(map
            .borrow()
            .iter()
            .filter(|(k, _)| !crate::value::is_internal_key(k))
            .map(|(k, _)| Value::string(k.as_str()))
            .collect()),
        other => Err(VmError::new(format!("Нельзя итерировать по типу '{}'", other.type_name()), span)),
    }
}

fn for_of_values(src: &Value, span: Span) -> Result<Vec<Value>, VmError> {
    match src {
        Value::Array(a) => Ok(a.borrow().clone()),
        Value::Str(s) => Ok(s.chars().map(|c| Value::string(c.to_string())).collect()),
        other => Err(VmError::new(format!("Нельзя итерировать по типу '{}'", other.type_name()), span)),
    }
}

fn spread_into_values(src: &Value, span: Span) -> Result<Vec<Value>, VmError> {
    match src {
        Value::Array(a) => Ok(a.borrow().clone()),
        Value::Str(s) => Ok(s.chars().map(|c| Value::string(c.to_string())).collect()),
        other => Err(VmError::new(format!("Нельзя развернуть тип '{}' в массив", other.type_name()), span)),
    }
}

fn add_values(a: &Value, b: &Value) -> Value {
    if matches!(a, Value::Str(_)) || matches!(b, Value::Str(_)) {
        let mut s = a.to_ecma_string();
        s.push_str(&b.to_ecma_string());
        Value::string(s)
    } else {
        Value::Number(a.to_number() + b.to_number())
    }
}

#[derive(Debug, Clone, Copy)]
enum BigOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
    Lt,
    Gt,
    Le,
    Ge,
}

fn bigint_op(op: BigOp, a: i128, b: i128, span: Span) -> Result<Value, VmError> {
    let checked = match op {
        BigOp::Add => a.checked_add(b),
        BigOp::Sub => a.checked_sub(b),
        BigOp::Mul => a.checked_mul(b),
        BigOp::Div => {
            if b == 0 {
                return Err(VmError::new("Деление на ноль", span));
            }
            a.checked_div(b)
        }
        BigOp::Mod => {
            if b == 0 {
                return Err(VmError::new("Деление на ноль", span));
            }
            a.checked_rem(b)
        }
        BigOp::Pow => {
            if b < 0 {
                return Err(VmError::new("Отрицательный показатель степени у бигцелого", span));
            }
            if b > u32::MAX as i128 { None } else { a.checked_pow(b as u32) }
        }
        BigOp::Lt => return Ok(Value::Bool(a < b)),
        BigOp::Gt => return Ok(Value::Bool(a > b)),
        BigOp::Le => return Ok(Value::Bool(a <= b)),
        BigOp::Ge => return Ok(Value::Bool(a >= b)),
    };
    checked.map(Value::BigInt).ok_or_else(|| VmError::new("Переполнение бигцелого", span))
}

fn array_index(index: &Value, len: usize) -> Option<usize> {
    let n = index.to_number();
    if !n.is_finite() || n.fract() != 0.0 || n < 0.0 {
        return None;
    }
    let i = n as usize;
    if i < len { Some(i) } else { None }
}

fn get_index(obj: &Value, index: &Value, span: Span) -> Result<Value, VmError> {
    match obj {
        Value::Array(a) => {
            let arr = a.borrow();
            Ok(array_index(index, arr.len()).map(|i| arr[i].clone()).unwrap_or(Value::Undefined))
        }
        Value::Object(map) => Ok(map.borrow().get(&index.to_ecma_string()).cloned().unwrap_or(Value::Undefined)),
        Value::Str(s) => {
            let n = index.to_number();
            if n.is_finite() && n.fract() == 0.0 && n >= 0.0 {
                let i = n as usize;
                Ok(s.chars().nth(i).map(|c| Value::string(c.to_string())).unwrap_or(Value::Undefined))
            } else {
                Ok(Value::Undefined)
            }
        }
        other => Err(VmError::new(format!("нельзя индексировать тип '{}'", other.type_name()), span)),
    }
}

fn set_index(obj: &Value, index: &Value, value: Value, span: Span) -> Result<(), VmError> {
    match obj {
        Value::Array(a) => {
            let n = index.to_number();
            if !n.is_finite() || n.fract() != 0.0 || n < 0.0 {
                return Err(VmError::new("индекс массива должен быть неотрицательным целым", span));
            }
            let i = n as usize;
            let mut arr = a.borrow_mut();
            let len = arr.len();
            match arr.get_mut(i) {
                Some(slot) => {
                    *slot = value;
                    Ok(())
                }
                None => Err(VmError::new(format!("Индекс {i} вне диапазона (длина {len})"), span)),
            }
        }
        Value::Object(map) => {
            map.borrow_mut().insert(index.to_ecma_string(), value);
            Ok(())
        }
        other => Err(VmError::new(format!("нельзя индексировать тип '{}'", other.type_name()), span)),
    }
}

fn get_prop(obj: &Value, name: &str, span: Span) -> Result<Value, VmError> {
    match obj {
        Value::Builtin(base) => Ok(Value::Builtin(Rc::from(format!("{base}.{name}").as_str()))),
        Value::Object(map) => Ok(map.borrow().get(name).cloned().unwrap_or(Value::Undefined)),
        Value::RegExp { .. } => Ok(crate::regexp::member(obj, name).unwrap_or(Value::Undefined)),
        other => Err(VmError::new(format!("нельзя читать свойство '{name}' у типа '{}'", other.type_name()), span)),
    }
}
