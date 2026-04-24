use yps_lexer::Span;

use crate::error::RuntimeError;
use crate::interpreter::Interpreter;
use crate::stdlib::{as_number, as_string, require_args};
use crate::value::Value;

pub fn call(
    _interp: &mut Interpreter,
    receiver: Value,
    method: &str,
    args: Vec<Value>,
    span: Span,
) -> Result<(Value, Option<Value>), RuntimeError> {
    let s = match receiver {
        Value::String(s) => s,
        _ => unreachable!(),
    };
    match method {
        "charAt" | "символВ" => {
            require_args(&args, 1, span, "charAt")?;
            let idx = as_number(&args[0], span, "charAt")? as usize;
            let ch = s.chars().nth(idx).map(|c| c.to_string()).unwrap_or_default();
            Ok((Value::String(ch), None))
        }
        "indexOf" | "найтиПодстроку" => {
            require_args(&args, 1, span, "indexOf")?;
            let needle = as_string(&args[0], span, "indexOf")?;
            if let Some(byte_pos) = s.find(needle) {
                let char_pos = s[..byte_pos].chars().count();
                Ok((Value::Number(char_pos as f64), None))
            } else {
                Ok((Value::Number(-1.0), None))
            }
        }
        "includes" | "содержит" => {
            require_args(&args, 1, span, "includes")?;
            let needle = as_string(&args[0], span, "includes")?;
            Ok((Value::Boolean(s.contains(needle)), None))
        }
        "slice" | "отрезать" => {
            let chars: Vec<char> = s.chars().collect();
            let len = chars.len() as isize;
            let start = if args.is_empty() {
                0
            } else {
                let n = as_number(&args[0], span, "slice")? as isize;
                if n < 0 { (len + n).max(0) } else { n.min(len) }
            };
            let end = if args.len() < 2 {
                len
            } else {
                let n = as_number(&args[1], span, "slice")? as isize;
                if n < 0 { (len + n).max(0) } else { n.min(len) }
            };
            let out: String =
                if start < end { chars[start as usize..end as usize].iter().collect() } else { String::new() };
            Ok((Value::String(out), None))
        }
        "substring" | "подстрока" => {
            let chars: Vec<char> = s.chars().collect();
            let len = chars.len() as isize;
            let start_raw = if args.is_empty() { 0 } else { as_number(&args[0], span, "substring")? as isize };
            let end_raw = if args.len() < 2 { len } else { as_number(&args[1], span, "substring")? as isize };
            let s0 = start_raw.clamp(0, len);
            let e0 = end_raw.clamp(0, len);
            let (s1, e1) = if s0 <= e0 { (s0, e0) } else { (e0, s0) };
            let out: String = chars[s1 as usize..e1 as usize].iter().collect();
            Ok((Value::String(out), None))
        }
        "toUpperCase" | "вВерхнийРегистр" => Ok((Value::String(s.to_uppercase()), None)),
        "toLowerCase" | "вНижнийРегистр" => Ok((Value::String(s.to_lowercase()), None)),
        "trim" | "обрезать" => Ok((Value::String(s.trim().to_string()), None)),
        "trimStart" | "обрезатьСлева" => Ok((Value::String(s.trim_start().to_string()), None)),
        "trimEnd" | "обрезатьСправа" => Ok((Value::String(s.trim_end().to_string()), None)),
        "split" | "разбить" => {
            if args.is_empty() {
                return Ok((Value::Array(vec![Value::String(s)]), None));
            }
            let sep = as_string(&args[0], span, "split")?;
            let parts: Vec<Value> = if sep.is_empty() {
                s.chars().map(|c| Value::String(c.to_string())).collect()
            } else {
                s.split(sep).map(|p| Value::String(p.to_string())).collect()
            };
            Ok((Value::Array(parts), None))
        }
        "replace" | "заменить" => {
            require_args(&args, 2, span, "replace")?;
            let from = as_string(&args[0], span, "replace")?;
            let to = as_string(&args[1], span, "replace")?;
            Ok((Value::String(s.replacen(from, to, 1)), None))
        }
        "replaceAll" | "заменитьВсе" => {
            require_args(&args, 2, span, "replaceAll")?;
            let from = as_string(&args[0], span, "replaceAll")?;
            let to = as_string(&args[1], span, "replaceAll")?;
            Ok((Value::String(s.replace(from, to)), None))
        }
        "startsWith" | "начинаетсяС" => {
            require_args(&args, 1, span, "startsWith")?;
            let needle = as_string(&args[0], span, "startsWith")?;
            Ok((Value::Boolean(s.starts_with(needle)), None))
        }
        "endsWith" | "заканчиваетсяНа" => {
            require_args(&args, 1, span, "endsWith")?;
            let needle = as_string(&args[0], span, "endsWith")?;
            Ok((Value::Boolean(s.ends_with(needle)), None))
        }
        "repeat" | "повторить" => {
            require_args(&args, 1, span, "repeat")?;
            let count = as_number(&args[0], span, "repeat")?;
            if count < 0.0 || !count.is_finite() {
                return Err(RuntimeError::new("Некорректное количество повторений", span));
            }
            Ok((Value::String(s.repeat(count as usize)), None))
        }
        "padStart" | "дополнитьСлева" => {
            require_args(&args, 1, span, "padStart")?;
            let target_len = as_number(&args[0], span, "padStart")? as usize;
            let fill =
                if args.len() > 1 { as_string(&args[1], span, "padStart")?.to_string() } else { " ".to_string() };
            Ok((Value::String(pad(&s, target_len, &fill, true)), None))
        }
        "padEnd" | "дополнитьСправа" => {
            require_args(&args, 1, span, "padEnd")?;
            let target_len = as_number(&args[0], span, "padEnd")? as usize;
            let fill = if args.len() > 1 { as_string(&args[1], span, "padEnd")?.to_string() } else { " ".to_string() };
            Ok((Value::String(pad(&s, target_len, &fill, false)), None))
        }
        "at" | "поИндексу" => {
            require_args(&args, 1, span, "at")?;
            let idx = as_number(&args[0], span, "at")? as isize;
            let chars: Vec<char> = s.chars().collect();
            let len = chars.len() as isize;
            let real = if idx < 0 { len + idx } else { idx };
            if real < 0 || real >= len {
                Ok((Value::Undefined, None))
            } else {
                Ok((Value::String(chars[real as usize].to_string()), None))
            }
        }
        "concat" | "присоединить" => {
            let mut out = s;
            for a in args {
                out.push_str(&a.to_string());
            }
            Ok((Value::String(out), None))
        }
        _ => Err(RuntimeError::new(format!("У строки нет метода '{method}'"), span)),
    }
}

fn pad(s: &str, target_len: usize, fill: &str, at_start: bool) -> String {
    let cur_len = s.chars().count();
    if cur_len >= target_len || fill.is_empty() {
        return s.to_string();
    }
    let needed = target_len - cur_len;
    let fill_chars: Vec<char> = fill.chars().collect();
    let mut padding = String::new();
    for i in 0..needed {
        padding.push(fill_chars[i % fill_chars.len()]);
    }
    if at_start { format!("{padding}{s}") } else { format!("{s}{padding}") }
}
