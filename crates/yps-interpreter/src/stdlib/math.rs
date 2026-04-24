use std::cell::Cell;

use yps_lexer::Span;

use crate::error::RuntimeError;
use crate::interpreter::Interpreter;
use crate::stdlib::{as_number, builtin, object_of, require_args};
use crate::value::Value;

pub fn build_object() -> Value {
    object_of(&[
        ("ПИ", Value::Number(std::f64::consts::PI)),
        ("Е", Value::Number(std::f64::consts::E)),
        ("пол", builtin("Матан.пол")),
        ("потолок", builtin("Матан.потолок")),
        ("округлить", builtin("Матан.округлить")),
        ("модуль", builtin("Матан.модуль")),
        ("мин", builtin("Матан.мин")),
        ("макс", builtin("Матан.макс")),
        ("степень", builtin("Матан.степень")),
        ("корень", builtin("Матан.корень")),
        ("рандом", builtin("Матан.рандом")),
        ("знак", builtin("Матан.знак")),
        ("обрезать", builtin("Матан.обрезать")),
        ("лог", builtin("Матан.лог")),
        ("синус", builtin("Матан.синус")),
        ("косинус", builtin("Матан.косинус")),
        ("тангенс", builtin("Матан.тангенс")),
    ])
}

pub fn call_static(
    _interp: &mut Interpreter,
    method: &str,
    args: Vec<Value>,
    span: Span,
) -> Result<Value, RuntimeError> {
    match method {
        "пол" => {
            require_args(&args, 1, span, "Матан.пол")?;
            Ok(Value::Number(as_number(&args[0], span, "Матан.пол")?.floor()))
        }
        "потолок" => {
            require_args(&args, 1, span, "Матан.потолок")?;
            Ok(Value::Number(as_number(&args[0], span, "Матан.потолок")?.ceil()))
        }
        "округлить" => {
            require_args(&args, 1, span, "Матан.округлить")?;
            Ok(Value::Number(as_number(&args[0], span, "Матан.округлить")?.round()))
        }
        "модуль" => {
            require_args(&args, 1, span, "Матан.модуль")?;
            Ok(Value::Number(as_number(&args[0], span, "Матан.модуль")?.abs()))
        }
        "мин" => {
            if args.is_empty() {
                return Ok(Value::Number(f64::INFINITY));
            }
            let mut result = f64::INFINITY;
            for (i, a) in args.iter().enumerate() {
                let n = as_number(a, span, &format!("Матан.мин(аргумент {})", i + 1))?;
                if n.is_nan() {
                    return Ok(Value::Number(f64::NAN));
                }
                if n < result {
                    result = n;
                }
            }
            Ok(Value::Number(result))
        }
        "макс" => {
            if args.is_empty() {
                return Ok(Value::Number(f64::NEG_INFINITY));
            }
            let mut result = f64::NEG_INFINITY;
            for (i, a) in args.iter().enumerate() {
                let n = as_number(a, span, &format!("Матан.макс(аргумент {})", i + 1))?;
                if n.is_nan() {
                    return Ok(Value::Number(f64::NAN));
                }
                if n > result {
                    result = n;
                }
            }
            Ok(Value::Number(result))
        }
        "степень" => {
            require_args(&args, 2, span, "Матан.степень")?;
            let base = as_number(&args[0], span, "Матан.степень")?;
            let exp = as_number(&args[1], span, "Матан.степень")?;
            Ok(Value::Number(base.powf(exp)))
        }
        "корень" => {
            require_args(&args, 1, span, "Матан.корень")?;
            Ok(Value::Number(as_number(&args[0], span, "Матан.корень")?.sqrt()))
        }
        "рандом" => Ok(Value::Number(xorshift_random())),
        "знак" => {
            require_args(&args, 1, span, "Матан.знак")?;
            let n = as_number(&args[0], span, "Матан.знак")?;
            Ok(Value::Number(if n == 0.0 { 0.0 } else { n.signum() }))
        }
        "обрезать" => {
            require_args(&args, 1, span, "Матан.обрезать")?;
            Ok(Value::Number(as_number(&args[0], span, "Матан.обрезать")?.trunc()))
        }
        "лог" => {
            require_args(&args, 1, span, "Матан.лог")?;
            Ok(Value::Number(as_number(&args[0], span, "Матан.лог")?.ln()))
        }
        "синус" => {
            require_args(&args, 1, span, "Матан.синус")?;
            Ok(Value::Number(as_number(&args[0], span, "Матан.синус")?.sin()))
        }
        "косинус" => {
            require_args(&args, 1, span, "Матан.косинус")?;
            Ok(Value::Number(as_number(&args[0], span, "Матан.косинус")?.cos()))
        }
        "тангенс" => {
            require_args(&args, 1, span, "Матан.тангенс")?;
            Ok(Value::Number(as_number(&args[0], span, "Матан.тангенс")?.tan()))
        }
        _ => Err(RuntimeError::new(format!("У 'Матан' нет метода '{method}'"), span)),
    }
}

thread_local! {
    static RNG_STATE: Cell<u64> = const { Cell::new(0x12345678_9ABCDEF0) };
}

fn xorshift_random() -> f64 {
    RNG_STATE.with(|state| {
        let mut s = state.get();
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        state.set(s);
        (s >> 11) as f64 / ((1u64 << 53) as f64)
    })
}
