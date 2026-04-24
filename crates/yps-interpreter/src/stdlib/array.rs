use yps_lexer::Span;

use crate::error::RuntimeError;
use crate::interpreter::Interpreter;
use crate::stdlib::{as_number, builtin, object_of, require_args};
use crate::value::Value;

pub fn build_object() -> Value {
    object_of(&[
        ("являетсяПомойкой", builtin("Помойка.являетсяПомойкой")),
        ("из", builtin("Помойка.из")),
        ("нового", builtin("Помойка.нового")),
    ])
}

pub fn call_static(
    _interp: &mut Interpreter,
    method: &str,
    args: Vec<Value>,
    span: Span,
) -> Result<Value, RuntimeError> {
    match method {
        "являетсяПомойкой" => {
            require_args(&args, 1, span, "Помойка.являетсяПомойкой")?;
            Ok(Value::Boolean(matches!(&args[0], Value::Array(_))))
        }
        "из" => {
            require_args(&args, 1, span, "Помойка.из")?;
            match &args[0] {
                Value::Array(a) => Ok(Value::Array(a.clone())),
                Value::String(s) => Ok(Value::Array(s.chars().map(|c| Value::String(c.to_string())).collect())),
                other => Err(RuntimeError::new(format!("Помойка.из не поддерживает '{}'", other.type_name()), span)),
            }
        }
        "нового" => Ok(Value::Array(args)),
        _ => Err(RuntimeError::new(format!("У 'Помойка' нет метода '{method}'"), span)),
    }
}

