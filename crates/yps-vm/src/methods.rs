use std::cell::RefCell;
use std::cmp::Ordering;
use std::rc::Rc;

use unicode_normalization::UnicodeNormalization;
use yps_lexer::Span;

use crate::error::VmError;
use crate::value::{ForIter, Value};
use crate::vm::Vm;

const MAX_STRING_LEN: usize = 50_000_000;

fn arr(values: Vec<Value>) -> Value {
    Value::Array(Rc::new(RefCell::new(values)))
}

fn iter_of(values: Vec<Value>) -> Value {
    Value::ForIter(Rc::new(RefCell::new(ForIter::Values { values, index: 0 })))
}

fn require_args(args: &[Value], min: usize, span: Span, method: &str) -> Result<(), VmError> {
    if args.len() < min {
        Err(VmError::new(format!("'{method}' ожидает минимум {min} аргумент(ов), получено {}", args.len()), span))
    } else {
        Ok(())
    }
}

fn as_number(v: &Value, span: Span, ctx: &str) -> Result<f64, VmError> {
    match v {
        Value::Number(n) => Ok(*n),
        _ => Err(VmError::new(format!("'{ctx}' ожидает число, получено '{}'", v.type_name()), span)),
    }
}

fn as_string<'a>(v: &'a Value, span: Span, ctx: &str) -> Result<&'a str, VmError> {
    match v {
        Value::Str(s) => Ok(s),
        _ => Err(VmError::new(format!("'{ctx}' ожидает строку, получено '{}'", v.type_name()), span)),
    }
}

fn same_value_zero(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => x == y || (x.is_nan() && y.is_nan()),
        _ => crate::value::strict_eq(a, b),
    }
}

