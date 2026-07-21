use yps_lexer::Span;

use crate::error::RuntimeError;
use crate::interpreter::Interpreter;
use crate::symbols;
use crate::value::{Value, same_value};

use super::{builtin, object_of, require_args};

pub fn build_object() -> Value {
    object_of(&[
        ("получить", builtin("Отражение.получить")),
        ("установить", builtin("Отражение.установить")),
        ("есть", builtin("Отражение.есть")),
        ("удалить", builtin("Отражение.удалить")),
        ("прототипОт", builtin("Отражение.прототипОт")),
        ("назначитьПрототип", builtin("Отражение.назначитьПрототип")),
        ("собственныеКлючи", builtin("Отражение.собственныеКлючи")),
        ("определитьСвойство", builtin("Отражение.определитьСвойство")),
        ("описатьСвойство", builtin("Отражение.описатьСвойство")),
        ("расширяем", builtin("Отражение.расширяем")),
        ("запретитьРасширение", builtin("Отражение.запретитьРасширение")),
        ("применить", builtin("Отражение.применить")),
        ("построить", builtin("Отражение.построить")),
    ])
}

pub fn call_static(
    interp: &mut Interpreter,
    method: &str,
    args: Vec<Value>,
    span: Span,
) -> Result<Value, RuntimeError> {
    match method {
        "получить" => {
            require_args(&args, 2, span, "Отражение.получить")?;
            let obj = args[0].clone();
            let key = args[1].to_string();
            reflect_get(interp, obj, &key, span)
        }
        "установить" => {
            require_args(&args, 3, span, "Отражение.установить")?;
            let obj = args[0].clone();
            let key = args[1].to_string();
            let value = args[2].clone();
            reflect_set(interp, obj, &key, value, span)
        }
        "есть" => {
            require_args(&args, 2, span, "Отражение.есть")?;
            let obj = args[0].clone();
            let key = args[1].to_string();
            reflect_has(interp, obj, &key, span)
        }
        "удалить" => {
            require_args(&args, 2, span, "Отражение.удалить")?;
            let obj = args[0].clone();
            let key = args[1].to_string();
            reflect_delete(interp, obj, &key, span)
        }
        "прототипОт" => {
            require_args(&args, 1, span, "Отражение.прототипОт")?;
            reflect_get_prototype_of(interp, args[0].clone(), span)
        }
        "назначитьПрототип" => {
            require_args(&args, 2, span, "Отражение.назначитьПрототип")?;
            let obj = args[0].clone();
            let proto = args[1].clone();
            reflect_set_prototype_of(interp, obj, proto, span)
        }
        "собственныеКлючи" => {
            require_args(&args, 1, span, "Отражение.собственныеКлючи")?;
            reflect_own_keys(interp, args[0].clone(), span)
        }
        "определитьСвойство" => {
            require_args(&args, 3, span, "Отражение.определитьСвойство")?;
            let obj = args[0].clone();
            let key = args[1].to_string();
            let descriptor = args[2].clone();
            reflect_define_property(interp, obj, &key, descriptor, span)
        }
        "описатьСвойство" => {
            require_args(&args, 2, span, "Отражение.описатьСвойство")?;
            let obj = args[0].clone();
            let key = args[1].to_string();
            reflect_get_own_property_descriptor(interp, obj, &key, span)
        }
        "расширяем" => {
            require_args(&args, 1, span, "Отражение.расширяем")?;
            reflect_is_extensible(interp, args[0].clone(), span)
        }
        "запретитьРасширение" => {
            require_args(&args, 1, span, "Отражение.запретитьРасширение")?;
            reflect_prevent_extensions(interp, args[0].clone(), span)
        }
        "применить" => {
            require_args(&args, 3, span, "Отражение.применить")?;
            let func = args[0].clone();
            let args_val = args[2].clone();
            let call_args = match args_val {
                Value::Array(a) => a.borrow().0.clone(),
                Value::Undefined | Value::Null => vec![],
                other => {
                    return Err(RuntimeError::new(
                        format!("'Отражение.применить' ожидает массив аргументов, получено '{}'", other.type_name()),
                        span,
                    ));
                }
            };
            interp.call_function(func, call_args, span)
        }
        "построить" => {
            require_args(&args, 2, span, "Отражение.построить")?;
            let constructor = args[0].clone();
            let call_args = match args[1].clone() {
                Value::Array(a) => a.borrow().0.clone(),
                Value::Undefined | Value::Null => vec![],
                other => {
                    return Err(RuntimeError::new(
                        format!("'Отражение.построить' ожидает массив аргументов, получено '{}'", other.type_name()),
                        span,
                    ));
                }
            };
            interp.construct_instance(constructor, call_args, span)
        }
        _ => Err(RuntimeError::new(format!("У 'Отражение' нет метода '{method}'"), span)),
    }
}

