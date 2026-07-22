use yps_lexer::Span;

use crate::error::RuntimeError;
use crate::interpreter::Interpreter;
use crate::stdlib::{as_number, builtin, object_of, require_args};
use crate::value::Value;

pub fn build_object() -> Value {
    object_of(&[
        ("raw", builtin("Строка.raw")),
        ("изСимволов", builtin("Строка.изСимволов")),
        ("fromCharCode", builtin("Строка.fromCharCode")),
        ("изКодовТочек", builtin("Строка.изКодовТочек")),
        ("fromCodePoint", builtin("Строка.fromCodePoint")),
    ])
}

pub fn call_static(
    _interp: &mut Interpreter,
    method: &str,
    args: Vec<Value>,
    span: Span,
) -> Result<Value, RuntimeError> {
    match method {
        "raw" => raw(args, span),
        "изСимволов" | "fromCharCode" => from_char_code(&args, span),
        "изКодовТочек" | "fromCodePoint" => from_code_point(&args, span),
        _ => Err(RuntimeError::new(format!("У 'Строка' нет метода '{method}'"), span)),
    }
}

fn raw(args: Vec<Value>, span: Span) -> Result<Value, RuntimeError> {
    require_args(&args, 1, span, "Строка.raw")?;
    let mut it = args.into_iter();
    let strings = it.next().unwrap();
    let substitutions: Vec<Value> = it.collect();
    let raw_arr = match &strings {
        Value::Object(map) => {
            let m = map.borrow();
            m.get("raw").or_else(|| m.get("сырьё")).cloned()
        }
        _ => None,
    };
    let raw_items = match raw_arr {
        Some(Value::Array(a)) => a.borrow().0.clone(),
        _ => {
            return Err(RuntimeError::new(
                "'Строка.raw' ожидает объект со свойством 'raw' (массив), получено без него",
                span,
            ));
        }
    };
    let mut out = String::new();
    for (i, part) in raw_items.iter().enumerate() {
        out.push_str(&part.to_string());
        if let Some(sub) = substitutions.get(i) {
            out.push_str(&sub.to_string());
        }
    }
    Ok(Value::String(out.into()))
}

fn from_char_code(args: &[Value], span: Span) -> Result<Value, RuntimeError> {
    let mut units = Vec::with_capacity(args.len());
    for a in args {
        let n = as_number(a, span, "Строка.изСимволов")?;
        units.push(n as u16);
    }
    Ok(Value::String(String::from_utf16_lossy(&units).into()))
}

fn from_code_point(args: &[Value], span: Span) -> Result<Value, RuntimeError> {
    let mut out = String::new();
    for a in args {
        let n = as_number(a, span, "Строка.изКодовТочек")?;
        if n < 0.0 || n > 0x10FFFF as f64 || n.fract() != 0.0 {
            return Err(RuntimeError::new(format!("Некорректная кодовая точка: {n}"), span));
        }
        match char::from_u32(n as u32) {
            Some(c) => out.push(c),
            None => return Err(RuntimeError::new(format!("Некорректная кодовая точка: {n}"), span)),
        }
    }
    Ok(Value::String(out.into()))
}
