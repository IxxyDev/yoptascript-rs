use yps_lexer::Span;

use crate::error::RuntimeError;
use crate::interpreter::Interpreter;
use crate::stdlib::require_args;
use crate::value::Value;

pub fn construct(args: Vec<Value>, span: Span) -> Result<Value, RuntimeError> {
    if args.is_empty() {
        return Ok(Value::Set(Vec::new()));
    }
    match &args[0] {
        Value::Array(items) => {
            let mut out: Vec<Value> = Vec::with_capacity(items.len());
            for v in items {
                if !out.contains(v) {
                    out.push(v.clone());
                }
            }
            Ok(Value::Set(out))
        }
        Value::Set(s) => Ok(Value::Set(s.clone())),
        Value::Undefined | Value::Null => Ok(Value::Set(Vec::new())),
        other => {
            Err(RuntimeError::new(format!("'Набор' ожидает массив или набор, получено '{}'", other.type_name()), span))
        }
    }
}

pub fn call(
    interp: &mut Interpreter,
    receiver: Value,
    method: &str,
    args: Vec<Value>,
    span: Span,
) -> Result<(Value, Option<Value>), RuntimeError> {
    let items = match receiver {
        Value::Set(s) => s,
        _ => unreachable!(),
    };
    match method {
        "add" | "добавить" => {
            require_args(&args, 1, span, "add")?;
            let mut items = items;
            let val = args.into_iter().next().unwrap();
            if !items.contains(&val) {
                items.push(val);
            }
            Ok((Value::Set(items.clone()), Some(Value::Set(items))))
        }
        "has" | "имеет" => {
            require_args(&args, 1, span, "has")?;
            Ok((Value::Boolean(items.contains(&args[0])), None))
        }
        "delete" | "удалить" => {
            require_args(&args, 1, span, "delete")?;
            let mut items = items;
            let removed = if let Some(idx) = items.iter().position(|v| v == &args[0]) {
                items.remove(idx);
                true
            } else {
                false
            };
            Ok((Value::Boolean(removed), Some(Value::Set(items))))
        }
        "clear" | "очистить" => Ok((Value::Undefined, Some(Value::Set(Vec::new())))),
        "size" | "размер" => Ok((Value::Number(items.len() as f64), None)),
        "values" | "значения" => Ok((Value::Array(items), None)),
        "forEach" | "каждый" => {
            require_args(&args, 1, span, "forEach")?;
            let callback = args.into_iter().next().unwrap();
            for v in &items {
                interp.call_function(callback.clone(), vec![v.clone()], span)?;
            }
            Ok((Value::Undefined, Some(Value::Set(items))))
        }
        "union" | "объединение" => set_op(items, args, span, |a, b| a || b),
        "intersection" | "пересечение" => set_op(items, args, span, |a, b| a && b),
        "difference" | "разница" => {
            require_args(&args, 1, span, "difference")?;
            let other = extract_set_like(&args[0], span)?;
            let result: Vec<Value> = items.into_iter().filter(|v| !other.contains(v)).collect();
            Ok((Value::Set(result), None))
        }
        "symmetricDifference" | "симметричнаяРазница" => {
            require_args(&args, 1, span, "symmetricDifference")?;
            let other = extract_set_like(&args[0], span)?;
            let mut result: Vec<Value> = items.iter().filter(|v| !other.contains(v)).cloned().collect();
            for v in &other {
                if !items.contains(v) && !result.contains(v) {
                    result.push(v.clone());
                }
            }
            Ok((Value::Set(result), None))
        }
        "isSubsetOf" | "подмножествоОт" => {
            require_args(&args, 1, span, "isSubsetOf")?;
            let other = extract_set_like(&args[0], span)?;
            Ok((Value::Boolean(items.iter().all(|v| other.contains(v))), None))
        }
        "isSupersetOf" | "надмножествоОт" => {
            require_args(&args, 1, span, "isSupersetOf")?;
            let other = extract_set_like(&args[0], span)?;
            Ok((Value::Boolean(other.iter().all(|v| items.contains(v))), None))
        }
        "isDisjointFrom" | "непересекаетсяС" => {
            require_args(&args, 1, span, "isDisjointFrom")?;
            let other = extract_set_like(&args[0], span)?;
            Ok((Value::Boolean(!items.iter().any(|v| other.contains(v))), None))
        }
        _ => Err(RuntimeError::new(format!("У набора нет метода '{method}'"), span)),
    }
}

fn set_op<F>(items: Vec<Value>, args: Vec<Value>, span: Span, keep: F) -> Result<(Value, Option<Value>), RuntimeError>
where
    F: Fn(bool, bool) -> bool,
{
    require_args(&args, 1, span, "set операция")?;
    let other = extract_set_like(&args[0], span)?;
    let mut result: Vec<Value> = Vec::new();
    for v in &items {
        if keep(true, other.contains(v)) {
            result.push(v.clone());
        }
    }
    for v in &other {
        if !items.contains(v) && keep(false, true) && !result.contains(v) {
            result.push(v.clone());
        }
    }
    Ok((Value::Set(result), None))
}

fn extract_set_like(v: &Value, span: Span) -> Result<Vec<Value>, RuntimeError> {
    match v {
        Value::Set(s) => Ok(s.clone()),
        Value::Array(a) => {
            let mut out = Vec::with_capacity(a.len());
            for el in a {
                if !out.contains(el) {
                    out.push(el.clone());
                }
            }
            Ok(out)
        }
        other => Err(RuntimeError::new(format!("Ожидался набор или массив, получен '{}'", other.type_name()), span)),
    }
}
