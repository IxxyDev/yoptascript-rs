pub mod abort;
pub mod array;
pub mod console;
pub mod data_view;
pub mod date;
pub mod error;
pub mod fs;
pub mod iterator;
pub mod json;
pub mod map;
pub mod math;
pub mod network;
pub mod number;
pub mod object;
pub mod process;
pub mod promise;
pub mod proxy;
pub mod reflect;
pub mod regexp;
pub mod set;
pub mod stdio;
pub mod string;
pub mod string_ns;
pub mod symbol;
pub mod typed_array;
pub mod weak;

use indexmap::IndexMap;
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
        Value::Array(_) => array::call(interp, receiver, method, args, span).map(|v| (v, None)),
        Value::String(_) => string::call(interp, receiver, method, args, span),
        Value::Number(_) => number::call_instance(interp, receiver, method, args, span),
        Value::Map(_) => map::call(interp, receiver, method, args, span).map(|v| (v, None)),
        Value::Set(_) => set::call(interp, receiver, method, args, span).map(|v| (v, None)),
        Value::Symbol { .. } => symbol::call_instance(interp, receiver, method, args, span),
        Value::Date(_) => date::call_instance(interp, receiver, method, args, span),
        Value::Promise { .. } => promise::call(interp, receiver, method, args, span),
        Value::Iterator(_) => iterator::call(interp, receiver, method, args, span),
        Value::RegExp(_) => regexp::call(interp, receiver, method, args, span),
        Value::TypedArray(_) => typed_array::call(interp, receiver, method, args, span),
        Value::DataView { .. } => data_view::call(interp, receiver, method, args, span),
        Value::AbortController { .. } | Value::AbortSignal { .. } => abort::call(interp, receiver, method, args, span),
        Value::WeakMap(_) => weak::call_weak_map(receiver, method, args, span).map(|v| (v, None)),
        Value::WeakSet(_) => weak::call_weak_set(receiver, method, args, span).map(|v| (v, None)),
        Value::WeakRef(_) => weak::call_weak_ref(receiver, method, args, span).map(|v| (v, None)),
        Value::FinalizationRegistry(_) => weak::call_registry(receiver, method, args, span).map(|v| (v, None)),
        _ => Err(RuntimeError::new(format!("Тип '{}' не имеет метода '{method}'", receiver.type_name()), span)),
    }
}

pub(crate) fn has_builtin_methods(value: &Value) -> bool {
    matches!(
        value,
        Value::Array(_)
            | Value::String(_)
            | Value::Number(_)
            | Value::Map(_)
            | Value::Set(_)
            | Value::Symbol { .. }
            | Value::Date(_)
            | Value::Promise { .. }
            | Value::Iterator(_)
            | Value::RegExp(_)
            | Value::TypedArray(_)
            | Value::DataView { .. }
            | Value::AbortController { .. }
            | Value::AbortSignal { .. }
            | Value::WeakMap(_)
            | Value::WeakSet(_)
            | Value::WeakRef(_)
            | Value::FinalizationRegistry(_)
    )
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
    if let Some(stripped) = namespaced.strip_prefix("Дата.") {
        return Some(date::call_static(interp, stripped, args, span));
    }
    if let Some(stripped) = namespaced.strip_prefix("СловоПацана.") {
        return Some(promise::call_static(interp, stripped, args, span));
    }
    if let Some(stripped) = namespaced.strip_prefix("Итератор.") {
        return Some(iterator::call_static(interp, stripped, args, span));
    }
    if let Some(stripped) = namespaced.strip_prefix("Отражение.") {
        return Some(reflect::call_static(interp, stripped, args, span));
    }
    if let Some(stripped) = namespaced.strip_prefix("Строка.") {
        return Some(string_ns::call_static(interp, stripped, args, span));
    }
    if let Some(stripped) = namespaced.strip_prefix("Косяк.") {
        return Some(error::call_static(interp, stripped, args, span));
    }
    if let Some(stripped) = namespaced.strip_prefix("ФС.") {
        return Some(fs::call_static(interp, stripped, args, span));
    }
    if let Some(stripped) = namespaced.strip_prefix("Процесс.") {
        return Some(process::call_static(interp, stripped, args, span));
    }
    if let Some(stripped) = namespaced.strip_prefix("Сеть.") {
        return Some(network::call_static(interp, stripped, args, span));
    }
    if let Some(stripped) = namespaced.strip_prefix("СигналОтмены.") {
        if stripped == "любой" {
            let sigs = match args.into_iter().next().unwrap_or(Value::Undefined) {
                Value::Array(a) => a.borrow().0.clone(),
                other => {
                    return Some(Err(RuntimeError::new(
                        format!("'СигналОтмены.любой' ожидает массив, получено '{}'", other.type_name()),
                        span,
                    )));
                }
            };
            return Some(abort::signal_any(interp, sigs, span));
        }
        if stripped == "отВремени" {
            let ms = match args.into_iter().next().unwrap_or(Value::Undefined) {
                Value::Number(n) if n.is_finite() && n >= 0.0 => n as u64,
                other => {
                    return Some(Err(RuntimeError::new(
                        format!(
                            "'СигналОтмены.отВремени' ожидает миллисекунды числом, получено '{}'",
                            other.type_name()
                        ),
                        span,
                    )));
                }
            };
            return Some(Ok(abort::make_timeout_signal(interp, ms)));
        }
        return Some(Err(RuntimeError::new(format!("У 'СигналОтмены' нет статического метода '{stripped}'"), span)));
    }
    if namespaced == "КонтроллёрОтмены" {
        return Some(Ok(abort::make_controller()));
    }
    if namespaced == "Карта" {
        return Some(map::construct(args, span));
    }
    if namespaced == "Набор" {
        return Some(set::construct(args, span));
    }
    if namespaced == "СлабаяКарта" {
        return Some(weak::construct_weak_map(args, span));
    }
    if namespaced == "СлабыйНабор" {
        return Some(weak::construct_weak_set(args, span));
    }
    if namespaced == "СлабаяСсылка" {
        return Some(weak::construct_weak_ref(args, span));
    }
    if namespaced == "РеестрФинализации" {
        return Some(weak::construct_registry(interp, args, span));
    }
    if namespaced == "Симбол" {
        return Some(symbol::construct(args, span));
    }
    if namespaced == "Посредник" {
        return Some(proxy::construct(args, span));
    }
    if namespaced == "СловоПацана" {
        return Some(promise::construct(interp, args, span));
    }
    if namespaced == "ОбластьБайтов" {
        return Some(typed_array::construct_array_buffer(args, span));
    }
    if namespaced == "ОбзорБайтов" {
        return Some(data_view::construct(args, span));
    }
    if let Some(kind) = typed_array::kind_from_name(namespaced) {
        return Some(typed_array::construct(kind, args, span));
    }
    None
}

