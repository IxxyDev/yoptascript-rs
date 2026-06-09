use std::rc::Rc;

use yps_lexer::Span;

use crate::error::RuntimeError;
use crate::value::Value;

pub const GET: &[&str] = &["получить", "get"];
pub const SET: &[&str] = &["установить", "set"];
pub const HAS: &[&str] = &["есть", "has"];
pub const DELETE: &[&str] = &["удалить", "deleteProperty"];
pub const APPLY: &[&str] = &["применить", "apply"];
pub const CONSTRUCT: &[&str] = &["построить", "construct"];

pub fn construct(args: Vec<Value>, span: Span) -> Result<Value, RuntimeError> {
    let target = args.first().cloned().unwrap_or(Value::Undefined);
    let handler = args.get(1).cloned().unwrap_or(Value::Undefined);
    if !is_object_like(&target) {
        return Err(RuntimeError::new(
            format!("'Посредник' ожидает объект-цель, получено '{}'", target.type_name()),
            span,
        ));
    }
    if !matches!(handler, Value::Object(_)) {
        return Err(RuntimeError::new(
            format!("'Посредник' ожидает объект-обработчик, получено '{}'", handler.type_name()),
            span,
        ));
    }
    Ok(Value::Proxy { target: Rc::new(target), handler: Rc::new(handler) })
}

fn is_object_like(v: &Value) -> bool {
    matches!(
        v,
        Value::Object(_)
            | Value::Array(_)
            | Value::Function { .. }
            | Value::BuiltinFunction(_)
            | Value::Class(_)
            | Value::Proxy { .. }
    )
}

pub fn trap(handler: &Value, names: &[&str]) -> Option<Value> {
    if let Value::Object(map) = handler {
        let map = map.borrow();
        for name in names {
            if let Some(value) = map.get(*name)
                && !matches!(value, Value::Undefined | Value::Null)
            {
                return Some(value.clone());
            }
        }
    }
    None
}
