use yps_lexer::Span;

use crate::error::RuntimeError;
use crate::interpreter::Interpreter;
use crate::symbols;
use crate::value::Value;

use super::{builtin, object_of, require_args};

pub fn build_object() -> Value {
    object_of(&[
        ("получить", builtin("Отражение.получить")),
        ("есть", builtin("Отражение.есть")),
        ("прототипОт", builtin("Отражение.прототипОт")),
        ("собственныеКлючи", builtin("Отражение.собственныеКлючи")),
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
        "есть" => {
            require_args(&args, 2, span, "Отражение.есть")?;
            let obj = args[0].clone();
            let key = args[1].to_string();
            reflect_has(obj, &key, span)
        }
        "прототипОт" => {
            require_args(&args, 1, span, "Отражение.прототипОт")?;
            reflect_get_prototype_of(args[0].clone(), span)
        }
        "собственныеКлючи" => {
            require_args(&args, 1, span, "Отражение.собственныеКлючи")?;
            reflect_own_keys(args[0].clone(), span)
        }
        "применить" => {
            require_args(&args, 3, span, "Отражение.применить")?;
            let func = args[0].clone();
            let args_val = args[2].clone();
            let call_args = match args_val {
                Value::Array(a) => a.borrow().clone(),
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
                Value::Array(a) => a.borrow().clone(),
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

fn reflect_has(obj: Value, key: &str, span: Span) -> Result<Value, RuntimeError> {
    match obj {
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

fn reflect_get_prototype_of(obj: Value, span: Span) -> Result<Value, RuntimeError> {
    match obj {
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

fn reflect_own_keys(obj: Value, span: Span) -> Result<Value, RuntimeError> {
    match obj {
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
