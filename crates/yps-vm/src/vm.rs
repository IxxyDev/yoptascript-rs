use std::cell::RefCell;
use std::cmp::Ordering;
use std::io::{self, Write};
use std::rc::Rc;

use yps_lexer::Span;

use crate::builtins;
use crate::chunk::{Constant, FnProto, Op};
use crate::error::VmError;
use crate::value::{Closure, ObjMap, Upvalue, UpvalueState, Value, abstract_eq, strict_eq, to_int32, to_uint32};

const MAX_CALL_DEPTH: usize = 1000;

struct CallFrame {
    closure: Rc<Closure>,
    ip: usize,
    base: usize,
}

pub struct Vm {
    stack: Vec<Value>,
    frames: Vec<CallFrame>,
    globals: Vec<(String, Value, bool)>,
    open_upvalues: Vec<Upvalue>,
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
        Vm { stack: Vec::new(), frames: Vec::new(), globals: Vec::new(), open_upvalues: Vec::new(), out }
    }

    pub fn run(&mut self, proto: Rc<FnProto>) -> Result<(), VmError> {
        let closure = Rc::new(Closure { proto, upvalues: Vec::new() });
        self.stack.push(Value::Function(Rc::clone(&closure)));
        self.frames.push(CallFrame { closure, ip: 0, base: 0 });
        self.run_loop()
    }

    fn global_get(&self, name: &str) -> Option<&Value> {
        self.globals.iter().rev().find(|(n, _, _)| n == name).map(|(_, v, _)| v)
    }

    fn run_loop(&mut self) -> Result<(), VmError> {
        loop {
            let frame_idx = self.frames.len() - 1;
            let closure = Rc::clone(&self.frames[frame_idx].closure);
            let chunk = &closure.proto.chunk;
            let ip = self.frames[frame_idx].ip;
            let op = chunk.code[ip];
            let span = chunk.spans[ip];
            self.frames[frame_idx].ip = ip + 1;
            let base = self.frames[frame_idx].base;

            match op {
                Op::Constant(idx) => match &chunk.constants[idx as usize] {
                    Constant::Number(n) => self.stack.push(Value::Number(*n)),
                    Constant::Str(s) => self.stack.push(Value::Str(Rc::clone(s))),
                    Constant::Proto(_) => return Err(VmError::new("прото загружен как константа", span)),
                },
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
                Op::Neg => {
                    let a = self.pop();
                    self.stack.push(Value::Number(-a.to_number()));
                }
                Op::Pos => {
                    let a = self.pop();
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
                    let b = self.pop();
                    let a = self.pop();
                    self.stack.push(add_values(&a, &b));
                }
                Op::Sub => self.numeric_bin(span, |a, b| a - b)?,
                Op::Mul => self.numeric_bin(span, |a, b| a * b)?,
                Op::Div => {
                    let b = self.pop();
                    let a = self.pop();
                    if let (Value::Number(_), Value::Number(d)) = (&a, &b)
                        && *d == 0.0
                    {
                        return Err(VmError::new("Деление на ноль", span));
                    }
                    self.stack.push(Value::Number(a.to_number() / b.to_number()));
                }
                Op::Mod => self.numeric_bin(span, |a, b| a % b)?,
                Op::Pow => self.numeric_bin(span, |a, b| a.powf(b))?,
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
                Op::Lt => self.compare(span, |o| o == Ordering::Less)?,
                Op::Gt => self.compare(span, |o| o == Ordering::Greater)?,
                Op::Le => self.compare(span, |o| o != Ordering::Greater)?,
                Op::Ge => self.compare(span, |o| o != Ordering::Less)?,

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

                Op::Call(argc) => self.do_call(argc as usize, span)?,
                Op::Closure(idx) => self.do_closure(chunk, idx, base),
                Op::Return => {
                    let result = self.pop();
                    let frame = self.frames.pop().unwrap();
                    self.close_upvalues(frame.base);
                    self.stack.truncate(frame.base);
                    if self.frames.is_empty() {
                        return Ok(());
                    }
                    self.stack.push(result);
                }

                Op::NewArray(n) => {
                    let n = n as usize;
                    let at = self.stack.len() - n;
                    let elems: Vec<Value> = self.stack.split_off(at);
                    self.stack.push(Value::Array(Rc::new(RefCell::new(elems))));
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
                Op::GetIndex => {
                    let index = self.pop();
                    let obj = self.pop();
                    self.stack.push(get_index(&obj, &index, span)?);
                }
                Op::SetIndex => {
                    let value = self.pop();
                    let index = self.pop();
                    let obj = self.pop();
                    set_index(&obj, &index, value.clone(), span)?;
                    self.stack.push(value);
                }
                Op::GetProp(idx) => {
                    let name = self.const_str(chunk, idx);
                    let obj = self.pop();
                    self.stack.push(get_prop(&obj, &name, span)?);
                }
                Op::SetProp(idx) => {
                    let name = self.const_str(chunk, idx);
                    let value = self.pop();
                    let obj = self.pop();
                    match &obj {
                        Value::Object(map) => {
                            map.borrow_mut().insert(name, value.clone());
                            self.stack.push(value);
                        }
                        other => {
                            return Err(VmError::new(
                                format!("нельзя задать свойство '{name}' у типа '{}'", other.type_name()),
                                span,
                            ));
                        }
                    }
                }
            }
        }
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
                let result = builtins::call_builtin(&mut *self.out, &name, args, span)?;
                self.stack.push(result);
                Ok(())
            }
            Value::Function(closure) => {
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
                self.frames.push(CallFrame { closure, ip: 0, base });
                Ok(())
            }
            other => Err(VmError::new(format!("значение типа '{}' не является функцией", other.type_name()), span)),
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

    fn compare(&mut self, span: Span, pred: impl Fn(Ordering) -> bool) -> Result<(), VmError> {
        let b = self.pop();
        let a = self.pop();
        match (&a, &b) {
            (Value::Number(x), Value::Number(y)) => {
                let result = x.partial_cmp(y).is_some_and(&pred);
                self.stack.push(Value::Bool(result));
                Ok(())
            }
            _ => Err(VmError::new(
                format!("Сравнение требует числа, получено '{}' и '{}'", a.type_name(), b.type_name()),
                span,
            )),
        }
    }

    fn pop(&mut self) -> Value {
        self.stack.pop().expect("stack underflow")
    }

    fn peek(&self, depth: usize) -> &Value {
        &self.stack[self.stack.len() - 1 - depth]
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
        other => Err(VmError::new(format!("нельзя читать свойство '{name}' у типа '{}'", other.type_name()), span)),
    }
}
