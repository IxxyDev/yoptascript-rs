use yps_lexer::Span;

use crate::error::RuntimeError;
use crate::interpreter::Interpreter;
use crate::stdlib::{builtin, object_of, require_args};
use crate::value::Value;

pub fn build_static() -> Value {
    object_of(&[("отПар", builtin("Карта.отПар"))])
}

pub fn construct(args: Vec<Value>, span: Span) -> Result<Value, RuntimeError> {
    if args.is_empty() {
        return Ok(Value::Map(Vec::new()));
    }
    match &args[0] {
        Value::Array(entries) => entries_to_map(entries, span),
        Value::Map(m) => Ok(Value::Map(m.clone())),
        Value::Undefined | Value::Null => Ok(Value::Map(Vec::new())),
        other => Err(RuntimeError::new(
            format!("'Карта' ожидает массив пар или карту, получено '{}'", other.type_name()),
            span,
        )),
    }
}

pub fn call_static(
    _interp: &mut Interpreter,
    method: &str,
    args: Vec<Value>,
    span: Span,
) -> Result<Value, RuntimeError> {
    match method {
        "отПар" => {
            require_args(&args, 1, span, "Карта.отПар")?;
            match &args[0] {
                Value::Array(entries) => entries_to_map(entries, span),
                Value::Map(m) => Ok(Value::Map(m.clone())),
                other => Err(RuntimeError::new(
                    format!("'Карта.отПар' ожидает массив пар, получено '{}'", other.type_name()),
                    span,
                )),
            }
        }
        _ => Err(RuntimeError::new(format!("У 'Карта' нет метода '{method}'"), span)),
    }
}

pub fn call(
    interp: &mut Interpreter,
    receiver: Value,
    method: &str,
    args: Vec<Value>,
    span: Span,
) -> Result<(Value, Option<Value>), RuntimeError> {
    let entries = match receiver {
        Value::Map(m) => m,
        _ => unreachable!(),
    };
    match method {
        "set" | "поставить" => {
            require_args(&args, 2, span, "set")?;
            let mut entries = entries;
            let mut iter = args.into_iter();
            let key = iter.next().unwrap();
            let val = iter.next().unwrap();
            if let Some(idx) = find_index(&entries, &key) {
                entries[idx].1 = val;
            } else {
                entries.push((key, val));
            }
            Ok((Value::Map(entries.clone()), Some(Value::Map(entries))))
        }
        "get" | "взять" => {
            require_args(&args, 1, span, "get")?;
            let key = &args[0];
            let val = find_index(&entries, key).map(|i| entries[i].1.clone()).unwrap_or(Value::Undefined);
            Ok((val, None))
        }
        "has" | "имеет" => {
            require_args(&args, 1, span, "has")?;
            Ok((Value::Boolean(find_index(&entries, &args[0]).is_some()), None))
        }
        "delete" | "удалить" => {
            require_args(&args, 1, span, "delete")?;
            let mut entries = entries;
            let removed = if let Some(idx) = find_index(&entries, &args[0]) {
                entries.remove(idx);
                true
            } else {
                false
            };
            Ok((Value::Boolean(removed), Some(Value::Map(entries))))
        }
        "clear" | "очистить" => Ok((Value::Undefined, Some(Value::Map(Vec::new())))),
        "size" | "размер" => Ok((Value::Number(entries.len() as f64), None)),
        "keys" | "ключи" => {
            let keys: Vec<Value> = entries.into_iter().map(|(k, _)| k).collect();
            Ok((Value::Array(keys), None))
        }
        "values" | "значения" => {
            let vals: Vec<Value> = entries.into_iter().map(|(_, v)| v).collect();
            Ok((Value::Array(vals), None))
        }
        "entries" | "записи" => {
            let pairs: Vec<Value> = entries.into_iter().map(|(k, v)| Value::Array(vec![k, v])).collect();
            Ok((Value::Array(pairs), None))
        }
        "forEach" | "каждый" => {
            require_args(&args, 1, span, "forEach")?;
            let callback = args.into_iter().next().unwrap();
            for (k, v) in &entries {
                interp.call_function(callback.clone(), vec![v.clone(), k.clone()], span)?;
            }
            Ok((Value::Undefined, Some(Value::Map(entries))))
        }
        _ => Err(RuntimeError::new(format!("У карты нет метода '{method}'"), span)),
    }
}

fn find_index(entries: &[(Value, Value)], key: &Value) -> Option<usize> {
    entries.iter().position(|(k, _)| k == key)
}

fn entries_to_map(entries: &[Value], span: Span) -> Result<Value, RuntimeError> {
    let mut out: Vec<(Value, Value)> = Vec::with_capacity(entries.len());
    for entry in entries {
        match entry {
            Value::Array(pair) if pair.len() >= 2 => {
                let key = pair[0].clone();
                let val = pair[1].clone();
                if let Some(idx) = find_index(&out, &key) {
                    out[idx].1 = val;
                } else {
                    out.push((key, val));
                }
            }
            _ => return Err(RuntimeError::new("Каждая запись Карты должна быть [ключ, значение]", span)),
        }
    }
    Ok(Value::Map(out))
}
