use std::rc::Rc;

use yps_lexer::Span;

use crate::error::RuntimeError;
use crate::interpreter::Interpreter;
use crate::stdlib::{as_string, require_args};
use crate::value::Value;

pub fn compile(pattern: &str, flags: &str, span: Span) -> Result<Rc<regex::Regex>, RuntimeError> {
    let mut prefix = String::new();
    let mut has_inline = false;
    for c in flags.chars() {
        match c {
            'i' | 'm' | 's' | 'x' => {
                if !has_inline {
                    prefix.push_str("(?");
                    has_inline = true;
                }
                prefix.push(c);
            }
            'g' | 'u' | 'y' | 'd' => {}
            other => {
                return Err(RuntimeError::new(format!("Неизвестный флаг regex: '{other}'"), span));
            }
        }
    }
    if has_inline {
        prefix.push(')');
    }
    let full = format!("{prefix}{pattern}");
    regex::Regex::new(&full).map(Rc::new).map_err(|e| RuntimeError::new(format!("Ошибка regex: {e}"), span))
}

pub fn call(
    _interp: &mut Interpreter,
    receiver: Value,
    method: &str,
    args: Vec<Value>,
    span: Span,
) -> Result<(Value, Option<Value>), RuntimeError> {
    let (pattern, flags, compiled) = match &receiver {
        Value::RegExp { pattern, flags, compiled } => (pattern.clone(), flags.clone(), Rc::clone(compiled)),
        _ => return Err(RuntimeError::new("Ожидался regex", span)),
    };

    let result = match method {
        "проверить" | "test" => {
            require_args(&args, 1, span, "regex.проверить")?;
            let s = as_string(&args[0], span, "regex.проверить")?;
            Value::Boolean(compiled.is_match(s))
        }
        "найти" | "exec" => {
            require_args(&args, 1, span, "regex.найти")?;
            let s = as_string(&args[0], span, "regex.найти")?;
            match compiled.captures(s) {
                None => Value::Null,
                Some(caps) => {
                    let mut out = Vec::new();
                    for i in 0..caps.len() {
                        match caps.get(i) {
                            Some(m) => out.push(Value::String(m.as_str().to_string())),
                            None => out.push(Value::Undefined),
                        }
                    }
                    Value::Array(out)
                }
            }
        }
        "вСтроку" | "toString" => Value::String(format!("/{pattern}/{flags}")),
        "источник" | "source" => Value::String(pattern.clone()),
        "флаги" | "flags" => Value::String(flags.clone()),
        other => {
            return Err(RuntimeError::new(format!("У regex нет метода '{other}'"), span));
        }
    };
    Ok((result, None))
}

pub fn member(receiver: &Value, property: &str) -> Option<Value> {
    let (pattern, flags) = match receiver {
        Value::RegExp { pattern, flags, .. } => (pattern, flags),
        _ => return None,
    };
    match property {
        "источник" | "source" => Some(Value::String(pattern.clone())),
        "флаги" | "flags" => Some(Value::String(flags.clone())),
        _ => None,
    }
}
