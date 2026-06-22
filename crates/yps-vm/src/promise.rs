use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashSet};
use std::rc::Rc;
use std::time::{Duration, Instant};

use yps_lexer::Span;

use crate::error::VmError;
use crate::value::{AggregateKind, AggregateRole, AggregateState, CapKind, ObjMap, PromiseState, Value};
use crate::vm::Vm;

pub(crate) type Microtask = Box<dyn FnOnce(&mut Vm, Span) -> Result<(), VmError>>;
pub(crate) type Macrotask = Box<dyn FnOnce(&mut Vm, Span) -> Result<(), VmError>>;

pub(crate) struct ScheduledTask {
    pub deadline: Instant,
    pub seq: u64,
    pub id: u64,
    pub task: Macrotask,
}

impl Eq for ScheduledTask {}
impl PartialEq for ScheduledTask {
    fn eq(&self, other: &Self) -> bool {
        self.deadline == other.deadline && self.seq == other.seq
    }
}
impl Ord for ScheduledTask {
    fn cmp(&self, other: &Self) -> Ordering {
        other.deadline.cmp(&self.deadline).then_with(|| other.seq.cmp(&self.seq))
    }
}
impl PartialOrd for ScheduledTask {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

pub(crate) struct MacrotaskQueue {
    heap: BinaryHeap<ScheduledTask>,
    pub cancelled: HashSet<u64>,
    next_id: u64,
    next_seq: u64,
}

impl MacrotaskQueue {
    pub fn new() -> Self {
        Self { heap: BinaryHeap::new(), cancelled: HashSet::new(), next_id: 1, next_seq: 0 }
    }

    pub fn schedule(&mut self, delay: Duration, task: Macrotask) -> u64 {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        let seq = self.next_seq;
        self.next_seq = self.next_seq.wrapping_add(1);
        self.heap.push(ScheduledTask { deadline: Instant::now() + delay, seq, id, task });
        id
    }

    pub fn schedule_with_id(&mut self, id: u64, delay: Duration, task: Macrotask) {
        let seq = self.next_seq;
        self.next_seq = self.next_seq.wrapping_add(1);
        self.heap.push(ScheduledTask { deadline: Instant::now() + delay, seq, id, task });
    }

    pub fn cancel(&mut self, id: u64) {
        self.cancelled.insert(id);
    }

    pub fn allocate_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        id
    }

    pub fn is_empty(&self) -> bool {
        self.heap.iter().all(|t| self.cancelled.contains(&t.id))
    }

    fn next_deadline(&self) -> Option<Instant> {
        for t in self.heap.iter() {
            if !self.cancelled.contains(&t.id) {
                return Some(t.deadline);
            }
        }
        None
    }

    fn pop_ready(&mut self) -> Option<ScheduledTask> {
        while let Some(top) = self.heap.peek() {
            if self.cancelled.contains(&top.id) {
                self.heap.pop();
                continue;
            }
            if top.deadline <= Instant::now() {
                return self.heap.pop();
            }
            return None;
        }
        None
    }

    pub fn pop_next_blocking(&mut self) -> Option<ScheduledTask> {
        loop {
            let next_deadline = self.next_deadline()?;
            let now = Instant::now();
            if next_deadline > now {
                std::thread::sleep(next_deadline - now);
            }
            if let Some(task) = self.pop_ready() {
                return Some(task);
            }
        }
    }
}

impl Default for MacrotaskQueue {
    fn default() -> Self {
        Self::new()
    }
}

pub(crate) fn make_fulfilled_promise(value: Value) -> Value {
    if let Value::Promise { .. } = &value {
        return value;
    }
    Value::Promise { state: Rc::new(RefCell::new(PromiseState::Fulfilled(value))) }
}

pub(crate) fn make_rejected_promise(value: Value) -> Value {
    Value::Promise { state: Rc::new(RefCell::new(PromiseState::Rejected(value))) }
}

pub(crate) fn make_pending_promise() -> (Value, Value, Value) {
    let state = Rc::new(RefCell::new(PromiseState::Pending { on_resolve: Vec::new(), on_reject: Vec::new() }));
    let promise = Value::Promise { state: Rc::clone(&state) };
    let resolve = Value::PromiseCapability { state: Rc::clone(&state), kind: CapKind::Resolve };
    let reject = Value::PromiseCapability { state, kind: CapKind::Reject };
    (promise, resolve, reject)
}

impl Vm {
    pub(crate) fn drain_microtasks(&mut self, span: Span) -> Result<(), VmError> {
        while let Some(task) = self.microtasks.pop_front() {
            task(self, span)?;
        }
        Ok(())
    }

