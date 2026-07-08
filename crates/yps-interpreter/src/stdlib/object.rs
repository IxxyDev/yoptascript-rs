use std::collections::HashMap;

use indexmap::IndexMap;
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
        ("заморозить", builtin("Кент.заморозить")),
        ("заморожен", builtin("Кент.заморожен")),
        ("определитьСвойство", builtin("Кент.определитьСвойство")),
        ("описатьСвойство", builtin("Кент.описатьСвойство")),
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
                        if target_rc.borrow().frozen {
                            continue;
                        }
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
                    let has = if symbols::is_internal_key(&key) {
                        false
                    } else {
                        let guard = map.borrow();
                        guard.contains_key(&key)
                            || guard.contains_key(&symbols::getter_key(&key))
                            || guard.contains_key(&symbols::setter_key(&key))
                    };
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
                Value::Array(a) => a.borrow().0.clone(),
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
            let mut result = IndexMap::new();
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
                    let mut map = IndexMap::new();
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
            let mut map = IndexMap::new();
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
            if !target_rc.borrow().frozen {
                target_rc.borrow_mut().insert(symbols::PROTO.to_string(), proto);
            }
            Ok(target)
        }
        "заморозить" => {
            require_args(&args, 1, span, "Кент.заморозить")?;
            let target = args.into_iter().next().unwrap();
            if let Value::Object(map) = &target {
                map.borrow_mut().frozen = true;
            }
            Ok(target)
        }
        "заморожен" => {
            require_args(&args, 1, span, "Кент.заморожен")?;
            match &args[0] {
                Value::Object(map) => Ok(Value::Boolean(map.borrow().frozen)),
                _ => Ok(Value::Boolean(true)),
            }
        }
        "определитьСвойство" => {
            require_args(&args, 3, span, "Кент.определитьСвойство")?;
            let mut iter = args.into_iter();
            let target = iter.next().unwrap();
            let key = iter.next().unwrap().to_string();
            let descriptor = iter.next().unwrap();
            let target_rc = match &target {
                Value::Object(m) => m.clone(),
                _ => return Err(RuntimeError::new("Кент.определитьСвойство ожидает объект", span)),
            };
            let desc_map = match &descriptor {
                Value::Object(m) => m.clone(),
                _ => {
                    return Err(RuntimeError::new("Кент.определитьСвойство: дескриптор должен быть объектом", span));
                }
            };
            let has_value = desc_map.borrow().contains_key("значение");
            let getter = desc_map.borrow().get("получить").cloned();
            let setter = desc_map.borrow().get("установить").cloned();
            let has_accessor = getter.is_some() || setter.is_some();
            if has_value && has_accessor {
                return Err(RuntimeError::new(
                    "Дескриптор не может одновременно содержать 'значение' и 'получить'/'установить'",
                    span,
                ));
            }
            if let Some(getter) = &getter
                && !matches!(getter, Value::Undefined)
                && !getter.is_callable()
            {
                return Err(RuntimeError::new("Кент.определитьСвойство: 'получить' должно быть функцией", span));
            }
            if let Some(setter) = &setter
                && !matches!(setter, Value::Undefined)
                && !setter.is_callable()
            {
                return Err(RuntimeError::new("Кент.определитьСвойство: 'установить' должно быть функцией", span));
            }
            if target_rc.borrow().frozen {
                return Ok(target);
            }
            let mut guard = target_rc.borrow_mut();
            if has_accessor {
                guard.shift_remove(&key);
                guard.insert(symbols::getter_key(&key), getter.unwrap_or(Value::Undefined));
                guard.insert(symbols::setter_key(&key), setter.unwrap_or(Value::Undefined));
            } else {
                guard.shift_remove(&symbols::getter_key(&key));
                guard.shift_remove(&symbols::setter_key(&key));
                let value = desc_map.borrow().get("значение").cloned().unwrap_or(Value::Undefined);
                guard.insert(key, value);
            }
            drop(guard);
            Ok(target)
        }
        "описатьСвойство" => {
            require_args(&args, 2, span, "Кент.описатьСвойство")?;
            let key = args[1].to_string();
            match &args[0] {
                Value::Object(map) if !symbols::is_internal_key(&key) => {
                    let guard = map.borrow();
                    let getter = guard.get(&symbols::getter_key(&key)).cloned();
                    let setter = guard.get(&symbols::setter_key(&key)).cloned();
                    if getter.is_some() || setter.is_some() {
                        let mut desc = IndexMap::new();
                        desc.insert("получить".to_string(), getter.unwrap_or(Value::Undefined));
                        desc.insert("установить".to_string(), setter.unwrap_or(Value::Undefined));
                        return Ok(Value::object(desc));
                    }
                    if let Some(value) = guard.get(&key).cloned() {
                        let mut desc = IndexMap::new();
                        desc.insert("значение".to_string(), value);
                        return Ok(Value::object(desc));
                    }
                    Ok(Value::Undefined)
                }
                Value::Object(_) => Ok(Value::Undefined),
                _ => Err(RuntimeError::new("Кент.описатьСвойство ожидает объект", span)),
            }
        }
        _ => Err(RuntimeError::new(format!("У 'Кент' нет метода '{method}'"), span)),
    }
}
