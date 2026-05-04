use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use yps_lexer::Span;

use crate::error::RuntimeError;
use crate::interpreter::Interpreter;
use crate::stdlib::require_args;
use crate::value::{PromiseState, Value};

pub fn construct(interp: &mut Interpreter, args: Vec<Value>, span: Span) -> Result<Value, RuntimeError> {
    require_args(&args, 1, span, "СловоПацана")?;
    let executor = args.into_iter().next().unwrap();
    if !matches!(executor, Value::Function { .. } | Value::BuiltinFunction(_)) {
        return Err(RuntimeError::new("'СловоПацана' ожидает функцию-исполнитель", span));
    }
    let (promise, resolve, reject) = Interpreter::make_pending_promise();
    interp.call_function(executor, vec![resolve, reject], span)?;
    Ok(promise)
}

pub fn call_static(
    interp: &mut Interpreter,
    method: &str,
    args: Vec<Value>,
    span: Span,
) -> Result<Value, RuntimeError> {
    match method {
        "решить" => {
            let val = args.into_iter().next().unwrap_or(Value::Undefined);
            Ok(Interpreter::make_fulfilled_promise(val))
        }
        "отвергнуть" => {
            let val = args.into_iter().next().unwrap_or(Value::Undefined);
            Ok(Interpreter::make_rejected_promise(val))
        }
        "всех" => {
            require_args(&args, 1, span, "СловоПацана.всех")?;
            let arr = expect_array(args.into_iter().next().unwrap(), "СловоПацана.всех", span)?;
            let mut results: Vec<Value> = Vec::with_capacity(arr.len());
            for item in arr {
                match settle_value(interp, item, span)? {
                    PromiseOutcome::Fulfilled(v) => results.push(v),
                    PromiseOutcome::Rejected(v) => return Ok(Interpreter::make_rejected_promise(v)),
                }
            }
            Ok(Interpreter::make_fulfilled_promise(Value::Array(results)))
        }
        "всехУстаканить" => {
            require_args(&args, 1, span, "СловоПацана.всехУстаканить")?;
            let arr = expect_array(args.into_iter().next().unwrap(), "СловоПацана.всехУстаканить", span)?;
            let mut results: Vec<Value> = Vec::with_capacity(arr.len());
            for item in arr {
                let mut entry = HashMap::new();
                match settle_value(interp, item, span)? {
                    PromiseOutcome::Fulfilled(v) => {
                        entry.insert("статус".to_string(), Value::String("выполнено".to_string()));
                        entry.insert("значение".to_string(), v);
                    }
                    PromiseOutcome::Rejected(v) => {
                        entry.insert("статус".to_string(), Value::String("отклонено".to_string()));
                        entry.insert("причина".to_string(), v);
                    }
                }
                results.push(Value::Object(entry));
            }
            Ok(Interpreter::make_fulfilled_promise(Value::Array(results)))
        }
        "любой" => {
            require_args(&args, 1, span, "СловоПацана.любой")?;
            let arr = expect_array(args.into_iter().next().unwrap(), "СловоПацана.любой", span)?;
            let mut errors: Vec<Value> = Vec::with_capacity(arr.len());
            for item in arr {
                match settle_value(interp, item, span)? {
                    PromiseOutcome::Fulfilled(v) => return Ok(Interpreter::make_fulfilled_promise(v)),
                    PromiseOutcome::Rejected(v) => errors.push(v),
                }
            }
            let mut agg = HashMap::new();
            agg.insert("name".to_string(), Value::String("ВсёОбосралось".to_string()));
            agg.insert("message".to_string(), Value::String("Все обещания отклонены".to_string()));
            agg.insert("errors".to_string(), Value::Array(errors));
            Ok(Interpreter::make_rejected_promise(Value::Object(agg)))
        }
        "гонка" => {
            require_args(&args, 1, span, "СловоПацана.гонка")?;
            let arr = expect_array(args.into_iter().next().unwrap(), "СловоПацана.гонка", span)?;
            let first = arr
                .into_iter()
                .next()
                .ok_or_else(|| RuntimeError::new("'СловоПацана.гонка' требует непустой массив", span))?;
            match settle_value(interp, first, span)? {
                PromiseOutcome::Fulfilled(v) => Ok(Interpreter::make_fulfilled_promise(v)),
                PromiseOutcome::Rejected(v) => Ok(Interpreter::make_rejected_promise(v)),
            }
        }
        "сРешалками" => {
            let (promise, resolve, reject) = Interpreter::make_pending_promise();
            let mut map = HashMap::new();
            map.insert("обещание".to_string(), promise);
            map.insert("решить".to_string(), resolve);
            map.insert("отвергнуть".to_string(), reject);
            Ok(Value::Object(map))
        }
        "попробовать" => {
            require_args(&args, 1, span, "СловоПацана.попробовать")?;
            let func = args.into_iter().next().unwrap();
            match interp.call_function(func, vec![], span) {
                Ok(val) => {
                    if let Value::Promise { .. } = &val {
                        Ok(val)
                    } else {
                        Ok(Interpreter::make_fulfilled_promise(val))
                    }
                }
                Err(e) => Ok(Interpreter::make_rejected_promise(Value::String(e.message))),
            }
        }
        _ => Err(RuntimeError::new(format!("У 'СловоПацана' нет метода '{method}'"), span)),
    }
}