    pub(crate) fn enqueue_microtask(&mut self, task: Microtask) {
        self.microtasks.push_back(task);
    }

    pub(crate) fn drive_event_loop(&mut self, span: Span) -> Result<(), VmError> {
        loop {
            self.drain_microtasks(span)?;
            if self.macrotasks.is_empty() {
                return Ok(());
            }
            let Some(task) = self.macrotasks.pop_next_blocking() else {
                return Ok(());
            };
            (task.task)(self, span)?;
        }
    }

    pub(crate) fn settle_promise(
        state: &Rc<RefCell<PromiseState>>,
        kind: CapKind,
        value: Value,
        vm: &mut Vm,
        span: Span,
    ) -> Result<(), VmError> {
        let already_settled = !matches!(&*state.borrow(), PromiseState::Pending { .. });
        if already_settled {
            return Ok(());
        }
        match kind {
            CapKind::Resolve => {
                if let Value::Promise { state: other } = &value {
                    if Rc::ptr_eq(state, other) {
                        return Err(VmError::new("Обещание не может разрешить само себя", span));
                    }
                    let other_state = other.borrow().clone();
                    match other_state {
                        PromiseState::Fulfilled(v) => Self::set_fulfilled(state, v, vm),
                        PromiseState::Rejected(v) => Self::set_rejected(state, v, vm),
                        PromiseState::Pending { .. } => {
                            let resolve_cap =
                                Value::PromiseCapability { state: Rc::clone(state), kind: CapKind::Resolve };
                            let reject_cap =
                                Value::PromiseCapability { state: Rc::clone(state), kind: CapKind::Reject };
                            if let PromiseState::Pending { on_resolve, on_reject } = &mut *other.borrow_mut() {
                                on_resolve.push(resolve_cap);
                                on_reject.push(reject_cap);
                            }
                        }
                    }
                } else {
                    Self::set_fulfilled(state, value, vm);
                }
            }
            CapKind::Reject => Self::set_rejected(state, value, vm),
        }
        Ok(())
    }

    fn set_fulfilled(state: &Rc<RefCell<PromiseState>>, value: Value, vm: &mut Vm) {
        let callbacks: Vec<Value> = match &mut *state.borrow_mut() {
            PromiseState::Pending { on_resolve, .. } => std::mem::take(on_resolve),
            _ => return,
        };
        *state.borrow_mut() = PromiseState::Fulfilled(value.clone());
        for cb in callbacks {
            let val_cloned = value.clone();
            vm.enqueue_microtask(Box::new(move |vm, span| vm.call_value(cb, None, &[val_cloned], span).map(|_| ())));
        }
    }

    fn set_rejected(state: &Rc<RefCell<PromiseState>>, value: Value, vm: &mut Vm) {
        let callbacks: Vec<Value> = match &mut *state.borrow_mut() {
            PromiseState::Pending { on_reject, .. } => std::mem::take(on_reject),
            _ => return,
        };
        *state.borrow_mut() = PromiseState::Rejected(value.clone());
        for cb in callbacks {
            let val_cloned = value.clone();
            vm.enqueue_microtask(Box::new(move |vm, span| vm.call_value(cb, None, &[val_cloned], span).map(|_| ())));
        }
    }

    pub(crate) fn do_await(&mut self, value: Value, span: Span) -> Result<Value, VmError> {
        let Value::Promise { state } = value else {
            return Ok(value);
        };
        loop {
            self.drain_microtasks(span)?;
            let snapshot = state.borrow().clone();
            match snapshot {
                PromiseState::Fulfilled(v) => return Ok(v),
                PromiseState::Rejected(v) => return Err(VmError::new(uncaught_message(&v), span).with_thrown(v)),
                PromiseState::Pending { .. } => {
                    if self.macrotasks.is_empty() {
                        return Err(VmError::new("Обещание не разрешено и очередь задач пуста", span));
                    }
                    let Some(task) = self.macrotasks.pop_next_blocking() else {
                        return Err(VmError::new("Обещание не разрешено: нет готовых задач", span));
                    };
                    (task.task)(self, span)?;
                }
            }
        }
    }

    pub(crate) fn rejection_reason(&self, e: VmError) -> Value {
        match e.thrown {
            Some(val) => *val,
            None => self.error_object(e.message),
        }
    }

