use std::cell::RefCell;
use std::collections::HashMap;

use yps_lexer::Span;

use crate::error::RuntimeError;
use crate::value::Value;

pub const HOST_CALLBACK_PREFIX: &str = "__host_cb__";

pub type HostCallbackFn = Box<dyn Fn(Vec<Value>, Span) -> Result<Value, RuntimeError>>;

thread_local! {
    static REGISTRY: RefCell<HashMap<u64, std::rc::Rc<HostCallbackFn>>> = RefCell::new(HashMap::new());
    static NEXT_ID: RefCell<u64> = const { RefCell::new(0) };
}

thread_local! {
    static CALL_THIS: RefCell<Option<Value>> = const { RefCell::new(None) };
}

pub fn set_call_this(this: Option<Value>) {
    CALL_THIS.with(|t| *t.borrow_mut() = this);
}

#[must_use]
pub fn take_call_this() -> Option<Value> {
    CALL_THIS.with(|t| t.borrow_mut().take())
}

#[must_use]
pub fn register(callback: HostCallbackFn) -> String {
    let id = NEXT_ID.with(|n| {
        let mut n = n.borrow_mut();
        let id = *n;
        *n += 1;
        id
    });
    REGISTRY.with(|r| r.borrow_mut().insert(id, std::rc::Rc::new(callback)));
    format!("{HOST_CALLBACK_PREFIX}{id}")
}

pub fn unregister(name: &str) {
    if let Some(id) = parse_id(name) {
        REGISTRY.with(|r| r.borrow_mut().remove(&id));
    }
}

#[must_use]
pub fn is_host_callback(name: &str) -> bool {
    parse_id(name).is_some()
}

fn parse_id(name: &str) -> Option<u64> {
    name.strip_prefix(HOST_CALLBACK_PREFIX).and_then(|s| s.parse::<u64>().ok())
}

pub fn invoke(name: &str, args: Vec<Value>, span: Span) -> Option<Result<Value, RuntimeError>> {
    let id = parse_id(name)?;
    let cb = REGISTRY.with(|r| r.borrow().get(&id).cloned());
    let Some(cb) = cb else {
        return Some(Err(RuntimeError::new("хост-замыкание VM уже недоступно", span)));
    };
    let result = cb(args, span);
    Some(result)
}