pub fn call(
    interp: &mut Interpreter,
    receiver: Value,
    method: &str,
    args: Vec<Value>,
    span: Span,
) -> Result<(Value, Option<Value>), RuntimeError> {
    let state = match &receiver {
        Value::Promise { state } => Rc::clone(state),
        _ => unreachable!(),
    };
    match method {
        "потом" => {
            let mut iter = args.into_iter();
            let on_fulfill = iter.next().unwrap_or(Value::Undefined);
            let on_reject = iter.next().unwrap_or(Value::Undefined);
            let new_promise = chain_promise(interp, &state, on_fulfill, on_reject, span)?;
            Ok((new_promise, None))
        }
        "ловить" => {
            let on_reject = args.into_iter().next().unwrap_or(Value::Undefined);
            let new_promise = chain_promise(interp, &state, Value::Undefined, on_reject, span)?;
            Ok((new_promise, None))
        }
        "наконец" => {
            require_args(&args, 1, span, "наконец")?;
            let cb = args.into_iter().next().unwrap();
            let new_promise = finally_promise(interp, &state, cb, span)?;
            Ok((new_promise, None))
        }
        _ => Err(RuntimeError::new(format!("У обещания нет метода '{method}'"), span)),
    }
}

enum PromiseOutcome {
    Fulfilled(Value),
    Rejected(Value),
}

fn expect_array(v: Value, ctx: &str, span: Span) -> Result<Vec<Value>, RuntimeError> {
    match v {
        Value::Array(a) => Ok(a),
        other => Err(RuntimeError::new(format!("'{ctx}' ожидает массив, получено '{}'", other.type_name()), span)),
    }
}

fn settle_value(interp: &mut Interpreter, value: Value, span: Span) -> Result<PromiseOutcome, RuntimeError> {
    match value {
        Value::Promise { state } => {
            interp.drain_microtasks(span)?;
            let snap = state.borrow().clone();
            match snap {
                PromiseState::Fulfilled(v) => Ok(PromiseOutcome::Fulfilled(v)),
                PromiseState::Rejected(v) => Ok(PromiseOutcome::Rejected(v)),
                PromiseState::Pending { .. } => Err(RuntimeError::new("обещание не разрешено синхронно", span)),
            }
        }
        other => Ok(PromiseOutcome::Fulfilled(other)),
    }
}

fn chain_promise(
    interp: &mut Interpreter,
    state: &Rc<RefCell<PromiseState>>,
    on_fulfill: Value,
    on_reject: Value,
    _span: Span,
) -> Result<Value, RuntimeError> {
    let (new_promise, resolve_cap, reject_cap) = Interpreter::make_pending_promise();
    let snap = state.borrow().clone();
    match snap {
        PromiseState::Fulfilled(v) => {
            interp.enqueue_microtask(Box::new(move |interp, span| {
                invoke_handler(interp, on_fulfill, v, resolve_cap, reject_cap, true, span)
            }));
        }
        PromiseState::Rejected(v) => {
            interp.enqueue_microtask(Box::new(move |interp, span| {
                invoke_handler(interp, on_reject, v, resolve_cap, reject_cap, false, span)
            }));
        }
        PromiseState::Pending { .. } => {
            let resolve_cb = Value::PromiseThenHandler {
                handler: Box::new(on_fulfill),
                resolve: Box::new(resolve_cap.clone()),
                reject: Box::new(reject_cap.clone()),
                is_fulfill: true,
            };
            let reject_cb = Value::PromiseThenHandler {
                handler: Box::new(on_reject),
                resolve: Box::new(resolve_cap),
                reject: Box::new(reject_cap),
                is_fulfill: false,
            };
            if let PromiseState::Pending { on_resolve, on_reject } = &mut *state.borrow_mut() {
                on_resolve.push(resolve_cb);
                on_reject.push(reject_cb);
            }
        }
    }
    Ok(new_promise)
}

fn finally_promise(
    interp: &mut Interpreter,
    state: &Rc<RefCell<PromiseState>>,
    cb: Value,
    _span: Span,
) -> Result<Value, RuntimeError> {
    let (new_promise, resolve_cap, reject_cap) = Interpreter::make_pending_promise();
    let snap = state.borrow().clone();
    match snap {
        PromiseState::Fulfilled(v) => {
            interp.enqueue_microtask(Box::new(move |interp, span| {
                interp.call_function(cb, vec![], span)?;
                interp.call_function(resolve_cap, vec![v], span)?;
                Ok(())
            }));
        }
        PromiseState::Rejected(v) => {
            interp.enqueue_microtask(Box::new(move |interp, span| {
                interp.call_function(cb, vec![], span)?;
                interp.call_function(reject_cap, vec![v], span)?;
                Ok(())
            }));
        }
        PromiseState::Pending { .. } => {
            let resolve_cb = Value::PromiseFinallyHandler { cb: Box::new(cb.clone()), cap: Box::new(resolve_cap) };
            let reject_cb = Value::PromiseFinallyHandler { cb: Box::new(cb), cap: Box::new(reject_cap) };
            if let PromiseState::Pending { on_resolve, on_reject } = &mut *state.borrow_mut() {
                on_resolve.push(resolve_cb);
                on_reject.push(reject_cb);
            }
        }
    }
    Ok(new_promise)
}

pub(crate) fn invoke_handler(
    interp: &mut Interpreter,
    handler: Value,
    val: Value,
    resolve_cap: Value,
    reject_cap: Value,
    is_fulfill: bool,
    span: Span,
) -> Result<(), RuntimeError> {
    if matches!(handler, Value::Undefined | Value::Null) {
        let cap = if is_fulfill { resolve_cap } else { reject_cap };
        interp.call_function(cap, vec![val], span)?;
        return Ok(());
    }
    match interp.call_function(handler, vec![val], span) {
        Ok(result) => {
            interp.call_function(resolve_cap, vec![result], span)?;
        }
        Err(e) => {
            interp.call_function(reject_cap, vec![Value::String(e.message)], span)?;
        }
    }
    Ok(())
}