    pub(crate) fn dynamic_import(&mut self, source: Value, span: Span) -> Result<Value, VmError> {
        let path = match source {
            Value::Str(s) => s,
            other => {
                return Ok(make_rejected_promise(Value::string(format!(
                    "Аргумент динамического импорта должен быть строкой, получено '{}'",
                    other.type_name()
                ))));
            }
        };
        match self.load_module_exports(&path, span) {
            Ok(exports) => {
                let mut map = ObjMap::new();
                for (k, v) in exports.iter() {
                    map.insert(k.clone(), v.clone());
                }
                Ok(make_fulfilled_promise(Value::Object(Rc::new(RefCell::new(map)))))
            }
            Err(err) => Ok(make_rejected_promise(Value::string(err.message))),
        }
    }
}

fn uncaught_message(value: &Value) -> String {
    format!("Необработанное исключение: {value}")
}

pub(crate) fn call_promise_static(vm: &mut Vm, method: &str, args: Vec<Value>, span: Span) -> Result<Value, VmError> {
    match method {
        "решить" => Ok(make_fulfilled_promise(args.into_iter().next().unwrap_or(Value::Undefined))),
        "отвергнуть" => Ok(make_rejected_promise(args.into_iter().next().unwrap_or(Value::Undefined))),
        "всех" => {
            let arr = expect_array(args.into_iter().next().unwrap_or(Value::Undefined), "СловоПацана.всех", span)?;
            Ok(run_aggregate(vm, AggregateKind::All, arr, span))
        }
        "всехУстаканить" => {
            let arr =
                expect_array(args.into_iter().next().unwrap_or(Value::Undefined), "СловоПацана.всехУстаканить", span)?;
            Ok(run_aggregate(vm, AggregateKind::AllSettled, arr, span))
        }
        "любой" => {
            let arr = expect_array(args.into_iter().next().unwrap_or(Value::Undefined), "СловоПацана.любой", span)?;
            Ok(run_aggregate(vm, AggregateKind::Any, arr, span))
        }
        "гонка" => {
            let arr = expect_array(args.into_iter().next().unwrap_or(Value::Undefined), "СловоПацана.гонка", span)?;
            if arr.is_empty() {
                return Err(VmError::new("'СловоПацана.гонка' требует непустой массив", span));
            }
            Ok(run_aggregate(vm, AggregateKind::Race, arr, span))
        }
        "сРешалками" => {
            let (promise, resolve, reject) = make_pending_promise();
            let mut map = ObjMap::new();
            map.insert("обещание".to_string(), promise);
            map.insert("решить".to_string(), resolve);
            map.insert("отвергнуть".to_string(), reject);
            Ok(Value::Object(Rc::new(RefCell::new(map))))
        }
        "попробовать" => {
            let func = args.into_iter().next().unwrap_or(Value::Undefined);
            match vm.call_value(func, None, &[], span) {
                Ok(val) => {
                    if let Value::Promise { .. } = &val {
                        Ok(val)
                    } else {
                        Ok(make_fulfilled_promise(val))
                    }
                }
                Err(e) => Ok(make_rejected_promise(vm.rejection_reason(e))),
            }
        }
        _ => Err(VmError::new(format!("У 'СловоПацана' нет метода '{method}'"), span)),
    }
}

pub(crate) fn call_promise_method(
    vm: &mut Vm,
    state: &Rc<RefCell<PromiseState>>,
    method: &str,
    args: Vec<Value>,
    span: Span,
) -> Result<Value, VmError> {
    match method {
        "потом" => {
            let mut iter = args.into_iter();
            let on_fulfill = iter.next().unwrap_or(Value::Undefined);
            let on_reject = iter.next().unwrap_or(Value::Undefined);
            chain_promise(vm, state, on_fulfill, on_reject, span)
        }
        "ловить" => {
            let on_reject = args.into_iter().next().unwrap_or(Value::Undefined);
            chain_promise(vm, state, Value::Undefined, on_reject, span)
        }
        "наконец" => {
            let cb = args.into_iter().next().unwrap_or(Value::Undefined);
            finally_promise(vm, state, cb, span)
        }
        _ => Err(VmError::new(format!("У обещания нет метода '{method}'"), span)),
    }
}

fn expect_array(v: Value, ctx: &str, span: Span) -> Result<Vec<Value>, VmError> {
    match v {
        Value::Array(a) => Ok(a.borrow().clone()),
        other => Err(VmError::new(format!("'{ctx}' ожидает массив, получено '{}'", other.type_name()), span)),
    }
}

fn array_value(items: Vec<Value>) -> Value {
    Value::Array(Rc::new(RefCell::new(items)))
}

