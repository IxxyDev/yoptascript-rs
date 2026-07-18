use std::cell::Cell;

use yps_lexer::Span;

use crate::error::RuntimeError;
use crate::interpreter::Interpreter;
use crate::stdlib::{as_number, builtin, object_of, require_args};
use crate::value::{Value, to_int_n, to_uint_n};

pub fn build_object() -> Value {
    object_of(&[
        ("ПИ", Value::Number(std::f64::consts::PI)),
        ("Е", Value::Number(std::f64::consts::E)),
        ("ЛН2", Value::Number(std::f64::consts::LN_2)),
        ("ЛН10", Value::Number(std::f64::consts::LN_10)),
        ("ЛОГ2Е", Value::Number(std::f64::consts::LOG2_E)),
        ("ЛОГ10Е", Value::Number(std::f64::consts::LOG10_E)),
        ("КОРЕНЬ2", Value::Number(std::f64::consts::SQRT_2)),
        ("КОРЕНЬ0_5", Value::Number(std::f64::consts::FRAC_1_SQRT_2)),
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
        ("арксинус", builtin("Матан.арксинус")),
        ("арккосинус", builtin("Матан.арккосинус")),
        ("арктангенс", builtin("Матан.арктангенс")),
        ("арктангенс2", builtin("Матан.арктангенс2")),
        ("кубическийКорень", builtin("Матан.кубическийКорень")),
        ("гипотенуза", builtin("Матан.гипотенуза")),
        ("лог2", builtin("Матан.лог2")),
        ("лог10", builtin("Матан.лог10")),
        ("лог1п", builtin("Матан.лог1п")),
        ("эксп", builtin("Матан.эксп")),
        ("экспМ1", builtin("Матан.экспМ1")),
        ("гиперСинус", builtin("Матан.гиперСинус")),
        ("гиперКосинус", builtin("Матан.гиперКосинус")),
        ("гиперТангенс", builtin("Матан.гиперТангенс")),
        ("аркГиперСинус", builtin("Матан.аркГиперСинус")),
        ("аркГиперКосинус", builtin("Матан.аркГиперКосинус")),
        ("аркГиперТангенс", builtin("Матан.аркГиперТангенс")),
        ("дробь32", builtin("Матан.дробь32")),
        ("нулиСлева32", builtin("Матан.нулиСлева32")),
        ("умножить32", builtin("Матан.умножить32")),
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
            Ok(Value::Number(if n == 0.0 || n.is_nan() { n } else { n.signum() }))
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
        "арксинус" => {
            require_args(&args, 1, span, "Матан.арксинус")?;
            Ok(Value::Number(as_number(&args[0], span, "Матан.арксинус")?.asin()))
        }
        "арккосинус" => {
            require_args(&args, 1, span, "Матан.арккосинус")?;
            Ok(Value::Number(as_number(&args[0], span, "Матан.арккосинус")?.acos()))
        }
        "арктангенс" => {
            require_args(&args, 1, span, "Матан.арктангенс")?;
            Ok(Value::Number(as_number(&args[0], span, "Матан.арктангенс")?.atan()))
        }
        "арктангенс2" => {
            require_args(&args, 2, span, "Матан.арктангенс2")?;
            let y = as_number(&args[0], span, "Матан.арктангенс2")?;
            let x = as_number(&args[1], span, "Матан.арктангенс2")?;
            Ok(Value::Number(y.atan2(x)))
        }
        "кубическийКорень" => {
            require_args(&args, 1, span, "Матан.кубическийКорень")?;
            Ok(Value::Number(as_number(&args[0], span, "Матан.кубическийКорень")?.cbrt()))
        }
        "гипотенуза" => {
            let mut sum_sq = 0.0f64;
            for (i, a) in args.iter().enumerate() {
                let n = as_number(a, span, &format!("Матан.гипотенуза(аргумент {})", i + 1))?;
                sum_sq += n * n;
            }
            Ok(Value::Number(sum_sq.sqrt()))
        }
        "лог2" => {
            require_args(&args, 1, span, "Матан.лог2")?;
            Ok(Value::Number(as_number(&args[0], span, "Матан.лог2")?.log2()))
        }
        "лог10" => {
            require_args(&args, 1, span, "Матан.лог10")?;
            Ok(Value::Number(as_number(&args[0], span, "Матан.лог10")?.log10()))
        }
        "лог1п" => {
            require_args(&args, 1, span, "Матан.лог1п")?;
            Ok(Value::Number(as_number(&args[0], span, "Матан.лог1п")?.ln_1p()))
        }
        "эксп" => {
            require_args(&args, 1, span, "Матан.эксп")?;
            Ok(Value::Number(as_number(&args[0], span, "Матан.эксп")?.exp()))
        }
        "экспМ1" => {
            require_args(&args, 1, span, "Матан.экспМ1")?;
            Ok(Value::Number(as_number(&args[0], span, "Матан.экспМ1")?.exp_m1()))
        }
        "гиперСинус" => {
            require_args(&args, 1, span, "Матан.гиперСинус")?;
            Ok(Value::Number(as_number(&args[0], span, "Матан.гиперСинус")?.sinh()))
        }
        "гиперКосинус" => {
            require_args(&args, 1, span, "Матан.гиперКосинус")?;
            Ok(Value::Number(as_number(&args[0], span, "Матан.гиперКосинус")?.cosh()))
        }
        "гиперТангенс" => {
            require_args(&args, 1, span, "Матан.гиперТангенс")?;
            Ok(Value::Number(as_number(&args[0], span, "Матан.гиперТангенс")?.tanh()))
        }
        "аркГиперСинус" => {
            require_args(&args, 1, span, "Матан.аркГиперСинус")?;
            Ok(Value::Number(as_number(&args[0], span, "Матан.аркГиперСинус")?.asinh()))
        }
        "аркГиперКосинус" => {
            require_args(&args, 1, span, "Матан.аркГиперКосинус")?;
            Ok(Value::Number(as_number(&args[0], span, "Матан.аркГиперКосинус")?.acosh()))
        }
        "аркГиперТангенс" => {
            require_args(&args, 1, span, "Матан.аркГиперТангенс")?;
            Ok(Value::Number(as_number(&args[0], span, "Матан.аркГиперТангенс")?.atanh()))
        }
        "дробь32" => {
            require_args(&args, 1, span, "Матан.дробь32")?;
            Ok(Value::Number(as_number(&args[0], span, "Матан.дробь32")? as f32 as f64))
        }
        "нулиСлева32" => {
            require_args(&args, 1, span, "Матан.нулиСлева32")?;
            let n = as_number(&args[0], span, "Матан.нулиСлева32")?;
            Ok(Value::Number((to_uint_n(n, 32) as u32).leading_zeros() as f64))
        }
        "умножить32" => {
            require_args(&args, 2, span, "Матан.умножить32")?;
            let a = to_int_n(as_number(&args[0], span, "Матан.умножить32")?, 32) as i32;
            let b = to_int_n(as_number(&args[1], span, "Матан.умножить32")?, 32) as i32;
            Ok(Value::Number(a.wrapping_mul(b) as f64))
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