pub fn call(
    interp: &mut Interpreter,
    receiver: Value,
    method: &str,
    args: Vec<Value>,
    span: Span,
) -> Result<(Value, Option<Value>), RuntimeError> {
    let arr = match receiver {
        Value::Array(a) => a,
        _ => unreachable!(),
    };
    match method {
        "push" | "добавить" | "втолкнуть" => {
            let mut new_arr = arr;
            for a in args {
                new_arr.push(a);
            }
            let len = new_arr.len() as f64;
            Ok((Value::Number(len), Some(Value::Array(new_arr))))
        }
        "pop" | "вытолкнуть" => {
            let mut new_arr = arr;
            let popped = new_arr.pop().unwrap_or(Value::Undefined);
            Ok((popped, Some(Value::Array(new_arr))))
        }
        "shift" | "снять" => {
            let mut new_arr = arr;
            if new_arr.is_empty() {
                Ok((Value::Undefined, Some(Value::Array(new_arr))))
            } else {
                let first = new_arr.remove(0);
                Ok((first, Some(Value::Array(new_arr))))
            }
        }
        "unshift" | "подсунуть" => {
            let mut new_arr = arr;
            for (i, a) in args.into_iter().enumerate() {
                new_arr.insert(i, a);
            }
            let len = new_arr.len() as f64;
            Ok((Value::Number(len), Some(Value::Array(new_arr))))
        }
        "slice" | "отрезать" => {
            let len = arr.len() as isize;
            let start =
                if args.is_empty() { 0 } else { normalize_index(as_number(&args[0], span, "slice")? as isize, len) };
            let end =
                if args.len() < 2 { len } else { normalize_index(as_number(&args[1], span, "slice")? as isize, len) };
            let s = start.min(len).max(0) as usize;
            let e = end.min(len).max(0) as usize;
            let out = if s < e { arr[s..e].to_vec() } else { Vec::new() };
            Ok((Value::Array(out), None))
        }
        "indexOf" | "найтиИндекс" => {
            require_args(&args, 1, span, "indexOf")?;
            let target = &args[0];
            let start = if args.len() > 1 { as_number(&args[1], span, "indexOf")? as usize } else { 0 };
            let idx = arr.iter().enumerate().skip(start).find(|(_, v)| values_equal(v, target)).map(|(i, _)| i);
            Ok((Value::Number(idx.map(|i| i as f64).unwrap_or(-1.0)), None))
        }
        "includes" | "включает" => {
            require_args(&args, 1, span, "includes")?;
            let target = &args[0];
            let found = arr.iter().any(|v| values_equal(v, target));
            Ok((Value::Boolean(found), None))
        }
        "join" | "склеить" => {
            let sep = if args.is_empty() {
                ",".to_string()
            } else {
                match &args[0] {
                    Value::Undefined => ",".to_string(),
                    v => v.to_string(),
                }
            };
            let parts: Vec<String> = arr
                .iter()
                .map(|v| match v {
                    Value::Null | Value::Undefined => String::new(),
                    other => other.to_string(),
                })
                .collect();
            Ok((Value::String(parts.join(&sep)), None))
        }
        "reverse" | "перевернуть" => {
            let mut new_arr = arr;
            new_arr.reverse();
            Ok((Value::Array(new_arr.clone()), Some(Value::Array(new_arr))))
        }
        "concat" | "склеитьМассивы" => {
            let mut new_arr = arr;
            for a in args {
                match a {
                    Value::Array(inner) => new_arr.extend(inner),
                    other => new_arr.push(other),
                }
            }
            Ok((Value::Array(new_arr), None))
        }
        "sort" | "сортировать" => {
            let mut new_arr = arr;
            if args.is_empty() {
                new_arr.sort_by_key(|a| a.to_string());
                Ok((Value::Array(new_arr.clone()), Some(Value::Array(new_arr))))
            } else {
                let cmp = args.into_iter().next().unwrap();
                let mut err: Option<RuntimeError> = None;
                new_arr.sort_by(|a, b| {
                    if err.is_some() {
                        return std::cmp::Ordering::Equal;
                    }
                    match interp.call_function(cmp.clone(), vec![a.clone(), b.clone()], span) {
                        Ok(Value::Number(n)) if n < 0.0 => std::cmp::Ordering::Less,
                        Ok(Value::Number(n)) if n > 0.0 => std::cmp::Ordering::Greater,
                        Ok(_) => std::cmp::Ordering::Equal,
                        Err(e) => {
                            err = Some(e);
                            std::cmp::Ordering::Equal
                        }
                    }
                });
                if let Some(e) = err {
                    return Err(e);
                }
                Ok((Value::Array(new_arr.clone()), Some(Value::Array(new_arr))))
            }
        }
        "map" | "преобразовать" => {
            require_args(&args, 1, span, "map")?;
            let callback = args.into_iter().next().unwrap();
            let mut result = Vec::with_capacity(arr.len());
            for (i, el) in arr.into_iter().enumerate() {
                let v = interp.call_function(callback.clone(), vec![el, Value::Number(i as f64)], span)?;
                result.push(v);
            }
            Ok((Value::Array(result), None))
        }
        "filter" | "отфильтровать" => {
            require_args(&args, 1, span, "filter")?;
            let callback = args.into_iter().next().unwrap();
            let mut result = Vec::new();
            for (i, el) in arr.into_iter().enumerate() {
                let keep = interp.call_function(callback.clone(), vec![el.clone(), Value::Number(i as f64)], span)?;
                if keep.is_truthy() {
                    result.push(el);
                }
            }
            Ok((Value::Array(result), None))
        }
        "reduce" | "свернуть" => {
            require_args(&args, 1, span, "reduce")?;
            let mut iter = args.into_iter();
            let callback = iter.next().unwrap();
            let initial = iter.next();
            let mut acc = match initial {
                Some(v) => v,
                None => {
                    if arr.is_empty() {
                        return Err(RuntimeError::new("reduce пустого массива без начального значения", span));
                    }
                    let mut it = arr.into_iter();
                    let first = it.next().unwrap();
                    let mut acc = first;
                    for (i, el) in it.enumerate() {
                        acc = interp.call_function(
                            callback.clone(),
                            vec![acc, el, Value::Number((i + 1) as f64)],
                            span,
                        )?;
                    }
                    return Ok((acc, None));
                }
            };
            for (i, el) in arr.into_iter().enumerate() {
                acc = interp.call_function(callback.clone(), vec![acc, el, Value::Number(i as f64)], span)?;
            }
            Ok((acc, None))
        }
        "forEach" | "каждый" => {
            require_args(&args, 1, span, "forEach")?;
            let callback = args.into_iter().next().unwrap();
            for (i, el) in arr.into_iter().enumerate() {
                interp.call_function(callback.clone(), vec![el, Value::Number(i as f64)], span)?;
            }
            Ok((Value::Undefined, None))
        }
        "find" | "найти" => {
            require_args(&args, 1, span, "find")?;
            let callback = args.into_iter().next().unwrap();
            for (i, el) in arr.into_iter().enumerate() {
                let matched =
                    interp.call_function(callback.clone(), vec![el.clone(), Value::Number(i as f64)], span)?;
                if matched.is_truthy() {
                    return Ok((el, None));
                }
            }
            Ok((Value::Undefined, None))
        }
        "findIndex" | "найтиИндексПо" => {
            require_args(&args, 1, span, "findIndex")?;
            let callback = args.into_iter().next().unwrap();
            for (i, el) in arr.into_iter().enumerate() {
                let matched = interp.call_function(callback.clone(), vec![el, Value::Number(i as f64)], span)?;
                if matched.is_truthy() {
                    return Ok((Value::Number(i as f64), None));
                }
            }
            Ok((Value::Number(-1.0), None))
        }
        "some" | "некоторые" => {
            require_args(&args, 1, span, "some")?;
            let callback = args.into_iter().next().unwrap();
            for (i, el) in arr.into_iter().enumerate() {
                let matched = interp.call_function(callback.clone(), vec![el, Value::Number(i as f64)], span)?;
                if matched.is_truthy() {
                    return Ok((Value::Boolean(true), None));
                }
            }
            Ok((Value::Boolean(false), None))
        }
        "every" | "все" => {
            require_args(&args, 1, span, "every")?;
            let callback = args.into_iter().next().unwrap();
            for (i, el) in arr.into_iter().enumerate() {
                let matched = interp.call_function(callback.clone(), vec![el, Value::Number(i as f64)], span)?;
                if !matched.is_truthy() {
                    return Ok((Value::Boolean(false), None));
                }
            }
            Ok((Value::Boolean(true), None))
        }
        "at" | "поИндексу" => {
            require_args(&args, 1, span, "at")?;
            let idx = as_number(&args[0], span, "at")? as isize;
            let len = arr.len() as isize;
            let real = if idx < 0 { len + idx } else { idx };
            if real < 0 || real >= len { Ok((Value::Undefined, None)) } else { Ok((arr[real as usize].clone(), None)) }
        }
        "flat" | "плоский" => {
            let depth = if args.is_empty() { 1.0 } else { as_number(&args[0], span, "flat")? };
            Ok((Value::Array(flatten(arr, depth as isize)), None))
        }
        "flatMap" | "плоскоПреобразовать" => {
            require_args(&args, 1, span, "flatMap")?;
            let callback = args.into_iter().next().unwrap();
            let mut result = Vec::new();
            for (i, el) in arr.into_iter().enumerate() {
                let v = interp.call_function(callback.clone(), vec![el, Value::Number(i as f64)], span)?;
                match v {
                    Value::Array(inner) => result.extend(inner),
                    other => result.push(other),
                }
            }
            Ok((Value::Array(result), None))
        }
        "findLast" | "найтиПоследний" => {
            require_args(&args, 1, span, "findLast")?;
            let callback = args.into_iter().next().unwrap();
            for i in (0..arr.len()).rev() {
                let el = arr[i].clone();
                let matched =
                    interp.call_function(callback.clone(), vec![el.clone(), Value::Number(i as f64)], span)?;
                if matched.is_truthy() {
                    return Ok((el, None));
                }
            }
            Ok((Value::Undefined, None))
        }
        "findLastIndex" | "найтиПоследнийИндекс" => {
            require_args(&args, 1, span, "findLastIndex")?;
            let callback = args.into_iter().next().unwrap();
            for i in (0..arr.len()).rev() {
                let el = arr[i].clone();
                let matched = interp.call_function(callback.clone(), vec![el, Value::Number(i as f64)], span)?;
                if matched.is_truthy() {
                    return Ok((Value::Number(i as f64), None));
                }
            }
            Ok((Value::Number(-1.0), None))
        }
        "toReversed" | "перевёрнутый" => {
            let mut new_arr = arr;
            new_arr.reverse();
            Ok((Value::Array(new_arr), None))
        }
        "toSorted" | "отсортированный" => {
            let mut new_arr = arr;
            if args.is_empty() {
                new_arr.sort_by_key(|a| a.to_string());
                Ok((Value::Array(new_arr), None))
            } else {
                let cmp = args.into_iter().next().unwrap();
                let mut err: Option<RuntimeError> = None;
                new_arr.sort_by(|a, b| {
                    if err.is_some() {
                        return std::cmp::Ordering::Equal;
                    }
                    match interp.call_function(cmp.clone(), vec![a.clone(), b.clone()], span) {
                        Ok(Value::Number(n)) if n < 0.0 => std::cmp::Ordering::Less,
                        Ok(Value::Number(n)) if n > 0.0 => std::cmp::Ordering::Greater,
                        Ok(_) => std::cmp::Ordering::Equal,
                        Err(e) => {
                            err = Some(e);
                            std::cmp::Ordering::Equal
                        }
                    }
                });
                if let Some(e) = err {
                    return Err(e);
                }
                Ok((Value::Array(new_arr), None))
            }
        }
        "with" | "сЗаменой" => {
            require_args(&args, 2, span, "with")?;
            let idx = as_number(&args[0], span, "with")? as isize;
            let len = arr.len() as isize;
            let real = if idx < 0 { len + idx } else { idx };
            if real < 0 || real >= len {
                return Err(RuntimeError::new(format!("Индекс {idx} вне диапазона"), span));
            }
            let mut new_arr = arr;
            new_arr[real as usize] = args.into_iter().nth(1).unwrap();
            Ok((Value::Array(new_arr), None))
        }
        _ => Err(RuntimeError::new(format!("У массива нет метода '{method}'"), span)),
    }
}

fn normalize_index(idx: isize, len: isize) -> isize {
    if idx < 0 { (len + idx).max(0) } else { idx }
}

fn values_equal(a: &Value, b: &Value) -> bool {
    a == b
}

fn flatten(arr: Vec<Value>, depth: isize) -> Vec<Value> {
    let mut result = Vec::new();
    for v in arr {
        match v {
            Value::Array(inner) if depth > 0 => {
                result.extend(flatten(inner, depth - 1));
            }
            other => result.push(other),
        }
    }
    result
}
