use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::{Rc, Weak};

use crate::value::{
    AggregateState, ClassDef, Closure, Delegate, ForIter, GenState, ObjMap, PromiseState, UpvalueState, Value,
};

type ObjCell = Rc<RefCell<ObjMap>>;
type ArrCell = Rc<RefCell<Vec<Value>>>;
type UpCell = Rc<RefCell<UpvalueState>>;

#[derive(Default)]
pub(crate) struct GcRegistry {
    objects: RefCell<Vec<Weak<RefCell<ObjMap>>>>,
    arrays: RefCell<Vec<Weak<RefCell<Vec<Value>>>>>,
    upvalues: RefCell<Vec<Weak<RefCell<UpvalueState>>>>,
}

impl GcRegistry {
    pub(crate) fn track_object(&self, rc: &ObjCell) {
        self.objects.borrow_mut().push(Rc::downgrade(rc));
    }

    pub(crate) fn track_array(&self, rc: &ArrCell) {
        self.arrays.borrow_mut().push(Rc::downgrade(rc));
    }

    pub(crate) fn track_upvalue(&self, rc: &UpCell) {
        self.upvalues.borrow_mut().push(Rc::downgrade(rc));
    }

    pub(crate) fn live_count(&self) -> usize {
        prune(&self.objects) + prune(&self.arrays) + prune(&self.upvalues)
    }
}

fn prune<T>(cells: &RefCell<Vec<Weak<T>>>) -> usize {
    let mut v = cells.borrow_mut();
    v.retain(|w| w.strong_count() > 0);
    v.len()
}

enum Work {
    Value(Value),
    Closure(Rc<Closure>),
    Class(Rc<ClassDef>),
    Gen(Rc<RefCell<GenState>>),
    ForIter(Rc<RefCell<ForIter>>),
    Promise(Rc<RefCell<PromiseState>>),
    Aggregate(Rc<RefCell<AggregateState>>),
    Upvalue(UpCell),
}

#[derive(Default)]
pub(crate) struct Marker {
    marked_objects: HashSet<usize>,
    marked_arrays: HashSet<usize>,
    marked_upvalues: HashSet<usize>,
    seen: HashSet<usize>,
    work: Vec<Work>,
}

impl Marker {
    pub(crate) fn push_value(&mut self, value: &Value) {
        self.work.push(Work::Value(value.clone()));
    }

    pub(crate) fn push_upvalue(&mut self, up: &UpCell) {
        self.work.push(Work::Upvalue(Rc::clone(up)));
    }

