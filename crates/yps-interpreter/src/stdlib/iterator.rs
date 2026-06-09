use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use yps_lexer::Span;

use crate::error::RuntimeError;
use crate::interpreter::Interpreter;
use crate::stdlib::{builtin, object_of, require_args};
use crate::symbols;
use crate::value::{IteratorState, Value};

fn borrow_iter_mut<'a>(
    rc: &'a Rc<RefCell<IteratorState>>,
    span: Span,
) -> Result<std::cell::RefMut<'a, IteratorState>, RuntimeError> {
    rc.try_borrow_mut().map_err(|_| RuntimeError::new("Итератор уже выполняется", span))
}

const MAX_ITERATOR_DEPTH: usize = 200;

fn adapter_depth(state: &IteratorState, budget: usize) -> usize {
    if budget == 0 {
        return usize::MAX;
    }
    match state {
        IteratorState::Map { inner, .. }
        | IteratorState::Filter { inner, .. }
        | IteratorState::Take { inner, .. }
        | IteratorState::Drop { inner, .. } => adapter_depth(inner, budget - 1).saturating_add(1),
        IteratorState::Concat { iters } => {
            iters.iter().map(|it| adapter_depth(it, budget - 1)).max().unwrap_or(0).saturating_add(1)
        }
        _ => 0,
    }
}

