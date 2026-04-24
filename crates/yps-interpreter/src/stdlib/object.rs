use std::collections::HashMap;

use yps_lexer::Span;

use crate::error::RuntimeError;
use crate::interpreter::Interpreter;
use crate::stdlib::{builtin, object_of, require_args};
use crate::value::Value;

pub fn build_object() -> Value {
    object_of(&[
        ("ключи", builtin("Кент.ключи")),
        ("значения", builtin("Кент.значения")),
        ("записи", builtin("Кент.записи")),
        ("назначить", builtin("Кент.назначить")),
        ("имеетСвоё", builtin("Кент.имеетСвоё")),
        ("изЗаписей", builtin("Кент.изЗаписей")),
    ])
}

pub fn call_static(
    _interp: &mut Interpreter,
    method: &str,
    args: Vec<Value>,
    span: Span,
) -> Result<Value, RuntimeError> {
    match method {
        "ключи" => {
            require_args(&args, 1, span, "Кент.ключи")?;
            match &args[0] {
                Value::Object(map) => {
                    let keys: Vec<Value> = map
                        .keys()
                        .filter(|k| !k.starts_with("__") || (!k.starts_with("__get_") && !k.starts_with("__set_")))
                        .filter(|k| k.as_str() != "__class__")
                        .map(|k| Value::String(k.clone()))
                        .collect();
                    Ok(Value::Array(keys))
                }
                _ => Err(RuntimeError::new("Кент.ключи ожидает объект", span)),
            }
        }
        "значения" => {
            require_args(&args, 1, span, "Кент.значения")?;
            match &args[0] {
                Value::Object(map) => {
                    let vals: Vec<Value> =
                        map.iter().filter(|(k, _)| !is_internal_key(k)).map(|(_, v)| v.clone()).collect();
                    Ok(Value::Array(vals))
                }
                _ => Err(RuntimeError::new("Кент.значения ожидает объект", span)),
            }
        }
        "записи" => {
            require_args(&args, 1, span, "Кент.записи")?;
            match &args[0] {
                Value::Object(map) => {
                    let entries: Vec<Value> = map
                        .iter()
                        .filter(|(k, _)| !is_internal_key(k))
                        .map(|(k, v)| Value::Array(vec![Value::String(k.clone()), v.clone()]))
                        .collect();
                    Ok(Value::Array(entries))
                }
                _ => Err(RuntimeError::new("Кент.записи ожидает объект", span)),
            }
        }
        "назначить" => {
            require_args(&args, 1, span, "Кент.назначить")?;
            let mut iter = args.into_iter();
            let target = iter.next().unwrap();
            let mut map = match target {
                Value::Object(m) => m,
                _ => return Err(RuntimeError::new("Кент.назначить ожидает объект", span)),
            };
            for src in iter {
                match src {
                    Value::Object(m) => {
                        for (k, v) in m {
                            map.insert(k, v);
                        }
                    }
                    Value::Null | Value::Undefined => {}
                    _ => return Err(RuntimeError::new("Кент.назначить: источник должен быть объектом", span)),
                }
            }
            Ok(Value::Object(map))
        }
        "имеетСвоё" => {
            require_args(&args, 2, span, "Кент.имеетСвоё")?;
            let key = args[1].to_string();
            match &args[0] {
                Value::Object(map) => Ok(Value::Boolean(map.contains_key(&key))),
                _ => Err(RuntimeError::new("Кент.имеетСвоё ожидает объект", span)),
            }
        }
        "изЗаписей" => {
            require_args(&args, 1, span, "Кент.изЗаписей")?;
            match &args[0] {
                Value::Array(entries) => {
                    let mut map = HashMap::new();
                    for entry in entries {
                        match entry {
                            Value::Array(pair) if pair.len() >= 2 => {
                                map.insert(pair[0].to_string(), pair[1].clone());
                            }
                            _ => {
                                return Err(RuntimeError::new(
                                    "Кент.изЗаписей: каждая запись — [ключ, значение]",
                                    span,
                                ));
                            }
                        }
                    }
                    Ok(Value::Object(map))
                }
                _ => Err(RuntimeError::new("Кент.изЗаписей ожидает массив", span)),
            }
        }
        _ => Err(RuntimeError::new(format!("У 'Кент' нет метода '{method}'"), span)),
    }
}

fn is_internal_key(k: &str) -> bool {
    k == "__class__" || k.starts_with("__get_") || k.starts_with("__set_")
}
