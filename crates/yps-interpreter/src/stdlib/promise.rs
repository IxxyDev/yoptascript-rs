use std::cell::RefCell;
use std::rc::Rc;

use indexmap::IndexMap;

use yps_lexer::Span;

use crate::error::RuntimeError;
use crate::interpreter::{GcRoot, Interpreter};
use crate::stdlib::require_args;
use crate::value::{AggregateKind, AggregateRole, AggregateState, PromiseState, ThenHandlerData, Value};

pub fn construct(interp: &mut Interpreter, args: Vec<Value>, span: Span) -> Result<Value, RuntimeError> {
    require_args(&args, 1, span, "СловоПацана")?;
    let executor = args.into_iter().next().unwrap();
    if !executor.is_callable() {
        return Err(RuntimeError::new("'СловоПацана' ожидает функцию-исполнитель", span));
    }
    let (promise, resolve, reject) = Interpreter::make_pending_promise();
    if let Err(e) = interp.call_function(executor, vec![resolve, reject.clone()], span) {
        match e.thrown {
            Some(val) => {
                interp.call_function(reject, vec![*val], span)?;
            }
            None => return Err(e),
        }
    }
    Ok(promise)
}

pub(crate) fn rejection_reason(e: RuntimeError) -> Value {
    match e.thrown {
        Some(val) => *val,
        None => {
            let mut map = IndexMap::new();
            map.insert(
                crate::symbols::ERROR_NAME_FIELD.to_string(),
                Value::String(crate::symbols::ERROR_NAME.to_string().into()),
            );
            map.insert(crate::symbols::ERROR_MESSAGE_FIELD.to_string(), Value::String(e.message.into()));
            Value::object(map)
        }
    }
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
            Ok(run_aggregate(interp, AggregateKind::All, arr, span))
        }
        "всехУстаканить" => {
            require_args(&args, 1, span, "СловоПацана.всехУстаканить")?;
            let arr = expect_array(args.into_iter().next().unwrap(), "СловоПацана.всехУстаканить", span)?;
            Ok(run_aggregate(interp, AggregateKind::AllSettled, arr, span))
        }
        "любой" => {
            require_args(&args, 1, span, "СловоПацана.любой")?;
            let arr = expect_array(args.into_iter().next().unwrap(), "СловоПацана.любой", span)?;
            Ok(run_aggregate(interp, AggregateKind::Any, arr, span))
        }
        "гонка" => {
            require_args(&args, 1, span, "СловоПацана.гонка")?;
            let arr = expect_array(args.into_iter().next().unwrap(), "СловоПацана.гонка", span)?;
            if arr.is_empty() {
                return Err(RuntimeError::new("'СловоПацана.гонка' требует непустой массив", span));
            }
            Ok(run_aggregate(interp, AggregateKind::Race, arr, span))
        }
        "отПодождать" => {
            require_args(&args, 1, span, "СловоПацана.отПодождать")?;
            match args.into_iter().next().unwrap() {
                Value::AbortSignal { state } => Ok(crate::stdlib::abort::get_or_init_signal_promise(&state)),
                other => Err(RuntimeError::new(
                    format!("'СловоПацана.отПодождать' ожидает сигнал отмены, получено '{}'", other.type_name()),
                    span,
                )),
            }
        }
        "сРешалками" => {
            let (promise, resolve, reject) = Interpreter::make_pending_promise();
            let mut map = IndexMap::new();
            map.insert("обещание".to_string(), promise);
            map.insert("решить".to_string(), resolve);
            map.insert("отвергнуть".to_string(), reject);
            Ok(Value::object(map))
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
                Err(e) => Ok(Interpreter::make_rejected_promise(rejection_reason(e))),
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

fn expect_array(v: Value, ctx: &str, span: Span) -> Result<Vec<Value>, RuntimeError> {
    match v {
        Value::Array(a) => Ok(a.borrow().0.clone()),
        other => Err(RuntimeError::new(format!("'{ctx}' ожидает массив, получено '{}'", other.type_name()), span)),
    }
}

fn run_aggregate(interp: &mut Interpreter, kind: AggregateKind, items: Vec<Value>, span: Span) -> Value {
    let (promise, resolve_cap, reject_cap) = Interpreter::make_pending_promise();
    let n = items.len();

    if n == 0 {
        match kind {
            AggregateKind::All | AggregateKind::AllSettled => {
                interp.enqueue_microtask(
                    vec![GcRoot::Value(resolve_cap.clone())],
                    Box::new(move |interp, sp| {
                        interp.call_function(resolve_cap, vec![Value::array(Vec::new())], sp).map(|_| ())
                    }),
                );
            }
            AggregateKind::Any => {
                let err = aggregate_error(Vec::new());
                interp.enqueue_microtask(
                    vec![GcRoot::Value(reject_cap.clone()), GcRoot::Value(err.clone())],
                    Box::new(move |interp, sp| interp.call_function(reject_cap, vec![err], sp).map(|_| ())),
                );
            }
            AggregateKind::Race => {}
        }
        return promise;
    }

    let state = Rc::new(RefCell::new(AggregateState {
        kind,
        remaining: n,
        results: vec![Value::Undefined; n],
        resolve: resolve_cap,
        reject: reject_cap,
        settled: false,
    }));

    for (idx, item) in items.into_iter().enumerate() {
        attach_aggregate(interp, &state, idx, item, span);
    }

    promise
}

fn attach_aggregate(
    interp: &mut Interpreter,
    state: &Rc<RefCell<AggregateState>>,
    index: usize,
    value: Value,
    _span: Span,
) {
    let fulfill = Value::PromiseAggregateHandler { state: Rc::clone(state), index, role: AggregateRole::Fulfill };
    let reject = Value::PromiseAggregateHandler { state: Rc::clone(state), index, role: AggregateRole::Reject };
    match value {
        Value::Promise { state: p_state } => {
            let snap = p_state.borrow().clone();
            match snap {
                PromiseState::Fulfilled(v) => {
                    interp.enqueue_microtask(
                        vec![GcRoot::Value(fulfill.clone()), GcRoot::Value(v.clone())],
                        Box::new(move |interp, sp| interp.call_function(fulfill, vec![v], sp).map(|_| ())),
                    );
                }
                PromiseState::Rejected(v) => {
                    interp.enqueue_microtask(
                        vec![GcRoot::Value(reject.clone()), GcRoot::Value(v.clone())],
                        Box::new(move |interp, sp| interp.call_function(reject, vec![v], sp).map(|_| ())),
                    );
                }
                PromiseState::Pending { .. } => {
                    if let PromiseState::Pending { on_resolve, on_reject } = &mut *p_state.borrow_mut() {
                        on_resolve.push(fulfill);
                        on_reject.push(reject);
                    }
                }
            }
        }
        other => {
            interp.enqueue_microtask(
                vec![GcRoot::Value(fulfill.clone()), GcRoot::Value(other.clone())],
                Box::new(move |interp, sp| interp.call_function(fulfill, vec![other], sp).map(|_| ())),
            );
        }
    }
}

pub(crate) fn apply_aggregate(
    interp: &mut Interpreter,
    state: Rc<RefCell<AggregateState>>,
    index: usize,
    role: AggregateRole,
    value: Value,
    span: Span,
) -> Result<(), RuntimeError> {
    let (kind, settled) = {
        let s = state.borrow();
        (s.kind, s.settled)
    };
    if settled {
        return Ok(());
    }

    let action: Option<(Value, Value)> = match (kind, role) {
        (AggregateKind::All, AggregateRole::Fulfill) => {
            let mut s = state.borrow_mut();
            s.results[index] = value;
            s.remaining -= 1;
            if s.remaining == 0 {
                s.settled = true;
                let res = std::mem::take(&mut s.results);
                Some((s.resolve.clone(), Value::array(res)))
            } else {
                None
            }
        }
        (AggregateKind::All, AggregateRole::Reject) => {
            let mut s = state.borrow_mut();
            s.settled = true;
            Some((s.reject.clone(), value))
        }
        (AggregateKind::AllSettled, role) => {
            let mut entry = IndexMap::new();
            match role {
                AggregateRole::Fulfill => {
                    entry.insert("статус".to_string(), Value::String("выполнено".into()));
                    entry.insert("значение".to_string(), value);
                }
                AggregateRole::Reject => {
                    entry.insert("статус".to_string(), Value::String("отклонено".into()));
                    entry.insert("причина".to_string(), value);
                }
            }
            let mut s = state.borrow_mut();
            s.results[index] = Value::object(entry);
            s.remaining -= 1;
            if s.remaining == 0 {
                s.settled = true;
                let res = std::mem::take(&mut s.results);
                Some((s.resolve.clone(), Value::array(res)))
            } else {
                None
            }
        }
        (AggregateKind::Any, AggregateRole::Fulfill) => {
            let mut s = state.borrow_mut();
            s.settled = true;
            Some((s.resolve.clone(), value))
        }
        (AggregateKind::Any, AggregateRole::Reject) => {
            let mut s = state.borrow_mut();
            s.results[index] = value;
            s.remaining -= 1;
            if s.remaining == 0 {
                s.settled = true;
                let errs = std::mem::take(&mut s.results);
                Some((s.reject.clone(), aggregate_error(errs)))
            } else {
                None
            }
        }
        (AggregateKind::Race, role) => {
            let mut s = state.borrow_mut();
            s.settled = true;
            let cap = match role {
                AggregateRole::Fulfill => s.resolve.clone(),
                AggregateRole::Reject => s.reject.clone(),
            };
            Some((cap, value))
        }
    };

    if let Some((cap, val)) = action {
        interp.call_function(cap, vec![val], span)?;
    }
    Ok(())
}

fn aggregate_error(errors: Vec<Value>) -> Value {
    let mut agg = IndexMap::new();
    agg.insert("name".to_string(), Value::String("ВсёОбосралось".into()));
    agg.insert("message".to_string(), Value::String("Все обещания отклонены".into()));
    agg.insert("errors".to_string(), Value::array(errors));
    Value::object(agg)
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
            interp.enqueue_microtask(
                vec![
                    GcRoot::Value(on_fulfill.clone()),
                    GcRoot::Value(v.clone()),
                    GcRoot::Value(resolve_cap.clone()),
                    GcRoot::Value(reject_cap.clone()),
                ],
                Box::new(move |interp, span| {
                    invoke_handler(interp, on_fulfill, v, resolve_cap, reject_cap, true, span)
                }),
            );
        }
        PromiseState::Rejected(v) => {
            interp.enqueue_microtask(
                vec![
                    GcRoot::Value(on_reject.clone()),
                    GcRoot::Value(v.clone()),
                    GcRoot::Value(resolve_cap.clone()),
                    GcRoot::Value(reject_cap.clone()),
                ],
                Box::new(move |interp, span| {
                    invoke_handler(interp, on_reject, v, resolve_cap, reject_cap, false, span)
                }),
            );
        }
        PromiseState::Pending { .. } => {
            let resolve_cb = Value::PromiseThenHandler(Box::new(ThenHandlerData {
                handler: on_fulfill,
                resolve: resolve_cap.clone(),
                reject: reject_cap.clone(),
                is_fulfill: true,
            }));
            let reject_cb = Value::PromiseThenHandler(Box::new(ThenHandlerData {
                handler: on_reject,
                resolve: resolve_cap,
                reject: reject_cap,
                is_fulfill: false,
            }));
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
            interp.enqueue_microtask(
                vec![GcRoot::Value(cb.clone()), GcRoot::Value(resolve_cap.clone()), GcRoot::Value(v.clone())],
                Box::new(move |interp, span| {
                    interp.call_function(cb, vec![], span)?;
                    interp.call_function(resolve_cap, vec![v], span)?;
                    Ok(())
                }),
            );
        }
        PromiseState::Rejected(v) => {
            interp.enqueue_microtask(
                vec![GcRoot::Value(cb.clone()), GcRoot::Value(reject_cap.clone()), GcRoot::Value(v.clone())],
                Box::new(move |interp, span| {
                    interp.call_function(cb, vec![], span)?;
                    interp.call_function(reject_cap, vec![v], span)?;
                    Ok(())
                }),
            );
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
            interp.call_function(reject_cap, vec![rejection_reason(e)], span)?;
        }
    }
    Ok(())
}
