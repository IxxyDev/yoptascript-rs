use std::cell::RefCell;
use std::rc::Rc;

use yps_lexer::Span;

use crate::error::RuntimeError;
use crate::interpreter::Interpreter;
use crate::stdlib::{as_number, builtin, object_of, require_args};
use crate::value::{ArrayStore, IteratorState, Value, same_value_zero};

pub fn build_object() -> Value {
    object_of(&[
        ("являетсяПомойкой", builtin("Помойка.являетсяПомойкой")),
        ("извне", builtin("Помойка.извне")),
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
        "извне" => {
            require_args(&args, 1, span, "Помойка.извне")?;
            match &args[0] {
                Value::Array(a) => Ok(Value::array(a.borrow().0.clone())),
                Value::String(s) => Ok(Value::array(s.chars().map(|c| Value::String(c.to_string())).collect())),
                other => Err(RuntimeError::new(format!("Помойка.извне не поддерживает '{}'", other.type_name()), span)),
            }
        }
        "нового" => Ok(Value::array(args)),
        _ => Err(RuntimeError::new(format!("У 'Помойка' нет метода '{method}'"), span)),
    }
}

pub fn call(
    interp: &mut Interpreter,
    receiver: Value,
    method: &str,
    args: Vec<Value>,
    span: Span,
) -> Result<Value, RuntimeError> {
    let rc = match receiver {
        Value::Array(a) => a,
        _ => unreachable!(),
    };
    match method {
        "push" | "добавить" | "втолкнуть" => {
            let mut guard = rc.borrow_mut();
            for a in args {
                guard.push(a);
            }
            let len = guard.len() as f64;
            Ok(Value::Number(len))
        }
        "pop" | "вытолкнуть" => {
            let popped = rc.borrow_mut().pop().unwrap_or(Value::Undefined);
            Ok(popped)
        }
        "shift" | "снять" => {
            let mut guard = rc.borrow_mut();
            if guard.is_empty() { Ok(Value::Undefined) } else { Ok(guard.remove(0)) }
        }
        "unshift" | "подсунуть" => {
            let mut guard = rc.borrow_mut();
            for (i, a) in args.into_iter().enumerate() {
                guard.insert(i, a);
            }
            Ok(Value::Number(guard.len() as f64))
        }
        "slice" | "отрезать" => {
            let snapshot = rc.borrow().clone();
            let len = snapshot.len() as isize;
            let start =
                if args.is_empty() { 0 } else { normalize_index(as_number(&args[0], span, "slice")? as isize, len) };
            let end =
                if args.len() < 2 { len } else { normalize_index(as_number(&args[1], span, "slice")? as isize, len) };
            let s = start.min(len).max(0) as usize;
            let e = end.min(len).max(0) as usize;
            let out = if s < e { snapshot[s..e].to_vec() } else { Vec::new() };
            Ok(Value::array(out))
        }
        "indexOf" | "найтиИндекс" => {
            require_args(&args, 1, span, "indexOf")?;
            let target = &args[0];
            let snapshot = rc.borrow().clone();
            let len = snapshot.len() as isize;
            let start = if args.len() > 1 {
                let raw = as_number(&args[1], span, "indexOf")? as isize;
                if raw < 0 { (len + raw).max(0) } else { raw }
            } else {
                0
            } as usize;
            let idx = snapshot.iter().enumerate().skip(start).find(|(_, v)| *v == target).map(|(i, _)| i);
            Ok(Value::Number(idx.map(|i| i as f64).unwrap_or(-1.0)))
        }
        "lastIndexOf" | "найтиПоследнийПо" => {
            require_args(&args, 1, span, "lastIndexOf")?;
            let target = &args[0];
            let snapshot = rc.borrow().clone();
            let len = snapshot.len() as isize;
            let start = if args.len() > 1 {
                let raw = as_number(&args[1], span, "lastIndexOf")? as isize;
                if raw < 0 { len + raw } else { raw.min(len - 1) }
            } else {
                len - 1
            };
            let mut idx = -1.0;
            let mut i = start;
            while i >= 0 {
                if &snapshot[i as usize] == target {
                    idx = i as f64;
                    break;
                }
                i -= 1;
            }
            Ok(Value::Number(idx))
        }
        "includes" | "включает" => {
            require_args(&args, 1, span, "includes")?;
            let target = &args[0];
            let snapshot = rc.borrow().clone();
            let len = snapshot.len() as isize;
            let start = if args.len() > 1 {
                let raw = as_number(&args[1], span, "includes")? as isize;
                if raw < 0 { (len + raw).max(0) } else { raw }
            } else {
                0
            } as usize;
            let found = snapshot.iter().skip(start).any(|v| same_value_zero(v, target));
            Ok(Value::Boolean(found))
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
            let snapshot = rc.borrow().clone();
            let parts: Vec<String> = snapshot.iter().map(|v| join_element(v, &sep)).collect();
            Ok(Value::String(parts.join(&sep)))
        }
        "reverse" | "перевернуть" => {
            rc.borrow_mut().reverse();
            Ok(Value::Array(rc))
        }
        "concat" | "склеитьМассивы" => {
            let mut new_arr = rc.borrow().0.clone();
            for a in args {
                match a {
                    Value::Array(inner) => new_arr.extend(inner.borrow().iter().cloned()),
                    other => new_arr.push(other),
                }
            }
            Ok(Value::array(new_arr))
        }
        "sort" | "сортировать" => {
            let mut snapshot = rc.borrow().0.clone();
            sort_snapshot(interp, &mut snapshot, args, span)?;
            *rc.borrow_mut() = ArrayStore(snapshot);
            Ok(Value::Array(rc))
        }
        "map" | "преобразовать" => {
            require_args(&args, 1, span, "map")?;
            let callback = args.into_iter().next().unwrap();
            let snapshot = rc.borrow().0.clone();
            let mut result = Vec::with_capacity(snapshot.len());
            for (i, el) in snapshot.into_iter().enumerate() {
                let v = interp.call_function(
                    callback.clone(),
                    vec![el, Value::Number(i as f64), Value::Array(rc.clone())],
                    span,
                )?;
                result.push(v);
            }
            Ok(Value::array(result))
        }
        "filter" | "отфильтровать" => {
            require_args(&args, 1, span, "filter")?;
            let callback = args.into_iter().next().unwrap();
            let snapshot = rc.borrow().0.clone();
            let mut result = Vec::new();
            for (i, el) in snapshot.into_iter().enumerate() {
                let keep = interp.call_function(
                    callback.clone(),
                    vec![el.clone(), Value::Number(i as f64), Value::Array(rc.clone())],
                    span,
                )?;
                if keep.is_truthy() {
                    result.push(el);
                }
            }
            Ok(Value::array(result))
        }
        "reduce" | "свернуть" => {
            require_args(&args, 1, span, "reduce")?;
            let mut iter = args.into_iter();
            let callback = iter.next().unwrap();
            let initial = iter.next();
            let snapshot = rc.borrow().0.clone();
            let mut acc = match initial {
                Some(v) => v,
                None => {
                    if snapshot.is_empty() {
                        return Err(RuntimeError::new("reduce пустого массива без начального значения", span));
                    }
                    let mut it = snapshot.into_iter();
                    let first = it.next().unwrap();
                    let mut acc = first;
                    for (i, el) in it.enumerate() {
                        acc = interp.call_function(
                            callback.clone(),
                            vec![acc, el, Value::Number((i + 1) as f64), Value::Array(rc.clone())],
                            span,
                        )?;
                    }
                    return Ok(acc);
                }
            };
            for (i, el) in snapshot.into_iter().enumerate() {
                acc = interp.call_function(
                    callback.clone(),
                    vec![acc, el, Value::Number(i as f64), Value::Array(rc.clone())],
                    span,
                )?;
            }
            Ok(acc)
        }
        "reduceRight" | "свернутьСправа" => {
            require_args(&args, 1, span, "reduceRight")?;
            let mut iter = args.into_iter();
            let callback = iter.next().unwrap();
            let initial = iter.next();
            let snapshot = rc.borrow().0.clone();
            let len = snapshot.len();
            match initial {
                Some(v) => {
                    let mut acc = v;
                    for i in (0..len).rev() {
                        acc = interp.call_function(
                            callback.clone(),
                            vec![acc, snapshot[i].clone(), Value::Number(i as f64), Value::Array(rc.clone())],
                            span,
                        )?;
                    }
                    Ok(acc)
                }
                None => {
                    if snapshot.is_empty() {
                        return Err(RuntimeError::new("reduceRight пустого массива без начального значения", span));
                    }
                    let mut acc = snapshot[len - 1].clone();
                    for i in (0..len - 1).rev() {
                        acc = interp.call_function(
                            callback.clone(),
                            vec![acc, snapshot[i].clone(), Value::Number(i as f64), Value::Array(rc.clone())],
                            span,
                        )?;
                    }
                    Ok(acc)
                }
            }
        }
        "forEach" | "каждый" => {
            require_args(&args, 1, span, "forEach")?;
            let callback = args.into_iter().next().unwrap();
            let snapshot = rc.borrow().0.clone();
            for (i, el) in snapshot.into_iter().enumerate() {
                interp.call_function(
                    callback.clone(),
                    vec![el, Value::Number(i as f64), Value::Array(rc.clone())],
                    span,
                )?;
            }
            Ok(Value::Undefined)
        }
        "find" | "найти" => {
            require_args(&args, 1, span, "find")?;
            let callback = args.into_iter().next().unwrap();
            let snapshot = rc.borrow().0.clone();
            for (i, el) in snapshot.into_iter().enumerate() {
                let matched = interp.call_function(
                    callback.clone(),
                    vec![el.clone(), Value::Number(i as f64), Value::Array(rc.clone())],
                    span,
                )?;
                if matched.is_truthy() {
                    return Ok(el);
                }
            }
            Ok(Value::Undefined)
        }
        "findIndex" | "найтиИндексПо" => {
            require_args(&args, 1, span, "findIndex")?;
            let callback = args.into_iter().next().unwrap();
            let snapshot = rc.borrow().0.clone();
            for (i, el) in snapshot.into_iter().enumerate() {
                let matched = interp.call_function(
                    callback.clone(),
                    vec![el, Value::Number(i as f64), Value::Array(rc.clone())],
                    span,
                )?;
                if matched.is_truthy() {
                    return Ok(Value::Number(i as f64));
                }
            }
            Ok(Value::Number(-1.0))
        }
        "some" | "некоторые" => {
            require_args(&args, 1, span, "some")?;
            let callback = args.into_iter().next().unwrap();
            let snapshot = rc.borrow().0.clone();
            for (i, el) in snapshot.into_iter().enumerate() {
                let matched = interp.call_function(
                    callback.clone(),
                    vec![el, Value::Number(i as f64), Value::Array(rc.clone())],
                    span,
                )?;
                if matched.is_truthy() {
                    return Ok(Value::Boolean(true));
                }
            }
            Ok(Value::Boolean(false))
        }
        "every" | "все" => {
            require_args(&args, 1, span, "every")?;
            let callback = args.into_iter().next().unwrap();
            let snapshot = rc.borrow().0.clone();
            for (i, el) in snapshot.into_iter().enumerate() {
                let matched = interp.call_function(
                    callback.clone(),
                    vec![el, Value::Number(i as f64), Value::Array(rc.clone())],
                    span,
                )?;
                if !matched.is_truthy() {
                    return Ok(Value::Boolean(false));
                }
            }
            Ok(Value::Boolean(true))
        }
        "at" | "поИндексу" => {
            require_args(&args, 1, span, "at")?;
            let idx = as_number(&args[0], span, "at")? as isize;
            let guard = rc.borrow();
            let len = guard.len() as isize;
            let real = if idx < 0 { len + idx } else { idx };
            if real < 0 || real >= len { Ok(Value::Undefined) } else { Ok(guard[real as usize].clone()) }
        }
        "flat" | "плоский" => {
            let depth = if args.is_empty() { 1.0 } else { as_number(&args[0], span, "flat")? };
            let snapshot = rc.borrow().0.clone();
            Ok(Value::array(flatten(snapshot, depth as isize)))
        }
        "flatMap" | "плоскоПреобразовать" => {
            require_args(&args, 1, span, "flatMap")?;
            let callback = args.into_iter().next().unwrap();
            let snapshot = rc.borrow().0.clone();
            let mut result = Vec::new();
            for (i, el) in snapshot.into_iter().enumerate() {
                let v = interp.call_function(
                    callback.clone(),
                    vec![el, Value::Number(i as f64), Value::Array(rc.clone())],
                    span,
                )?;
                match v {
                    Value::Array(inner) => result.extend(inner.borrow().iter().cloned()),
                    other => result.push(other),
                }
            }
            Ok(Value::array(result))
        }
        "findLast" | "найтиПоследний" => {
            require_args(&args, 1, span, "findLast")?;
            let callback = args.into_iter().next().unwrap();
            let snapshot = rc.borrow().clone();
            for i in (0..snapshot.len()).rev() {
                let el = snapshot[i].clone();
                let matched = interp.call_function(
                    callback.clone(),
                    vec![el.clone(), Value::Number(i as f64), Value::Array(rc.clone())],
                    span,
                )?;
                if matched.is_truthy() {
                    return Ok(el);
                }
            }
            Ok(Value::Undefined)
        }
        "findLastIndex" | "найтиПоследнийИндекс" => {
            require_args(&args, 1, span, "findLastIndex")?;
            let callback = args.into_iter().next().unwrap();
            let snapshot = rc.borrow().clone();
            for i in (0..snapshot.len()).rev() {
                let el = snapshot[i].clone();
                let matched = interp.call_function(
                    callback.clone(),
                    vec![el, Value::Number(i as f64), Value::Array(rc.clone())],
                    span,
                )?;
                if matched.is_truthy() {
                    return Ok(Value::Number(i as f64));
                }
            }
            Ok(Value::Number(-1.0))
        }
        "toReversed" | "перевёрнутый" => {
            let mut new_arr = rc.borrow().0.clone();
            new_arr.reverse();
            Ok(Value::array(new_arr))
        }
        "toSorted" | "отсортированный" => {
            let mut new_arr = rc.borrow().0.clone();
            sort_snapshot(interp, &mut new_arr, args, span)?;
            Ok(Value::array(new_arr))
        }
        "splice" | "вырезать" => {
            let snapshot = rc.borrow().0.clone();
            let (new_arr, removed) = splice_impl(snapshot, &args, span)?;
            *rc.borrow_mut() = ArrayStore(new_arr);
            Ok(Value::array(removed))
        }
        "toSpliced" | "вырезанный" => {
            let snapshot = rc.borrow().0.clone();
            let (new_arr, _removed) = splice_impl(snapshot, &args, span)?;
            Ok(Value::array(new_arr))
        }
        "with" | "сЗаменой" => {
            require_args(&args, 2, span, "with")?;
            let idx = as_number(&args[0], span, "with")? as isize;
            let mut new_arr = rc.borrow().0.clone();
            let len = new_arr.len() as isize;
            let real = if idx < 0 { len + idx } else { idx };
            if real < 0 || real >= len {
                return Err(RuntimeError::new(format!("Индекс {idx} вне диапазона"), span));
            }
            new_arr[real as usize] = args.into_iter().nth(1).unwrap();
            Ok(Value::array(new_arr))
        }
        "fill" | "заполнить" => {
            let len = rc.borrow().len() as isize;
            let mut args = args.into_iter();
            let value = args.next().unwrap_or(Value::Undefined);
            let start = match args.next() {
                Some(v) => clamp_index(as_number(&v, span, "fill")? as isize, len),
                None => 0,
            };
            let end = match args.next() {
                Some(v) => clamp_index(as_number(&v, span, "fill")? as isize, len),
                None => len,
            };
            if start < end {
                let mut guard = rc.borrow_mut();
                for slot in &mut guard.0[start as usize..end as usize] {
                    *slot = value.clone();
                }
            }
            Ok(Value::Array(rc))
        }
        "copyWithin" | "копироватьВнутри" => {
            require_args(&args, 1, span, "copyWithin")?;
            let len = rc.borrow().len() as isize;
            let mut args = args.into_iter();
            let target = clamp_index(as_number(&args.next().unwrap(), span, "copyWithin")? as isize, len);
            let start = match args.next() {
                Some(v) => clamp_index(as_number(&v, span, "copyWithin")? as isize, len),
                None => 0,
            };
            let end = match args.next() {
                Some(v) => clamp_index(as_number(&v, span, "copyWithin")? as isize, len),
                None => len,
            };
            let count = (end - start).max(0).min(len - target);
            if count > 0 {
                let snapshot = rc.borrow().0.clone();
                let mut guard = rc.borrow_mut();
                for i in 0..count as usize {
                    guard.0[target as usize + i] = snapshot[start as usize + i].clone();
                }
            }
            Ok(Value::Array(rc))
        }
        "entries" | "записи" => {
            let entries: Vec<(Value, Value)> =
                rc.borrow().0.iter().enumerate().map(|(i, v)| (Value::Number(i as f64), v.clone())).collect();
            let state = IteratorState::MapEntries { entries, index: 0 };
            Ok(Value::Iterator(Rc::new(RefCell::new(state))))
        }
        "keys" | "ключи" => {
            let len = rc.borrow().len();
            let values: Vec<Value> = (0..len).map(|i| Value::Number(i as f64)).collect();
            let state = IteratorState::Array { values, index: 0 };
            Ok(Value::Iterator(Rc::new(RefCell::new(state))))
        }
        "values" | "значения" => {
            let values = rc.borrow().0.clone();
            let state = IteratorState::Array { values, index: 0 };
            Ok(Value::Iterator(Rc::new(RefCell::new(state))))
        }
        _ => Err(RuntimeError::new(format!("У массива нет метода '{method}'"), span)),
    }
}

pub fn method_exists(name: &str) -> bool {
    matches!(
        name,
        "push"
            | "добавить"
            | "втолкнуть"
            | "pop"
            | "вытолкнуть"
            | "shift"
            | "снять"
            | "unshift"
            | "подсунуть"
            | "slice"
            | "отрезать"
            | "indexOf"
            | "найтиИндекс"
            | "lastIndexOf"
            | "найтиПоследнийПо"
            | "includes"
            | "включает"
            | "join"
            | "склеить"
            | "reverse"
            | "перевернуть"
            | "concat"
            | "склеитьМассивы"
            | "sort"
            | "сортировать"
            | "map"
            | "преобразовать"
            | "filter"
            | "отфильтровать"
            | "reduce"
            | "свернуть"
            | "reduceRight"
            | "свернутьСправа"
            | "forEach"
            | "каждый"
            | "find"
            | "найти"
            | "findIndex"
            | "найтиИндексПо"
            | "some"
            | "некоторые"
            | "every"
            | "все"
            | "at"
            | "поИндексу"
            | "flat"
            | "плоский"
            | "flatMap"
            | "плоскоПреобразовать"
            | "findLast"
            | "найтиПоследний"
            | "findLastIndex"
            | "найтиПоследнийИндекс"
            | "toReversed"
            | "перевёрнутый"
            | "toSorted"
            | "отсортированный"
            | "splice"
            | "вырезать"
            | "toSpliced"
            | "вырезанный"
            | "with"
            | "сЗаменой"
            | "fill"
            | "заполнить"
            | "copyWithin"
            | "копироватьВнутри"
            | "entries"
            | "записи"
            | "keys"
            | "ключи"
            | "values"
            | "значения"
    )
}

fn sort_snapshot(
    interp: &mut Interpreter,
    arr: &mut [Value],
    args: Vec<Value>,
    span: Span,
) -> Result<(), RuntimeError> {
    if args.is_empty() {
        arr.sort_by_key(|a| a.to_string());
        return Ok(());
    }
    let cmp = args.into_iter().next().unwrap();
    let mut err: Option<RuntimeError> = None;
    arr.sort_by(|a, b| {
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
    Ok(())
}

fn normalize_index(idx: isize, len: isize) -> isize {
    if idx < 0 { (len + idx).max(0) } else { idx }
}

fn clamp_index(idx: isize, len: isize) -> isize {
    normalize_index(idx, len).min(len)
}

fn splice_impl(arr: Vec<Value>, args: &[Value], span: Span) -> Result<(Vec<Value>, Vec<Value>), RuntimeError> {
    let len = arr.len() as isize;
    let start_raw = if args.is_empty() { 0 } else { as_number(&args[0], span, "splice")? as isize };
    let start = if start_raw < 0 { (len + start_raw).max(0) } else { start_raw.min(len) } as usize;
    let delete_count = if args.len() < 2 {
        arr.len() - start
    } else {
        let n = as_number(&args[1], span, "splice")? as isize;
        n.max(0).min(len - start as isize) as usize
    };
    let inserts: Vec<Value> = if args.len() > 2 { args[2..].to_vec() } else { Vec::new() };
    let mut new_arr = arr;
    let removed: Vec<Value> = new_arr.splice(start..start + delete_count, inserts).collect();
    Ok((new_arr, removed))
}

fn flatten(arr: Vec<Value>, depth: isize) -> Vec<Value> {
    let mut result = Vec::new();
    for v in arr {
        match v {
            Value::Array(inner) if depth > 0 => {
                result.extend(flatten(inner.borrow().0.clone(), depth - 1));
            }
            other => result.push(other),
        }
    }
    result
}

fn join_element(v: &Value, sep: &str) -> String {
    match v {
        Value::Null | Value::Undefined => String::new(),
        Value::Array(inner) => {
            let parts: Vec<String> = inner.borrow().iter().map(|e| join_element(e, sep)).collect();
            parts.join(sep)
        }
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    fn eval(src: &str) -> crate::value::Value {
        let source = yps_lexer::SourceFile::new("test".to_string(), src.to_string());
        let (tokens, _) = yps_lexer::Lexer::new(&source).tokenize();
        let (program, _) = yps_parser::Parser::new(&tokens, &source).parse_program();
        crate::interpreter::Interpreter::new().run_repl(&program).unwrap().unwrap()
    }

    #[test]
    fn nan_includes_nan() {
        assert_eq!(eval("[нихуя].включает(нихуя);"), crate::value::Value::Boolean(true));
    }

    #[test]
    fn nan_index_of_nan_is_minus_one() {
        assert_eq!(eval("[нихуя].найтиИндекс(нихуя);"), crate::value::Value::Number(-1.0));
    }

    #[test]
    fn index_of_negative_from_index() {
        assert_eq!(eval("[1,2,3,2,1].indexOf(2, -3);"), crate::value::Value::Number(3.0));
    }

    #[test]
    fn includes_honors_from_index() {
        assert_eq!(eval("[1,2,3].includes(1, 2);"), crate::value::Value::Boolean(false));
        assert_eq!(eval("[1,2,3].includes(3, 2);"), crate::value::Value::Boolean(true));
    }

    #[test]
    fn last_index_of_basic_and_from_index() {
        assert_eq!(eval("[1,2,3,2,1].lastIndexOf(2);"), crate::value::Value::Number(3.0));
        assert_eq!(eval("[1,2,3,2,1].lastIndexOf(2, 2);"), crate::value::Value::Number(1.0));
        assert_eq!(eval("[1,2,3,2,1].lastIndexOf(2, -3);"), crate::value::Value::Number(1.0));
        assert_eq!(eval("[1,2,3].lastIndexOf(9);"), crate::value::Value::Number(-1.0));
    }

    #[test]
    fn reduce_right_with_and_without_init() {
        assert_eq!(eval("[1,2,3].reduceRight((а,б)=>а+\"-\"+б);"), crate::value::Value::String("3-2-1".to_string()));
        assert_eq!(eval("[1,2,3].reduceRight((а,б)=>а+б, 10);"), crate::value::Value::Number(16.0));
    }

    #[test]
    fn fill_full_range() {
        assert_eq!(eval("[1,2,3,4,5].fill(0).join(\",\");"), crate::value::Value::String("0,0,0,0,0".to_string()));
    }

    #[test]
    fn fill_start_end() {
        assert_eq!(
            eval("[1,2,3,4,5].fill(0, 1, 3).join(\",\");"),
            crate::value::Value::String("1,0,0,4,5".to_string())
        );
    }

    #[test]
    fn fill_negative_indices() {
        assert_eq!(eval("[1,2,3,4,5].fill(0, -3).join(\",\");"), crate::value::Value::String("1,2,0,0,0".to_string()));
    }

    #[test]
    fn fill_russian_alias() {
        assert_eq!(eval("[1,2,3,4,5].заполнить(9).join(\",\");"), crate::value::Value::String("9,9,9,9,9".to_string()));
    }

    #[test]
    fn copy_within_forward() {
        assert_eq!(
            eval("[1,2,3,4,5].copyWithin(0, 3).join(\",\");"),
            crate::value::Value::String("4,5,3,4,5".to_string())
        );
    }

    #[test]
    fn copy_within_with_target_offset() {
        assert_eq!(
            eval("[1,2,3,4,5].copyWithin(1, 3).join(\",\");"),
            crate::value::Value::String("1,4,5,4,5".to_string())
        );
    }

    #[test]
    fn copy_within_russian_alias() {
        assert_eq!(
            eval("[1,2,3,4,5].копироватьВнутри(0, 3).join(\",\");"),
            crate::value::Value::String("4,5,3,4,5".to_string())
        );
    }

    #[test]
    fn entries_keys_values_are_iterators() {
        assert_eq!(eval("тип([\"a\",\"b\"].entries());"), crate::value::Value::String("итератор".to_string()));
        assert_eq!(
            eval("[...[\"a\",\"b\",\"c\"].keys()].join(\",\");"),
            crate::value::Value::String("0,1,2".to_string())
        );
        assert_eq!(eval("[...[\"a\",\"b\"].values()].join(\",\");"), crate::value::Value::String("a,b".to_string()));
        assert_eq!(
            eval("[...[\"a\",\"b\"].entries()][0].join(\",\");"),
            crate::value::Value::String("0,a".to_string())
        );
    }

    #[test]
    fn keys_values_russian_aliases() {
        assert_eq!(eval("[...[10,20].ключи()].join(\",\");"), crate::value::Value::String("0,1".to_string()));
        assert_eq!(eval("[...[10,20].значения()].join(\",\");"), crate::value::Value::String("10,20".to_string()));
    }
}