fn reflect_get(interp: &mut Interpreter, obj: Value, key: &str, span: Span) -> Result<Value, RuntimeError> {
    match &obj {
        Value::Object(map) => {
            if let Some(val) = map.borrow().get(key) {
                return Ok(val.clone());
            }
            let proto = map.borrow().get(symbols::PROTO).cloned();
            if let Some(proto) = proto {
                match proto {
                    Value::Class(_) | Value::Null => return Ok(Value::Undefined),
                    _ => return reflect_get(interp, proto, key, span),
                }
            }
            Ok(Value::Undefined)
        }
        Value::Array(arr) => {
            if key == "length" || key == "длина" {
                return Ok(Value::Number(arr.borrow().len() as f64));
            }
            if let Ok(idx) = key.parse::<usize>() {
                return Ok(arr.borrow().get(idx).cloned().unwrap_or(Value::Undefined));
            }
            Ok(Value::Undefined)
        }
        _ => interp.eval_member(obj, key, span),
    }
}

fn reflect_set(
    interp: &mut Interpreter,
    obj: Value,
    key: &str,
    value: Value,
    span: Span,
) -> Result<Value, RuntimeError> {
    match &obj {
        Value::Proxy { target, handler } => {
            let (t, h) = ((**target).clone(), (**handler).clone());
            interp.proxy_set(&t, &h, key, value, obj.clone(), span)?;
            Ok(Value::Boolean(true))
        }
        Value::Object(map) => {
            let ok = map.borrow().can_write_key(key);
            if ok {
                map.borrow_mut().insert(key.to_string(), value);
            }
            Ok(Value::Boolean(ok))
        }
        Value::Array(arr) => {
            if let Ok(idx) = key.parse::<usize>() {
                let mut guard = arr.borrow_mut();
                if let Some(slot) = guard.get_mut(idx) {
                    *slot = value;
                    return Ok(Value::Boolean(true));
                }
            }
            Ok(Value::Boolean(false))
        }
        _ => Err(RuntimeError::new(
            format!("'Отражение.установить' ожидает объект или массив, получено '{}'", obj.type_name()),
            span,
        )),
    }
}

fn reflect_delete(interp: &mut Interpreter, obj: Value, key: &str, span: Span) -> Result<Value, RuntimeError> {
    match &obj {
        Value::Proxy { target, handler } => {
            let (t, h) = ((**target).clone(), (**handler).clone());
            Ok(Value::Boolean(interp.proxy_delete(&t, &h, key, span)?))
        }
        Value::Object(map) => {
            let can = map.borrow().can_delete();
            if can {
                map.borrow_mut().shift_remove(key);
            }
            Ok(Value::Boolean(can))
        }
        _ => {
            Err(RuntimeError::new(format!("'Отражение.удалить' ожидает объект, получено '{}'", obj.type_name()), span))
        }
    }
}

fn reflect_has(interp: &mut Interpreter, obj: Value, key: &str, span: Span) -> Result<Value, RuntimeError> {
    match &obj {
        Value::Proxy { target, handler } => {
            let (t, h) = ((**target).clone(), (**handler).clone());
            Ok(Value::Boolean(interp.proxy_has(&t, &h, key, span)?))
        }
        Value::Object(map) => Ok(Value::Boolean(map.borrow().contains_key(key))),
        Value::Array(arr) => {
            if key == "length" || key == "длина" {
                return Ok(Value::Boolean(true));
            }
            if let Ok(idx) = key.parse::<usize>() {
                return Ok(Value::Boolean(idx < arr.borrow().len()));
            }
            Ok(Value::Boolean(false))
        }
        _ => Err(RuntimeError::new(
            format!("'Отражение.есть' ожидает объект или массив, получено '{}'", obj.type_name()),
            span,
        )),
    }
}

fn reflect_get_prototype_of(interp: &mut Interpreter, obj: Value, span: Span) -> Result<Value, RuntimeError> {
    match &obj {
        Value::Proxy { target, handler } => {
            let (t, h) = ((**target).clone(), (**handler).clone());
            interp.proxy_get_prototype_of(&t, &h, span)
        }
        Value::Object(map) => {
            if let Some(proto) = map.borrow().get(symbols::PROTO) {
                return Ok(proto.clone());
            }
            Ok(Value::Null)
        }
        Value::Null | Value::Undefined => Err(RuntimeError::new(
            format!("'Отражение.прототипОт' ожидает объект, получено '{}'", obj.type_name()),
            span,
        )),
        _ => Ok(Value::Null),
    }
}

