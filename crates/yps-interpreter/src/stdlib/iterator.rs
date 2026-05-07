use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use yps_lexer::Span;

use crate::error::RuntimeError;
use crate::interpreter::Interpreter;
use crate::stdlib::{builtin, object_of, require_args};
use crate::value::{IteratorState, Value};

pub fn build_object() -> Value {
    object_of(&[
        ("от", builtin("Итератор.от")),
        ("from", builtin("Итератор.от")),
        ("склеить", builtin("Итератор.склеить")),
        ("concat", builtin("Итератор.склеить")),
    ])
}

pub fn call_static(
    _interp: &mut Interpreter,
    method: &str,
    args: Vec<Value>,
    span: Span,
) -> Result<Value, RuntimeError> {
    match method {
        "от" | "from" => {
            require_args(&args, 1, span, "Итератор.от")?;
            let value = args.into_iter().next().unwrap();
            if let Value::Iterator(rc) = value {
                return Ok(Value::Iterator(rc));
            }
            let state = state_from_value(value, span, "Итератор.от")?;
            Ok(Value::Iterator(Rc::new(RefCell::new(state))))
        }
        "склеить" | "concat" => {
            let mut iters: VecDeque<IteratorState> = VecDeque::new();
            for (i, arg) in args.into_iter().enumerate() {
                let state = state_from_value(arg, span, &format!("Итератор.склеить(аргумент {})", i + 1))?;
                iters.push_back(state);
            }
            Ok(Value::Iterator(Rc::new(RefCell::new(IteratorState::Concat { iters }))))
        }
        _ => Err(RuntimeError::new(format!("У 'Итератор' нет метода '{method}'"), span)),
    }
}

