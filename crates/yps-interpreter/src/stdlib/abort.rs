use std::cell::RefCell;
use std::mem;
use std::rc::Rc;

use yps_lexer::Span;

use crate::error::RuntimeError;
use crate::interpreter::Interpreter;
use crate::value::{AbortState, CapKind, Value};

pub fn make_controller() -> Value {
    let state = Rc::new(RefCell::new(AbortState {
        aborted: false,
        reason: Value::Undefined,
        next_token: 0,
        listeners: Vec::new(),
        promise: RefCell::new(None),
    }));
    Value::AbortController { state }
}

pub(crate) fn make_abort_error(message: &str) -> Value {
    let mut map = std::collections::HashMap::new();
    map.insert("name".to_string(), Value::String("ОшибкаОтмены".to_string()));
    map.insert("message".to_string(), Value::String(message.to_string()));
    Value::object(map)
}

pub fn signal_any(interp: &mut Interpreter, sigs: Vec<Value>, span: Span) -> Result<Value, RuntimeError> {
    let ctrl_state = Rc::new(RefCell::new(AbortState {
        aborted: false,
        reason: Value::Undefined,
        next_token: 0,
        listeners: Vec::new(),
        promise: RefCell::new(None),
    }));
    let result_signal = Value::AbortSignal { state: Rc::clone(&ctrl_state) };

    for sig in sigs {
        let sig_state = match &sig {
            Value::AbortSignal { state } => Rc::clone(state),
            other => {
                return Err(RuntimeError::new(
                    format!("'СигналОтмены.любой' ожидает массив сигналов, получено '{}'", other.type_name()),
                    span,
                ));
            }
        };

        if sig_state.borrow().aborted {
            let reason = sig_state.borrow().reason.clone();
            abort_state(&ctrl_state, reason, interp, span)?;
            return Ok(result_signal);
        }

        let listener = Value::AbortListener { target: Rc::clone(&ctrl_state) };
        let id = {
            let mut st = sig_state.borrow_mut();
            let id = st.next_token;
            st.next_token += 1;
            st.listeners.push((id, listener));
            id
        };
        let _ = id;
    }

    Ok(result_signal)
}

pub(crate) fn abort_state(
    state: &Rc<RefCell<AbortState>>,
    reason: Value,
    interp: &mut Interpreter,
    span: Span,
) -> Result<(), RuntimeError> {
    {
        let mut st = state.borrow_mut();
        if st.aborted {
            return Ok(());
        }
        st.aborted = true;
        st.reason = reason;
    }
    let listeners = mem::take(&mut state.borrow_mut().listeners);
    for (_id, cb) in listeners {
        fire_listener(cb, state, interp, span)?;
    }
    Ok(())
}

fn fire_listener(
    cb: Value,
    source: &Rc<RefCell<AbortState>>,
    interp: &mut Interpreter,
    span: Span,
) -> Result<(), RuntimeError> {
    match cb {
        Value::Undefined => {}
        Value::AbortListener { target } => {
            let reason = source.borrow().reason.clone();
            if let Err(e) = abort_state(&target, reason, interp, span) {
                eprintln!("необработанное исключение в 'отмена': {}", e.message);
            }
        }
        Value::AbortCancelTimer { timer_id } => {
            interp.cancel_macrotask(timer_id);
        }
        Value::AbortRejectPromise { reject_cap, reason_from_signal } => {
            let reason = if reason_from_signal {
                source.borrow().reason.clone()
            } else {
                make_abort_error("Операция отменена")
            };
            if let Value::PromiseCapability { state, kind: CapKind::Reject } = *reject_cap
                && let Err(e) = Interpreter::settle_promise(&state, CapKind::Reject, reason, interp, span)
            {
                eprintln!("необработанное исключение в 'отмена': {}", e.message);
            }
        }
        other => {
            if let Err(e) = interp.call_function(other, vec![], span) {
                eprintln!("необработанное исключение в 'отмена': {}", e.message);
            }
        }
    }
    Ok(())
}