pub fn build_globals() -> Vec<(String, Value)> {
    vec![
        ("Матан".to_string(), math::build_object()),
        ("Строка".to_string(), string_ns::build_object()),
        ("Кент".to_string(), object::build_object()),
        ("Хуйня".to_string(), number::build_object()),
        ("Жсон".to_string(), json::build_object()),
        ("Помойка".to_string(), array::build_object()),
        ("Карта".to_string(), Value::BuiltinFunction("Карта".to_string())),
        ("Набор".to_string(), Value::BuiltinFunction("Набор".to_string())),
        ("СлабаяКарта".to_string(), Value::BuiltinFunction("СлабаяКарта".to_string())),
        ("СлабыйНабор".to_string(), Value::BuiltinFunction("СлабыйНабор".to_string())),
        ("СлабаяСсылка".to_string(), Value::BuiltinFunction("СлабаяСсылка".to_string())),
        ("РеестрФинализации".to_string(), Value::BuiltinFunction("РеестрФинализации".to_string())),
        ("Симбол".to_string(), Value::BuiltinFunction("Симбол".to_string())),
        ("Дата".to_string(), Value::BuiltinFunction("Дата".to_string())),
        ("СловоПацана".to_string(), Value::BuiltinFunction("СловоПацана".to_string())),
        ("Итератор".to_string(), iterator::build_object()),
        ("КонтроллёрОтмены".to_string(), Value::BuiltinFunction("КонтроллёрОтмены".to_string())),
        ("СигналОтмены".to_string(), Value::BuiltinFunction("СигналОтмены".to_string())),
        ("ФС".to_string(), fs::build_object()),
        ("Процесс".to_string(), process::build_object()),
        ("Сеть".to_string(), network::build_object()),
        ("Отражение".to_string(), reflect::build_object()),
        ("Посредник".to_string(), Value::BuiltinFunction("Посредник".to_string())),
        ("ОбластьБайтов".to_string(), Value::BuiltinFunction("ОбластьБайтов".to_string())),
        ("Ц8Массив".to_string(), Value::BuiltinFunction("Ц8Массив".to_string())),
        ("Ц8ОграниченныйМассив".to_string(), Value::BuiltinFunction("Ц8ОграниченныйМассив".to_string())),
        ("Ч8Массив".to_string(), Value::BuiltinFunction("Ч8Массив".to_string())),
        ("Ц16Массив".to_string(), Value::BuiltinFunction("Ц16Массив".to_string())),
        ("Ч16Массив".to_string(), Value::BuiltinFunction("Ч16Массив".to_string())),
        ("Ц32Массив".to_string(), Value::BuiltinFunction("Ц32Массив".to_string())),
        ("Ч32Массив".to_string(), Value::BuiltinFunction("Ч32Массив".to_string())),
        ("Др32Массив".to_string(), Value::BuiltinFunction("Др32Массив".to_string())),
        ("Др64Массив".to_string(), Value::BuiltinFunction("Др64Массив".to_string())),
        ("ОбзорБайтов".to_string(), Value::BuiltinFunction("ОбзорБайтов".to_string())),
    ]
}

pub(crate) fn builtin(name: &str) -> Value {
    Value::BuiltinFunction(name.to_string())
}

pub(crate) fn object_of(pairs: &[(&str, Value)]) -> Value {
    let mut map = IndexMap::new();
    for (k, v) in pairs {
        map.insert((*k).to_string(), v.clone());
    }
    Value::object(map)
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
