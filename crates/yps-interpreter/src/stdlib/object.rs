use std::collections::HashMap;

use yps_lexer::Span;

use crate::error::RuntimeError;
use crate::interpreter::Interpreter;
use crate::stdlib::{builtin, object_of, require_args};
use crate::symbols;
use crate::value::Value;

pub fn build_object() -> Value {
    object_of(&[
        ("ключи", builtin("Кент.ключи")),
        ("значения", builtin("Кент.значения")),
        ("записи", builtin("Кент.записи")),
        ("назначить", builtin("Кент.назначить")),
        ("имеетСвоё", builtin("Кент.имеетСвоё")),
        ("изЗаписей", builtin("Кент.изЗаписей")),
        ("группировать", builtin("Кент.группировать")),
        ("создать", builtin("Кент.создать")),
        ("прототип", builtin("Кент.прототип")),
        ("назначитьПрототип", builtin("Кент.назначитьПрототип")),
    ])
}

pub fn call_static(
    interp: &mut Interpreter,
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
                        .borrow()
                        .keys()
                        .filter(|k| !symbols::is_internal_key(k))
                        .map(|k| Value::String(k.clone()))
                        .collect();
                    Ok(Value::array(keys))
                }
                _ => Err(RuntimeError::new("Кент.ключи ожидает объект", span)),
            }
        }
        "значения" => {
            require_args(&args, 1, span, "Кент.значения")?;
            match &args[0] {
                Value::Object(map) => {
                    let vals: Vec<Value> = map
                        .borrow()
                        .iter()
                        .filter(|(k, _)| !symbols::is_internal_key(k))
                        .map(|(_, v)| v.clone())
                        .collect();
                    Ok(Value::array(vals))
                }
                _ => Err(RuntimeError::new("Кент.значения ожидает объект", span)),
            }
        }
        "записи" => {
            require_args(&args, 1, span, "Кент.записи")?;
            match &args[0] {
                Value::Object(map) => {
                    let entries: Vec<Value> = map
                        .borrow()
                        .iter()
                        .filter(|(k, _)| !symbols::is_internal_key(k))
                        .map(|(k, v)| Value::array(vec![Value::String(k.clone()), v.clone()]))
                        .collect();
                    Ok(Value::array(entries))
                }
                _ => Err(RuntimeError::new("Кент.записи ожидает объект", span)),
            }
        }
        "назначить" => {
            require_args(&args, 1, span, "Кент.назначить")?;
            let mut iter = args.into_iter();
            let target = iter.next().unwrap();
            let target_rc = match &target {
                Value::Object(m) => m.clone(),
                _ => return Err(RuntimeError::new("Кент.назначить ожидает объект", span)),
            };
            for src in iter {
                match src {
                    Value::Object(m) => {
                        let entries: Vec<(String, Value)> = m
                            .borrow()
                            .iter()
                            .filter(|(k, _)| !symbols::is_internal_key(k))
                            .map(|(k, v)| (k.clone(), v.clone()))
                            .collect();
                        let mut guard = target_rc.borrow_mut();
                        for (k, v) in entries {
                            guard.insert(k, v);
                        }
                    }
                    Value::Null | Value::Undefined => {}
                    _ => return Err(RuntimeError::new("Кент.назначить: источник должен быть объектом", span)),
                }
            }
            Ok(target)
        }
        "имеетСвоё" => {
            require_args(&args, 2, span, "Кент.имеетСвоё")?;
            let key = args[1].to_string();
            match &args[0] {
                Value::Object(map) => {
                    let has = !symbols::is_internal_key(&key) && map.borrow().contains_key(&key);
                    Ok(Value::Boolean(has))
                }
                _ => Err(RuntimeError::new("Кент.имеетСвоё ожидает объект", span)),
            }
        }
        "группировать" => {
            require_args(&args, 2, span, "Кент.группировать")?;
            let mut iter = args.into_iter();
            let collection = iter.next().unwrap();
            let callback = iter.next().unwrap();
            let items: Vec<Value> = match collection {
                Value::Array(a) => a.borrow().clone(),
                Value::Set(s) => s.borrow().iter().map(|k| k.as_value().clone()).collect(),
                Value::Map(entries) => {
                    entries.borrow().iter().map(|(k, v)| Value::array(vec![k.as_value().clone(), v.clone()])).collect()
                }
                Value::String(s) => s.chars().map(|c| Value::String(c.to_string())).collect(),
                other => {
                    return Err(RuntimeError::new(
                        format!(
                            "Кент.группировать ожидает массив/набор/карту/строку, получено '{}'",
                            other.type_name()
                        ),
                        span,
                    ));
                }
            };
            let mut groups: HashMap<String, Vec<Value>> = HashMap::new();
            let mut order: Vec<String> = Vec::new();
            for (i, item) in items.into_iter().enumerate() {
                let key_val =
                    interp.call_function(callback.clone(), vec![item.clone(), Value::Number(i as f64)], span)?;
                let key = key_val.to_string();
                let entry = groups.entry(key.clone()).or_insert_with(|| {
                    order.push(key.clone());
                    Vec::new()
                });
                entry.push(item);
            }
            let mut result = HashMap::new();
            for k in order {
                if let Some(vals) = groups.remove(&k) {
                    result.insert(k, Value::array(vals));
                }
            }
            Ok(Value::object(result))
        }
        "изЗаписей" => {
            require_args(&args, 1, span, "Кент.изЗаписей")?;
            match &args[0] {
                Value::Array(entries) => {
                    let mut map = HashMap::new();
                    for entry in entries.borrow().iter() {
                        match entry {
                            Value::Array(pair) if pair.borrow().len() >= 2 => {
                                let pair = pair.borrow();
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
                    Ok(Value::object(map))
                }
                _ => Err(RuntimeError::new("Кент.изЗаписей ожидает массив", span)),
            }
        }
        "создать" => {
            require_args(&args, 1, span, "Кент.создать")?;
            let proto = args.into_iter().next().unwrap();
            match &proto {
                Value::Object(_) | Value::Class(_) | Value::Null => {}
                other => {
                    return Err(RuntimeError::new(
                        format!("Кент.создать ожидает объект, класс или ноль, получено '{}'", other.type_name()),
                        span,
                    ));
                }
            }
            let mut map = HashMap::new();
            if !matches!(proto, Value::Null) {
                map.insert(symbols::PROTO.to_string(), proto);
            }
            Ok(Value::object(map))
        }
        "прототип" => {
            require_args(&args, 1, span, "Кент.прототип")?;
            match &args[0] {
                Value::Object(map) => {
                    let proto = map.borrow().get(symbols::PROTO).cloned();
                    match proto {
                        Some(Value::Class(cls)) => Ok(Interpreter::class_prototype_object(&cls)),
                        Some(Value::WeakClass(w)) => match w.upgrade() {
                            Some(cls) => Ok(Interpreter::class_prototype_object(&cls)),
                            None => Ok(Value::Null),
                        },
                        Some(other) => Ok(other),
                        None => Ok(Value::Null),
                    }
                }
                _ => Ok(Value::Null),
            }
        }
        "назначитьПрототип" => {
            require_args(&args, 2, span, "Кент.назначитьПрототип")?;
            let mut iter = args.into_iter();
            let target = iter.next().unwrap();
            let proto = iter.next().unwrap();
            match (&target, &proto) {
                (Value::Object(_), Value::Object(_) | Value::Class(_) | Value::Null) => {}
                _ => {
                    return Err(RuntimeError::new("Кент.назначитьПрототип ожидает (объект, объект|класс|ноль)", span));
                }
            }
            let target_rc = match &target {
                Value::Object(m) => m.clone(),
                _ => unreachable!(),
            };
            target_rc.borrow_mut().insert(symbols::PROTO.to_string(), proto);
            Ok(target)
        }
        _ => Err(RuntimeError::new(format!("У 'Кент' нет метода '{method}'"), span)),
    }
}
