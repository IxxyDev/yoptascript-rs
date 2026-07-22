use std::fs;
use std::path::Path;

use yps_lexer::Span;

use crate::error::RuntimeError;
use crate::interpreter::Interpreter;
use crate::stdlib::{as_string, builtin, object_of, require_args};
use crate::value::Value;

pub fn build_object() -> Value {
    object_of(&[
        ("прочитать", builtin("ФС.прочитать")),
        ("записать", builtin("ФС.записать")),
        ("дописать", builtin("ФС.дописать")),
        ("удалить", builtin("ФС.удалить")),
        ("существует", builtin("ФС.существует")),
        ("этоПапка", builtin("ФС.этоПапка")),
        ("этоФайл", builtin("ФС.этоФайл")),
        ("список", builtin("ФС.список")),
        ("создатьПапку", builtin("ФС.создатьПапку")),
        ("удалитьПапку", builtin("ФС.удалитьПапку")),
    ])
}

pub fn call_static(
    _interp: &mut Interpreter,
    method: &str,
    args: Vec<Value>,
    span: Span,
) -> Result<Value, RuntimeError> {
    match method {
        "прочитать" => {
            require_args(&args, 1, span, "ФС.прочитать")?;
            let path = as_string(&args[0], span, "ФС.прочитать")?;
            fs::read_to_string(path).map(Value::string).map_err(|e| io_err("ФС.прочитать", path, e, span))
        }
        "записать" => {
            require_args(&args, 2, span, "ФС.записать")?;
            let path = as_string(&args[0], span, "ФС.записать")?;
            let body = as_string(&args[1], span, "ФС.записать")?;
            fs::write(path, body).map(|()| Value::Undefined).map_err(|e| io_err("ФС.записать", path, e, span))
        }
        "дописать" => {
            use std::io::Write;
            require_args(&args, 2, span, "ФС.дописать")?;
            let path = as_string(&args[0], span, "ФС.дописать")?;
            let body = as_string(&args[1], span, "ФС.дописать")?;
            let mut file = fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .map_err(|e| io_err("ФС.дописать", path, e, span))?;
            file.write_all(body.as_bytes()).map_err(|e| io_err("ФС.дописать", path, e, span))?;
            Ok(Value::Undefined)
        }
        "удалить" => {
            require_args(&args, 1, span, "ФС.удалить")?;
            let path = as_string(&args[0], span, "ФС.удалить")?;
            fs::remove_file(path).map(|()| Value::Undefined).map_err(|e| io_err("ФС.удалить", path, e, span))
        }
        "существует" => {
            require_args(&args, 1, span, "ФС.существует")?;
            let path = as_string(&args[0], span, "ФС.существует")?;
            Ok(Value::Boolean(Path::new(path).exists()))
        }
        "этоПапка" => {
            require_args(&args, 1, span, "ФС.этоПапка")?;
            let path = as_string(&args[0], span, "ФС.этоПапка")?;
            Ok(Value::Boolean(Path::new(path).is_dir()))
        }
        "этоФайл" => {
            require_args(&args, 1, span, "ФС.этоФайл")?;
            let path = as_string(&args[0], span, "ФС.этоФайл")?;
            Ok(Value::Boolean(Path::new(path).is_file()))
        }
        "список" => {
            require_args(&args, 1, span, "ФС.список")?;
            let path = as_string(&args[0], span, "ФС.список")?;
            let entries = fs::read_dir(path).map_err(|e| io_err("ФС.список", path, e, span))?;
            let mut out = Vec::new();
            for entry in entries {
                let entry = entry.map_err(|e| io_err("ФС.список", path, e, span))?;
                out.push(Value::String(entry.file_name().to_string_lossy().into_owned().into()));
            }
            Ok(Value::array(out))
        }
        "создатьПапку" => {
            require_args(&args, 1, span, "ФС.создатьПапку")?;
            let path = as_string(&args[0], span, "ФС.создатьПапку")?;
            fs::create_dir_all(path).map(|()| Value::Undefined).map_err(|e| io_err("ФС.создатьПапку", path, e, span))
        }
        "удалитьПапку" => {
            require_args(&args, 1, span, "ФС.удалитьПапку")?;
            let path = as_string(&args[0], span, "ФС.удалитьПапку")?;
            fs::remove_dir_all(path).map(|()| Value::Undefined).map_err(|e| io_err("ФС.удалитьПапку", path, e, span))
        }
        _ => Err(RuntimeError::new(format!("У 'ФС' нет метода '{method}'"), span)),
    }
}

