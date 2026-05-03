pub mod array;
pub mod json;
pub mod map;
pub mod math;
pub mod number;
pub mod object;
pub mod set;
pub mod string;
pub mod symbol;

use std::collections::HashMap;

use yps_lexer::Span;

use crate::error::RuntimeError;
use crate::interpreter::Interpreter;
use crate::value::Value;

pub fn call_method(
    interp: &mut Interpreter,
    receiver: Value,
    method: &str,
    args: Vec<Value>,
    span: Span,
) -> Result<(Value, Option<Value>), RuntimeError> {
    match &receiver {
        Value::Array(_) => array::call(interp, receiver, method, args, span),
        Value::String(_) => string::call(interp, receiver, method, args, span),
        Value::Number(_) => number::call_instance(interp, receiver, method, args, span),
        Value::Map(_) => map::call(interp, receiver, method, args, span),
        Value::Set(_) => set::call(interp, receiver, method, args, span),
        Value::Symbol { .. } => symbol::call_instance(interp, receiver, method, args, span),
        _ => Err(RuntimeError::new(format!("Тип '{}' не имеет метода '{method}'", receiver.type_name()), span)),
    }
}

pub fn call_static_namespaced(
    interp: &mut Interpreter,
    namespaced: &str,
    args: Vec<Value>,
    span: Span,
) -> Option<Result<Value, RuntimeError>> {
    if let Some(stripped) = namespaced.strip_prefix("Матан.") {
        return Some(math::call_static(interp, stripped, args, span));
    }
    if let Some(stripped) = namespaced.strip_prefix("Помойка.") {
        return Some(array::call_static(interp, stripped, args, span));
    }
    if let Some(stripped) = namespaced.strip_prefix("Кент.") {
        return Some(object::call_static(interp, stripped, args, span));
    }
    if let Some(stripped) = namespaced.strip_prefix("Хуйня.") {
        return Some(number::call_static(interp, stripped, args, span));
    }
    if let Some(stripped) = namespaced.strip_prefix("Жсон.") {
        return Some(json::call_static(interp, stripped, args, span));
    }
    if let Some(stripped) = namespaced.strip_prefix("Карта.") {
        return Some(map::call_static(interp, stripped, args, span));
    }
    if let Some(stripped) = namespaced.strip_prefix("Симбол.") {
        return Some(symbol::call_static(interp, stripped, args, span));
    }
    if namespaced == "Карта" {
        return Some(map::construct(args, span));
    }
    if namespaced == "Набор" {
        return Some(set::construct(args, span));
    }
    if namespaced == "Симбол" {
        return Some(symbol::construct(args, span));
    }
    None
}

pub fn build_globals() -> Vec<(String, Value)> {
    vec![
        ("Матан".to_string(), math::build_object()),
        ("Кент".to_string(), object::build_object()),
        ("Хуйня".to_string(), number::build_object()),
        ("Жсон".to_string(), json::build_object()),
        ("Помойка".to_string(), array::build_object()),
        ("Карта".to_string(), Value::BuiltinFunction("Карта".to_string())),
        ("Набор".to_string(), Value::BuiltinFunction("Набор".to_string())),
        ("Симбол".to_string(), Value::BuiltinFunction("Симбол".to_string())),
    ]
}

pub(crate) fn builtin(name: &str) -> Value {
    Value::BuiltinFunction(name.to_string())
}

pub(crate) fn object_of(pairs: &[(&str, Value)]) -> Value {
    let mut map = HashMap::new();
    for (k, v) in pairs {
        map.insert((*k).to_string(), v.clone());
    }
    Value::Object(map)
}

pub(crate) fn require_args(args: &[Value], min: usize, span: Span, method: &str) -> Result<(), RuntimeError> {
    if args.len() < min {
        Err(RuntimeError::new(format!("'{method}' ожидает минимум {min} аргумент(ов), получено {}", args.len()), span))
    } else {
        Ok(())
    }
}

pub(crate) fn as_number(v: &Value, span: Span, ctx: &str) -> Result<f64, RuntimeError> {
    match v {
        Value::Number(n) => Ok(*n),
        _ => Err(RuntimeError::new(format!("'{ctx}' ожидает число, получено '{}'", v.type_name()), span)),
    }
}

pub(crate) fn as_string<'a>(v: &'a Value, span: Span, ctx: &str) -> Result<&'a str, RuntimeError> {
    match v {
        Value::String(s) => Ok(s),
        _ => Err(RuntimeError::new(format!("'{ctx}' ожидает строку, получено '{}'", v.type_name()), span)),
    }
}
