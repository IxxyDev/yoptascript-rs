use indexmap::IndexMap;
use yps_lexer::Span;

use crate::error::RuntimeError;
use crate::interpreter::Interpreter;
use crate::stdlib::{builtin, object_of, require_args};
use crate::value::{MapKey, Value};

pub fn build_static() -> Value {
    object_of(&[("отПар", builtin("Карта.отПар"))])
}

pub fn construct(args: Vec<Value>, span: Span) -> Result<Value, RuntimeError> {
    if args.is_empty() {
        return Ok(Value::map(IndexMap::new()));
    }
    match &args[0] {
        Value::Array(entries) => entries_to_map(&entries.borrow(), span),
        Value::Map(m) => Ok(Value::map(m.borrow().clone())),
        Value::Undefined | Value::Null => Ok(Value::map(IndexMap::new())),
        other => Err(RuntimeError::new(
            format!("'Карта' ожидает массив пар или карту, получено '{}'", other.type_name()),
            span,
        )),
    }
}

pub fn call_static(
    _interp: &mut Interpreter,
    method: &str,
    args: Vec<Value>,
    span: Span,
) -> Result<Value, RuntimeError> {
    match method {
        "отПар" => {
            require_args(&args, 1, span, "Карта.отПар")?;
            match &args[0] {
                Value::Array(entries) => entries_to_map(&entries.borrow(), span),
                Value::Map(m) => Ok(Value::map(m.borrow().clone())),
                other => Err(RuntimeError::new(
                    format!("'Карта.отПар' ожидает массив пар, получено '{}'", other.type_name()),
                    span,
                )),
            }
        }
        _ => Err(RuntimeError::new(format!("У 'Карта' нет метода '{method}'"), span)),
    }
}

pub fn call(
    interp: &mut Interpreter,
    receiver: Value,
    method: &str,
    args: Vec<Value>,
    span: Span,
) -> Result<Value, RuntimeError> {
    let map = match receiver {
        Value::Map(m) => m,
        _ => unreachable!(),
    };
    match method {
        "set" | "поставить" => {
            require_args(&args, 2, span, "set")?;
            let mut iter = args.into_iter();
            let key = iter.next().unwrap();
            let val = iter.next().unwrap();
            map.borrow_mut().insert(MapKey::new(key), val);
            Ok(Value::Map(map))
        }
        "get" | "взять" => {
            require_args(&args, 1, span, "get")?;
            let val = map.borrow().get(&MapKey::new(args[0].clone())).cloned().unwrap_or(Value::Undefined);
            Ok(val)
        }
        "has" | "имеет" => {
            require_args(&args, 1, span, "has")?;
            Ok(Value::Boolean(map.borrow().contains_key(&MapKey::new(args[0].clone()))))
        }
        "delete" | "удалить" => {
            require_args(&args, 1, span, "delete")?;
            let removed = map.borrow_mut().shift_remove(&MapKey::new(args[0].clone())).is_some();
            Ok(Value::Boolean(removed))
        }
        "clear" | "очистить" => {
            map.borrow_mut().clear();
            Ok(Value::Undefined)
        }
        "size" | "размер" => Ok(Value::Number(map.borrow().len() as f64)),
        "keys" | "ключи" => {
            let keys: Vec<Value> = map.borrow().keys().map(|k| k.0.clone()).collect();
            Ok(Value::array(keys))
        }
        "values" | "значения" => {
            let vals: Vec<Value> = map.borrow().values().cloned().collect();
            Ok(Value::array(vals))
        }
        "entries" | "записи" => {
            let pairs: Vec<Value> =
                map.borrow().iter().map(|(k, v)| Value::array(vec![k.0.clone(), v.clone()])).collect();
            Ok(Value::array(pairs))
        }
        "getOrInsert" | "взятьИлиВставить" => {
            require_args(&args, 2, span, "getOrInsert")?;
            let mut iter = args.into_iter();
            let key = iter.next().unwrap();
            let default = iter.next().unwrap();
            let mut borrowed = map.borrow_mut();
            let val = borrowed.entry(MapKey::new(key)).or_insert(default).clone();
            Ok(val)
        }
        "getOrInsertComputed" | "взятьИлиВычислить" => {
            require_args(&args, 2, span, "getOrInsertComputed")?;
            let mut iter = args.into_iter();
            let key = iter.next().unwrap();
            let callback = iter.next().unwrap();
            if let Some(existing) = map.borrow().get(&MapKey::new(key.clone())) {
                return Ok(existing.clone());
            }
            let computed = interp.call_function(callback, vec![key.clone()], span)?;
            map.borrow_mut().entry(MapKey::new(key)).or_insert(computed.clone());
            Ok(computed)
        }
        "forEach" | "каждый" => {
            require_args(&args, 1, span, "forEach")?;
            let callback = args.into_iter().next().unwrap();
            let snapshot: Vec<(Value, Value)> = map.borrow().iter().map(|(k, v)| (k.0.clone(), v.clone())).collect();
            for (k, v) in snapshot {
                interp.call_function(callback.clone(), vec![v, k], span)?;
            }
            Ok(Value::Undefined)
        }
        _ => Err(RuntimeError::new(format!("У карты нет метода '{method}'"), span)),
    }
}

fn entries_to_map(entries: &[Value], span: Span) -> Result<Value, RuntimeError> {
    let mut out: IndexMap<MapKey, Value> = IndexMap::with_capacity(entries.len());
    for entry in entries {
        match entry {
            Value::Array(pair) if pair.borrow().len() >= 2 => {
                let pair = pair.borrow();
                let key = pair[0].clone();
                let val = pair[1].clone();
                out.insert(MapKey::new(key), val);
            }
            _ => return Err(RuntimeError::new("Каждая запись Карты должна быть [ключ, значение]", span)),
        }
    }
    Ok(Value::map(out))
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
    fn map_nan_key_found() {
        assert_eq!(eval("Карта([[нихуя, 42]]).взять(нихуя);"), crate::value::Value::Number(42.0));
    }

    #[test]
    fn map_negative_zero_key_normalized() {
        let key = eval("гыы м = захуярить Карта(); м.set(-0, 1); м.keys()[0];");
        match key {
            crate::value::Value::Number(n) => {
                assert_eq!(n, 0.0);
                assert!(!n.is_sign_negative(), "ключ -0 должен нормализоваться в +0");
            }
            other => panic!("ожидалось число, получено {other:?}"),
        }
    }
}
