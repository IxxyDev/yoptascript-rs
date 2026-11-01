use std::cell::RefCell;
use std::rc::Rc;

use yps_interpreter::value::{AbortState, Value as IValue};
use yps_lexer::Span;

use crate::bridge::{interp_to_vm, listen_for_abort};
use crate::error::VmError;
use crate::value::{CapKind, Value};
use crate::vm::Vm;

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
