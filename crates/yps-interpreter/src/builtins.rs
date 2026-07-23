use std::cell::RefCell;
use std::rc::Rc;

use yps_lexer::Span;

use crate::error::RuntimeError;
use crate::interpreter::coercion::{BigIntOperand, bigint_from_operand};
use crate::stdlib;
use crate::stdlib::regexp;
use crate::symbols;
use crate::value::{RegExpData, Value};

pub fn call_builtin(name: &str, args: Vec<Value>, span: Span) -> Result<Value, RuntimeError> {
    if let Some(method) = name.strip_prefix("сказать.") {
        return stdlib::console::dispatch(method, args, span);
    }
    match name {
        s if s == symbols::ERROR_NAME => stdlib::error::construct(args, span),
        "этоКосяк" => is_kosyak(args, span),
        "RegExp" => construct_regexp(args, span),
        "Дата" => crate::stdlib::date::construct(args, span),
        "сказать" => {
            let parts: Vec<String> = args.iter().map(|a| a.to_string()).collect();
            println!("{}", parts.join(" "));
            Ok(Value::Undefined)
        }
        "прочестьСтроку" => stdlib::stdio::read_line(span),
        "прочестьВсё" => stdlib::stdio::read_all(span),
        "длина" => {
            if args.len() != 1 {
                return Err(RuntimeError::new("'длина' принимает 1 аргумент", span));
            }
            match &args[0] {
                Value::String(s) => Ok(Value::Number(s.chars().count() as f64)),
                Value::Array(a) => Ok(Value::Number(a.borrow().len() as f64)),
                Value::TypedArray(ta) => Ok(Value::Number(ta.length as f64)),
                Value::Object(map) => {
                    let len_val = {
                        let m = map.borrow();
                        m.get("длина").or_else(|| m.get("length")).cloned()
                    };
                    match len_val {
                        Some(Value::Number(n)) => Ok(Value::Number(n)),
                        _ => Err(RuntimeError::new("'длина' не работает с типом 'объект'", span)),
                    }
                }
                other => Err(RuntimeError::new(format!("'длина' не работает с типом '{}'", other.type_name()), span)),
            }
        }
        "тип" => {
            if args.len() != 1 {
                return Err(RuntimeError::new("'тип' принимает 1 аргумент", span));
            }
            Ok(Value::String(args[0].type_name().to_string().into()))
        }
        "число" => {
            if args.len() != 1 {
                return Err(RuntimeError::new("'число' принимает 1 аргумент", span));
            }
            let n = match &args[0] {
                Value::BigInt(b) => *b as f64,
                other => crate::interpreter::coercion::to_number(other),
            };
            Ok(Value::Number(n))
        }
        "БигЦелое" => {
            if args.len() != 1 {
                return Err(RuntimeError::new("'БигЦелое' принимает 1 аргумент", span));
            }
            let operand = match &args[0] {
                Value::BigInt(n) => BigIntOperand::Int(*n),
                Value::Number(n) => BigIntOperand::Float(*n),
                Value::String(s) => BigIntOperand::Text(s),
                Value::Boolean(b) => BigIntOperand::Flag(*b),
                other => BigIntOperand::Other(other.type_name()),
            };
            bigint_from_operand(operand).map(Value::BigInt).map_err(|m| RuntimeError::new(m, span))
        }
        "строка" => {
            if args.len() != 1 {
                return Err(RuntimeError::new("'строка' принимает 1 аргумент", span));
            }
            let text = match &args[0] {
                Value::Number(n) => crate::interpreter::coercion::number_to_string(*n),
                other => other.to_string(),
            };
            Ok(Value::String(text.into()))
        }
        "втолкнуть" => {
            if args.len() != 2 {
                return Err(RuntimeError::new("'втолкнуть' принимает 2 аргумента (массив, значение)", span));
            }
            let mut args = args.into_iter();
            let arr = args.next().unwrap();
            let val = args.next().unwrap();
            match arr {
                Value::Array(a) => {
                    a.borrow_mut().push(val);
                    Ok(Value::Array(a))
                }
                _ => Err(RuntimeError::new("первый аргумент 'втолкнуть' должен быть массивом", span)),
            }
        }
        _ => Err(RuntimeError::new(format!("Неизвестная встроенная функция: '{name}'"), span)),
    }
}

