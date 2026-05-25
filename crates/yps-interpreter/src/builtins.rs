use std::rc::Rc;

use yps_lexer::Span;

use crate::error::RuntimeError;
use crate::stdlib;
use crate::stdlib::regexp;
use crate::symbols;
use crate::value::Value;

pub fn call_builtin(name: &str, args: Vec<Value>, span: Span) -> Result<Value, RuntimeError> {
    match name {
        s if s == symbols::ERROR_NAME => stdlib::error::construct(args, span),
        "этоКосяк" => is_kosyak(args, span),
        "RegExp" => construct_regexp(args, span),
        "сказать" => {
            let parts: Vec<String> = args.iter().map(|a| a.to_string()).collect();
            println!("{}", parts.join(" "));
            Ok(Value::Undefined)
        }
        "длина" => {
            if args.len() != 1 {
                return Err(RuntimeError::new("'длина' принимает 1 аргумент", span));
            }
            match &args[0] {
                Value::String(s) => Ok(Value::Number(s.chars().count() as f64)),
                Value::Array(a) => Ok(Value::Number(a.len() as f64)),
                other => Err(RuntimeError::new(format!("'длина' не работает с типом '{}'", other.type_name()), span)),
            }
        }
        "тип" => {
            if args.len() != 1 {
                return Err(RuntimeError::new("'тип' принимает 1 аргумент", span));
            }
            Ok(Value::String(args[0].type_name().to_string()))
        }
        "число" => {
            if args.len() != 1 {
                return Err(RuntimeError::new("'число' принимает 1 аргумент", span));
            }
            match &args[0] {
                Value::Number(n) => Ok(Value::Number(*n)),
                Value::String(s) => match s.parse::<f64>() {
                    Ok(n) => Ok(Value::Number(n)),
                    Err(_) => Ok(Value::Null),
                },
                Value::Boolean(b) => Ok(Value::Number(if *b { 1.0 } else { 0.0 })),
                _ => Ok(Value::Null),
            }
        }
        "БигЦелое" => {
            if args.len() != 1 {
                return Err(RuntimeError::new("'БигЦелое' принимает 1 аргумент", span));
            }
            match &args[0] {
                Value::BigInt(n) => Ok(Value::BigInt(*n)),
                Value::Number(n) => {
                    if !n.is_finite() || n.fract() != 0.0 {
                        return Err(RuntimeError::new("БигЦелое требует целое число", span));
                    }
                    if *n < i128::MIN as f64 || *n > i128::MAX as f64 {
                        return Err(RuntimeError::new("Число вне диапазона бигцелого", span));
                    }
                    Ok(Value::BigInt(*n as i128))
                }
                Value::String(s) => s
                    .trim()
                    .parse::<i128>()
                    .map(Value::BigInt)
                    .map_err(|_| RuntimeError::new(format!("Нельзя разобрать '{s}' как бигцелое"), span)),
                Value::Boolean(b) => Ok(Value::BigInt(if *b { 1 } else { 0 })),
                other => {
                    Err(RuntimeError::new(format!("Нельзя сконвертировать '{}' в бигцелое", other.type_name()), span))
                }
            }
        }
        "строка" => {
            if args.len() != 1 {
                return Err(RuntimeError::new("'строка' принимает 1 аргумент", span));
            }
            Ok(Value::String(args[0].to_string()))
        }
        "втолкнуть" => {
            if args.len() != 2 {
                return Err(RuntimeError::new("'втолкнуть' принимает 2 аргумента (массив, значение)", span));
            }
            let mut args = args.into_iter();
            let arr = args.next().unwrap();
            let val = args.next().unwrap();
            match arr {
                Value::Array(mut a) => {
                    a.push(val);
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
        "чутка",
        "отменаЧутки",
        "интервал",
        "отменаИнтервала",
        "сразу",
        "наСледующемТике",
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
        Some(Value::String(s)) => Some(s.clone()),
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
            Ok(Value::RegExp { pattern, flags, compiled })
        }
        Value::RegExp { pattern, flags, compiled } => match flags_override {
            None => Ok(Value::RegExp { pattern, flags, compiled: Rc::clone(&compiled) }),
            Some(new_flags) => {
                let recompiled = regexp::compile(&pattern, &new_flags, span)?;
                Ok(Value::RegExp { pattern, flags: new_flags, compiled: recompiled })
            }
        },
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