fn run_aggregate(vm: &mut Vm, kind: AggregateKind, items: Vec<Value>, span: Span) -> Value {
    let (promise, resolve_cap, reject_cap) = make_pending_promise();
    let n = items.len();

    if n == 0 {
        match kind {
            AggregateKind::All | AggregateKind::AllSettled => {
                vm.enqueue_microtask(Box::new(move |vm, sp| {
                    vm.call_value(resolve_cap, None, &[array_value(Vec::new())], sp).map(|_| ())
                }));
            }
            AggregateKind::Any => {
                let err = aggregate_error(Vec::new());
                vm.enqueue_microtask(Box::new(move |vm, sp| vm.call_value(reject_cap, None, &[err], sp).map(|_| ())));
            }
            AggregateKind::Race => {}
        }
        return promise;
    }

    let state = Rc::new(RefCell::new(AggregateState {
        kind,
        remaining: n,
        results: vec![Value::Undefined; n],
        resolve: resolve_cap,
        reject: reject_cap,
        settled: false,
    }));

    for (idx, item) in items.into_iter().enumerate() {
        attach_aggregate(vm, &state, idx, item, span);
    }

    promise
}

fn attach_aggregate(vm: &mut Vm, state: &Rc<RefCell<AggregateState>>, index: usize, value: Value, _span: Span) {
    let fulfill = Value::PromiseAggregateHandler { state: Rc::clone(state), index, role: AggregateRole::Fulfill };
    let reject = Value::PromiseAggregateHandler { state: Rc::clone(state), index, role: AggregateRole::Reject };
    match value {
        Value::Promise { state: p_state } => {
            let snap = p_state.borrow().clone();
            match snap {
                PromiseState::Fulfilled(v) => {
                    vm.enqueue_microtask(Box::new(move |vm, sp| vm.call_value(fulfill, None, &[v], sp).map(|_| ())));
                }
                PromiseState::Rejected(v) => {
                    vm.enqueue_microtask(Box::new(move |vm, sp| vm.call_value(reject, None, &[v], sp).map(|_| ())));
                }
                PromiseState::Pending { .. } => {
                    if let PromiseState::Pending { on_resolve, on_reject } = &mut *p_state.borrow_mut() {
                        on_resolve.push(fulfill);
                        on_reject.push(reject);
                    }
                }
            }
        }
        other => {
            vm.enqueue_microtask(Box::new(move |vm, sp| vm.call_value(fulfill, None, &[other], sp).map(|_| ())));
        }
    }
}

pub(crate) fn apply_aggregate(
    vm: &mut Vm,
    state: Rc<RefCell<AggregateState>>,
    index: usize,
    role: AggregateRole,
    value: Value,
    span: Span,
) -> Result<(), VmError> {
    let (kind, settled) = {
        let s = state.borrow();
        (s.kind, s.settled)
    };

    let action: Option<(Value, Value)> = match (kind, role) {
        (AggregateKind::All, AggregateRole::Fulfill) => {
            if settled {
                None
            } else {
                let mut s = state.borrow_mut();
                s.results[index] = value;
                s.remaining -= 1;
                if s.remaining == 0 {
                    s.settled = true;
                    let res = std::mem::take(&mut s.results);
                    Some((s.resolve.clone(), array_value(res)))
                } else {
                    None
                }
            }
        }
        (AggregateKind::All, AggregateRole::Reject) => {
            if settled {
                None
            } else {
                let mut s = state.borrow_mut();
                s.settled = true;
                Some((s.reject.clone(), value))
            }
        }
        (AggregateKind::AllSettled, role) => {
            let mut entry = ObjMap::new();
            match role {
                AggregateRole::Fulfill => {
                    entry.insert("статус".to_string(), Value::string("выполнено"));
                    entry.insert("значение".to_string(), value);
                }
                AggregateRole::Reject => {
                    entry.insert("статус".to_string(), Value::string("отклонено"));
                    entry.insert("причина".to_string(), value);
                }
            }
            let mut s = state.borrow_mut();
            s.results[index] = Value::Object(Rc::new(RefCell::new(entry)));
            s.remaining -= 1;
            if s.remaining == 0 {
                s.settled = true;
                let res = std::mem::take(&mut s.results);
                Some((s.resolve.clone(), array_value(res)))
            } else {
                None
            }
        }
        (AggregateKind::Any, AggregateRole::Fulfill) => {
            if settled {
                None
            } else {
                let mut s = state.borrow_mut();
                s.settled = true;
                Some((s.resolve.clone(), value))
            }
        }
        (AggregateKind::Any, AggregateRole::Reject) => {
            if settled {
                None
            } else {
                let mut s = state.borrow_mut();
                s.results[index] = value;
                s.remaining -= 1;
                if s.remaining == 0 {
                    s.settled = true;
                    let errs = std::mem::take(&mut s.results);
                    Some((s.reject.clone(), aggregate_error(errs)))
                } else {
                    None
                }
            }
        }
        (AggregateKind::Race, role) => {
            if settled {
                None
            } else {
                let mut s = state.borrow_mut();
                s.settled = true;
                let cap = match role {
                    AggregateRole::Fulfill => s.resolve.clone(),
                    AggregateRole::Reject => s.reject.clone(),
                };
                Some((cap, value))
            }
        }
    };

    if let Some((cap, val)) = action {
        vm.call_value(cap, None, &[val], span)?;
    }
    Ok(())
}

