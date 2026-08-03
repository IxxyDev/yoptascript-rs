use std::cell::RefCell;
use std::collections::HashMap;
use std::time::Instant;

use yps_lexer::Span;

use crate::error::RuntimeError;
use crate::output::OutputSink;
use crate::value::Value;

thread_local! {
    static TIMERS: RefCell<HashMap<String, Instant>> = RefCell::new(HashMap::new());
}

fn join(args: &[Value]) -> String {
    args.iter().map(Value::to_string).collect::<Vec<_>>().join(" ")
}

pub fn say(sink: &mut dyn OutputSink, args: &[Value]) -> Result<Value, RuntimeError> {
    sink.write_line(&join(args));
    Ok(Value::Undefined)
}

pub fn dispatch(sink: &mut dyn OutputSink, method: &str, args: Vec<Value>, span: Span) -> Result<Value, RuntimeError> {
    match method {
        "ошибка" | "предупреждение" => {
            sink.write_error_line(&join(&args));
            Ok(Value::Undefined)
        }
        "инфо" | "отладка" => {
            sink.write_line(&join(&args));
            Ok(Value::Undefined)
        }
        "таблица" => {
            print_table(sink, &args);
            Ok(Value::Undefined)
        }
        "время" => start_timer(&label_of(&args), span),
        "времяСтоп" => stop_timer(sink, &label_of(&args), span),
        _ => Err(RuntimeError::new(format!("У 'сказать' нет метода '{method}'"), span)),
    }
}

#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
fn start_timer(label: &str, _span: Span) -> Result<Value, RuntimeError> {
    let now = Instant::now();
    TIMERS.with(|t| t.borrow_mut().insert(label.to_string(), now));
    Ok(Value::Undefined)
}

#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
fn stop_timer(sink: &mut dyn OutputSink, label: &str, span: Span) -> Result<Value, RuntimeError> {
    let elapsed = TIMERS.with(|t| t.borrow_mut().remove(label));
    match elapsed {
        Some(start) => {
            let ms = start.elapsed().as_secs_f64() * 1000.0;
            sink.write_line(&format!("{label}: {ms} мс"));
            Ok(Value::Undefined)
        }
        None => Err(RuntimeError::new(format!("Таймер '{label}' не запущен"), span)),
    }
}

/// `Instant::now()` паникует на wasm32-unknown-unknown; сказать.время/времяСтоп там
/// недоступны, но перехватываемой ошибкой, как остальные Instant-зависимые пути.
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
fn start_timer(_label: &str, span: Span) -> Result<Value, RuntimeError> {
    Err(RuntimeError::new("'сказать.время' недоступен в браузере", span))
}

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
fn stop_timer(_sink: &mut dyn OutputSink, _label: &str, span: Span) -> Result<Value, RuntimeError> {
    Err(RuntimeError::new("'сказать.времяСтоп' недоступен в браузере", span))
}

fn label_of(args: &[Value]) -> String {
    match args.first() {
        Some(value) => value.to_string(),
        None => "по-умолчанию".to_string(),
    }
}

fn print_table(sink: &mut dyn OutputSink, args: &[Value]) {
    match args.first() {
        Some(Value::Array(items)) => {
            for (i, v) in items.borrow().iter().enumerate() {
                sink.write_line(&format!("{i}\t{v}"));
            }
        }
        Some(Value::Object(map)) => {
            for (k, v) in map.borrow().iter() {
                sink.write_line(&format!("{k}\t{v}"));
            }
        }
        Some(other) => sink.write_line(&other.to_string()),
        None => sink.write_line(""),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::BufferSink;

    #[test]
    fn info_writes_message_and_returns_undefined() {
        let mut sink = BufferSink::new();
        let r = dispatch(&mut sink, "инфо", vec![Value::String("привет".into())], Span { start: 0, end: 0 }).unwrap();
        assert_eq!(r, Value::Undefined);
        assert!(sink.take().contains("привет"));
    }

    #[test]
    fn error_writes_message_and_returns_undefined() {
        let mut sink = BufferSink::new();
        let r = dispatch(&mut sink, "ошибка", vec![Value::String("боль".into())], Span { start: 0, end: 0 }).unwrap();
        assert_eq!(r, Value::Undefined);
        assert!(sink.take().contains("боль"));
    }

    #[test]
    fn time_stop_reports_label_and_returns_undefined() {
        let mut sink = BufferSink::new();
        dispatch(&mut sink, "время", vec![Value::String("метка".into())], Span { start: 0, end: 0 }).unwrap();
        let r =
            dispatch(&mut sink, "времяСтоп", vec![Value::String("метка".into())], Span { start: 0, end: 0 }).unwrap();
        assert_eq!(r, Value::Undefined);
        assert!(sink.take().contains("метка"));
    }

    #[test]
    fn time_stop_without_start_errors() {
        let err = dispatch(
            &mut BufferSink::new(),
            "времяСтоп",
            vec![Value::String("неткой".into())],
            Span { start: 0, end: 0 },
        )
        .unwrap_err();
        assert!(err.message.contains("неткой"));
    }

    #[test]
    fn unknown_method_errors() {
        let err = dispatch(&mut BufferSink::new(), "неизвестно", vec![], Span { start: 0, end: 0 }).unwrap_err();
        assert!(err.message.contains("сказать"));
    }
}