pub fn builtin_names() -> &'static [&'static str] {
    &[
        "сказать",
        "длина",
        "тип",
        "число",
        "БигЦелое",
        "строка",
        "втолкнуть",
        symbols::ERROR_NAME,
        "этоКосяк",
        "RegExp",
        "Дата",
        "чутка",
        "отменаЧутки",
        "интервал",
        "отменаИнтервала",
        "сразу",
        "наСледующемТике",
        "подождать",
        "сОчередить",
        "прочестьСтроку",
        "прочестьВсё",
        "сказать.ошибка",
        "сказать.предупреждение",
        "сказать.инфо",
        "сказать.отладка",
        "сказать.таблица",
        "сказать.время",
        "сказать.времяСтоп",
    ]
}

fn construct_regexp(args: Vec<Value>, span: Span) -> Result<Value, RuntimeError> {
    let mut it = args.into_iter();
    let first = match it.next() {
        Some(v) => v,
        None => return Err(RuntimeError::new("RegExp требует pattern", span)),
    };
    let second = it.next();

    let flags_override = match &second {
        Some(Value::String(s)) => Some(s.to_string()),
        Some(Value::Undefined) | Some(Value::Null) | None => None,
        Some(other) => {
            return Err(RuntimeError::new(
                format!("RegExp ожидает строку flags, получено '{}'", other.type_name()),
                span,
            ));
        }
    };

    match first {
        Value::String(pattern) => {
            let flags = flags_override.unwrap_or_default();
            let compiled = regexp::compile(&pattern, &flags, span)?;
            Ok(Value::RegExp(Rc::new(RegExpData {
                pattern: pattern.to_string(),
                flags,
                compiled,
                last_index: Rc::new(RefCell::new(0)),
            })))
        }
        Value::RegExp(re) => {
            let pattern = re.pattern.clone();
            let flags = re.flags.clone();
            let compiled = Rc::clone(&re.compiled);
            match flags_override {
                None => Ok(Value::RegExp(Rc::new(RegExpData {
                    pattern,
                    flags,
                    compiled,
                    last_index: Rc::new(RefCell::new(0)),
                }))),
                Some(new_flags) => {
                    let recompiled = regexp::compile(&pattern, &new_flags, span)?;
                    Ok(Value::RegExp(Rc::new(RegExpData {
                        pattern,
                        flags: new_flags,
                        compiled: recompiled,
                        last_index: Rc::new(RefCell::new(0)),
                    })))
                }
            }
        }
        other => Err(RuntimeError::new(
            format!("RegExp ожидает строку или regex как pattern, получено '{}'", other.type_name()),
            span,
        )),
    }
}

fn is_kosyak(args: Vec<Value>, span: Span) -> Result<Value, RuntimeError> {
    if args.is_empty() {
        return Err(RuntimeError::new("'этоКосяк' ожидает 1 аргумент", span));
    }
    Ok(Value::Boolean(stdlib::error::is_error(&args)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span() -> Span {
        Span { start: 0, end: 0 }
    }

    fn chislo(v: Value) -> f64 {
        match call_builtin("число", vec![v], span()).unwrap() {
            Value::Number(n) => n,
            other => panic!("ожидалось число, получено {other:?}"),
        }
    }

    #[test]
    fn chislo_sleduet_ecma_tonumber() {
        assert_eq!(chislo(Value::Number(3.5)), 3.5);
        assert_eq!(chislo(Value::String("42".into())), 42.0);
        assert_eq!(chislo(Value::String("  7  ".into())), 7.0);
        assert_eq!(chislo(Value::String(String::new().into())), 0.0);
        assert_eq!(chislo(Value::String("0x10".into())), 16.0);
        assert_eq!(chislo(Value::String("Infinity".into())), f64::INFINITY);
        assert!(chislo(Value::String("мусор".into())).is_nan());
        assert_eq!(chislo(Value::Null), 0.0);
        assert!(chislo(Value::Undefined).is_nan());
        assert_eq!(chislo(Value::Boolean(true)), 1.0);
        assert_eq!(chislo(Value::Boolean(false)), 0.0);
        assert_eq!(chislo(Value::BigInt(10)), 10.0);
    }
}