fn aggregate_error(errors: Vec<Value>) -> Value {
    let mut agg = ObjMap::new();
    agg.insert("name".to_string(), Value::string("ВсёОбосралось"));
    agg.insert("message".to_string(), Value::string("Все обещания отклонены"));
    agg.insert("errors".to_string(), array_value(errors));
    Value::Object(Rc::new(RefCell::new(agg)))
}

fn chain_promise(
    vm: &mut Vm,
    state: &Rc<RefCell<PromiseState>>,
    on_fulfill: Value,
    on_reject: Value,
    _span: Span,
) -> Result<Value, VmError> {
    let (new_promise, resolve_cap, reject_cap) = make_pending_promise();
    let snap = state.borrow().clone();
    match snap {
        PromiseState::Fulfilled(v) => {
            vm.enqueue_microtask(Box::new(move |vm, span| {
                invoke_handler(vm, on_fulfill, v, resolve_cap, reject_cap, true, span)
            }));
        }
        PromiseState::Rejected(v) => {
            vm.enqueue_microtask(Box::new(move |vm, span| {
                invoke_handler(vm, on_reject, v, resolve_cap, reject_cap, false, span)
            }));
        }
        PromiseState::Pending { .. } => {
            let resolve_cb = Value::PromiseThenHandler {
                handler: Box::new(on_fulfill),
                resolve: Box::new(resolve_cap.clone()),
                reject: Box::new(reject_cap.clone()),
                is_fulfill: true,
            };
            let reject_cb = Value::PromiseThenHandler {
                handler: Box::new(on_reject),
                resolve: Box::new(resolve_cap),
                reject: Box::new(reject_cap),
                is_fulfill: false,
            };
            if let PromiseState::Pending { on_resolve, on_reject } = &mut *state.borrow_mut() {
                on_resolve.push(resolve_cb);
                on_reject.push(reject_cb);
            }
        }
    }
    Ok(new_promise)
}

fn finally_promise(vm: &mut Vm, state: &Rc<RefCell<PromiseState>>, cb: Value, _span: Span) -> Result<Value, VmError> {
    let (new_promise, resolve_cap, reject_cap) = make_pending_promise();
    let snap = state.borrow().clone();
    match snap {
        PromiseState::Fulfilled(v) => {
            vm.enqueue_microtask(Box::new(move |vm, span| {
                vm.call_value(cb, None, &[], span)?;
                vm.call_value(resolve_cap, None, &[v], span)?;
                Ok(())
            }));
        }
        PromiseState::Rejected(v) => {
            vm.enqueue_microtask(Box::new(move |vm, span| {
                vm.call_value(cb, None, &[], span)?;
                vm.call_value(reject_cap, None, &[v], span)?;
                Ok(())
            }));
        }
        PromiseState::Pending { .. } => {
            let resolve_cb = Value::PromiseFinallyHandler { cb: Box::new(cb.clone()), cap: Box::new(resolve_cap) };
            let reject_cb = Value::PromiseFinallyHandler { cb: Box::new(cb), cap: Box::new(reject_cap) };
            if let PromiseState::Pending { on_resolve, on_reject } = &mut *state.borrow_mut() {
                on_resolve.push(resolve_cb);
                on_reject.push(reject_cb);
            }
        }
    }
    Ok(new_promise)
}

pub(crate) fn invoke_handler(
    vm: &mut Vm,
    handler: Value,
    val: Value,
    resolve_cap: Value,
    reject_cap: Value,
    is_fulfill: bool,
    span: Span,
) -> Result<(), VmError> {
    if matches!(handler, Value::Undefined | Value::Null) {
        let cap = if is_fulfill { resolve_cap } else { reject_cap };
        vm.call_value(cap, None, &[val], span)?;
        return Ok(());
    }
    match vm.call_value(handler, None, &[val], span) {
        Ok(result) => {
            vm.call_value(resolve_cap, None, &[result], span)?;
        }
        Err(e) => {
            let reason = vm.rejection_reason(e);
            vm.call_value(reject_cap, None, &[reason], span)?;
        }
    }
    Ok(())
}
