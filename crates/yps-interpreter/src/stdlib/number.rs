use yps_lexer::Span;

use crate::error::RuntimeError;
use crate::interpreter::Interpreter;
use crate::stdlib::{as_number, builtin, object_of, require_args};
use crate::value::Value;

pub fn build_object() -> Value {
    object_of(&[
        ("конечна", builtin("Хуйня.конечна")),
        ("целая", builtin("Хуйня.целая")),
        ("нихуя", builtin("Хуйня.нихуя")),
        ("разобратьЦелое", builtin("Хуйня.разобратьЦелое")),
        ("разобратьЧисло", builtin("Хуйня.разобратьЧисло")),
        ("МАКС", Value::Number(f64::MAX)),
        ("МИН", Value::Number(f64::MIN_POSITIVE)),
        ("БЕСКОНЕЧНОСТЬ", Value::Number(f64::INFINITY)),
        ("НЕЧИСЛО", Value::Number(f64::NAN)),
    ])
}

pub fn call_static(
    _interp: &mut Interpreter,
    method: &str,
    args: Vec<Value>,
    span: Span,
) -> Result<Value, RuntimeError> {
    match method {
        "конечна" => {
            require_args(&args, 1, span, "Хуйня.конечна")?;
            Ok(Value::Boolean(matches!(&args[0], Value::Number(n) if n.is_finite())))
        }
        "целая" => {
            require_args(&args, 1, span, "Хуйня.целая")?;
            Ok(Value::Boolean(matches!(&args[0], Value::Number(n) if n.is_finite() && n.fract() == 0.0)))
        }
        "нихуя" => {
            require_args(&args, 1, span, "Хуйня.нихуя")?;
            Ok(Value::Boolean(matches!(&args[0], Value::Number(n) if n.is_nan())))
        }
        "разобратьЦелое" => {
            require_args(&args, 1, span, "Хуйня.разобратьЦелое")?;
            let s = match &args[0] {
                Value::String(s) => s.clone(),
                Value::Number(n) => return Ok(Value::Number(n.trunc())),
                other => return Ok(Value::Number(coerce_to_f64(other).map(|n| n.trunc()).unwrap_or(f64::NAN))),
            };
            let radix = if args.len() > 1 {
                match &args[1] {
                    Value::Number(n) => *n as u32,
                    _ => 10,
                }
            } else {
                10
            };
            Ok(parse_int(&s, radix))
        }
        "разобратьЧисло" => {
            require_args(&args, 1, span, "Хуйня.разобратьЧисло")?;
            let s = match &args[0] {
                Value::String(s) => s.clone(),
                Value::Number(n) => return Ok(Value::Number(*n)),
                _ => return Ok(Value::Number(f64::NAN)),
            };
            Ok(parse_float(&s))
        }
        _ => Err(RuntimeError::new(format!("У 'Хуйня' нет метода '{method}'"), span)),
    }
}

fn coerce_to_f64(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => Some(*n),
        Value::Boolean(b) => Some(if *b { 1.0 } else { 0.0 }),
        Value::String(s) => s.trim().parse::<f64>().ok(),
        _ => None,
    }
}

fn parse_int(s: &str, radix: u32) -> Value {
    let trimmed = s.trim_start();
    if trimmed.is_empty() {
        return Value::Number(f64::NAN);
    }
    let (sign, rest) = match trimmed.chars().next() {
        Some('+') => (1.0, &trimmed[1..]),
        Some('-') => (-1.0, &trimmed[1..]),
        _ => (1.0, trimmed),
    };
    let mut chars = rest.chars();
    let mut value: i128 = 0;
    let mut consumed = false;
    while let Some(c) = chars.clone().next() {
        let digit = c.to_digit(radix);
        match digit {
            Some(d) => {
                value = value * radix as i128 + d as i128;
                consumed = true;
                chars.next();
            }
            None => break,
        }
    }
    if !consumed {
        return Value::Number(f64::NAN);
    }
    Value::Number(sign * value as f64)
}

fn parse_float(s: &str) -> Value {
    let trimmed = s.trim_start();
    let bytes = trimmed.as_bytes();
    let mut i = 0;
    if i < bytes.len() && (bytes[i] == b'+' || bytes[i] == b'-') {
        i += 1;
    }
    let mut saw_digit = false;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        saw_digit = true;
        i += 1;
    }
    if i < bytes.len() && bytes[i] == b'.' {
        i += 1;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            saw_digit = true;
            i += 1;
        }
    }
    if saw_digit && i < bytes.len() && (bytes[i] == b'e' || bytes[i] == b'E') {
        let mut j = i + 1;
        if j < bytes.len() && (bytes[j] == b'+' || bytes[j] == b'-') {
            j += 1;
        }
        let exp_start = j;
        while j < bytes.len() && bytes[j].is_ascii_digit() {
            j += 1;
        }
        if j > exp_start {
            i = j;
        }
    }
    if !saw_digit {
        return Value::Number(f64::NAN);
    }
    trimmed[..i].parse::<f64>().map(Value::Number).unwrap_or(Value::Number(f64::NAN))
}

pub fn call_instance(
    _interp: &mut Interpreter,
    receiver: Value,
    method: &str,
    args: Vec<Value>,
    span: Span,
) -> Result<(Value, Option<Value>), RuntimeError> {
    let n = as_number(&receiver, span, "метод числа")?;
    match method {
        "вСтроку" => Ok((Value::String(Value::Number(n).to_string()), None)),
        "фиксированный" => {
            let digits =
                if args.is_empty() { 0 } else { as_number(&args[0], span, "фиксированный")? as usize };
            Ok((Value::String(format!("{n:.*}", digits)), None))
        }
        _ => Err(RuntimeError::new(format!("У числа нет метода '{method}'"), span)),
    }
}