pub fn call(
    interp: &mut Interpreter,
    receiver: Value,
    method: &str,
    args: Vec<Value>,
    span: Span,
) -> Result<(Value, Option<Value>), RuntimeError> {
    let rc = match receiver {
        Value::Iterator(rc) => rc,
        _ => unreachable!("call dispatched on non-iterator value"),
    };

    match method {
        "следующий" | "next" => {
            let mut state = rc.borrow_mut();
            match next(interp, &mut state, span)? {
                Some(value) => Ok((make_result(value, false), None)),
                None => Ok((make_result(Value::Undefined, true), None)),
            }
        }
        "map" | "преобразовать" => {
            require_args(&args, 1, span, "map")?;
            let func = args.into_iter().next().unwrap();
            let inner = std::mem::replace(&mut *rc.borrow_mut(), IteratorState::Done);
            let new_state = IteratorState::Map { inner: Box::new(inner), func, index: 0 };
            Ok((Value::Iterator(Rc::new(RefCell::new(new_state))), None))
        }
        "filter" | "отфильтровать" => {
            require_args(&args, 1, span, "filter")?;
            let func = args.into_iter().next().unwrap();
            let inner = std::mem::replace(&mut *rc.borrow_mut(), IteratorState::Done);
            let new_state = IteratorState::Filter { inner: Box::new(inner), func, index: 0 };
            Ok((Value::Iterator(Rc::new(RefCell::new(new_state))), None))
        }
        "take" | "взять" => {
            require_args(&args, 1, span, "take")?;
            let n = expect_count(&args[0], span, "take")?;
            let inner = std::mem::replace(&mut *rc.borrow_mut(), IteratorState::Done);
            let new_state = IteratorState::Take { inner: Box::new(inner), remaining: n };
            Ok((Value::Iterator(Rc::new(RefCell::new(new_state))), None))
        }
        "drop" | "пропустить" => {
            require_args(&args, 1, span, "drop")?;
            let n = expect_count(&args[0], span, "drop")?;
            let inner = std::mem::replace(&mut *rc.borrow_mut(), IteratorState::Done);
            let new_state = IteratorState::Drop { inner: Box::new(inner), count: n, dropped: false };
            Ok((Value::Iterator(Rc::new(RefCell::new(new_state))), None))
        }
        "toArray" | "вМассив" => {
            let values = drain(interp, &rc, span)?;
            Ok((Value::Array(values), None))
        }
        "forEach" | "каждый" => {
            require_args(&args, 1, span, "forEach")?;
            let func = args.into_iter().next().unwrap();
            let mut idx = 0usize;
            loop {
                let v = {
                    let mut state = rc.borrow_mut();
                    next(interp, &mut state, span)?
                };
                match v {
                    Some(val) => {
                        interp.call_function(func.clone(), vec![val, Value::Number(idx as f64)], span)?;
                        idx += 1;
                    }
                    None => break,
                }
            }
            Ok((Value::Undefined, None))
        }
        "reduce" | "свернуть" => {
            require_args(&args, 1, span, "reduce")?;
            let mut iter = args.into_iter();
            let func = iter.next().unwrap();
            let initial = iter.next();
            let mut acc = match initial {
                Some(v) => v,
                None => {
                    let first = {
                        let mut state = rc.borrow_mut();
                        next(interp, &mut state, span)?
                    };
                    match first {
                        Some(v) => v,
                        None => {
                            return Err(RuntimeError::new("reduce пустого итератора без начального значения", span));
                        }
                    }
                }
            };
            let mut idx = 0usize;
            loop {
                let v = {
                    let mut state = rc.borrow_mut();
                    next(interp, &mut state, span)?
                };
                match v {
                    Some(val) => {
                        acc = interp.call_function(func.clone(), vec![acc, val, Value::Number(idx as f64)], span)?;
                        idx += 1;
                    }
                    None => break,
                }
            }
            Ok((acc, None))
        }
        "some" | "некоторые" => {
            require_args(&args, 1, span, "some")?;
            let func = args.into_iter().next().unwrap();
            let mut idx = 0usize;
            loop {
                let v = {
                    let mut state = rc.borrow_mut();
                    next(interp, &mut state, span)?
                };
                match v {
                    Some(val) => {
                        let m = interp.call_function(func.clone(), vec![val, Value::Number(idx as f64)], span)?;
                        if m.is_truthy() {
                            return Ok((Value::Boolean(true), None));
                        }
                        idx += 1;
                    }
                    None => return Ok((Value::Boolean(false), None)),
                }
            }
        }
        "every" | "все" => {
            require_args(&args, 1, span, "every")?;
            let func = args.into_iter().next().unwrap();
            let mut idx = 0usize;
            loop {
                let v = {
                    let mut state = rc.borrow_mut();
                    next(interp, &mut state, span)?
                };
                match v {
                    Some(val) => {
                        let m = interp.call_function(func.clone(), vec![val, Value::Number(idx as f64)], span)?;
                        if !m.is_truthy() {
                            return Ok((Value::Boolean(false), None));
                        }
                        idx += 1;
                    }
                    None => return Ok((Value::Boolean(true), None)),
                }
            }
        }
        "find" | "найти" => {
            require_args(&args, 1, span, "find")?;
            let func = args.into_iter().next().unwrap();
            let mut idx = 0usize;
            loop {
                let v = {
                    let mut state = rc.borrow_mut();
                    next(interp, &mut state, span)?
                };
                match v {
                    Some(val) => {
                        let m =
                            interp.call_function(func.clone(), vec![val.clone(), Value::Number(idx as f64)], span)?;
                        if m.is_truthy() {
                            return Ok((val, None));
                        }
                        idx += 1;
                    }
                    None => return Ok((Value::Undefined, None)),
                }
            }
        }
        _ => Err(RuntimeError::new(format!("Итератор не имеет метода '{method}'"), span)),
    }
}

pub fn drain(
    interp: &mut Interpreter,
    rc: &Rc<RefCell<IteratorState>>,
    span: Span,
) -> Result<Vec<Value>, RuntimeError> {
    let mut out = Vec::new();
    loop {
        let v = {
            let mut state = rc.borrow_mut();
            next(interp, &mut state, span)?
        };
        match v {
            Some(val) => out.push(val),
            None => return Ok(out),
        }
    }
}

