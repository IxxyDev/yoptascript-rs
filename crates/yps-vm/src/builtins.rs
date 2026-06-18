use yps_lexer::Span;

use crate::error::VmError;
use crate::value::{Value, number_to_string};

pub fn is_builtin(name: &str) -> bool {
    if let Some(method) = name.strip_prefix("сказать.") {
        return matches!(method, "ошибка" | "предупреждение" | "инфо" | "отладка");
    }
    matches!(name, "сказать" | "длина" | "тип" | "число" | "строка" | "втолкнуть")
}

pub fn call_builtin(out: &mut dyn std::io::Write, name: &str, args: Vec<Value>, span: Span) -> Result<Value, VmError> {
    if let Some(method) = name.strip_prefix("сказать.") {
        return console_method(out, method, &args, span);
    }
    match name {
        "сказать" => {
            let parts: Vec<String> = args.iter().map(|a| a.to_string()).collect();
            let _ = writeln!(out, "{}", parts.join(" "));
            Ok(Value::Undefined)
        }
        "длина" => builtin_dlina(&args, span),
        "тип" => {
            check_arity("тип", &args, 1, span)?;
            Ok(Value::string(args[0].type_name()))
        }
        "число" => {
            check_arity("число", &args, 1, span)?;
            Ok(Value::Number(args[0].to_number()))
        }
        "строка" => {
            check_arity("строка", &args, 1, span)?;
            let text = match &args[0] {
                Value::Number(n) => number_to_string(*n),
                other => other.to_string(),
            };
            Ok(Value::string(text))
        }
        "втолкнуть" => builtin_vtolknut(args, span),
        other => Err(VmError::new(format!("встроенная функция '{other}' не поддерживается VM"), span)),
    }
}

fn console_method(out: &mut dyn std::io::Write, method: &str, args: &[Value], span: Span) -> Result<Value, VmError> {
    let line: String = args.iter().map(|a| a.to_string()).collect::<Vec<_>>().join(" ");
    match method {
        "инфо" | "отладка" => {
            let _ = writeln!(out, "{line}");
            Ok(Value::Undefined)
        }
        "ошибка" | "предупреждение" => {
            eprintln!("{line}");
            Ok(Value::Undefined)
        }
        other => Err(VmError::new(format!("у 'сказать' нет метода '{other}' в VM"), span)),
    }
}

fn builtin_dlina(args: &[Value], span: Span) -> Result<Value, VmError> {
    check_arity("длина", args, 1, span)?;
    match &args[0] {
        Value::Str(s) => Ok(Value::Number(s.chars().count() as f64)),
        Value::Array(a) => Ok(Value::Number(a.borrow().len() as f64)),
        Value::Object(map) => {
            let m = map.borrow();
            match m.get("длина").or_else(|| m.get("length")) {
                Some(Value::Number(n)) => Ok(Value::Number(*n)),
                _ => Err(VmError::new("'длина' не работает с типом 'объект'", span)),
            }
        }
        other => Err(VmError::new(format!("'длина' не работает с типом '{}'", other.type_name()), span)),
    }
}

fn builtin_vtolknut(args: Vec<Value>, span: Span) -> Result<Value, VmError> {
    check_arity("втолкнуть", &args, 2, span)?;
    let mut it = args.into_iter();
    let arr = it.next().unwrap();
    let val = it.next().unwrap();
    match arr {
        Value::Array(a) => {
            a.borrow_mut().push(val);
            Ok(Value::Array(a))
        }
        _ => Err(VmError::new("первый аргумент 'втолкнуть' должен быть массивом", span)),
    }
}

fn check_arity(name: &str, args: &[Value], expected: usize, span: Span) -> Result<(), VmError> {
    if args.len() != expected {
        return Err(VmError::new(format!("'{name}' принимает {expected} аргумент(ов), получено {}", args.len()), span));
    }
    Ok(())
}
