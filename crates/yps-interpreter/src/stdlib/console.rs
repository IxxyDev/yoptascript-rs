use std::cell::RefCell;
use std::collections::HashMap;
use std::io::Write;
use std::time::Instant;

use yps_lexer::Span;

use crate::error::RuntimeError;
use crate::value::Value;

thread_local! {
    static TIMERS: RefCell<HashMap<String, Instant>> = RefCell::new(HashMap::new());
}

fn join(args: &[Value]) -> String {
    args.iter().map(Value::to_string).collect::<Vec<_>>().join(" ")
}

pub fn dispatch(method: &str, args: Vec<Value>, span: Span) -> Result<Value, RuntimeError> {
    match method {
        "ошибка" | "предупреждение" => {
            let line = join(&args);
            let mut err = std::io::stderr().lock();
            let _ = writeln!(err, "{line}");
            Ok(Value::Undefined)
        }
        "инфо" | "отладка" => {
            println!("{}", join(&args));
            Ok(Value::Undefined)
        }
        "таблица" => {
            print_table(&args);
            Ok(Value::Undefined)
        }
        "время" => {
            let label = label_of(&args);
            TIMERS.with(|t| {
                t.borrow_mut().insert(label, Instant::now());
            });
            Ok(Value::Undefined)
        }
        "времяСтоп" => {
            let label = label_of(&args);
            let elapsed = TIMERS.with(|t| t.borrow_mut().remove(&label));
            match elapsed {
                Some(start) => {
                    let ms = start.elapsed().as_secs_f64() * 1000.0;
                    println!("{label}: {ms} мс");
                    Ok(Value::Undefined)
                }
                None => Err(RuntimeError::new(format!("Таймер '{label}' не запущен"), span)),
            }
        }
        _ => Err(RuntimeError::new(format!("У 'сказать' нет метода '{method}'"), span)),
    }
}

fn label_of(args: &[Value]) -> String {
    match args.first() {
        Some(Value::String(s)) => s.clone(),
        Some(other) => other.to_string(),
        None => "по-умолчанию".to_string(),
    }
}

fn print_table(args: &[Value]) {
    match args.first() {
        Some(Value::Array(items)) => {
            for (i, v) in items.borrow().iter().enumerate() {
                println!("{i}\t{v}");
            }
        }
        Some(Value::Object(map)) => {
            for (k, v) in map.borrow().iter() {
                println!("{k}\t{v}");
            }
        }
        Some(other) => println!("{other}"),
        None => println!(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn info_returns_undefined() {
        let r = dispatch("инфо", vec![Value::String("привет".into())], Span { start: 0, end: 0 }).unwrap();
        assert_eq!(r, Value::Undefined);
    }

    #[test]
    fn error_returns_undefined() {
        let r = dispatch("ошибка", vec![Value::String("боль".into())], Span { start: 0, end: 0 }).unwrap();
        assert_eq!(r, Value::Undefined);
    }

    #[test]
    fn time_then_time_stop() {
        dispatch("время", vec![Value::String("метка".into())], Span { start: 0, end: 0 }).unwrap();
        let r = dispatch("времяСтоп", vec![Value::String("метка".into())], Span { start: 0, end: 0 }).unwrap();
        assert_eq!(r, Value::Undefined);
    }

    #[test]
    fn time_stop_without_start_errors() {
        let err = dispatch("времяСтоп", vec![Value::String("неткой".into())], Span { start: 0, end: 0 }).unwrap_err();
        assert!(err.message.contains("неткой"));
    }

    #[test]
    fn unknown_method_errors() {
        let err = dispatch("неизвестно", vec![], Span { start: 0, end: 0 }).unwrap_err();
        assert!(err.message.contains("сказать"));
    }
}
