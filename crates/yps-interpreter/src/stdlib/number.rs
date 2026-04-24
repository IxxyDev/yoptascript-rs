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
        _ => Err(RuntimeError::new(format!("У 'Хуйня' нет метода '{method}'"), span)),
    }
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