fn reflect_set_prototype_of(
    interp: &mut Interpreter,
    obj: Value,
    proto: Value,
    span: Span,
) -> Result<Value, RuntimeError> {
    match (&obj, &proto) {
        (Value::Proxy { target, handler }, _) => {
            let (t, h) = ((**target).clone(), (**handler).clone());
            Ok(Value::Boolean(interp.proxy_set_prototype_of(&t, &h, proto, span)?))
        }
        (Value::Object(map), Value::Object(_) | Value::Class(_) | Value::Null) => {
            let current = map.borrow().get(symbols::PROTO).cloned().unwrap_or(Value::Null);
            let unchanged = same_value(&current, &proto);
            let allowed = {
                let guard = map.borrow();
                !guard.frozen && (unchanged || guard.extensible)
            };
            if allowed {
                map.borrow_mut().insert(symbols::PROTO.to_string(), proto);
            }
            Ok(Value::Boolean(allowed))
        }
        _ => Err(RuntimeError::new("'Отражение.назначитьПрототип' ожидает (объект, объект|класс|ноль)", span)),
    }
}

fn reflect_own_keys(interp: &mut Interpreter, obj: Value, span: Span) -> Result<Value, RuntimeError> {
    match &obj {
        Value::Proxy { target, handler } => {
            let (t, h) = ((**target).clone(), (**handler).clone());
            Ok(Value::array(interp.proxy_own_keys(&t, &h, span)?))
        }
        Value::Object(map) => {
            let keys: Vec<Value> = map
                .borrow()
                .keys()
                .filter(|k| !symbols::is_internal_key(k) && !k.starts_with('#'))
                .map(|k| Value::String(k.clone()))
                .collect();
            Ok(Value::array(keys))
        }
        Value::Array(arr) => {
            let mut keys: Vec<Value> = (0..arr.borrow().len()).map(|i| Value::String(i.to_string())).collect();
            keys.push(Value::String("length".to_string()));
            Ok(Value::array(keys))
        }
        _ => Err(RuntimeError::new(
            format!("'Отражение.собственныеКлючи' ожидает объект или массив, получено '{}'", obj.type_name()),
            span,
        )),
    }
}

fn reflect_define_property(
    interp: &mut Interpreter,
    obj: Value,
    key: &str,
    descriptor: Value,
    span: Span,
) -> Result<Value, RuntimeError> {
    match &obj {
        Value::Proxy { target, handler } => {
            let (t, h) = ((**target).clone(), (**handler).clone());
            Ok(Value::Boolean(interp.proxy_define_property(&t, &h, key, descriptor, span)?))
        }
        Value::Object(map) => {
            crate::stdlib::object::define_property_impl(map, key, &descriptor, "Отражение.определитьСвойство", span)?;
            Ok(Value::Boolean(true))
        }
        _ => Err(RuntimeError::new(
            format!("'Отражение.определитьСвойство' ожидает объект, получено '{}'", obj.type_name()),
            span,
        )),
    }
}

fn reflect_get_own_property_descriptor(
    interp: &mut Interpreter,
    obj: Value,
    key: &str,
    span: Span,
) -> Result<Value, RuntimeError> {
    match &obj {
        Value::Proxy { target, handler } => {
            let (t, h) = ((**target).clone(), (**handler).clone());
            interp.proxy_get_own_property_descriptor(&t, &h, key, span)
        }
        Value::Object(map) if !symbols::is_internal_key(key) => {
            Ok(crate::stdlib::object::describe_property_impl(&map.borrow(), key).unwrap_or(Value::Undefined))
        }
        Value::Object(_) => Ok(Value::Undefined),
        _ => Err(RuntimeError::new(
            format!("'Отражение.описатьСвойство' ожидает объект, получено '{}'", obj.type_name()),
            span,
        )),
    }
}

fn reflect_is_extensible(interp: &mut Interpreter, obj: Value, span: Span) -> Result<Value, RuntimeError> {
    match &obj {
        Value::Proxy { target, handler } => {
            let (t, h) = ((**target).clone(), (**handler).clone());
            Ok(Value::Boolean(interp.proxy_is_extensible(&t, &h, span)?))
        }
        Value::Object(map) => Ok(Value::Boolean(map.borrow().extensible)),
        _ => Err(RuntimeError::new(
            format!("'Отражение.расширяем' ожидает объект, получено '{}'", obj.type_name()),
            span,
        )),
    }
}

fn reflect_prevent_extensions(interp: &mut Interpreter, obj: Value, span: Span) -> Result<Value, RuntimeError> {
    match &obj {
        Value::Proxy { target, handler } => {
            let (t, h) = ((**target).clone(), (**handler).clone());
            Ok(Value::Boolean(interp.proxy_prevent_extensions(&t, &h, span)?))
        }
        Value::Object(map) => {
            map.borrow_mut().prevent_extensions();
            Ok(Value::Boolean(true))
        }
        _ => Err(RuntimeError::new(
            format!("'Отражение.запретитьРасширение' ожидает объект, получено '{}'", obj.type_name()),
            span,
        )),
    }
}
