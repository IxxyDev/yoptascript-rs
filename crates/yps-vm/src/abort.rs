use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use yps_interpreter::stdlib::abort::{abort_state, make_abort_error};
use yps_interpreter::value::{AbortState, Value as IValue};
use yps_lexer::Span;

use crate::bridge::{interp_to_vm, listen_for_abort, map_err, with_interp};
use crate::error::VmError;
use crate::promise::make_pending_promise;
use crate::value::{CapKind, Value};
use crate::vm::{Vm, report_async_error};

pub(crate) type SignalState = Rc<RefCell<AbortState>>;

pub(crate) fn signal_from_options(
    options: Option<&Value>,
    fn_name: &str,
    position: &str,
    span: Span,
) -> Result<Option<SignalState>, VmError> {
    let map = match options {
        None | Some(Value::Undefined | Value::Null) => return Ok(None),
        Some(Value::Object(map)) => map,
        Some(other) => {
            return Err(VmError::new(
                format!(
                    "'{fn_name}': {position} аргумент должен быть объектом опций, получено '{}'",
                    other.type_name()
                ),
                span,
            ));
        }
    };
    match map.borrow().get("сигнал") {
        None | Some(Value::Undefined | Value::Null) => Ok(None),
        Some(Value::Host(IValue::AbortSignal { state })) => Ok(Some(Rc::clone(state))),
        Some(other) => Err(VmError::new(
            format!("'{fn_name}': ожидался 'СигналОтмены' в опциях, получено '{}'", other.type_name()),
            span,
        )),
    }
}

pub(crate) fn is_aborted(signal: Option<&SignalState>) -> bool {
    signal.is_some_and(|state| state.borrow().aborted)
}

pub(crate) fn cancel_timer_on_abort(state: &SignalState, id: u64) {
    listen_for_abort(state, move |vm, _| vm.macrotasks.cancel(id));
}

fn reject_with_reason(vm: &mut Vm, reject: &Value, signal: &SignalState, span: Span) {
    let Value::PromiseCapability { state, .. } = reject else {
        return;
    };
    let reason = interp_to_vm(&signal.borrow().reason).unwrap_or(Value::Undefined);
    let _ = Vm::settle_promise(state, CapKind::Reject, reason, vm, span);
}

pub(crate) fn reject_wait_on_abort(state: &SignalState, reject: Value, timer_id: u64) {
    let signal = Rc::clone(state);
    listen_for_abort(state, move |vm, span| {
        vm.macrotasks.cancel(timer_id);
        reject_with_reason(vm, &reject, &signal, span);
    });
}

pub(crate) fn reject_wait_now(vm: &mut Vm, state: &SignalState, reject: Value) {
    let signal = Rc::clone(state);
    vm.enqueue_microtask(
        vec![reject.clone()],
        Box::new(move |vm, span| {
            reject_with_reason(vm, &reject, &signal, span);
            Ok(())
        }),
    );
}

pub(crate) fn signal_promise(vm: &mut Vm, state: &SignalState, span: Span) -> Value {
    let key = Rc::as_ptr(state) as usize;
    if let Some((_, promise)) = vm.signal_promises.get(&key) {
        return promise.clone();
    }
    let (promise, _resolve, reject) = make_pending_promise();
    if state.borrow().aborted {
        reject_with_reason(vm, &reject, state, span);
    } else {
        let signal = Rc::clone(state);
        listen_for_abort(state, move |vm, span| reject_with_reason(vm, &reject, &signal, span));
    }
    vm.signal_promises.insert(key, (Rc::clone(state), promise.clone()));
    promise
}

pub(crate) fn subscribe(vm: &mut Vm, state: &SignalState, callback: &Value) -> Option<Value> {
    if !matches!(callback, Value::Function(_) | Value::Builtin(_)) {
        return None;
    }
    vm.signal_listeners.push(callback.clone());
    if !state.borrow().aborted {
        return None;
    }
    let callback = callback.clone();
    vm.enqueue_microtask(
        vec![callback.clone()],
        Box::new(move |vm, span| vm.call_value(callback, None, &[], span).map(|_| ())),
    );
    Some(Value::Host(IValue::AbortUnsubscribe { state: Rc::clone(state), token: u64::MAX }))
}

pub(crate) fn timeout_signal(vm: &mut Vm, args: &[Value], span: Span) -> Result<Value, VmError> {
    let ms = match args.first() {
        Some(Value::Number(n)) if n.is_finite() && *n >= 0.0 => *n as u64,
        other => {
            return Err(VmError::new(
                format!(
                    "'СигналОтмены.отВремени' ожидает миллисекунды числом, получено '{}'",
                    other.map_or("неопределено", Value::type_name)
                ),
                span,
            ));
        }
    };
    let state: SignalState = Rc::default();
    let signal = Rc::clone(&state);
    vm.macrotasks.schedule(
        Duration::from_millis(ms),
        Vec::new(),
        Box::new(move |vm, sp| {
            let aborted = with_interp(vm, |interp| {
                abort_state(&signal, make_abort_error("Тайм-аут"), interp, sp).map_err(map_err)
            });
            if let Err(e) = aborted {
                report_async_error("отВремени", &e);
            }
            Ok(())
        }),
    );
    Ok(Value::Host(IValue::AbortSignal { state }))
}
