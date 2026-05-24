use std::cell::RefCell;
use std::rc::Rc;

use yps_lexer::Span;

use crate::error::RuntimeError;
use crate::value::{CapKind, PromiseState, Value};

use super::{Interpreter, Microtask};

impl Interpreter {
    pub(crate) fn drain_microtasks(&mut self, span: Span) -> Result<(), RuntimeError> {
        while let Some(task) = self.microtasks.pop_front() {
            task(self, span)?;
        }
        Ok(())
    }

    pub(crate) fn enqueue_microtask(&mut self, task: Microtask) {
        self.microtasks.push_back(task);
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

    pub(crate) fn settle_promise(
        state: &Rc<RefCell<PromiseState>>,
        kind: CapKind,
        value: Value,
        interp: &mut Interpreter,
        span: Span,
    ) -> Result<(), RuntimeError> {
        let already_settled = !matches!(&*state.borrow(), PromiseState::Pending { .. });
        if already_settled {
            return Ok(());
        }
        match kind {
            CapKind::Resolve => {
                if let Value::Promise { state: other } = &value {
                    if Rc::ptr_eq(state, other) {
                        return Err(RuntimeError::new("Обещание не может разрешить само себя", span));
                    }
                    let other_state = other.borrow().clone();
                    match other_state {
                        PromiseState::Fulfilled(v) => {
                            Self::set_fulfilled(state, v, interp);
                        }
                        PromiseState::Rejected(v) => {
                            Self::set_rejected(state, v, interp);
                        }
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
                    Self::set_fulfilled(state, value, interp);
                }
            }
            CapKind::Reject => {
                Self::set_rejected(state, value, interp);
            }
        }
        Ok(())
    }

    fn set_fulfilled(state: &Rc<RefCell<PromiseState>>, value: Value, interp: &mut Interpreter) {
        let callbacks: Vec<Value> = match &mut *state.borrow_mut() {
            PromiseState::Pending { on_resolve, .. } => std::mem::take(on_resolve),
            _ => return,
        };
        *state.borrow_mut() = PromiseState::Fulfilled(value.clone());
        for cb in callbacks {
            let val_cloned = value.clone();
            interp.enqueue_microtask(Box::new(move |interp, span| {
                interp.call_function(cb, vec![val_cloned], span).map(|_| ())
            }));
        }
    }

    fn set_rejected(state: &Rc<RefCell<PromiseState>>, value: Value, interp: &mut Interpreter) {
        let callbacks: Vec<Value> = match &mut *state.borrow_mut() {
            PromiseState::Pending { on_reject, .. } => std::mem::take(on_reject),
            _ => return,
        };
        *state.borrow_mut() = PromiseState::Rejected(value.clone());
        for cb in callbacks {
            let val_cloned = value.clone();
            interp.enqueue_microtask(Box::new(move |interp, span| {
                interp.call_function(cb, vec![val_cloned], span).map(|_| ())
            }));
        }
    }

    pub(crate) fn do_await(&mut self, value: Value, span: Span) -> Result<Value, RuntimeError> {
        let Value::Promise { state } = value else {
            return Ok(value);
        };
        if self.await_depth >= super::MAX_AWAIT_DEPTH {
            return Err(RuntimeError::new(
                format!("Превышена глубина ожидания обещаний ({})", super::MAX_AWAIT_DEPTH),
                span,
            ));
        }
        self.await_depth += 1;
        let result = loop {
            if let Err(e) = self.drain_microtasks(span) {
                break Err(e);
            }
            let snapshot = state.borrow().clone();
            match snapshot {
                PromiseState::Fulfilled(v) => break Ok(v),
                PromiseState::Rejected(v) => break Err(RuntimeError::thrown(v, span)),
                PromiseState::Pending { .. } => {
                    if self.macrotasks.is_empty() {
                        break Err(RuntimeError::new("Обещание не разрешено и очередь задач пуста", span));
                    }
                    let Some(task) = self.macrotasks.pop_next_blocking() else {
                        break Err(RuntimeError::new("Обещание не разрешено: нет готовых задач", span));
                    };
                    if let Err(e) = (task.task)(self, span) {
                        break Err(e);
                    }
                }
            }
        };
        self.await_depth -= 1;
        result
    }
}
