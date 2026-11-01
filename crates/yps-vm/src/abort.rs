use std::cell::RefCell;
use std::rc::Rc;

use yps_interpreter::value::{AbortState, Value as IValue};

use crate::value::Value;
use crate::vm::Vm;

pub(crate) type SignalState = Rc<RefCell<AbortState>>;

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
