use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use crate::environment::EnvFrame;
use crate::value::{
    AbortState, AggregateState, ClassDef, GenFrame, GenState, IteratorState, PromiseState, TryState, Value,
};

use super::Interpreter;

pub(crate) enum GcRoot {
    Value(Value),
    Frame(Rc<RefCell<EnvFrame>>),
}

impl Interpreter {
    pub fn collect_cycles(&mut self) -> usize {
        self.collect_cycles_with_roots(&[])
    }

    pub(super) fn collect_cycles_with_roots(&mut self, extra_roots: &[Value]) -> usize {
        let mut marker = Marker::default();
        marker.work.push(Work::Frame(self.env.snapshot()));
        for value in &self.pending_initializers {
            marker.push_value(value);
        }
        for value in self.current_exports.values() {
            marker.push_value(value);
        }
        for module in self.module_cache.borrow().values() {
            module.for_each_export_value(|value| marker.push_value(value));
        }
        for value in extra_roots {
            marker.push_value(value);
        }
        for task in &self.microtasks {
            for root in &task.roots {
                marker.push_root(root);
            }
        }
        for root in self.macrotasks.roots() {
            marker.push_root(root);
        }
        marker.run();

        let registry = self.env.registry();
        let mut cleared = 0;
        for frame in registry.live_frames() {
            if !marker.marked_frames.contains(&(Rc::as_ptr(&frame) as usize)) {
                frame.borrow_mut().gc_clear();
                cleared += 1;
            }
        }
        registry.prune_and_count();
        cleared
    }
}

enum Work {
    Value(Value),
    Frame(Rc<RefCell<EnvFrame>>),
    Iter(Rc<RefCell<IteratorState>>),
    Promise(Rc<RefCell<PromiseState>>),
    Aggregate(Rc<RefCell<AggregateState>>),
    Abort(Rc<RefCell<AbortState>>),
    Class(Rc<ClassDef>),
}

#[derive(Default)]
struct Marker {
    marked_frames: HashSet<usize>,
    seen: HashSet<usize>,
    work: Vec<Work>,
}

impl Marker {
    fn push_value(&mut self, value: &Value) {
        self.work.push(Work::Value(value.clone()));
    }

    fn push_root(&mut self, root: &GcRoot) {
        match root {
            GcRoot::Value(value) => self.push_value(value),
            GcRoot::Frame(frame) => self.work.push(Work::Frame(Rc::clone(frame))),
        }
    }

    fn run(&mut self) {
        while let Some(item) = self.work.pop() {
            match item {
                Work::Value(value) => self.mark_value(&value),
                Work::Frame(frame) => self.mark_frame(&frame),
                Work::Iter(rc) => {
                    if self.seen.insert(Rc::as_ptr(&rc) as usize) {
                        let state = rc.borrow();
                        self.mark_iter_state(&state);
                    }
                }
                Work::Promise(rc) => {
                    if self.seen.insert(Rc::as_ptr(&rc) as usize) {
                        match &*rc.borrow() {
                            PromiseState::Pending { on_resolve, on_reject } => {
                                for handler in on_resolve.iter().chain(on_reject.iter()) {
                                    self.push_value(handler);
                                }
                            }
                            PromiseState::Fulfilled(value) | PromiseState::Rejected(value) => self.push_value(value),
                        }
                    }
                }
                Work::Aggregate(rc) => {
                    if self.seen.insert(Rc::as_ptr(&rc) as usize) {
                        let state = rc.borrow();
                        for value in &state.results {
                            self.push_value(value);
                        }
                        self.push_value(&state.resolve);
                        self.push_value(&state.reject);
                    }
                }
                Work::Abort(rc) => {
                    if self.seen.insert(Rc::as_ptr(&rc) as usize) {
                        let state = rc.borrow();
                        self.push_value(&state.reason);
                        for (_, listener) in &state.listeners {
                            self.push_value(listener);
                        }
                        if let Some(promise) = state.promise.borrow().as_ref() {
                            self.push_value(promise);
                        }
                    }
                }
                Work::Class(rc) => {
                    if self.seen.insert(Rc::as_ptr(&rc) as usize) {
                        self.mark_class(&rc);
                    }
                }
            }
        }
    }

    fn mark_frame(&mut self, frame: &Rc<RefCell<EnvFrame>>) {
        if self.marked_frames.insert(Rc::as_ptr(frame) as usize) {
            let borrowed = frame.borrow();
            for value in borrowed.gc_values() {
                self.push_value(value);
            }
            if let Some(parent) = borrowed.gc_parent() {
                self.work.push(Work::Frame(parent));
            }
        }
    }