fn get_or_init_signal_promise(state: &Rc<RefCell<AbortState>>) -> Value {
    {
        let st = state.borrow();
        if let Some(v) = st.promise.borrow().as_ref() {
            return v.clone();
        }
    }
    let aborted_now = state.borrow().aborted;
    if aborted_now {
        let reason = state.borrow().reason.clone();
        let prom = Interpreter::make_rejected_promise(reason);
        *state.borrow().promise.borrow_mut() = Some(prom.clone());
        return prom;
    }
    let (promise_value, _resolve_cap, reject_cap) = Interpreter::make_pending_promise();
    let listener = Value::AbortRejectPromise { reject_cap: Box::new(reject_cap), reason_from_signal: true };
    {
        let mut st = state.borrow_mut();
        let id = st.next_token;
        st.next_token += 1;
        st.listeners.push((id, listener));
    }
    *state.borrow().promise.borrow_mut() = Some(promise_value.clone());
    promise_value
}

pub fn make_timeout_signal(interp: &mut Interpreter, ms: u64) -> Value {
    let state = Rc::new(RefCell::new(AbortState {
        aborted: false,
        reason: Value::Undefined,
        next_token: 0,
        listeners: Vec::new(),
        promise: RefCell::new(None),
    }));
    let signal = Value::AbortSignal { state: Rc::clone(&state) };
    let state_for_task = Rc::clone(&state);
    interp.schedule_macrotask(
        std::time::Duration::from_millis(ms),
        Box::new(move |interp, sp| {
            let reason = make_abort_error("Тайм-аут");
            if let Err(e) = abort_state(&state_for_task, reason, interp, sp) {
                eprintln!("необработанное исключение в 'отВремени': {}", e.message);
            }
            Ok(())
        }),
    );
    signal
}

pub fn subscribe_timer_cancel(state: &Rc<RefCell<AbortState>>, timer_id: u64) {
    let mut st = state.borrow_mut();
    let id = st.next_token;
    st.next_token += 1;
    st.listeners.push((id, Value::AbortCancelTimer { timer_id }));
}

pub fn call(
    interp: &mut Interpreter,
    receiver: Value,
    method: &str,
    args: Vec<Value>,
    span: Span,
) -> Result<(Value, Option<Value>), RuntimeError> {
    match &receiver {
        Value::AbortController { state } => {
            let state = Rc::clone(state);
            match method {
                "отменить" => {
                    let reason = args.into_iter().next().unwrap_or(Value::Undefined);
                    abort_state(&state, reason, interp, span)?;
                    Ok((Value::Undefined, None))
                }
                _ => Err(RuntimeError::new(format!("У 'КонтроллёрОтмены' нет метода '{method}'"), span)),
            }
        }
        Value::AbortSignal { state } => {
            let state = Rc::clone(state);
            match method {
                "подписатьсяНаОтмену" => {
                    let cb = args.into_iter().next().unwrap_or(Value::Undefined);
                    if !matches!(cb, Value::Function { .. } | Value::BuiltinFunction(_)) {
                        return Err(RuntimeError::new(
                            format!("'подписатьсяНаОтмену' ожидает функцию, получено '{}'", cb.type_name()),
                            span,
                        ));
                    }

                    let already_aborted = state.borrow().aborted;
                    if already_aborted {
                        let cb_clone = cb.clone();
                        interp.enqueue_microtask(Box::new(move |interp, sp| {
                            interp.call_function(cb_clone, vec![], sp).map(|_| ())
                        }));
                        let noop = Value::AbortUnsubscribe { state: Rc::clone(&state), token: u64::MAX };
                        return Ok((noop, None));
                    }

                    let id = {
                        let mut st = state.borrow_mut();
                        let id = st.next_token;
                        st.next_token += 1;
                        st.listeners.push((id, cb));
                        id
                    };

                    let unsubscribe = Value::AbortUnsubscribe { state: Rc::clone(&state), token: id };
                    Ok((unsubscribe, None))
                }
                "выкинутьЕслиОтменён" => {
                    let (aborted, reason) = {
                        let st = state.borrow();
                        (st.aborted, st.reason.clone())
                    };
                    if aborted { Err(RuntimeError::thrown(reason, span)) } else { Ok((Value::Undefined, None)) }
                }
                _ => Err(RuntimeError::new(format!("У 'СигналОтмены' нет метода '{method}'"), span)),
            }
        }
        _ => unreachable!(),
    }
}

