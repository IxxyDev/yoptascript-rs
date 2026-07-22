use std::cell::RefCell;
use std::rc::Rc;

use unicode_normalization::UnicodeNormalization;
use yps_lexer::Span;

use crate::error::RuntimeError;
use crate::interpreter::Interpreter;
use crate::stdlib::{as_number, as_string, regexp, require_args};
use crate::value::{IteratorState, Value};

const MAX_STRING_LEN: usize = 50_000_000;

fn check_pad_budget(target_len: usize, fill: &str, span: Span) -> Result<(), RuntimeError> {
    let max_char_bytes = fill.chars().map(|c| c.len_utf8()).max().unwrap_or(1);
    if target_len.saturating_mul(max_char_bytes) > MAX_STRING_LEN {
        return Err(RuntimeError::new("Превышен лимит длины строки", span));
    }
    Ok(())
}

pub fn call(
    interp: &mut Interpreter,
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
            let idx = as_number(&args[0], span, "charAt")?;
            let units = utf16_units(&s);
            let out = if idx < 0.0 || idx as usize >= units.len() {
                String::new()
            } else {
                String::from_utf16_lossy(&units[idx as usize..idx as usize + 1])
            };
            Ok((Value::String(out.into()), None))
        }
        "charCodeAt" | "кодСимволаВ" => {
            let idx = if args.is_empty() { 0.0 } else { as_number(&args[0], span, "charCodeAt")? };
            let units = utf16_units(&s);
            if idx < 0.0 || idx as usize >= units.len() {
                Ok((Value::Number(f64::NAN), None))
            } else {
                Ok((Value::Number(units[idx as usize] as f64), None))
            }
        }
        "indexOf" | "найтиПодстроку" => {
            require_args(&args, 1, span, "indexOf")?;
            let needle = as_string(&args[0], span, "indexOf")?;
            let units = utf16_units(&s);
            let from = if args.len() > 1 { clamped_unit_index(&args[1], units.len(), span, "indexOf")? } else { 0 };
            match utf16_find(&units, needle, from) {
                Some(pos) => Ok((Value::Number(pos as f64), None)),
                None => Ok((Value::Number(-1.0), None)),
            }
        }
        "lastIndexOf" | "найтиПодстрокуСконца" => {
            require_args(&args, 1, span, "lastIndexOf")?;
            let needle = as_string(&args[0], span, "lastIndexOf")?;
            let units = utf16_units(&s);
            let needle_units: Vec<u16> = needle.encode_utf16().collect();
            let from = if args.len() > 1 {
                let n = as_number(&args[1], span, "lastIndexOf")?;
                if n.is_nan() { units.len() } else { (n as isize).clamp(0, units.len() as isize) as usize }
            } else {
                units.len()
            };
            let mut best: isize = -1;
            if needle_units.len() <= units.len() {
                let mut pos = from.min(units.len() - needle_units.len());
                loop {
                    if units[pos..pos + needle_units.len()] == needle_units[..] {
                        best = pos as isize;
                        break;
                    }
                    if pos == 0 {
                        break;
                    }
                    pos -= 1;
                }
            } else if needle_units.is_empty() {
                best = from.min(units.len()) as isize;
            }
            Ok((Value::Number(best as f64), None))
        }
        "includes" | "содержит" => {
            require_args(&args, 1, span, "includes")?;
            let needle = as_string(&args[0], span, "includes")?;
            let units = utf16_units(&s);
            let from = if args.len() > 1 { clamped_unit_index(&args[1], units.len(), span, "includes")? } else { 0 };
            Ok((Value::Boolean(utf16_find(&units, needle, from).is_some()), None))
        }
        "slice" | "отрезать" => {
            let units = utf16_units(&s);
            let len = units.len() as isize;
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
            let out = if start < end {
                String::from_utf16_lossy(&units[start as usize..end as usize])
            } else {
                String::new()
            };
            Ok((Value::String(out.into()), None))
        }
        "substring" | "подстрока" => {
            let units = utf16_units(&s);
            let len = units.len() as isize;
            let start_raw = if args.is_empty() { 0 } else { as_number(&args[0], span, "substring")? as isize };
            let end_raw = if args.len() < 2 { len } else { as_number(&args[1], span, "substring")? as isize };
            let s0 = start_raw.clamp(0, len);
            let e0 = end_raw.clamp(0, len);
            let (s1, e1) = if s0 <= e0 { (s0, e0) } else { (e0, s0) };
            let out = String::from_utf16_lossy(&units[s1 as usize..e1 as usize]);
            Ok((Value::String(out.into()), None))
        }
        "toUpperCase" | "вВерхнийРегистр" => Ok((Value::String(s.to_uppercase().into()), None)),
        "toLowerCase" | "вНижнийРегистр" => Ok((Value::String(s.to_lowercase().into()), None)),
        "trim" | "обрезать" => Ok((Value::String(s.trim().to_string().into()), None)),
        "trimStart" | "обрезатьСлева" => Ok((Value::String(s.trim_start().to_string().into()), None)),
        "trimEnd" | "обрезатьСправа" => Ok((Value::String(s.trim_end().to_string().into()), None)),
        "split" | "разбить" => {
            if args.is_empty() {
                return Ok((Value::array(vec![Value::String(s)]), None));
            }
            let limit = if args.len() > 1 && !matches!(args[1], Value::Undefined) {
                let n = as_number(&args[1], span, "split")?;
                if n.is_nan() || n < 0.0 { Some(0usize) } else { Some(n as usize) }
            } else {
                None
            };
            if let Some(0) = limit {
                return Ok((Value::array(Vec::new()), None));
            }
            if let Value::RegExp { compiled, .. } = &args[0] {
                let mut parts: Vec<Value> = regexp::split_string(compiled, &s, span)?;
                if let Some(lim) = limit {
                    parts.truncate(lim);
                }
                return Ok((Value::array(parts), None));
            }
            let sep = as_string(&args[0], span, "split")?;
            let mut parts: Vec<Value> = if sep.is_empty() {
                s.chars().map(|c| Value::String(c.to_string().into())).collect()
            } else {
                s.split(sep).map(|p| Value::String(p.to_string().into())).collect()
            };
            if let Some(lim) = limit {
                parts.truncate(lim);
            }
            Ok((Value::array(parts), None))
        }
        "replace" | "заменить" => {
            require_args(&args, 2, span, "replace")?;
            if let Value::RegExp { compiled, flags, .. } = &args[0] {
                let compiled = compiled.clone();
                let global = flags.contains('g');
                match &args[1] {
                    Value::String(rep) => {
                        let rep = rep.clone();
                        return Ok((
                            Value::String(regexp::replace_string(&compiled, &s, &rep, global, span)?.into()),
                            None,
                        ));
                    }
                    Value::Function { .. } | Value::BuiltinFunction(_) => {
                        let fn_val = args[1].clone();
                        let out = regexp::replace_with_fn(interp, &compiled, &s, fn_val, global, span)?;
                        return Ok((Value::String(out.into()), None));
                    }
                    other => {
                        return Err(RuntimeError::new(
                            format!("'replace' с regex ожидает строку или функцию, получено '{}'", other.type_name()),
                            span,
                        ));
                    }
                }
            }
            let from = as_string(&args[0], span, "replace")?;
            let to = as_string(&args[1], span, "replace")?;
            if let Some(byte_pos) = s.find(from) {
                let repl = expand_string_replacement(to, &s, byte_pos, from);
                let mut out = String::with_capacity(s.len());
                out.push_str(&s[..byte_pos]);
                out.push_str(&repl);
                out.push_str(&s[byte_pos + from.len()..]);
                Ok((Value::String(out.into()), None))
            } else {
                Ok((Value::String(s.clone()), None))
            }
        }
        "replaceAll" | "заменитьВсе" => {
            require_args(&args, 2, span, "replaceAll")?;
            if let Value::RegExp { compiled, flags, .. } = &args[0] {
                if !flags.contains('g') {
                    return Err(RuntimeError::new("replaceAll с regex требует флаг 'g'", span));
                }
                let compiled = compiled.clone();
                match &args[1] {
                    Value::String(rep) => {
                        let rep = rep.clone();
                        return Ok((
                            Value::String(regexp::replace_string(&compiled, &s, &rep, true, span)?.into()),
                            None,
                        ));
                    }
                    Value::Function { .. } | Value::BuiltinFunction(_) => {
                        let fn_val = args[1].clone();
                        let out = regexp::replace_with_fn(interp, &compiled, &s, fn_val, true, span)?;
                        return Ok((Value::String(out.into()), None));
                    }
                    other => {
                        return Err(RuntimeError::new(
                            format!(
                                "'replaceAll' с regex ожидает строку или функцию, получено '{}'",
                                other.type_name()
                            ),
                            span,
                        ));
                    }
                }
            }
            let from = as_string(&args[0], span, "replaceAll")?;
            let to = as_string(&args[1], span, "replaceAll")?;
            if from.is_empty() {
                return Ok((Value::String(s.replace(from, to).into()), None));
            }
            let mut out = String::with_capacity(s.len());
            let mut last = 0usize;
            while let Some(rel) = s[last..].find(from) {
                let byte_pos = last + rel;
                out.push_str(&s[last..byte_pos]);
                out.push_str(&expand_string_replacement(to, &s, byte_pos, from));
                last = byte_pos + from.len();
            }
            out.push_str(&s[last..]);
            Ok((Value::String(out.into()), None))
        }
        "match" | "совпадает" => {
            require_args(&args, 1, span, "match")?;
            let (compiled, flags) = match &args[0] {
                Value::RegExp { compiled, flags, .. } => (compiled, flags),
                other => {
                    return Err(RuntimeError::new(
                        format!("'match' ожидает regex, получено '{}'", other.type_name()),
                        span,
                    ));
                }
            };
            if flags.contains('g') {
                Ok((Value::array(regexp::match_all_global(compiled, &s, span)?), None))
            } else {
                Ok((regexp::match_first(compiled, &s, span)?, None))
            }
        }
        "matchAll" | "найтиВсе" => {
            require_args(&args, 1, span, "matchAll")?;
            let (compiled, flags) = match &args[0] {
                Value::RegExp { compiled, flags, .. } => (compiled, flags),
                other => {
                    return Err(RuntimeError::new(
                        format!("'matchAll' ожидает regex, получено '{}'", other.type_name()),
                        span,
                    ));
                }
            };
            if !flags.contains('g') {
                return Err(RuntimeError::new("matchAll требует флаг 'g'", span));
            }
            let state = IteratorState::RegexMatches { re: Rc::clone(compiled), input: s.to_string(), byte_pos: 0 };
            Ok((Value::Iterator(Rc::new(RefCell::new(state))), None))
        }
        "search" | "найтиИндекс" => {
            require_args(&args, 1, span, "search")?;
            let compiled = match &args[0] {
                Value::RegExp { compiled, .. } => compiled,
                other => {
                    return Err(RuntimeError::new(
                        format!("'search' ожидает regex, получено '{}'", other.type_name()),
                        span,
                    ));
                }
            };
            Ok((Value::Number(regexp::search_index(compiled, &s, span)? as f64), None))
        }
        "startsWith" | "начинаетсяС" => {
            require_args(&args, 1, span, "startsWith")?;
            let needle = as_string(&args[0], span, "startsWith")?;
            let units = utf16_units(&s);
            let pos = if args.len() > 1 { clamped_unit_index(&args[1], units.len(), span, "startsWith")? } else { 0 };
            let needle_units: Vec<u16> = needle.encode_utf16().collect();
            let ok =
                pos + needle_units.len() <= units.len() && units[pos..pos + needle_units.len()] == needle_units[..];
            Ok((Value::Boolean(ok), None))
        }
        "endsWith" | "заканчиваетсяНа" => {
            require_args(&args, 1, span, "endsWith")?;
            let needle = as_string(&args[0], span, "endsWith")?;
            let units = utf16_units(&s);
            let end = if args.len() > 1 && !matches!(args[1], Value::Undefined) {
                clamped_unit_index(&args[1], units.len(), span, "endsWith")?
            } else {
                units.len()
            };
            let needle_units: Vec<u16> = needle.encode_utf16().collect();
            let ok = needle_units.len() <= end && units[end - needle_units.len()..end] == needle_units[..];
            Ok((Value::Boolean(ok), None))
        }
        "repeat" | "повторить" => {
            require_args(&args, 1, span, "repeat")?;
            let count = as_number(&args[0], span, "repeat")?;
            if count < 0.0 || !count.is_finite() {
                return Err(RuntimeError::new("Некорректное количество повторений", span));
            }
            let count = count as usize;
            if s.len().saturating_mul(count) > MAX_STRING_LEN {
                return Err(RuntimeError::new("Превышен лимит длины строки", span));
            }
            Ok((Value::String(s.repeat(count).into()), None))
        }
        "padStart" | "дополнитьСлева" => {
            require_args(&args, 1, span, "padStart")?;
            let target_len = as_number(&args[0], span, "padStart")? as usize;
            let fill =
                if args.len() > 1 { as_string(&args[1], span, "padStart")?.to_string() } else { " ".to_string() };
            check_pad_budget(target_len, &fill, span)?;
            Ok((Value::String(pad(&s, target_len, &fill, true).into()), None))
        }
        "padEnd" | "дополнитьСправа" => {
            require_args(&args, 1, span, "padEnd")?;
            let target_len = as_number(&args[0], span, "padEnd")? as usize;
            let fill = if args.len() > 1 { as_string(&args[1], span, "padEnd")?.to_string() } else { " ".to_string() };
            check_pad_budget(target_len, &fill, span)?;
            Ok((Value::String(pad(&s, target_len, &fill, false).into()), None))
        }
        "at" | "поИндексу" => {
            require_args(&args, 1, span, "at")?;
            let idx = as_number(&args[0], span, "at")? as isize;
            let units = utf16_units(&s);
            let len = units.len() as isize;
            let real = if idx < 0 { len + idx } else { idx };
            if real < 0 || real >= len {
                Ok((Value::Undefined, None))
            } else {
                Ok((Value::String(String::from_utf16_lossy(&units[real as usize..real as usize + 1]).into()), None))
            }
        }
        "normalize" | "нормализовать" => {
            let form = if args.is_empty() || matches!(args[0], Value::Undefined) {
                "NFC".to_string()
            } else {
                as_string(&args[0], span, "normalize")?.to_string()
            };
            let normalized = match form.as_str() {
                "NFC" => s.nfc().collect::<String>(),
                "NFD" => s.nfd().collect::<String>(),
                "NFKC" => s.nfkc().collect::<String>(),
                "NFKD" => s.nfkd().collect::<String>(),
                _ => {
                    return Err(RuntimeError::new(format!("Некорректная форма нормализации: '{form}'"), span));
                }
            };
            Ok((Value::String(normalized.into()), None))
        }
        "codePointAt" | "кодТочки" => {
            let idx = if args.is_empty() { 0.0 } else { as_number(&args[0], span, "codePointAt")? };
            let units = utf16_units(&s);
            if idx < 0.0 || !idx.is_finite() || idx as usize >= units.len() {
                return Ok((Value::Undefined, None));
            }
            let pos = idx as usize;
            let first = units[pos];
            let is_leading_surrogate = (0xD800..=0xDBFF).contains(&first);
            if is_leading_surrogate && pos + 1 < units.len() {
                let second = units[pos + 1];
                if (0xDC00..=0xDFFF).contains(&second) {
                    let code = 0x10000 + ((first as u32 - 0xD800) << 10) + (second as u32 - 0xDC00);
                    return Ok((Value::Number(code as f64), None));
                }
            }
            Ok((Value::Number(first as f64), None))
        }
        "concat" | "присоединить" => {
            let mut out = s.to_string();
            for a in args {
                out.push_str(&a.to_string());
            }
            Ok((Value::String(out.into()), None))
        }
        _ => Err(RuntimeError::new(format!("У строки нет метода '{method}'"), span)),
    }
}