fn check_chain_depth(rc: &Rc<RefCell<IteratorState>>, span: Span) -> Result<(), RuntimeError> {
    if adapter_depth(&rc.borrow(), MAX_ITERATOR_DEPTH) >= MAX_ITERATOR_DEPTH {
        return Err(RuntimeError::new("Слишком длинная цепочка итераторов", span));
    }
    Ok(())
}

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
                if adapter_depth(&state, MAX_ITERATOR_DEPTH) >= MAX_ITERATOR_DEPTH {
                    return Err(RuntimeError::new("Слишком длинная цепочка итераторов", span));
                }
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
            let mut state_borrow = borrow_iter_mut(&rc, span)?;
            if let IteratorState::Generator(gen_state) = &mut *state_borrow {
                let outcome = crate::interpreter::generator::step_generator(
                    interp,
                    gen_state,
                    crate::interpreter::generator::GenInput::Send(Value::Undefined),
                    span,
                )?;
                match outcome {
                    crate::interpreter::generator::StepOutcome::Yielded(v) => Ok((make_result(v, false), None)),
                    crate::interpreter::generator::StepOutcome::Done(v) => Ok((make_result(v, true), None)),
                }
            } else {
                match next(interp, &mut state_borrow, span)? {
                    Some(value) => Ok((make_result(value, false), None)),
                    None => Ok((make_result(Value::Undefined, true), None)),
                }
            }
        }
        "вернуть" | "return" => {
            let arg = args.into_iter().next().unwrap_or(Value::Undefined);
            let mut state_borrow = borrow_iter_mut(&rc, span)?;
            match &mut *state_borrow {
                IteratorState::Generator(gen_state) => {
                    if gen_state.completed {
                        return Ok((make_result(arg, true), None));
                    }
                    let outcome = crate::interpreter::generator::step_generator(
                        interp,
                        gen_state,
                        crate::interpreter::generator::GenInput::Return(arg),
                        span,
                    )?;
                    match outcome {
                        crate::interpreter::generator::StepOutcome::Yielded(v) => Ok((make_result(v, false), None)),
                        crate::interpreter::generator::StepOutcome::Done(v) => Ok((make_result(v, true), None)),
                    }
                }
                IteratorState::Done => Ok((make_result(arg, true), None)),
                _ => Err(RuntimeError::new("Метод 'вернуть' доступен только для генераторов", span)),
            }
        }
        "кинуть" | "throw" => {
            let arg = args.into_iter().next().unwrap_or(Value::Undefined);
            let mut state_borrow = borrow_iter_mut(&rc, span)?;
            match &mut *state_borrow {
                IteratorState::Generator(gen_state) => {
                    if gen_state.completed {
                        return Err(RuntimeError::thrown(arg, span));
                    }
                    let outcome = crate::interpreter::generator::step_generator(
                        interp,
                        gen_state,
                        crate::interpreter::generator::GenInput::Throw(arg),
                        span,
                    )?;
                    match outcome {
                        crate::interpreter::generator::StepOutcome::Yielded(v) => Ok((make_result(v, false), None)),
                        crate::interpreter::generator::StepOutcome::Done(v) => Ok((make_result(v, true), None)),
                    }
                }
                IteratorState::Done => Err(RuntimeError::thrown(arg, span)),
                _ => Err(RuntimeError::new("Метод 'кинуть' доступен только для генераторов", span)),
            }
        }
        "map" | "преобразовать" => {
            require_args(&args, 1, span, "map")?;
            check_chain_depth(&rc, span)?;
            let func = args.into_iter().next().unwrap();
            let inner = std::mem::replace(&mut *borrow_iter_mut(&rc, span)?, IteratorState::Done);
            let new_state = IteratorState::Map { inner: Box::new(inner), func, index: 0 };
            Ok((Value::Iterator(Rc::new(RefCell::new(new_state))), None))
        }
        "filter" | "отфильтровать" => {
            require_args(&args, 1, span, "filter")?;
            check_chain_depth(&rc, span)?;
            let func = args.into_iter().next().unwrap();
            let inner = std::mem::replace(&mut *borrow_iter_mut(&rc, span)?, IteratorState::Done);
            let new_state = IteratorState::Filter { inner: Box::new(inner), func, index: 0 };
            Ok((Value::Iterator(Rc::new(RefCell::new(new_state))), None))
        }
        "take" | "взять" => {
            require_args(&args, 1, span, "take")?;
            check_chain_depth(&rc, span)?;
            let n = expect_count(&args[0], span, "take")?;
            let inner = std::mem::replace(&mut *borrow_iter_mut(&rc, span)?, IteratorState::Done);
            let new_state = IteratorState::Take { inner: Box::new(inner), remaining: n };
            Ok((Value::Iterator(Rc::new(RefCell::new(new_state))), None))
        }
        "drop" | "пропустить" => {
            require_args(&args, 1, span, "drop")?;
            check_chain_depth(&rc, span)?;
            let n = expect_count(&args[0], span, "drop")?;
            let inner = std::mem::replace(&mut *borrow_iter_mut(&rc, span)?, IteratorState::Done);
            let new_state = IteratorState::Drop { inner: Box::new(inner), count: n, dropped: false };
            Ok((Value::Iterator(Rc::new(RefCell::new(new_state))), None))
        }
        "toArray" | "вМассив" => {
            let values = drain(interp, &rc, span)?;
            Ok((Value::array(values), None))
        }
        "forEach" | "каждый" => {
            require_args(&args, 1, span, "forEach")?;
            let func = args.into_iter().next().unwrap();
            let mut idx = 0usize;
            loop {
                let v = {
                    let mut state = borrow_iter_mut(&rc, span)?;
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
                        let mut state = borrow_iter_mut(&rc, span)?;
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
                    let mut state = borrow_iter_mut(&rc, span)?;
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
                    let mut state = borrow_iter_mut(&rc, span)?;
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
                    let mut state = borrow_iter_mut(&rc, span)?;
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
                    let mut state = borrow_iter_mut(&rc, span)?;
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
            let mut state = borrow_iter_mut(rc, span)?;
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
            Ok(Some(Value::array(vec![k, v])))
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
        IteratorState::RegexMatches { re, input, byte_pos } => {
            if *byte_pos > input.len() {
                *state = IteratorState::Done;
                return Ok(None);
            }
            let re = Rc::clone(re);
            match re.captures_at(input, *byte_pos) {
                None => {
                    *state = IteratorState::Done;
                    Ok(None)
                }
                Some(caps) => {
                    let whole = caps.get(0).expect("match group 0");
                    let new_pos = if whole.end() == whole.start() { whole.end() + 1 } else { whole.end() };
                    let obj = crate::stdlib::regexp::build_match_object(&caps, input, &re, false);
                    *byte_pos = new_pos;
                    Ok(Some(obj))
                }
            }
        }
        IteratorState::Generator(gen_state) => {
            let result = crate::interpreter::generator::step_generator(
                interp,
                gen_state,
                crate::interpreter::generator::GenInput::Send(Value::Undefined),
                span,
            )?;
            match result {
                crate::interpreter::generator::StepOutcome::Yielded(v) => Ok(Some(v)),
                crate::interpreter::generator::StepOutcome::Done(_) => Ok(None),
            }
        }
    }
}

pub fn state_from_value(value: Value, span: Span, ctx: &str) -> Result<IteratorState, RuntimeError> {
    match value {
        Value::Array(values) => Ok(IteratorState::Array { values: values.borrow().clone(), index: 0 }),
        Value::String(s) => Ok(IteratorState::Chars { chars: s.chars().collect(), index: 0 }),
        Value::Set(values) => Ok(IteratorState::Array {
            values: values.borrow().iter().map(|k| k.as_value().clone()).collect(),
            index: 0,
        }),
        Value::Map(entries) => Ok(IteratorState::MapEntries {
            entries: entries.borrow().iter().map(|(k, v)| (k.as_value().clone(), v.clone())).collect(),
            index: 0,
        }),
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
    map.insert(symbols::ITER_VALUE.to_string(), value);
    map.insert(symbols::ITER_DONE.to_string(), Value::Boolean(done));
    Value::object(map)
}
