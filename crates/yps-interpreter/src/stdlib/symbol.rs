use std::cell::{Cell, RefCell};
use std::collections::HashMap;

use yps_lexer::Span;

use crate::error::RuntimeError;
use crate::interpreter::Interpreter;
use crate::value::Value;

pub const ITERATOR_ID: u64 = 1;
pub const DISPOSE_ID: u64 = 2;
pub const ASYNC_ITERATOR_ID: u64 = 3;

thread_local! {
    static NEXT_ID: Cell<u64> = const { Cell::new(100) };
    static REGISTRY: RefCell<HashMap<String, u64>> = RefCell::new(HashMap::new());
}

fn fresh_id() -> u64 {
    NEXT_ID.with(|c| {
        let id = c.get();
        c.set(id + 1);
        id
    })
}

pub fn well_known(property: &str) -> Option<Value> {
    let (desc, id) = match property {
        "итератор" => ("Symbol.iterator", ITERATOR_ID),
        "расход" => ("Symbol.dispose", DISPOSE_ID),
        "асинхИтератор" => ("Symbol.asyncIterator", ASYNC_ITERATOR_ID),
        _ => return None,
    };
    Some(Value::Symbol { description: Some(desc.to_string()), id })
}

pub fn construct(args: Vec<Value>, _span: Span) -> Result<Value, RuntimeError> {
    let description = match args.into_iter().next() {
        Some(Value::Undefined) | None => None,
        Some(Value::String(s)) => Some(s),
        Some(other) => Some(other.to_string()),
    };
    Ok(Value::Symbol { description, id: fresh_id() })
}

pub fn call_static(
    _interp: &mut Interpreter,
    method: &str,
    args: Vec<Value>,
    span: Span,
) -> Result<Value, RuntimeError> {
    match method {
        "для" => {
            let key = match args.into_iter().next() {
                Some(Value::String(s)) => s,
                Some(other) => other.to_string(),
                None => return Err(RuntimeError::new("'Симбол.для' ожидает ключ", span)),
            };
            let id = REGISTRY.with(|r| {
                let mut map = r.borrow_mut();
                if let Some(id) = map.get(&key) {
                    return *id;
                }
                let id = fresh_id();
                map.insert(key.clone(), id);
                id
            });
            Ok(Value::Symbol { description: Some(key), id })
        }
        "ключДля" => {
            let sym = args.into_iter().next();
            if let Some(Value::Symbol { id, .. }) = sym {
                let key = REGISTRY.with(|r| r.borrow().iter().find(|(_, v)| **v == id).map(|(k, _)| k.clone()));
                Ok(key.map(Value::String).unwrap_or(Value::Undefined))
            } else {
                Err(RuntimeError::new("'Симбол.ключДля' ожидает символ", span))
            }
        }
        _ => Err(RuntimeError::new(format!("Неизвестный статический метод 'Симбол.{method}'"), span)),
    }
}

pub fn call_instance(
    _interp: &mut Interpreter,
    receiver: Value,
    method: &str,
    _args: Vec<Value>,
    span: Span,
) -> Result<(Value, Option<Value>), RuntimeError> {
    let Value::Symbol { description, .. } = &receiver else {
        return Err(RuntimeError::new("Метод вызван не на символе", span));
    };
    match method {
        "вСтроку" => {
            let s = match description {
                Some(d) => format!("Симбол({d})"),
                None => "Симбол()".to_string(),
            };
            Ok((Value::String(s), None))
        }
        _ => Err(RuntimeError::new(format!("Символ не имеет метода '{method}'"), span)),
    }
}

pub fn member(receiver: &Value, property: &str) -> Option<Value> {
    let Value::Symbol { description, .. } = receiver else { return None };
    if property == "описание" {
        return Some(match description {
            Some(d) => Value::String(d.clone()),
            None => Value::Undefined,
        });
    }
    None
}