pub fn method_exists(name: &str) -> bool {
    matches!(
        name,
        "charAt"
            | "символВ"
            | "charCodeAt"
            | "кодСимволаВ"
            | "indexOf"
            | "найтиПодстроку"
            | "lastIndexOf"
            | "найтиПодстрокуСконца"
            | "includes"
            | "содержит"
            | "slice"
            | "отрезать"
            | "substring"
            | "подстрока"
            | "toUpperCase"
            | "вВерхнийРегистр"
            | "toLowerCase"
            | "вНижнийРегистр"
            | "trim"
            | "обрезать"
            | "trimStart"
            | "обрезатьСлева"
            | "trimEnd"
            | "обрезатьСправа"
            | "split"
            | "разбить"
            | "replace"
            | "заменить"
            | "replaceAll"
            | "заменитьВсе"
            | "match"
            | "совпадает"
            | "matchAll"
            | "найтиВсе"
            | "search"
            | "найтиИндекс"
            | "startsWith"
            | "начинаетсяС"
            | "endsWith"
            | "заканчиваетсяНа"
            | "repeat"
            | "повторить"
            | "padStart"
            | "дополнитьСлева"
            | "padEnd"
            | "дополнитьСправа"
            | "at"
            | "поИндексу"
            | "concat"
            | "присоединить"
            | "codePointAt"
            | "кодТочки"
            | "normalize"
            | "нормализовать"
    )
}

