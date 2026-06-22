use std::env;

use indexmap::IndexMap;

use yps_lexer::Span;

use crate::error::RuntimeError;
use crate::interpreter::Interpreter;
use crate::stdlib::{as_string, builtin, object_of};
use crate::value::Value;

pub fn build_object() -> Value {
    object_of(&[
        ("аргументы", args_value()),
        ("переменные", env_value()),
        ("рабочаяПапка", cwd_value()),
        ("выход", builtin("Процесс.выход")),
        ("сменитьПапку", builtin("Процесс.сменитьПапку")),
        ("перем", builtin("Процесс.перем")),
    ])
}

fn args_value() -> Value {
    let argv: Vec<Value> = env::args().skip(1).map(Value::String).collect();
    Value::array(argv)
}

fn env_value() -> Value {
    let mut map = IndexMap::new();
    for (k, v) in env::vars() {
        map.insert(k, Value::String(v));
    }
    Value::object(map)
}

fn cwd_value() -> Value {
    match env::current_dir() {
        Ok(p) => Value::String(p.to_string_lossy().into_owned()),
        Err(_) => Value::String(String::new()),
    }
}

pub fn call_static(
    _interp: &mut Interpreter,
    method: &str,
    args: Vec<Value>,
    span: Span,
) -> Result<Value, RuntimeError> {
    match method {
        "выход" => {
            let code = match args.first() {
                Some(Value::Number(n)) => *n as i32,
                Some(Value::Undefined) | None => 0,
                Some(other) => {
                    return Err(RuntimeError::new(
                        format!("'Процесс.выход' ожидает число, получено '{}'", other.type_name()),
                        span,
                    ));
                }
            };
            std::process::exit(code);
        }
        "сменитьПапку" => {
            let path = match args.first() {
                Some(v) => as_string(v, span, "Процесс.сменитьПапку")?,
                None => return Err(RuntimeError::new("'Процесс.сменитьПапку' требует путь", span)),
            };
            env::set_current_dir(path)
                .map_err(|e| RuntimeError::new(format!("'Процесс.сменитьПапку' не смогла '{path}': {e}"), span))?;
            Ok(Value::Undefined)
        }
        "перем" => {
            let name = match args.first() {
                Some(v) => as_string(v, span, "Процесс.перем")?,
                None => return Err(RuntimeError::new("'Процесс.перем' требует имя", span)),
            };
            Ok(env::var(name).map(Value::String).unwrap_or(Value::Null))
        }
        _ => Err(RuntimeError::new(format!("У 'Процесс' нет метода '{method}'"), span)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_is_array() {
        match args_value() {
            Value::Array(_) => {}
            other => panic!("ожидался массив, получено {other:?}"),
        }
    }

    #[test]
    fn env_is_object() {
        match env_value() {
            Value::Object(_) => {}
            other => panic!("ожидался объект, получено {other:?}"),
        }
    }

    #[test]
    fn perem_missing_returns_null() {
        let mut interp = Interpreter::new();
        let v = call_static(
            &mut interp,
            "перем",
            vec![Value::String("__yps_definitely_unset_var__".into())],
            Span { start: 0, end: 0 },
        )
        .unwrap();
        assert_eq!(v, Value::Null);
    }

    #[test]
    fn perem_existing_returns_string() {
        unsafe {
            env::set_var("YPS_TEST_VAR_X", "значение");
        }
        let mut interp = Interpreter::new();
        let v =
            call_static(&mut interp, "перем", vec![Value::String("YPS_TEST_VAR_X".into())], Span { start: 0, end: 0 })
                .unwrap();
        assert_eq!(v, Value::String("значение".into()));
    }
}