    pub(crate) fn run(&mut self) {
        while let Some(item) = self.work.pop() {
            match item {
                Work::Value(value) => self.mark_value(&value),
                Work::Closure(rc) => self.mark_closure(&rc),
                Work::Class(rc) => {
                    if self.seen.insert(Rc::as_ptr(&rc) as usize) {
                        self.mark_class(&rc);
                    }
                }
                Work::Gen(rc) => {
                    if self.seen.insert(Rc::as_ptr(&rc) as usize) {
                        self.mark_gen(&rc.borrow());
                    }
                }
                Work::ForIter(rc) => {
                    if self.seen.insert(Rc::as_ptr(&rc) as usize) {
                        self.mark_for_iter(&rc.borrow());
                    }
                }
                Work::Promise(rc) => {
                    if self.seen.insert(Rc::as_ptr(&rc) as usize) {
                        self.mark_promise(&rc.borrow());
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
                Work::Upvalue(rc) => self.mark_upvalue(&rc),
            }
        }
    }

    fn mark_value(&mut self, value: &Value) {
        match value {
            Value::Number(_)
            | Value::BigInt(_)
            | Value::Str(_)
            | Value::Bool(_)
            | Value::Null
            | Value::Undefined
            | Value::Builtin(_)
            | Value::RegExp { .. }
            | Value::Host(_) => {}
            Value::Array(rc) => {
                if self.marked_arrays.insert(Rc::as_ptr(rc) as usize) {
                    for element in rc.borrow().iter() {
                        self.push_value(element);
                    }
                }
            }
            Value::Object(rc) => {
                if self.marked_objects.insert(Rc::as_ptr(rc) as usize) {
                    for (_, element) in rc.borrow().iter() {
                        self.push_value(element);
                    }
                }
            }
            Value::Function(rc) => self.work.push(Work::Closure(Rc::clone(rc))),
            Value::BoundMethod { receiver, .. } => self.push_value(receiver),
            Value::Class(rc) => self.work.push(Work::Class(Rc::clone(rc))),
            Value::Generator(rc) => self.work.push(Work::Gen(Rc::clone(rc))),
            Value::ForIter(rc) => self.work.push(Work::ForIter(Rc::clone(rc))),
            Value::Promise { state } | Value::PromiseCapability { state, .. } => {
                self.work.push(Work::Promise(Rc::clone(state)));
            }
            Value::PromiseThenHandler { handler, resolve, reject, .. } => {
                self.push_value(handler);
                self.push_value(resolve);
                self.push_value(reject);
            }
            Value::PromiseFinallyHandler { cb, cap } => {
                self.push_value(cb);
                self.push_value(cap);
            }
            Value::PromiseAggregateHandler { state, .. } => self.work.push(Work::Aggregate(Rc::clone(state))),
        }
    }

    fn mark_closure(&mut self, closure: &Rc<Closure>) {
        if self.seen.insert(Rc::as_ptr(closure) as usize) {
            for up in &closure.upvalues {
                self.push_upvalue(up);
            }
        }
    }

    fn mark_upvalue(&mut self, up: &UpCell) {
        if self.marked_upvalues.insert(Rc::as_ptr(up) as usize)
            && let UpvalueState::Closed(value) = &*up.borrow()
        {
            self.push_value(value);
        }
    }

    fn mark_class(&mut self, class: &Rc<ClassDef>) {
        let members = &class.members;
        for def in class
            .constructor
            .iter()
            .chain(members.methods.defs())
            .chain(members.getters.defs())
            .chain(members.setters.defs())
            .chain(members.static_methods.defs())
            .chain(members.static_getters.defs())
            .chain(members.static_setters.defs())
        {
            self.work.push(Work::Closure(Rc::clone(def)));
        }
        for (_, init, default) in &members.field_inits {
            if let Some(def) = init {
                self.work.push(Work::Closure(Rc::clone(def)));
            }
            if let Some(value) = default {
                self.push_value(value);
            }
        }
        for (_, value) in class.static_fields.borrow().iter() {
            self.push_value(value);
        }
        if let Some(parent) = &class.parent {
            self.work.push(Work::Class(Rc::clone(parent)));
        }
    }

    fn mark_gen(&mut self, state: &GenState) {
        self.work.push(Work::Closure(Rc::clone(&state.closure)));
        if let Some(owner) = &state.owner {
            self.work.push(Work::Class(Rc::clone(owner)));
        }
        for value in &state.stack {
            self.push_value(value);
        }
        for frame in &state.frames {
            self.work.push(Work::Closure(Rc::clone(&frame.closure)));
            if let Some(owner) = &frame.owner {
                self.work.push(Work::Class(Rc::clone(owner)));
            }
        }
        for up in &state.open_upvalues {
            self.push_upvalue(up);
        }
        self.push_value(&state.this);
        for value in &state.args {
            self.push_value(value);
        }
        if let Some(delegate) = &state.delegate {
            self.mark_delegate(delegate);
        }
    }

    fn mark_delegate(&mut self, delegate: &Delegate) {
        match delegate {
            Delegate::Generator(rc) => self.work.push(Work::Gen(Rc::clone(rc))),
            Delegate::Values { values, .. } => {
                for value in values {
                    self.push_value(value);
                }
            }
        }
    }

    fn mark_for_iter(&mut self, iter: &ForIter) {
        match iter {
            ForIter::Values { values, .. } => {
                for value in values {
                    self.push_value(value);
                }
            }
            ForIter::Generator(rc) => self.work.push(Work::Gen(Rc::clone(rc))),
        }
    }

    fn mark_promise(&mut self, state: &PromiseState) {
        match state {
            PromiseState::Pending { on_resolve, on_reject } => {
                for handler in on_resolve.iter().chain(on_reject.iter()) {
                    self.push_value(handler);
                }
            }
            PromiseState::Fulfilled(value) | PromiseState::Rejected(value) => self.push_value(value),
        }
    }

    pub(crate) fn sweep(&self, registry: &GcRegistry) -> usize {
        let mut cleared = 0;

        let mut objects = std::mem::take(&mut *registry.objects.borrow_mut());
        objects.retain(|weak| match weak.upgrade() {
            Some(rc) => {
                if !self.marked_objects.contains(&(Rc::as_ptr(&rc) as usize)) {
                    rc.borrow_mut().gc_clear();
                    cleared += 1;
                }
                true
            }
            None => false,
        });
        *registry.objects.borrow_mut() = objects;

        let mut arrays = std::mem::take(&mut *registry.arrays.borrow_mut());
        arrays.retain(|weak| match weak.upgrade() {
            Some(rc) => {
                if !self.marked_arrays.contains(&(Rc::as_ptr(&rc) as usize)) {
                    rc.borrow_mut().clear();
                    cleared += 1;
                }
                true
            }
            None => false,
        });
        *registry.arrays.borrow_mut() = arrays;

        let mut upvalues = std::mem::take(&mut *registry.upvalues.borrow_mut());
        upvalues.retain(|weak| match weak.upgrade() {
            Some(rc) => {
                if !self.marked_upvalues.contains(&(Rc::as_ptr(&rc) as usize)) {
                    *rc.borrow_mut() = UpvalueState::Closed(Value::Undefined);
                    cleared += 1;
                }
                true
            }
            None => false,
        });
        *registry.upvalues.borrow_mut() = upvalues;

        cleared
    }
}