    fn mark_value(&mut self, value: &Value) {
        match value {
            Value::Number(_)
            | Value::BigInt(_)
            | Value::String(_)
            | Value::Boolean(_)
            | Value::BuiltinFunction(_)
            | Value::Symbol { .. }
            | Value::RegExp { .. }
            | Value::Date(_)
            | Value::AbortCancelTimer { .. }
            | Value::ArrayBuffer(_)
            | Value::TypedArray { .. }
            | Value::DataView { .. }
            | Value::Undefined
            | Value::Null
            | Value::WeakClass(_) => {}
            Value::Array(rc) => {
                if self.seen.insert(Rc::as_ptr(rc) as usize) {
                    for element in rc.borrow().iter() {
                        self.push_value(element);
                    }
                }
            }
            Value::Object(rc) => {
                if self.seen.insert(Rc::as_ptr(rc) as usize) {
                    for element in rc.borrow().values() {
                        self.push_value(element);
                    }
                }
            }
            Value::Map(rc) => {
                if self.seen.insert(Rc::as_ptr(rc) as usize) {
                    for (key, val) in rc.borrow().iter() {
                        self.push_value(key.as_value());
                        self.push_value(val);
                    }
                }
            }
            Value::Set(rc) => {
                if self.seen.insert(Rc::as_ptr(rc) as usize) {
                    for element in rc.borrow().iter() {
                        self.push_value(element.as_value());
                    }
                }
            }
            Value::Function { env, .. } => self.work.push(Work::Frame(Rc::clone(env))),
            Value::BoundMethod { receiver, .. } => self.push_value(receiver),
            Value::Class(rc) => self.work.push(Work::Class(Rc::clone(rc))),
            Value::Promise { state } | Value::PromiseCapability { state, .. } => {
                self.work.push(Work::Promise(Rc::clone(state)));
            }
            Value::PromiseThenHandler { handler, resolve, reject, is_fulfill: _ } => {
                self.push_value(handler);
                self.push_value(resolve);
                self.push_value(reject);
            }
            Value::PromiseFinallyHandler { cb, cap } => {
                self.push_value(cb);
                self.push_value(cap);
            }
            Value::PromiseAggregateHandler { state, .. } => self.work.push(Work::Aggregate(Rc::clone(state))),
            Value::Iterator(rc) => self.work.push(Work::Iter(Rc::clone(rc))),
            Value::AbortController { state } | Value::AbortSignal { state } | Value::AbortUnsubscribe { state, .. } => {
                self.work.push(Work::Abort(Rc::clone(state)));
            }
            Value::AbortListener { target } => {
                if let Some(rc) = target.upgrade() {
                    self.work.push(Work::Abort(rc));
                }
            }
            Value::AbortRejectPromise { reject_cap, .. } => self.push_value(reject_cap),
            Value::Proxy { target, handler } => {
                self.push_value(target);
                self.push_value(handler);
            }
            Value::WeakMap(rc) => {
                if self.seen.insert(Rc::as_ptr(rc) as usize) {
                    for (_, value) in rc.borrow().values() {
                        self.push_value(value);
                    }
                }
            }
            Value::WeakSet(_) | Value::WeakRef(_) => {}
            Value::FinalizationRegistry(rc) => {
                if self.seen.insert(Rc::as_ptr(rc) as usize) {
                    let state = rc.borrow();
                    self.push_value(&state.callback);
                    for entry in &state.entries {
                        self.push_value(&entry.held);
                    }
                }
            }
        }
    }

    fn mark_iter_state(&mut self, state: &IteratorState) {
        match state {
            IteratorState::Chars { .. } | IteratorState::RegexMatches { .. } | IteratorState::Done => {}
            IteratorState::Array { values, .. } => {
                for value in values {
                    self.push_value(value);
                }
            }
            IteratorState::MapEntries { entries, .. } => {
                for (key, val) in entries {
                    self.push_value(key);
                    self.push_value(val);
                }
            }
            IteratorState::Map { inner, func, .. } | IteratorState::Filter { inner, func, .. } => {
                self.push_value(func);
                self.mark_iter_state(inner);
            }
            IteratorState::Take { inner, .. } | IteratorState::Drop { inner, .. } => self.mark_iter_state(inner),
            IteratorState::Concat { iters } => {
                for iter in iters {
                    self.mark_iter_state(iter);
                }
            }
            IteratorState::Generator(gen_state) => self.mark_gen_state(gen_state),
        }
    }

    fn mark_gen_state(&mut self, gen_state: &GenState) {
        self.work.push(Work::Frame(gen_state.env.snapshot()));
        for frame in &gen_state.frames {
            match frame {
                GenFrame::Block { .. } | GenFrame::While { .. } | GenFrame::DoWhile { .. } | GenFrame::For { .. } => {}
                GenFrame::ForIter { iter, .. } => self.work.push(Work::Iter(Rc::clone(iter))),
                GenFrame::Delegate { inner, .. } => self.work.push(Work::Iter(Rc::clone(inner))),
                GenFrame::TryCatch { state, .. } => match state {
                    TryState::FinallyAfterThrow(value) | TryState::FinallyAfterReturn(value) => self.push_value(value),
                    TryState::Trying
                    | TryState::InCatch
                    | TryState::FinallyNormal
                    | TryState::FinallyAfterBreak
                    | TryState::FinallyAfterContinue => {}
                },
            }
        }
    }

    fn mark_class(&mut self, class: &ClassDef) {
        for method in class
            .constructor
            .iter()
            .chain(class.methods.values())
            .chain(class.static_methods.values())
            .chain(class.getters.values())
            .chain(class.setters.values())
            .chain(class.static_getters.values())
            .chain(class.static_setters.values())
        {
            self.work.push(Work::Frame(Rc::clone(&method.env)));
        }
        for value in class.static_fields.borrow().values() {
            self.push_value(value);
        }
        for (_, _, default) in &class.field_inits {
            if let Some(value) = default {
                self.push_value(value);
            }
        }
        for value in &class.instance_initializers {
            self.push_value(value);
        }
        if let Some(parent) = &class.parent {
            self.work.push(Work::Class(Rc::clone(parent)));
        }
    }
}