pub fn get_property(receiver: &Value, prop: &str) -> Option<Value> {
    match receiver {
        Value::AbortController { state } => match prop {
            "сигнал" => Some(Value::AbortSignal { state: Rc::clone(state) }),
            _ => None,
        },
        Value::AbortSignal { state } => match prop {
            "отменён" => Some(Value::Boolean(state.borrow().aborted)),
            "причина" => Some(state.borrow().reason.clone()),
            "обещание" => Some(get_or_init_signal_promise(state)),
            _ => None,
        },
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interpreter::Interpreter;

    fn sp() -> Span {
        Span { start: 0, end: 0 }
    }

    fn run_script(src: &str) -> Interpreter {
        let source = yps_lexer::SourceFile::new("test".to_string(), src.to_string());
        let (tokens, _) = yps_lexer::Lexer::new(&source).tokenize();
        let (program, _) = yps_parser::Parser::new(&tokens, &source).parse_program();
        let mut interp = Interpreter::new();
        interp.run(&program).unwrap();
        interp
    }

    #[test]
    fn t1_abort_idempotent() {
        let ctrl_val = make_controller();
        let state = match &ctrl_val {
            Value::AbortController { state } => Rc::clone(state),
            _ => unreachable!(),
        };
        let mut interp = Interpreter::new();
        abort_state(&state, Value::String("первая".into()), &mut interp, sp()).unwrap();
        abort_state(&state, Value::String("вторая".into()), &mut interp, sp()).unwrap();
        assert_eq!(state.borrow().reason, Value::String("первая".into()));
    }

    #[test]
    fn t2_listener_fire_and_clear() {
        let ctrl_val = make_controller();
        let state = match &ctrl_val {
            Value::AbortController { state } => Rc::clone(state),
            _ => unreachable!(),
        };
        state.borrow_mut().listeners.push((0, Value::Undefined));
        let mut interp = Interpreter::new();
        abort_state(&state, Value::Undefined, &mut interp, sp()).unwrap();
        assert!(state.borrow().listeners.is_empty());
    }

    #[test]
    fn t3_late_subscribe_via_microtask() {
        let interp = run_script(
            r#"
            гыы контроллёр = захуярить КонтроллёрОтмены();
            контроллёр.отменить("ок");
            гыы вызван = лож;
            контроллёр.сигнал.подписатьсяНаОтмену(() => { вызван = правда; });
        "#,
        );
        assert_eq!(interp.get("вызван"), Some(Value::Boolean(true)));
    }

    #[test]
    fn t4_token_unsubscribe() {
        let ctrl_val = make_controller();
        let state = match &ctrl_val {
            Value::AbortController { state } => Rc::clone(state),
            _ => unreachable!(),
        };
        let id = {
            let mut st = state.borrow_mut();
            let id = st.next_token;
            st.next_token += 1;
            st.listeners.push((id, Value::Undefined));
            id
        };
        assert_eq!(state.borrow().listeners.len(), 1);
        state.borrow_mut().listeners.retain(|(tid, _)| *tid != id);
        assert!(state.borrow().listeners.is_empty());
    }

    #[test]
    fn t4b_script_unsubscribe_prevents_callback() {
        let interp = run_script(
            r#"
            гыы контроллёр = захуярить КонтроллёрОтмены();
            гыы вызван = лож;
            гыы отписка = контроллёр.сигнал.подписатьсяНаОтмену(() => { вызван = правда; });
            отписка();
            контроллёр.отменить();
        "#,
        );
        assert_eq!(interp.get("вызван"), Some(Value::Boolean(false)));
    }

    #[test]
    fn t5_signal_any_fires_on_first_abort() {
        let interp = run_script(
            r#"
            гыы а = захуярить КонтроллёрОтмены();
            гыы б = захуярить КонтроллёрОтмены();
            гыы комбо = СигналОтмены.любой([а.сигнал, б.сигнал]);
            гыы вызван = лож;
            комбо.подписатьсяНаОтмену(() => { вызван = правда; });
            а.отменить("причина_а");
        "#,
        );
        assert_eq!(interp.get("вызван"), Some(Value::Boolean(true)));
    }

    #[test]
    fn t6_timer_cancelled_by_signal() {
        let interp = run_script(
            r#"
            гыы контроллёр = захуярить КонтроллёрОтмены();
            гыы сработал = лож;
            чутка(() => { сработал = правда; }, 50, { сигнал: контроллёр.сигнал });
            контроллёр.отменить("стоп");
        "#,
        );
        assert_eq!(interp.get("сработал"), Some(Value::Boolean(false)));
    }

    #[test]
    fn t7_interval_stops_after_abort() {
        let interp = run_script(
            r#"
            гыы контроллёр = захуярить КонтроллёрОтмены();
            гыы счёт = 0;
            интервал(() => { счёт = счёт + 1; контроллёр.отменить(); }, 5, { сигнал: контроллёр.сигнал });
        "#,
        );
        assert_eq!(interp.get("счёт"), Some(Value::Number(1.0)));
    }

    #[test]
    fn t8_preaborted_signal_never_fires_timer() {
        let interp = run_script(
            r#"
            гыы контроллёр = захуярить КонтроллёрОтмены();
            контроллёр.отменить("заранее");
            гыы сработал = лож;
            чутка(() => { сработал = правда; }, 10, { сигнал: контроллёр.сигнал });
        "#,
        );
        assert_eq!(interp.get("сработал"), Some(Value::Boolean(false)));
    }

    #[test]
    fn typeof_and_identity() {
        let interp = run_script(
            r#"
            гыы контроллёр = захуярить КонтроллёрОтмены();
            гыы тк = тип(контроллёр);
            гыы тс = тип(контроллёр.сигнал);
            гыы равенство_контроллёра = контроллёр === контроллёр;
            гыы равенство_сигнала = контроллёр.сигнал === контроллёр.сигнал;
        "#,
        );
        assert_eq!(interp.get("тк"), Some(Value::String("контроллёрОтмены".into())));
        assert_eq!(interp.get("тс"), Some(Value::String("сигналОтмены".into())));
        assert_eq!(interp.get("равенство_контроллёра"), Some(Value::Boolean(true)));
        assert_eq!(interp.get("равенство_сигнала"), Some(Value::Boolean(true)));
    }

    #[test]
    fn throw_if_aborted_propagates_value_reason() {
        let interp = run_script(
            r#"
            гыы контроллёр = захуярить КонтроллёрОтмены();
            контроллёр.отменить({ код: 42 });
            гыы пойман = ноль;
            хапнуть {
                контроллёр.сигнал.выкинутьЕслиОтменён();
            } гоп(е) {
                пойман = е;
            }
        "#,
        );
        let caught = interp.get("пойман").unwrap();
        match caught {
            Value::Object(map) => {
                assert_eq!(map.borrow().get("код"), Some(&Value::Number(42.0)));
            }
            other => panic!("ожидался объект, получено {other:?}"),
        }
    }

    #[test]
    fn t5_signal_any_empty_never_aborts() {
        let interp = run_script(
            r#"
            гыы комбо = СигналОтмены.любой([]);
            гыы вызван = лож;
            комбо.подписатьсяНаОтмену(() => { вызван = правда; });
        "#,
        );
        assert_eq!(interp.get("вызван"), Some(Value::Boolean(false)));
    }
}
