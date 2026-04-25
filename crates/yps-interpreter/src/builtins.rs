use std::collections::HashMap;

use yps_lexer::Span;

use crate::error::RuntimeError;
use crate::value::Value;

pub fn call_builtin(name: &str, args: Vec<Value>, span: Span) -> Result<Value, RuntimeError> {
    match name {
        "Косяк" => construct_kosyak(args, span),
        "этоКосяк" => is_kosyak(args, span),
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
    &["сказать", "длина", "тип", "число", "строка", "втолкнуть", "Косяк", "этоКосяк"]
}

fn construct_kosyak(args: Vec<Value>, span: Span) -> Result<Value, RuntimeError> {
    if args.is_empty() {
        return Err(RuntimeError::new("'Косяк' ожидает минимум 1 аргумент (сообщение)", span));
    }
    let mut iter = args.into_iter();
    let message = iter.next().unwrap();
    let opts = iter.next();
    let mut map = HashMap::new();
    map.insert("name".to_string(), Value::String("Косяк".to_string()));
    map.insert("message".to_string(), Value::String(message.to_string()));
    if let Some(Value::Object(o)) = opts
        && let Some(cause) = o.get("cause")
    {
        map.insert("cause".to_string(), cause.clone());
    }
    Ok(Value::Object(map))
}

fn is_kosyak(args: Vec<Value>, span: Span) -> Result<Value, RuntimeError> {
    if args.is_empty() {
        return Err(RuntimeError::new("'этоКосяк' ожидает 1 аргумент", span));
    }
    if let Value::Object(map) = &args[0]
        && let Some(Value::String(name)) = map.get("name")
        && name == "Косяк"
    {
        return Ok(Value::Boolean(true));
    }
    Ok(Value::Boolean(false))
}