pub(crate) fn string_method_exists(name: &str) -> bool {
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

pub(crate) fn array_method_exists(name: &str) -> bool {
    matches!(
        name,
        "push"
            | "добавить"
            | "втолкнуть"
            | "pop"
            | "вытолкнуть"
            | "shift"
            | "снять"
            | "unshift"
            | "подсунуть"
            | "slice"
            | "отрезать"
            | "indexOf"
            | "найтиИндекс"
            | "lastIndexOf"
            | "найтиПоследнийПо"
            | "includes"
            | "включает"
            | "join"
            | "склеить"
            | "reverse"
            | "перевернуть"
            | "concat"
            | "склеитьМассивы"
            | "sort"
            | "сортировать"
            | "map"
            | "преобразовать"
            | "filter"
            | "отфильтровать"
            | "reduce"
            | "свернуть"
            | "reduceRight"
            | "свернутьСправа"
            | "forEach"
            | "каждый"
            | "find"
            | "найти"
            | "findIndex"
            | "найтиИндексПо"
            | "some"
            | "некоторые"
            | "every"
            | "все"
            | "at"
            | "поИндексу"
            | "flat"
            | "плоский"
            | "flatMap"
            | "плоскоПреобразовать"
            | "findLast"
            | "найтиПоследний"
            | "findLastIndex"
            | "найтиПоследнийИндекс"
            | "toReversed"
            | "перевёрнутый"
            | "toSorted"
            | "отсортированный"
            | "splice"
            | "вырезать"
            | "toSpliced"
            | "вырезанный"
            | "with"
            | "сЗаменой"
            | "fill"
            | "заполнить"
            | "copyWithin"
            | "копироватьВнутри"
            | "entries"
            | "записи"
            | "keys"
            | "ключи"
            | "values"
            | "значения"
    )
}

pub(crate) fn string_length(s: &str) -> f64 {
    s.encode_utf16().count() as f64
}

pub(crate) fn call_string(
    vm: &mut Vm,
    s: Rc<str>,
    method: &str,
    args: Vec<Value>,
    span: Span,
) -> Result<Value, VmError> {
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
            Ok(Value::string(out))
        }
        "charCodeAt" | "кодСимволаВ" => {
            let idx = if args.is_empty() { 0.0 } else { as_number(&args[0], span, "charCodeAt")? };
            let units = utf16_units(&s);
            if idx < 0.0 || idx as usize >= units.len() {
                Ok(Value::Number(f64::NAN))
            } else {
                Ok(Value::Number(units[idx as usize] as f64))
            }
        }
        "indexOf" | "найтиПодстроку" => {
            require_args(&args, 1, span, "indexOf")?;
            let needle = as_string(&args[0], span, "indexOf")?;
            let units = utf16_units(&s);
            let from = if args.len() > 1 { clamped_unit_index(&args[1], units.len(), span, "indexOf")? } else { 0 };
            match utf16_find(&units, needle, from) {
                Some(pos) => Ok(Value::Number(pos as f64)),
                None => Ok(Value::Number(-1.0)),
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
            Ok(Value::Number(best as f64))
        }
        "includes" | "содержит" => {
            require_args(&args, 1, span, "includes")?;
            let needle = as_string(&args[0], span, "includes")?;
            let units = utf16_units(&s);
            let from = if args.len() > 1 { clamped_unit_index(&args[1], units.len(), span, "includes")? } else { 0 };
            Ok(Value::Bool(utf16_find(&units, needle, from).is_some()))
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
            Ok(Value::string(out))
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
            Ok(Value::string(out))
        }
        "toUpperCase" | "вВерхнийРегистр" => Ok(Value::string(s.to_uppercase())),
        "toLowerCase" | "вНижнийРегистр" => Ok(Value::string(s.to_lowercase())),
        "trim" | "обрезать" => Ok(Value::string(s.trim())),
        "trimStart" | "обрезатьСлева" => Ok(Value::string(s.trim_start())),
        "trimEnd" | "обрезатьСправа" => Ok(Value::string(s.trim_end())),
        "split" | "разбить" => {
            if args.is_empty() {
                return Ok(arr(vec![Value::string(Rc::clone(&s))]));
            }
            let limit = if args.len() > 1 && !matches!(args[1], Value::Undefined) {
                let n = as_number(&args[1], span, "split")?;
                if n.is_nan() || n < 0.0 { Some(0usize) } else { Some(n as usize) }
            } else {
                None
            };
            if let Some(0) = limit {
                return Ok(arr(Vec::new()));
            }
            if let Value::RegExp { compiled, .. } = &args[0] {
                let mut parts = crate::regexp::split_string(compiled, &s, span)?;
                if let Some(lim) = limit {
                    parts.truncate(lim);
                }
                return Ok(arr(parts));
            }
            let sep = as_string(&args[0], span, "split")?;
            let mut parts: Vec<Value> = if sep.is_empty() {
                s.chars().map(|c| Value::string(c.to_string())).collect()
            } else {
                s.split(sep).map(|p| Value::string(p.to_string())).collect()
            };
            if let Some(lim) = limit {
                parts.truncate(lim);
            }
            Ok(arr(parts))
        }
        "replace" | "заменить" => {
            require_args(&args, 2, span, "replace")?;
            if let Value::RegExp { compiled, flags, .. } = &args[0] {
                let compiled = Rc::clone(compiled);
                let global = flags.contains('g');
                match &args[1] {
                    Value::Str(rep) => {
                        let rep = rep.to_string();
                        let out = interp_replace_string(&compiled, &s, &rep, global, span)?;
                        return Ok(Value::string(out));
                    }
                    Value::Function(_) | Value::Builtin(_) => {
                        let fn_val = args[1].clone();
                        let out = crate::regexp::replace_with_fn(vm, &compiled, &s, fn_val, global, span)?;
                        return Ok(Value::string(out));
                    }
                    other => {
                        return Err(VmError::new(
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
                Ok(Value::string(out))
            } else {
                Ok(Value::string(Rc::clone(&s)))
            }
        }
        "replaceAll" | "заменитьВсе" => {
            require_args(&args, 2, span, "replaceAll")?;
            if let Value::RegExp { compiled, flags, .. } = &args[0] {
                if !flags.contains('g') {
                    return Err(VmError::new("replaceAll с regex требует флаг 'g'", span));
                }
                let compiled = Rc::clone(compiled);
                match &args[1] {
                    Value::Str(rep) => {
                        let rep = rep.to_string();
                        let out = interp_replace_string(&compiled, &s, &rep, true, span)?;
                        return Ok(Value::string(out));
                    }
                    Value::Function(_) | Value::Builtin(_) => {
                        let fn_val = args[1].clone();
                        let out = crate::regexp::replace_with_fn(vm, &compiled, &s, fn_val, true, span)?;
                        return Ok(Value::string(out));
                    }
                    other => {
                        return Err(VmError::new(
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
                return Ok(Value::string(s.replace(from, to)));
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
            Ok(Value::string(out))
        }
        "match" | "совпадает" => {
            require_args(&args, 1, span, "match")?;
            let (compiled, flags) = match &args[0] {
                Value::RegExp { compiled, flags, .. } => (compiled, flags),
                other => {
                    return Err(VmError::new(format!("'match' ожидает regex, получено '{}'", other.type_name()), span));
                }
            };
            if flags.contains('g') {
                Ok(arr(crate::regexp::match_global_strings(compiled, &s, span)?))
            } else {
                crate::regexp::match_first(compiled, &s, span)
            }
        }
        "matchAll" | "найтиВсе" => {
            require_args(&args, 1, span, "matchAll")?;
            let (compiled, flags) = match &args[0] {
                Value::RegExp { compiled, flags, .. } => (compiled, flags),
                other => {
                    return Err(VmError::new(
                        format!("'matchAll' ожидает regex, получено '{}'", other.type_name()),
                        span,
                    ));
                }
            };
            if !flags.contains('g') {
                return Err(VmError::new("matchAll требует флаг 'g'", span));
            }
            Ok(iter_of(crate::regexp::match_all_objects(compiled, &s, span)?))
        }
        "search" | "найтиИндекс" => {
            require_args(&args, 1, span, "search")?;
            let compiled = match &args[0] {
                Value::RegExp { compiled, .. } => compiled,
                other => {
                    return Err(VmError::new(
                        format!("'search' ожидает regex, получено '{}'", other.type_name()),
                        span,
                    ));
                }
            };
            Ok(Value::Number(interp_search_index(compiled, &s, span)? as f64))
        }
        "startsWith" | "начинаетсяС" => {
            require_args(&args, 1, span, "startsWith")?;
            let needle = as_string(&args[0], span, "startsWith")?;
            let units = utf16_units(&s);
            let pos = if args.len() > 1 { clamped_unit_index(&args[1], units.len(), span, "startsWith")? } else { 0 };
            let needle_units: Vec<u16> = needle.encode_utf16().collect();
            let ok =
                pos + needle_units.len() <= units.len() && units[pos..pos + needle_units.len()] == needle_units[..];
            Ok(Value::Bool(ok))
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
            Ok(Value::Bool(ok))
        }
        "repeat" | "повторить" => {
            require_args(&args, 1, span, "repeat")?;
            let count = as_number(&args[0], span, "repeat")?;
            if count < 0.0 || !count.is_finite() {
                return Err(VmError::new("Некорректное количество повторений", span));
            }
            let count = count as usize;
            if s.len().saturating_mul(count) > MAX_STRING_LEN {
                return Err(VmError::new("Превышен лимит длины строки", span));
            }
            Ok(Value::string(s.repeat(count)))
        }
        "padStart" | "дополнитьСлева" => {
            require_args(&args, 1, span, "padStart")?;
            let target_len = as_number(&args[0], span, "padStart")? as usize;
            let fill =
                if args.len() > 1 { as_string(&args[1], span, "padStart")?.to_string() } else { " ".to_string() };
            check_pad_budget(target_len, &fill, span)?;
            Ok(Value::string(pad(&s, target_len, &fill, true)))
        }
        "padEnd" | "дополнитьСправа" => {
            require_args(&args, 1, span, "padEnd")?;
            let target_len = as_number(&args[0], span, "padEnd")? as usize;
            let fill = if args.len() > 1 { as_string(&args[1], span, "padEnd")?.to_string() } else { " ".to_string() };
            check_pad_budget(target_len, &fill, span)?;
            Ok(Value::string(pad(&s, target_len, &fill, false)))
        }
        "at" | "поИндексу" => {
            require_args(&args, 1, span, "at")?;
            let idx = as_number(&args[0], span, "at")? as isize;
            let units = utf16_units(&s);
            let len = units.len() as isize;
            let real = if idx < 0 { len + idx } else { idx };
            if real < 0 || real >= len {
                Ok(Value::Undefined)
            } else {
                Ok(Value::string(String::from_utf16_lossy(&units[real as usize..real as usize + 1])))
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
                    return Err(VmError::new(format!("Некорректная форма нормализации: '{form}'"), span));
                }
            };
            Ok(Value::string(normalized))
        }
        "codePointAt" | "кодТочки" => {
            let idx = if args.is_empty() { 0.0 } else { as_number(&args[0], span, "codePointAt")? };
            let units = utf16_units(&s);
            if idx < 0.0 || !idx.is_finite() || idx as usize >= units.len() {
                return Ok(Value::Undefined);
            }
            let pos = idx as usize;
            let first = units[pos];
            let is_leading_surrogate = (0xD800..=0xDBFF).contains(&first);
            if is_leading_surrogate && pos + 1 < units.len() {
                let second = units[pos + 1];
                if (0xDC00..=0xDFFF).contains(&second) {
                    let code = 0x10000 + ((first as u32 - 0xD800) << 10) + (second as u32 - 0xDC00);
                    return Ok(Value::Number(code as f64));
                }
            }
            Ok(Value::Number(first as f64))
        }
        "concat" | "присоединить" => {
            let mut out = s.to_string();
            for a in args {
                out.push_str(&a.to_string());
            }
            Ok(Value::string(out))
        }
        _ => Err(VmError::new(format!("У строки нет метода '{method}'"), span)),
    }
}

pub(crate) fn call_array(
    vm: &mut Vm,
    rc: Rc<RefCell<Vec<Value>>>,
    method: &str,
    args: Vec<Value>,
    span: Span,
) -> Result<Value, VmError> {
    match method {
        "push" | "добавить" | "втолкнуть" => {
            let mut guard = rc.borrow_mut();
            for a in args {
                guard.push(a);
            }
            Ok(Value::Number(guard.len() as f64))
        }
        "pop" | "вытолкнуть" => Ok(rc.borrow_mut().pop().unwrap_or(Value::Undefined)),
        "shift" | "снять" => {
            let mut guard = rc.borrow_mut();
            if guard.is_empty() { Ok(Value::Undefined) } else { Ok(guard.remove(0)) }
        }
        "unshift" | "подсунуть" => {
            let mut guard = rc.borrow_mut();
            for (i, a) in args.into_iter().enumerate() {
                guard.insert(i, a);
            }
            Ok(Value::Number(guard.len() as f64))
        }
        "slice" | "отрезать" => {
            let snapshot = rc.borrow().clone();
            let len = snapshot.len() as isize;
            let start =
                if args.is_empty() { 0 } else { normalize_index(as_number(&args[0], span, "slice")? as isize, len) };
            let end =
                if args.len() < 2 { len } else { normalize_index(as_number(&args[1], span, "slice")? as isize, len) };
            let s = start.min(len).max(0) as usize;
            let e = end.min(len).max(0) as usize;
            let out = if s < e { snapshot[s..e].to_vec() } else { Vec::new() };
            Ok(arr(out))
        }
        "indexOf" | "найтиИндекс" => {
            require_args(&args, 1, span, "indexOf")?;
            let target = &args[0];
            let snapshot = rc.borrow().clone();
            let len = snapshot.len() as isize;
            let start = if args.len() > 1 {
                let raw = as_number(&args[1], span, "indexOf")? as isize;
                if raw < 0 { (len + raw).max(0) } else { raw }
            } else {
                0
            } as usize;
            let idx = snapshot
                .iter()
                .enumerate()
                .skip(start)
                .find(|(_, v)| crate::value::strict_eq(v, target))
                .map(|(i, _)| i);
            Ok(Value::Number(idx.map(|i| i as f64).unwrap_or(-1.0)))
        }
        "lastIndexOf" | "найтиПоследнийПо" => {
            require_args(&args, 1, span, "lastIndexOf")?;
            let target = &args[0];
            let snapshot = rc.borrow().clone();
            let len = snapshot.len() as isize;
            let start = if args.len() > 1 {
                let raw = as_number(&args[1], span, "lastIndexOf")? as isize;
                if raw < 0 { len + raw } else { raw.min(len - 1) }
            } else {
                len - 1
            };
            let mut idx = -1.0;
            let mut i = start;
            while i >= 0 {
                if crate::value::strict_eq(&snapshot[i as usize], target) {
                    idx = i as f64;
                    break;
                }
                i -= 1;
            }
            Ok(Value::Number(idx))
        }
        "includes" | "включает" => {
            require_args(&args, 1, span, "includes")?;
            let target = &args[0];
            let snapshot = rc.borrow().clone();
            let len = snapshot.len() as isize;
            let start = if args.len() > 1 {
                let raw = as_number(&args[1], span, "includes")? as isize;
                if raw < 0 { (len + raw).max(0) } else { raw }
            } else {
                0
            } as usize;
            let found = snapshot.iter().skip(start).any(|v| same_value_zero(v, target));
            Ok(Value::Bool(found))
        }
        "join" | "склеить" => {
            let sep = if args.is_empty() {
                ",".to_string()
            } else {
                match &args[0] {
                    Value::Undefined => ",".to_string(),
                    v => v.to_string(),
                }
            };
            let snapshot = rc.borrow().clone();
            let parts: Vec<String> = snapshot.iter().map(|v| join_element(v, &sep)).collect();
            Ok(Value::string(parts.join(&sep)))
        }
        "reverse" | "перевернуть" => {
            rc.borrow_mut().reverse();
            Ok(Value::Array(rc))
        }
        "concat" | "склеитьМассивы" => {
            let mut new_arr = rc.borrow().clone();
            for a in args {
                match a {
                    Value::Array(inner) => new_arr.extend(inner.borrow().iter().cloned()),
                    other => new_arr.push(other),
                }
            }
            Ok(arr(new_arr))
        }
        "sort" | "сортировать" => {
            let mut snapshot = rc.borrow().clone();
            sort_snapshot(vm, &mut snapshot, args, span)?;
            *rc.borrow_mut() = snapshot;
            Ok(Value::Array(rc))
        }
        "map" | "преобразовать" => {
            require_args(&args, 1, span, "map")?;
            let callback = args.into_iter().next().unwrap();
            let snapshot = rc.borrow().clone();
            let mut result = Vec::with_capacity(snapshot.len());
            for (i, el) in snapshot.into_iter().enumerate() {
                let v = vm.call_value(
                    callback.clone(),
                    None,
                    &[el, Value::Number(i as f64), Value::Array(Rc::clone(&rc))],
                    span,
                )?;
                result.push(v);
            }
            Ok(arr(result))
        }
        "filter" | "отфильтровать" => {
            require_args(&args, 1, span, "filter")?;
            let callback = args.into_iter().next().unwrap();
            let snapshot = rc.borrow().clone();
            let mut result = Vec::new();
            for (i, el) in snapshot.into_iter().enumerate() {
                let keep = vm.call_value(
                    callback.clone(),
                    None,
                    &[el.clone(), Value::Number(i as f64), Value::Array(Rc::clone(&rc))],
                    span,
                )?;
                if keep.is_truthy() {
                    result.push(el);
                }
            }
            Ok(arr(result))
        }
        "reduce" | "свернуть" => {
            require_args(&args, 1, span, "reduce")?;
            let mut iter = args.into_iter();
            let callback = iter.next().unwrap();
            let initial = iter.next();
            let snapshot = rc.borrow().clone();
            let mut acc = match initial {
                Some(v) => v,
                None => {
                    if snapshot.is_empty() {
                        return Err(VmError::new("reduce пустого массива без начального значения", span));
                    }
                    let mut it = snapshot.into_iter();
                    let mut acc = it.next().unwrap();
                    for (i, el) in it.enumerate() {
                        acc = vm.call_value(
                            callback.clone(),
                            None,
                            &[acc, el, Value::Number((i + 1) as f64), Value::Array(Rc::clone(&rc))],
                            span,
                        )?;
                    }
                    return Ok(acc);
                }
            };
            for (i, el) in snapshot.into_iter().enumerate() {
                acc = vm.call_value(
                    callback.clone(),
                    None,
                    &[acc, el, Value::Number(i as f64), Value::Array(Rc::clone(&rc))],
                    span,
                )?;
            }
            Ok(acc)
        }
        "reduceRight" | "свернутьСправа" => {
            require_args(&args, 1, span, "reduceRight")?;
            let mut iter = args.into_iter();
            let callback = iter.next().unwrap();
            let initial = iter.next();
            let snapshot = rc.borrow().clone();
            let len = snapshot.len();
            match initial {
                Some(v) => {
                    let mut acc = v;
                    for i in (0..len).rev() {
                        acc = vm.call_value(
                            callback.clone(),
                            None,
                            &[acc, snapshot[i].clone(), Value::Number(i as f64), Value::Array(Rc::clone(&rc))],
                            span,
                        )?;
                    }
                    Ok(acc)
                }
                None => {
                    if snapshot.is_empty() {
                        return Err(VmError::new("reduceRight пустого массива без начального значения", span));
                    }
                    let mut acc = snapshot[len - 1].clone();
                    for i in (0..len - 1).rev() {
                        acc = vm.call_value(
                            callback.clone(),
                            None,
                            &[acc, snapshot[i].clone(), Value::Number(i as f64), Value::Array(Rc::clone(&rc))],
                            span,
                        )?;
                    }
                    Ok(acc)
                }
            }
        }
        "forEach" | "каждый" => {
            require_args(&args, 1, span, "forEach")?;
            let callback = args.into_iter().next().unwrap();
            let snapshot = rc.borrow().clone();
            for (i, el) in snapshot.into_iter().enumerate() {
                vm.call_value(
                    callback.clone(),
                    None,
                    &[el, Value::Number(i as f64), Value::Array(Rc::clone(&rc))],
                    span,
                )?;
            }
            Ok(Value::Undefined)
        }
        "find" | "найти" => {
            require_args(&args, 1, span, "find")?;
            let callback = args.into_iter().next().unwrap();
            let snapshot = rc.borrow().clone();
            for (i, el) in snapshot.into_iter().enumerate() {
                let matched = vm.call_value(
                    callback.clone(),
                    None,
                    &[el.clone(), Value::Number(i as f64), Value::Array(Rc::clone(&rc))],
                    span,
                )?;
                if matched.is_truthy() {
                    return Ok(el);
                }
            }
            Ok(Value::Undefined)
        }
        "findIndex" | "найтиИндексПо" => {
            require_args(&args, 1, span, "findIndex")?;
            let callback = args.into_iter().next().unwrap();
            let snapshot = rc.borrow().clone();
            for (i, el) in snapshot.into_iter().enumerate() {
                let matched = vm.call_value(
                    callback.clone(),
                    None,
                    &[el, Value::Number(i as f64), Value::Array(Rc::clone(&rc))],
                    span,
                )?;
                if matched.is_truthy() {
                    return Ok(Value::Number(i as f64));
                }
            }
            Ok(Value::Number(-1.0))
        }
        "some" | "некоторые" => {
            require_args(&args, 1, span, "some")?;
            let callback = args.into_iter().next().unwrap();
            let snapshot = rc.borrow().clone();
            for (i, el) in snapshot.into_iter().enumerate() {
                let matched = vm.call_value(
                    callback.clone(),
                    None,
                    &[el, Value::Number(i as f64), Value::Array(Rc::clone(&rc))],
                    span,
                )?;
                if matched.is_truthy() {
                    return Ok(Value::Bool(true));
                }
            }
            Ok(Value::Bool(false))
        }
        "every" | "все" => {
            require_args(&args, 1, span, "every")?;
            let callback = args.into_iter().next().unwrap();
            let snapshot = rc.borrow().clone();
            for (i, el) in snapshot.into_iter().enumerate() {
                let matched = vm.call_value(
                    callback.clone(),
                    None,
                    &[el, Value::Number(i as f64), Value::Array(Rc::clone(&rc))],
                    span,
                )?;
                if !matched.is_truthy() {
                    return Ok(Value::Bool(false));
                }
            }
            Ok(Value::Bool(true))
        }
        "at" | "поИндексу" => {
            require_args(&args, 1, span, "at")?;
            let idx = as_number(&args[0], span, "at")? as isize;
            let guard = rc.borrow();
            let len = guard.len() as isize;
            let real = if idx < 0 { len + idx } else { idx };
            if real < 0 || real >= len { Ok(Value::Undefined) } else { Ok(guard[real as usize].clone()) }
        }
        "flat" | "плоский" => {
            let depth = if args.is_empty() { 1.0 } else { as_number(&args[0], span, "flat")? };
            let snapshot = rc.borrow().clone();
            Ok(arr(flatten(snapshot, depth as isize)))
        }
        "flatMap" | "плоскоПреобразовать" => {
            require_args(&args, 1, span, "flatMap")?;
            let callback = args.into_iter().next().unwrap();
            let snapshot = rc.borrow().clone();
            let mut result = Vec::new();
            for (i, el) in snapshot.into_iter().enumerate() {
                let v = vm.call_value(
                    callback.clone(),
                    None,
                    &[el, Value::Number(i as f64), Value::Array(Rc::clone(&rc))],
                    span,
                )?;
                match v {
                    Value::Array(inner) => result.extend(inner.borrow().iter().cloned()),
                    other => result.push(other),
                }
            }
            Ok(arr(result))
        }
        "findLast" | "найтиПоследний" => {
            require_args(&args, 1, span, "findLast")?;
            let callback = args.into_iter().next().unwrap();
            let snapshot = rc.borrow().clone();
            for i in (0..snapshot.len()).rev() {
                let el = snapshot[i].clone();
                let matched = vm.call_value(
                    callback.clone(),
                    None,
                    &[el.clone(), Value::Number(i as f64), Value::Array(Rc::clone(&rc))],
                    span,
                )?;
                if matched.is_truthy() {
                    return Ok(el);
                }
            }
            Ok(Value::Undefined)
        }
        "findLastIndex" | "найтиПоследнийИндекс" => {
            require_args(&args, 1, span, "findLastIndex")?;
            let callback = args.into_iter().next().unwrap();
            let snapshot = rc.borrow().clone();
            for i in (0..snapshot.len()).rev() {
                let el = snapshot[i].clone();
                let matched = vm.call_value(
                    callback.clone(),
                    None,
                    &[el, Value::Number(i as f64), Value::Array(Rc::clone(&rc))],
                    span,
                )?;
                if matched.is_truthy() {
                    return Ok(Value::Number(i as f64));
                }
            }
            Ok(Value::Number(-1.0))
        }
        "toReversed" | "перевёрнутый" => {
            let mut new_arr = rc.borrow().clone();
            new_arr.reverse();
            Ok(arr(new_arr))
        }
        "toSorted" | "отсортированный" => {
            let mut new_arr = rc.borrow().clone();
            sort_snapshot(vm, &mut new_arr, args, span)?;
            Ok(arr(new_arr))
        }
        "splice" | "вырезать" => {
            let snapshot = rc.borrow().clone();
            let (new_arr, removed) = splice_impl(snapshot, &args, span)?;
            *rc.borrow_mut() = new_arr;
            Ok(arr(removed))
        }
        "toSpliced" | "вырезанный" => {
            let snapshot = rc.borrow().clone();
            let (new_arr, _removed) = splice_impl(snapshot, &args, span)?;
            Ok(arr(new_arr))
        }
        "with" | "сЗаменой" => {
            require_args(&args, 2, span, "with")?;
            let idx = as_number(&args[0], span, "with")? as isize;
            let mut new_arr = rc.borrow().clone();
            let len = new_arr.len() as isize;
            let real = if idx < 0 { len + idx } else { idx };
            if real < 0 || real >= len {
                return Err(VmError::new(format!("Индекс {idx} вне диапазона"), span));
            }
            new_arr[real as usize] = args.into_iter().nth(1).unwrap();
            Ok(arr(new_arr))
        }
        "fill" | "заполнить" => {
            let len = rc.borrow().len() as isize;
            let mut args = args.into_iter();
            let value = args.next().unwrap_or(Value::Undefined);
            let start = match args.next() {
                Some(v) => clamp_index(as_number(&v, span, "fill")? as isize, len),
                None => 0,
            };
            let end = match args.next() {
                Some(v) => clamp_index(as_number(&v, span, "fill")? as isize, len),
                None => len,
            };
            if start < end {
                let mut guard = rc.borrow_mut();
                for slot in &mut guard[start as usize..end as usize] {
                    *slot = value.clone();
                }
            }
            Ok(Value::Array(rc))
        }
        "copyWithin" | "копироватьВнутри" => {
            require_args(&args, 1, span, "copyWithin")?;
            let len = rc.borrow().len() as isize;
            let mut args = args.into_iter();
            let target = clamp_index(as_number(&args.next().unwrap(), span, "copyWithin")? as isize, len);
            let start = match args.next() {
                Some(v) => clamp_index(as_number(&v, span, "copyWithin")? as isize, len),
                None => 0,
            };
            let end = match args.next() {
                Some(v) => clamp_index(as_number(&v, span, "copyWithin")? as isize, len),
                None => len,
            };
            let count = (end - start).max(0).min(len - target);
            if count > 0 {
                let snapshot = rc.borrow().clone();
                let mut guard = rc.borrow_mut();
                for i in 0..count as usize {
                    guard[target as usize + i] = snapshot[start as usize + i].clone();
                }
            }
            Ok(Value::Array(rc))
        }
        "entries" | "записи" => {
            let entries: Vec<Value> =
                rc.borrow().iter().enumerate().map(|(i, v)| arr(vec![Value::Number(i as f64), v.clone()])).collect();
            Ok(iter_of(entries))
        }
        "keys" | "ключи" => {
            let len = rc.borrow().len();
            Ok(iter_of((0..len).map(|i| Value::Number(i as f64)).collect()))
        }
        "values" | "значения" => Ok(iter_of(rc.borrow().clone())),
        _ => Err(VmError::new(format!("У массива нет метода '{method}'"), span)),
    }
}

fn interp_replace_string(
    re: &Rc<crate::regexp::YopRegex>,
    s: &str,
    replacement: &str,
    global: bool,
    span: Span,
) -> Result<String, VmError> {
    yps_interpreter::stdlib::regexp::replace_string(re, s, replacement, global, span)
        .map_err(|e| VmError::new(e.message, e.span))
}

fn interp_search_index(re: &Rc<crate::regexp::YopRegex>, s: &str, span: Span) -> Result<i64, VmError> {
    yps_interpreter::stdlib::regexp::search_index(re, s, span).map_err(|e| VmError::new(e.message, e.span))
}

fn sort_snapshot(vm: &mut Vm, arr: &mut [Value], args: Vec<Value>, span: Span) -> Result<(), VmError> {
    if args.is_empty() {
        arr.sort_by_key(|a| a.to_string());
        return Ok(());
    }
    let cmp = args.into_iter().next().unwrap();
    let mut err: Option<VmError> = None;
    arr.sort_by(|a, b| {
        if err.is_some() {
            return Ordering::Equal;
        }
        match vm.call_value(cmp.clone(), None, &[a.clone(), b.clone()], span) {
            Ok(Value::Number(n)) if n < 0.0 => Ordering::Less,
            Ok(Value::Number(n)) if n > 0.0 => Ordering::Greater,
            Ok(_) => Ordering::Equal,
            Err(e) => {
                err = Some(e);
                Ordering::Equal
            }
        }
    });
    if let Some(e) = err {
        return Err(e);
    }
    Ok(())
}

fn normalize_index(idx: isize, len: isize) -> isize {
    if idx < 0 { (len + idx).max(0) } else { idx }
}

fn clamp_index(idx: isize, len: isize) -> isize {
    normalize_index(idx, len).min(len)
}

fn splice_impl(arr_in: Vec<Value>, args: &[Value], span: Span) -> Result<(Vec<Value>, Vec<Value>), VmError> {
    let len = arr_in.len() as isize;
    let start_raw = if args.is_empty() { 0 } else { as_number(&args[0], span, "splice")? as isize };
    let start = if start_raw < 0 { (len + start_raw).max(0) } else { start_raw.min(len) } as usize;
    let delete_count = if args.len() < 2 {
        arr_in.len() - start
    } else {
        let n = as_number(&args[1], span, "splice")? as isize;
        n.max(0).min(len - start as isize) as usize
    };
    let inserts: Vec<Value> = if args.len() > 2 { args[2..].to_vec() } else { Vec::new() };
    let mut new_arr = arr_in;
    let removed: Vec<Value> = new_arr.splice(start..start + delete_count, inserts).collect();
    Ok((new_arr, removed))
}

fn flatten(arr_in: Vec<Value>, depth: isize) -> Vec<Value> {
    let mut result = Vec::new();
    for v in arr_in {
        match v {
            Value::Array(inner) if depth > 0 => {
                result.extend(flatten(inner.borrow().clone(), depth - 1));
            }
            other => result.push(other),
        }
    }
    result
}

fn join_element(v: &Value, sep: &str) -> String {
    match v {
        Value::Null | Value::Undefined => String::new(),
        Value::Array(inner) => {
            let parts: Vec<String> = inner.borrow().iter().map(|e| join_element(e, sep)).collect();
            parts.join(sep)
        }
        other => other.to_string(),
    }
}

fn check_pad_budget(target_len: usize, fill: &str, span: Span) -> Result<(), VmError> {
    let max_char_bytes = fill.chars().map(|c| c.len_utf8()).max().unwrap_or(1);
    if target_len.saturating_mul(max_char_bytes) > MAX_STRING_LEN {
        return Err(VmError::new("Превышен лимит длины строки", span));
    }
    Ok(())
}

fn clamped_unit_index(arg: &Value, len: usize, span: Span, method: &str) -> Result<usize, VmError> {
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