fn clamped_unit_index(arg: &Value, len: usize, span: Span, method: &str) -> Result<usize, RuntimeError> {
    Ok((as_number(arg, span, method)? as isize).clamp(0, len as isize) as usize)
}

fn utf16_units(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

fn utf16_find(haystack: &[u16], needle: &str, from: usize) -> Option<usize> {
    let needle_units: Vec<u16> = needle.encode_utf16().collect();
    if needle_units.is_empty() {
        return Some(from.min(haystack.len()));
    }
    if needle_units.len() > haystack.len() {
        return None;
    }
    (from..=haystack.len() - needle_units.len()).find(|&i| haystack[i..i + needle_units.len()] == needle_units[..])
}

fn expand_string_replacement(repl: &str, source: &str, match_start: usize, matched: &str) -> String {
    if !repl.contains('$') {
        return repl.to_string();
    }
    let bytes = repl.as_bytes();
    let mut out = String::with_capacity(repl.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'$' && i + 1 < bytes.len() {
            match bytes[i + 1] {
                b'$' => {
                    out.push('$');
                    i += 2;
                    continue;
                }
                b'&' => {
                    out.push_str(matched);
                    i += 2;
                    continue;
                }
                b'`' => {
                    out.push_str(&source[..match_start]);
                    i += 2;
                    continue;
                }
                b'\'' => {
                    out.push_str(&source[match_start + matched.len()..]);
                    i += 2;
                    continue;
                }
                _ => {}
            }
        }
        out.push(repl[i..].chars().next().unwrap());
        i += repl[i..].chars().next().unwrap().len_utf8();
    }
    out
}

fn pad(s: &str, target_len: usize, fill: &str, at_start: bool) -> String {
    let cur_len = s.encode_utf16().count();
    if cur_len >= target_len || fill.is_empty() {
        return s.to_string();
    }
    let needed = target_len - cur_len;
    let fill_units: Vec<u16> = fill.encode_utf16().collect();
    let mut padding_units: Vec<u16> = Vec::with_capacity(needed);
    for i in 0..needed {
        padding_units.push(fill_units[i % fill_units.len()]);
    }
    let padding = String::from_utf16_lossy(&padding_units);
    if at_start { format!("{padding}{s}") } else { format!("{s}{padding}") }
}
