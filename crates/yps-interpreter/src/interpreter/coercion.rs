#![allow(dead_code)]

use crate::value::Value;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum PrimitiveHint {
    Number,
    String,
    Default,
}

pub(crate) fn to_boolean(value: &Value) -> bool {
    value.is_truthy()
}

pub(crate) fn to_number(value: &Value) -> f64 {
    match value {
        Value::Number(n) => *n,
        Value::Boolean(b) => {
            if *b {
                1.0
            } else {
                0.0
            }
        }
        Value::Null => 0.0,
        Value::Undefined => f64::NAN,
        Value::String(s) => string_to_number(s),
        _ => f64::NAN,
    }
}

pub(crate) fn string_to_number(s: &str) -> f64 {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return 0.0;
    }
    match trimmed {
        "Infinity" | "+Infinity" => return f64::INFINITY,
        "-Infinity" => return f64::NEG_INFINITY,
        _ => {}
    }
    if let Some(hex) = trimmed.strip_prefix("0x").or_else(|| trimmed.strip_prefix("0X")) {
        return i64::from_str_radix(hex, 16).map(|v| v as f64).unwrap_or(f64::NAN);
    }
    if let Some(oct) = trimmed.strip_prefix("0o").or_else(|| trimmed.strip_prefix("0O")) {
        return i64::from_str_radix(oct, 8).map(|v| v as f64).unwrap_or(f64::NAN);
    }
    if let Some(bin) = trimmed.strip_prefix("0b").or_else(|| trimmed.strip_prefix("0B")) {
        return i64::from_str_radix(bin, 2).map(|v| v as f64).unwrap_or(f64::NAN);
    }
    trimmed.parse::<f64>().unwrap_or(f64::NAN)
}

pub(crate) fn to_ecma_string(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Number(n) => number_to_string(*n),
        Value::Boolean(b) => {
            if *b {
                "true".to_string()
            } else {
                "false".to_string()
            }
        }
        Value::Null => "null".to_string(),
        Value::Undefined => "undefined".to_string(),
        Value::BigInt(n) => n.to_string(),
        Value::Object(_) => "[object Object]".to_string(),
        Value::Array(elements) => {
            let snapshot = elements.borrow().clone();
            let parts: Vec<String> = snapshot
                .iter()
                .map(|el| match el {
                    Value::Null | Value::Undefined => String::new(),
                    other => to_ecma_string(other),
                })
                .collect();
            parts.join(",")
        }
        _ => "[object Object]".to_string(),
    }
}

pub(crate) fn to_property_key(value: &Value) -> String {
    to_ecma_string(value)
}

pub(crate) fn coerce_to_f64_opt(value: &Value) -> Option<f64> {
    match value {
        Value::Number(n) => Some(*n),
        Value::Boolean(b) => Some(if *b { 1.0 } else { 0.0 }),
        Value::String(s) => s.trim().parse::<f64>().ok(),
        _ => None,
    }
}

pub(crate) fn number_to_string(n: f64) -> String {
    if n.is_nan() {
        return "NaN".to_string();
    }
    if n.is_infinite() {
        return if n > 0.0 { "Infinity".to_string() } else { "-Infinity".to_string() };
    }
    if n.fract() == 0.0 && n.abs() < 1e21 {
        return format!("{}", n as i64);
    }
    format!("{n}")
}

pub(crate) fn is_primitive(value: &Value) -> bool {
    matches!(
        value,
        Value::Number(_)
            | Value::BigInt(_)
            | Value::String(_)
            | Value::Boolean(_)
            | Value::Null
            | Value::Undefined
            | Value::Symbol { .. }
    )
}

pub(crate) fn to_primitive_builtin(value: &Value, hint: PrimitiveHint) -> Value {
    if is_primitive(value) {
        return value.clone();
    }
    match hint {
        PrimitiveHint::String => Value::String(to_ecma_string(value)),
        PrimitiveHint::Number | PrimitiveHint::Default => Value::String(to_ecma_string(value)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::rc::Rc;

    #[test]
    fn to_number_string_one() {
        assert_eq!(to_number(&Value::String("1".to_string())), 1.0);
    }

    #[test]
    fn to_number_true_is_one() {
        assert_eq!(to_number(&Value::Boolean(true)), 1.0);
    }

    #[test]
    fn to_number_false_is_zero() {
        assert_eq!(to_number(&Value::Boolean(false)), 0.0);
    }

    #[test]
    fn to_number_null_is_zero() {
        assert_eq!(to_number(&Value::Null), 0.0);
    }

    #[test]
    fn to_number_undefined_is_nan() {
        assert!(to_number(&Value::Undefined).is_nan());
    }

    #[test]
    fn to_number_empty_string_is_zero() {
        assert_eq!(to_number(&Value::String("   ".to_string())), 0.0);
    }

    #[test]
    fn to_number_garbage_is_nan() {
        assert!(to_number(&Value::String("абв".to_string())).is_nan());
    }

    #[test]
    fn to_ecma_string_object_is_object_object() {
        let obj = Value::Object(Rc::new(RefCell::new(HashMap::new())));
        assert_eq!(to_ecma_string(&obj), "[object Object]");
    }

    #[test]
    fn to_ecma_string_integer_has_no_fraction() {
        assert_eq!(to_ecma_string(&Value::Number(42.0)), "42");
    }

    #[test]
    fn to_ecma_string_float_keeps_fraction() {
        assert_eq!(to_ecma_string(&Value::Number(1.5)), "1.5");
    }

    #[test]
    fn to_ecma_string_array_joins_with_comma() {
        let arr = Value::Array(Rc::new(RefCell::new(vec![Value::Number(1.0), Value::Number(2.0), Value::Number(3.0)])));
        assert_eq!(to_ecma_string(&arr), "1,2,3");
    }

    #[test]
    fn to_ecma_string_array_null_undefined_blank() {
        let arr = Value::Array(Rc::new(RefCell::new(vec![Value::Null, Value::Undefined, Value::Number(1.0)])));
        assert_eq!(to_ecma_string(&arr), ",,1");
    }

    #[test]
    fn to_boolean_matches_truthiness() {
        assert!(to_boolean(&Value::Number(1.0)));
        assert!(!to_boolean(&Value::Number(0.0)));
        assert!(!to_boolean(&Value::String(String::new())));
        assert!(to_boolean(&Value::String("x".to_string())));
    }

    #[test]
    fn to_primitive_builtin_passes_primitive_through() {
        assert_eq!(to_primitive_builtin(&Value::Number(5.0), PrimitiveHint::Number), Value::Number(5.0));
    }
}