pub fn next(interp: &mut Interpreter, state: &mut IteratorState, span: Span) -> Result<Option<Value>, RuntimeError> {
    match state {
        IteratorState::Done => Ok(None),
        IteratorState::Array { values, index } => {
            if *index >= values.len() {
                *state = IteratorState::Done;
                return Ok(None);
            }
            let v = values[*index].clone();
            *index += 1;
            Ok(Some(v))
        }
        IteratorState::Chars { chars, index } => {
            if *index >= chars.len() {
                *state = IteratorState::Done;
                return Ok(None);
            }
            let c = chars[*index];
            *index += 1;
            Ok(Some(Value::String(c.to_string())))
        }
        IteratorState::MapEntries { entries, index } => {
            if *index >= entries.len() {
                *state = IteratorState::Done;
                return Ok(None);
            }
            let (k, v) = entries[*index].clone();
            *index += 1;
            Ok(Some(Value::Array(vec![k, v])))
        }
        IteratorState::Map { inner, func, index } => match next(interp, inner, span)? {
            None => {
                *state = IteratorState::Done;
                Ok(None)
            }
            Some(v) => {
                let i = *index;
                *index += 1;
                let mapped = interp.call_function(func.clone(), vec![v, Value::Number(i as f64)], span)?;
                Ok(Some(mapped))
            }
        },
        IteratorState::Filter { inner, func, index } => loop {
            match next(interp, inner, span)? {
                None => {
                    *state = IteratorState::Done;
                    return Ok(None);
                }
                Some(v) => {
                    let i = *index;
                    *index += 1;
                    let keep = interp.call_function(func.clone(), vec![v.clone(), Value::Number(i as f64)], span)?;
                    if keep.is_truthy() {
                        return Ok(Some(v));
                    }
                }
            }
        },
        IteratorState::Take { inner, remaining } => {
            if *remaining == 0 {
                *state = IteratorState::Done;
                return Ok(None);
            }
            match next(interp, inner, span)? {
                None => {
                    *state = IteratorState::Done;
                    Ok(None)
                }
                Some(v) => {
                    *remaining -= 1;
                    Ok(Some(v))
                }
            }
        }
        IteratorState::Drop { inner, count, dropped } => {
            if !*dropped {
                for _ in 0..*count {
                    if next(interp, inner, span)?.is_none() {
                        *state = IteratorState::Done;
                        return Ok(None);
                    }
                }
                *dropped = true;
            }
            match next(interp, inner, span)? {
                None => {
                    *state = IteratorState::Done;
                    Ok(None)
                }
                Some(v) => Ok(Some(v)),
            }
        }
        IteratorState::Concat { iters } => {
            while let Some(front) = iters.front_mut() {
                match next(interp, front, span)? {
                    Some(v) => return Ok(Some(v)),
                    None => {
                        iters.pop_front();
                    }
                }
            }
            *state = IteratorState::Done;
            Ok(None)
        }
    }
}

pub fn state_from_value(value: Value, span: Span, ctx: &str) -> Result<IteratorState, RuntimeError> {
    match value {
        Value::Array(values) => Ok(IteratorState::Array { values, index: 0 }),
        Value::String(s) => Ok(IteratorState::Chars { chars: s.chars().collect(), index: 0 }),
        Value::Set(values) => Ok(IteratorState::Array { values, index: 0 }),
        Value::Map(entries) => Ok(IteratorState::MapEntries { entries, index: 0 }),
        Value::Iterator(rc) => Ok(rc.borrow().clone()),
        other => Err(RuntimeError::new(
            format!("'{ctx}' ожидает итерируемое значение, получено '{}'", other.type_name()),
            span,
        )),
    }
}

fn expect_count(value: &Value, span: Span, method: &str) -> Result<usize, RuntimeError> {
    match value {
        Value::Number(n) if n.is_finite() && *n >= 0.0 => Ok(*n as usize),
        _ => Err(RuntimeError::new(format!("'{method}' ожидает неотрицательное число"), span)),
    }
}

fn make_result(value: Value, done: bool) -> Value {
    let mut map = std::collections::HashMap::new();
    map.insert("значение".to_string(), value);
    map.insert("готово".to_string(), Value::Boolean(done));
    Value::Object(map)
}
