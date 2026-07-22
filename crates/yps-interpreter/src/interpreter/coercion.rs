use crate::value::Value;

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

pub fn string_to_number(s: &str) -> f64 {
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
        Value::String(s) => s.to_string(),
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

pub(crate) fn coerce_to_f64_opt(value: &Value) -> Option<f64> {
    match value {
        Value::Number(n) => Some(*n),
        Value::Boolean(b) => Some(if *b { 1.0 } else { 0.0 }),
        Value::String(s) => s.trim().parse::<f64>().ok(),
        _ => None,
    }
}

pub fn number_to_string(n: f64) -> String {
    if n.is_nan() {
        return "NaN".to_string();
    }
    if n.is_infinite() {
        return if n > 0.0 { "Infinity".to_string() } else { "-Infinity".to_string() };
    }
    if n == 0.0 {
        return "0".to_string();
    }
    let abs = n.abs();
    if !(1e-6..1e21).contains(&abs) {
        return format_exponential(n);
    }
    if n.fract() == 0.0 && abs < 9.007_199_254_740_992e15 {
        return format!("{}", n as i64);
    }
    format!("{n}")
}

pub enum BigIntOperand<'a> {
    Int(i128),
    Float(f64),
    Text(&'a str),
    Flag(bool),
    Other(&'a str),
}

pub fn bigint_from_operand(operand: BigIntOperand<'_>) -> Result<i128, String> {
    match operand {
        BigIntOperand::Int(n) => Ok(n),
        BigIntOperand::Float(n) => {
            if !n.is_finite() || n.fract() != 0.0 {
                return Err("БигЦелое требует целое число".to_string());
            }
            if n < i128::MIN as f64 || n > i128::MAX as f64 {
                return Err("Число вне диапазона бигцелого".to_string());
            }
            Ok(n as i128)
        }
        BigIntOperand::Text(s) => s.trim().parse::<i128>().map_err(|_| format!("Нельзя разобрать '{s}' как бигцелое")),
        BigIntOperand::Flag(b) => Ok(if b { 1 } else { 0 }),
        BigIntOperand::Other(type_name) => Err(format!("Нельзя сконвертировать '{type_name}' в бигцелое")),
    }
}

fn format_exponential(n: f64) -> String {
    let raw = format!("{n:e}");
    match raw.split_once('e') {
        Some((mantissa, exp)) => {
            if let Some(rest) = exp.strip_prefix('-') {
                format!("{mantissa}e-{rest}")
            } else {
                format!("{mantissa}e+{exp}")
            }
        }
        None => raw,
    }
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

pub(crate) fn to_primitive_builtin(value: &Value) -> Value {
    if is_primitive(value) {
        return value.clone();
    }
    Value::String(to_ecma_string(value).into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::{ArrayStore, ObjectStore};
    use indexmap::IndexMap;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[test]
    fn to_number_string_one() {
        assert_eq!(to_number(&Value::String("1".into())), 1.0);
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
        assert_eq!(to_number(&Value::String("   ".into())), 0.0);
    }

    #[test]
    fn to_number_garbage_is_nan() {
        assert!(to_number(&Value::String("абв".into())).is_nan());
    }

    #[test]
    fn to_ecma_string_object_is_object_object() {
        let obj = Value::Object(Rc::new(RefCell::new(ObjectStore::new(IndexMap::new()))));
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
        let arr = Value::Array(Rc::new(RefCell::new(ArrayStore(vec![
            Value::Number(1.0),
            Value::Number(2.0),
            Value::Number(3.0),
        ]))));
        assert_eq!(to_ecma_string(&arr), "1,2,3");
    }

    #[test]
    fn to_ecma_string_array_null_undefined_blank() {
        let arr =
            Value::Array(Rc::new(RefCell::new(ArrayStore(vec![Value::Null, Value::Undefined, Value::Number(1.0)]))));
        assert_eq!(to_ecma_string(&arr), ",,1");
    }

    #[test]
    fn number_to_string_specials() {
        assert_eq!(number_to_string(f64::NAN), "NaN");
        assert_eq!(number_to_string(f64::INFINITY), "Infinity");
        assert_eq!(number_to_string(f64::NEG_INFINITY), "-Infinity");
        assert_eq!(number_to_string(-0.0), "0");
        assert_eq!(number_to_string(0.0), "0");
    }

    #[test]
    fn number_to_string_large_integers_no_i64_saturation() {
        assert_eq!(number_to_string(1e19), "10000000000000000000");
        assert_eq!(number_to_string(1e20), "100000000000000000000");
        assert_eq!(number_to_string(5e18), "5000000000000000000");
        assert_eq!(number_to_string(9_007_199_254_740_992.0), "9007199254740992");
    }

    #[test]
    fn number_to_string_v8_exponential() {
        assert_eq!(number_to_string(1e21), "1e+21");
        assert_eq!(number_to_string(1.5e21), "1.5e+21");
        assert_eq!(number_to_string(1e-7), "1e-7");
        assert_eq!(number_to_string(0.0000001), "1e-7");
        assert_eq!(number_to_string(123456.0), "123456");
        assert_eq!(number_to_string(0.000001), "0.000001");
        assert_eq!(number_to_string(0.0015), "0.0015");
    }

    #[test]
    fn string_to_number_radix_prefixes() {
        assert_eq!(string_to_number("0x10"), 16.0);
        assert_eq!(string_to_number("0b101"), 5.0);
        assert_eq!(string_to_number("0o17"), 15.0);
        assert_eq!(string_to_number("1e3"), 1000.0);
        assert_eq!(string_to_number("1.5e-3"), 0.0015);
    }

    #[test]
    fn to_primitive_builtin_passes_primitive_through() {
        assert_eq!(to_primitive_builtin(&Value::Number(5.0)), Value::Number(5.0));
    }

    #[test]
    fn to_primitive_builtin_object_stringifies() {
        let obj = Value::Object(Rc::new(RefCell::new(ObjectStore::new(IndexMap::new()))));
        assert_eq!(to_primitive_builtin(&obj), Value::String("[object Object]".into()));
    }
}
