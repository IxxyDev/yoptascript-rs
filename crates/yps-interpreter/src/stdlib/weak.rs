use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use yps_lexer::Span;

use crate::error::RuntimeError;
use crate::interpreter::Interpreter;
use crate::stdlib::require_args;
use crate::value::{FinRegEntry, FinRegState, Value, WeakKey, WeakMapStore, WeakSetStore};

fn weak_key(value: &Value, who: &str, span: Span) -> Result<WeakKey, RuntimeError> {
    WeakKey::try_from_value(value).ok_or_else(|| {
        RuntimeError::new(
            format!("'{who}' ожидает объект, массив, карту или набор, получено '{}'", value.type_name()),
            span,
        )
    })
}

pub fn construct_weak_map(args: Vec<Value>, span: Span) -> Result<Value, RuntimeError> {
    let store: HashMap<usize, (WeakKey, Value)> = HashMap::new();
    let store = Rc::new(RefCell::new(store));
    match args.into_iter().next() {
        None | Some(Value::Undefined) | Some(Value::Null) => {}
        Some(Value::Array(entries)) => {
            let bad_pair = || RuntimeError::new("Каждая запись СлабойКарты должна быть [ключ, значение]", span);
            for entry in entries.borrow().iter() {
                let Value::Array(pair) = entry else {
                    return Err(bad_pair());
                };
                let pair = pair.borrow();
                if pair.len() != 2 {
                    return Err(bad_pair());
                }
                let key = weak_key(&pair[0], "СлабаяКарта", span)?;
                store.borrow_mut().insert(key.ptr(), (key, pair[1].clone()));
            }
        }
        Some(other) => {
            return Err(RuntimeError::new(
                format!("'СлабаяКарта' ожидает массив пар, получено '{}'", other.type_name()),
                span,
            ));
        }
    }
    Ok(Value::WeakMap(store))
}

pub fn construct_weak_set(args: Vec<Value>, span: Span) -> Result<Value, RuntimeError> {
    let store: HashMap<usize, WeakKey> = HashMap::new();
    let store = Rc::new(RefCell::new(store));
    match args.into_iter().next() {
        None | Some(Value::Undefined) | Some(Value::Null) => {}
        Some(Value::Array(items)) => {
            for item in items.borrow().iter() {
                let key = weak_key(item, "СлабыйНабор", span)?;
                store.borrow_mut().insert(key.ptr(), key);
            }
        }
        Some(other) => {
            return Err(RuntimeError::new(
                format!("'СлабыйНабор' ожидает массив, получено '{}'", other.type_name()),
                span,
            ));
        }
    }
    Ok(Value::WeakSet(store))
}

pub fn construct_weak_ref(args: Vec<Value>, span: Span) -> Result<Value, RuntimeError> {
    require_args(&args, 1, span, "СлабаяСсылка")?;
    let key = weak_key(&args[0], "СлабаяСсылка", span)?;
    Ok(Value::WeakRef(Rc::new(key)))
}

pub fn construct_registry(interp: &mut Interpreter, args: Vec<Value>, span: Span) -> Result<Value, RuntimeError> {
    require_args(&args, 1, span, "РеестрФинализации")?;
    let callback = args.into_iter().next().unwrap();
    if !callback.is_callable() {
        return Err(RuntimeError::new(
            format!("'РеестрФинализации' ожидает функцию, получено '{}'", callback.type_name()),
            span,
        ));
    }
    let state = Rc::new(RefCell::new(FinRegState { callback, entries: Vec::new() }));
    interp.register_finalization_registry(&state);
    Ok(Value::FinalizationRegistry(state))
}

fn prune_map(store: &WeakMapStore) {
    store.borrow_mut().retain(|_, (key, _)| key.is_alive());
}

fn prune_set(store: &WeakSetStore) {
    store.borrow_mut().retain(|_, key| key.is_alive());
}

