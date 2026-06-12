use indexmap::IndexSet;
use yps_lexer::Span;

use crate::error::RuntimeError;
use crate::interpreter::Interpreter;
use crate::stdlib::require_args;
use crate::value::{MapKey, Value};

pub fn construct(args: Vec<Value>, span: Span) -> Result<Value, RuntimeError> {
    if args.is_empty() {
        return Ok(Value::set(IndexSet::new()));
    }
    match &args[0] {
        Value::Array(items) => {
            let items = items.borrow();
            let mut out: IndexSet<MapKey> = IndexSet::with_capacity(items.len());
            for v in items.iter() {
                out.insert(MapKey::new(v.clone()));
            }
            Ok(Value::set(out))
        }
        Value::Set(s) => Ok(Value::set(s.borrow().clone())),
        Value::Undefined | Value::Null => Ok(Value::set(IndexSet::new())),
        other => {
            Err(RuntimeError::new(format!("'Набор' ожидает массив или набор, получено '{}'", other.type_name()), span))
        }
    }
}

pub fn call(
    interp: &mut Interpreter,
    receiver: Value,
    method: &str,
    args: Vec<Value>,
    span: Span,
) -> Result<Value, RuntimeError> {
    let set = match receiver {
        Value::Set(s) => s,
        _ => unreachable!(),
    };
    match method {
        "add" | "добавить" => {
            require_args(&args, 1, span, "add")?;
            let val = args.into_iter().next().unwrap();
            set.borrow_mut().insert(MapKey::new(val));
            Ok(Value::Set(set))
        }
        "has" | "имеет" => {
            require_args(&args, 1, span, "has")?;
            Ok(Value::Boolean(set.borrow().contains(&MapKey::new(args[0].clone()))))
        }
        "delete" | "удалить" => {
            require_args(&args, 1, span, "delete")?;
            let removed = set.borrow_mut().shift_remove(&MapKey::new(args[0].clone()));
            Ok(Value::Boolean(removed))
        }
        "clear" | "очистить" => {
            set.borrow_mut().clear();
            Ok(Value::Undefined)
        }
        "size" | "размер" => Ok(Value::Number(set.borrow().len() as f64)),
        "values" | "значения" => {
            let vals: Vec<Value> = set.borrow().iter().map(|k| k.0.clone()).collect();
            Ok(Value::array(vals))
        }
        "forEach" | "каждый" => {
            require_args(&args, 1, span, "forEach")?;
            let callback = args.into_iter().next().unwrap();
            let snapshot: Vec<Value> = set.borrow().iter().map(|k| k.0.clone()).collect();
            for v in snapshot {
                interp.call_function(callback.clone(), vec![v], span)?;
            }
            Ok(Value::Undefined)
        }
        "union" | "объединение" => set_op(&set, args, span, |a, b| a || b),
        "intersection" | "пересечение" => set_op(&set, args, span, |a, b| a && b),
        "difference" | "разница" => {
            require_args(&args, 1, span, "difference")?;
            let other = extract_set_like(&args[0], span)?;
            let result: IndexSet<MapKey> = set.borrow().iter().filter(|v| !other.contains(*v)).cloned().collect();
            Ok(Value::set(result))
        }
        "symmetricDifference" | "симметричнаяРазница" => {
            require_args(&args, 1, span, "symmetricDifference")?;
            let other = extract_set_like(&args[0], span)?;
            let items = set.borrow();
            let mut result: IndexSet<MapKey> = items.iter().filter(|v| !other.contains(*v)).cloned().collect();
            for v in &other {
                if !items.contains(v) {
                    result.insert(v.clone());
                }
            }
            Ok(Value::set(result))
        }
        "isSubsetOf" | "подмножествоОт" => {
            require_args(&args, 1, span, "isSubsetOf")?;
            let other = extract_set_like(&args[0], span)?;
            Ok(Value::Boolean(set.borrow().iter().all(|v| other.contains(v))))
        }
        "isSupersetOf" | "надмножествоОт" => {
            require_args(&args, 1, span, "isSupersetOf")?;
            let other = extract_set_like(&args[0], span)?;
            let items = set.borrow();
            Ok(Value::Boolean(other.iter().all(|v| items.contains(v))))
        }
        "isDisjointFrom" | "непересекаетсяС" => {
            require_args(&args, 1, span, "isDisjointFrom")?;
            let other = extract_set_like(&args[0], span)?;
            Ok(Value::Boolean(!set.borrow().iter().any(|v| other.contains(v))))
        }
        _ => Err(RuntimeError::new(format!("У набора нет метода '{method}'"), span)),
    }
}

fn set_op<F>(
    set: &std::rc::Rc<std::cell::RefCell<IndexSet<MapKey>>>,
    args: Vec<Value>,
    span: Span,
    keep: F,
) -> Result<Value, RuntimeError>
where
    F: Fn(bool, bool) -> bool,
{
    require_args(&args, 1, span, "set операция")?;
    let other = extract_set_like(&args[0], span)?;
    let items = set.borrow();
    let mut result: IndexSet<MapKey> = IndexSet::new();
    for v in items.iter() {
        if keep(true, other.contains(v)) {
            result.insert(v.clone());
        }
    }
    for v in &other {
        if !items.contains(v) && keep(false, true) {
            result.insert(v.clone());
        }
    }
    Ok(Value::set(result))
}

fn extract_set_like(v: &Value, span: Span) -> Result<IndexSet<MapKey>, RuntimeError> {
    match v {
        Value::Set(s) => Ok(s.borrow().clone()),
        Value::Array(a) => {
            let a = a.borrow();
            let mut out: IndexSet<MapKey> = IndexSet::with_capacity(a.len());
            for el in a.iter() {
                out.insert(MapKey::new(el.clone()));
            }
            Ok(out)
        }
        other => Err(RuntimeError::new(format!("Ожидался набор или массив, получен '{}'", other.type_name()), span)),
    }
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
    fn set_dedup_nan() {
        assert_eq!(eval("Набор([нихуя, нихуя]).размер;"), crate::value::Value::Number(1.0));
    }

    #[test]
    fn set_union_nan() {
        assert_eq!(eval("Набор([нихуя]).объединение(Набор([нихуя])).размер;"), crate::value::Value::Number(1.0));
    }

    #[test]
    fn set_difference_nan() {
        assert_eq!(eval("Набор([нихуя]).разница(Набор([нихуя])).размер;"), crate::value::Value::Number(0.0));
    }

    #[test]
    fn set_negative_zero_value_normalized() {
        let v = eval("гыы с = захуярить Набор(); с.add(-0); с.значения()[0];");
        match v {
            crate::value::Value::Number(n) => {
                assert_eq!(n, 0.0);
                assert!(!n.is_sign_negative(), "значение -0 должно нормализоваться в +0");
            }
            other => panic!("ожидалось число, получено {other:?}"),
        }
    }
}