fn io_err(ctx: &str, path: &str, err: std::io::Error, span: Span) -> RuntimeError {
    RuntimeError::new(format!("'{ctx}' не смогла обработать '{path}': {err}"), span)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    fn tmp_path(name: &str) -> String {
        let mut p = env::temp_dir();
        p.push(format!("yps_fs_test_{}_{name}", std::process::id()));
        p.to_string_lossy().into_owned()
    }

    fn call(method: &str, args: Vec<Value>) -> Result<Value, RuntimeError> {
        let mut interp = Interpreter::new();
        call_static(&mut interp, method, args, Span { start: 0, end: 0 })
    }

    #[test]
    fn write_then_read_roundtrip() {
        let path = tmp_path("rw.txt");
        let _ = fs::remove_file(&path);
        call("записать", vec![Value::String(path.clone().into()), Value::String("привет".into())]).unwrap();
        let v = call("прочитать", vec![Value::String(path.clone().into())]).unwrap();
        assert_eq!(v, Value::String("привет".into()));
        call("удалить", vec![Value::String(path.into())]).unwrap();
    }

    #[test]
    fn exists_true_and_false() {
        let path = tmp_path("ex.txt");
        let _ = fs::remove_file(&path);
        assert_eq!(call("существует", vec![Value::String(path.clone().into())]).unwrap(), Value::Boolean(false));
        call("записать", vec![Value::String(path.clone().into()), Value::String("".into())]).unwrap();
        assert_eq!(call("существует", vec![Value::String(path.clone().into())]).unwrap(), Value::Boolean(true));
        call("удалить", vec![Value::String(path.into())]).unwrap();
    }

    #[test]
    fn list_returns_array() {
        let dir = tmp_path("list_dir");
        let _ = fs::remove_dir_all(&dir);
        call("создатьПапку", vec![Value::String(dir.clone().into())]).unwrap();
        let file_path = format!("{dir}/один.txt");
        call("записать", vec![Value::String(file_path.into()), Value::String("".into())]).unwrap();
        let v = call("список", vec![Value::String(dir.clone().into())]).unwrap();
        match v {
            Value::Array(items) => {
                assert_eq!(items.borrow().len(), 1);
                assert_eq!(items.borrow()[0], Value::String("один.txt".into()));
            }
            other => panic!("ожидался массив, получено {other:?}"),
        }
        call("удалитьПапку", vec![Value::String(dir.into())]).unwrap();
    }

    #[test]
    fn read_missing_errors() {
        let path = tmp_path("missing.txt");
        let _ = fs::remove_file(&path);
        let err = call("прочитать", vec![Value::String(path.into())]).unwrap_err();
        assert!(err.message.contains("ФС.прочитать"), "msg: {}", err.message);
    }

    #[test]
    fn append_extends_file() {
        let path = tmp_path("app.txt");
        let _ = fs::remove_file(&path);
        call("записать", vec![Value::String(path.clone().into()), Value::String("a".into())]).unwrap();
        call("дописать", vec![Value::String(path.clone().into()), Value::String("b".into())]).unwrap();
        let v = call("прочитать", vec![Value::String(path.clone().into())]).unwrap();
        assert_eq!(v, Value::String("ab".into()));
        call("удалить", vec![Value::String(path.into())]).unwrap();
    }
}