pub fn call_weak_map(receiver: Value, method: &str, args: Vec<Value>, span: Span) -> Result<Value, RuntimeError> {
    let store = match receiver {
        Value::WeakMap(store) => store,
        _ => unreachable!(),
    };
    prune_map(&store);
    match method {
        "set" | "поставить" => {
            require_args(&args, 2, span, "СлабаяКарта.поставить")?;
            let key = weak_key(&args[0], "СлабаяКарта.поставить", span)?;
            store.borrow_mut().insert(key.ptr(), (key, args[1].clone()));
            Ok(Value::WeakMap(store))
        }
        "get" | "взять" => {
            require_args(&args, 1, span, "СлабаяКарта.взять")?;
            let value = WeakKey::try_from_value(&args[0])
                .and_then(|key| store.borrow().get(&key.ptr()).map(|(_, v)| v.clone()))
                .unwrap_or(Value::Undefined);
            Ok(value)
        }
        "has" | "имеет" => {
            require_args(&args, 1, span, "СлабаяКарта.имеет")?;
            let found = WeakKey::try_from_value(&args[0]).is_some_and(|key| store.borrow().contains_key(&key.ptr()));
            Ok(Value::Boolean(found))
        }
        "delete" | "удалить" => {
            require_args(&args, 1, span, "СлабаяКарта.удалить")?;
            let removed =
                WeakKey::try_from_value(&args[0]).is_some_and(|key| store.borrow_mut().remove(&key.ptr()).is_some());
            Ok(Value::Boolean(removed))
        }
        _ => Err(RuntimeError::new(format!("У слабой карты нет метода '{method}'"), span)),
    }
}

pub fn call_weak_set(receiver: Value, method: &str, args: Vec<Value>, span: Span) -> Result<Value, RuntimeError> {
    let store = match receiver {
        Value::WeakSet(store) => store,
        _ => unreachable!(),
    };
    prune_set(&store);
    match method {
        "add" | "добавить" => {
            require_args(&args, 1, span, "СлабыйНабор.добавить")?;
            let key = weak_key(&args[0], "СлабыйНабор.добавить", span)?;
            store.borrow_mut().insert(key.ptr(), key);
            Ok(Value::WeakSet(store))
        }
        "has" | "имеет" => {
            require_args(&args, 1, span, "СлабыйНабор.имеет")?;
            let found = WeakKey::try_from_value(&args[0]).is_some_and(|key| store.borrow().contains_key(&key.ptr()));
            Ok(Value::Boolean(found))
        }
        "delete" | "удалить" => {
            require_args(&args, 1, span, "СлабыйНабор.удалить")?;
            let removed =
                WeakKey::try_from_value(&args[0]).is_some_and(|key| store.borrow_mut().remove(&key.ptr()).is_some());
            Ok(Value::Boolean(removed))
        }
        _ => Err(RuntimeError::new(format!("У слабого набора нет метода '{method}'"), span)),
    }
}

pub fn call_weak_ref(receiver: Value, method: &str, args: Vec<Value>, span: Span) -> Result<Value, RuntimeError> {
    let key = match receiver {
        Value::WeakRef(key) => key,
        _ => unreachable!(),
    };
    match method {
        "deref" | "разыменовать" => {
            require_args(&args, 0, span, "СлабаяСсылка.разыменовать")?;
            Ok(key.upgrade().unwrap_or(Value::Undefined))
        }
        _ => Err(RuntimeError::new(format!("У слабой ссылки нет метода '{method}'"), span)),
    }
}

pub fn call_registry(receiver: Value, method: &str, args: Vec<Value>, span: Span) -> Result<Value, RuntimeError> {
    let state = match receiver {
        Value::FinalizationRegistry(state) => state,
        _ => unreachable!(),
    };
    match method {
        "register" | "зарегистрировать" => {
            require_args(&args, 1, span, "РеестрФинализации.зарегистрировать")?;
            let target = weak_key(&args[0], "РеестрФинализации.зарегистрировать", span)?;
            let held = args.get(1).cloned().unwrap_or(Value::Undefined);
            let token = match args.get(2) {
                None | Some(Value::Undefined) => None,
                Some(value) => Some(weak_key(value, "РеестрФинализации.зарегистрировать", span)?),
            };
            state.borrow_mut().entries.push(FinRegEntry { target, held, token });
            Ok(Value::Undefined)
        }
        "unregister" | "снять" => {
            require_args(&args, 1, span, "РеестрФинализации.снять")?;
            let token = weak_key(&args[0], "РеестрФинализации.снять", span)?;
            let ptr = token.ptr();
            let mut state = state.borrow_mut();
            let before = state.entries.len();
            state.entries.retain(|entry| entry.token.as_ref().is_none_or(|t| t.ptr() != ptr));
            Ok(Value::Boolean(state.entries.len() != before))
        }
        _ => Err(RuntimeError::new(format!("У реестра финализации нет метода '{method}'"), span)),
    }
}
