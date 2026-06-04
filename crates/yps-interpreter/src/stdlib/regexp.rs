use std::collections::HashMap;
use std::rc::Rc;

use yps_lexer::Span;

use crate::error::RuntimeError;
use crate::interpreter::Interpreter;
use crate::stdlib::{as_string, require_args};
use crate::value::Value;

pub fn compile(pattern: &str, flags: &str, span: Span) -> Result<Rc<regex::Regex>, RuntimeError> {
    validate_pattern(pattern, span)?;
    let transformed = pre_transform(pattern);

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
    let full = format!("{prefix}{transformed}");
    regex::Regex::new(&full).map(Rc::new).map_err(|e| RuntimeError::new(format!("Ошибка regex /{pattern}/: {e}"), span))
}

fn validate_pattern(pattern: &str, span: Span) -> Result<(), RuntimeError> {
    let bytes = pattern.as_bytes();
    let mut i = 0;
    let mut in_class = false;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'\\' {
            if i + 1 < bytes.len() {
                let nxt = bytes[i + 1];
                if !in_class && (b'1'..=b'9').contains(&nxt) {
                    return Err(RuntimeError::new("backreferences не поддерживаются", span));
                }
                i += 2;
                continue;
            }
            i += 1;
            continue;
        }
        if !in_class {
            if b == b'[' {
                in_class = true;
                i += 1;
                continue;
            }
            if b == b'(' && i + 3 < bytes.len() && bytes[i + 1] == b'?' && bytes[i + 2] == b'<' {
                let c3 = bytes[i + 3];
                if c3 == b'=' || c3 == b'!' {
                    return Err(RuntimeError::new("lookbehind не поддерживается", span));
                }
            }
        } else if b == b']' {
            in_class = false;
        }
        i += 1;
    }
    Ok(())
}

fn pre_transform(pattern: &str) -> String {
    let bytes = pattern.as_bytes();
    let mut out = String::with_capacity(bytes.len());
    let mut i = 0;
    let mut in_class = false;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'\\' {
            out.push(b as char);
            if i + 1 < bytes.len() {
                let nb = bytes[i + 1];
                if nb < 0x80 {
                    out.push(nb as char);
                } else {
                    let ch_start = i + 1;
                    let rest = &pattern[ch_start..];
                    if let Some(ch) = rest.chars().next() {
                        out.push(ch);
                        i = ch_start + ch.len_utf8();
                        continue;
                    }
                }
                i += 2;
                continue;
            }
            i += 1;
            continue;
        }
        if !in_class {
            if b == b'[' {
                in_class = true;
                out.push('[');
                i += 1;
                continue;
            }
            if b == b'(' && i + 2 < bytes.len() && bytes[i + 1] == b'?' && bytes[i + 2] == b'<' {
                let after = if i + 3 < bytes.len() { bytes[i + 3] } else { 0 };
                if after != b'=' && after != b'!' {
                    out.push_str("(?P<");
                    i += 3;
                    continue;
                }
            }
        } else if b == b']' {
            in_class = false;
            out.push(']');
            i += 1;
            continue;
        }
        if b < 0x80 {
            out.push(b as char);
            i += 1;
        } else {
            let rest = &pattern[i..];
            if let Some(ch) = rest.chars().next() {
                out.push(ch);
                i += ch.len_utf8();
            } else {
                i += 1;
            }
        }
    }
    out
}

fn char_index_at(s: &str, byte_pos: usize) -> i64 {
    s[..byte_pos].chars().count() as i64
}

pub fn build_match_object(caps: &regex::Captures<'_>, s: &str, re: &regex::Regex, with_indices: bool) -> Value {
    let mut map: HashMap<String, Value> = HashMap::new();
    for i in 0..caps.len() {
        let key = i.to_string();
        match caps.get(i) {
            Some(m) => map.insert(key, Value::String(m.as_str().to_string())),
            None => map.insert(key, Value::Null),
        };
    }
    let whole = caps.get(0).expect("match group 0");
    map.insert("index".to_string(), Value::Number(char_index_at(s, whole.start()) as f64));
    map.insert("input".to_string(), Value::String(s.to_string()));

    let mut groups: HashMap<String, Value> = HashMap::new();
    let mut has_named = false;
    for name_opt in re.capture_names().flatten() {
        has_named = true;
        match caps.name(name_opt) {
            Some(m) => groups.insert(name_opt.to_string(), Value::String(m.as_str().to_string())),
            None => groups.insert(name_opt.to_string(), Value::Null),
        };
    }
    let groups_val = if has_named { Value::object(groups) } else { Value::Null };
    map.insert("groups".to_string(), groups_val);

    if with_indices {
        let pair = |m: regex::Match<'_>| {
            Value::array(vec![
                Value::Number(char_index_at(s, m.start()) as f64),
                Value::Number(char_index_at(s, m.end()) as f64),
            ])
        };
        let mut indices_obj: HashMap<String, Value> = HashMap::new();
        for i in 0..caps.len() {
            let v = caps.get(i).map(pair).unwrap_or(Value::Null);
            indices_obj.insert(i.to_string(), v);
        }
        let mut named_groups: HashMap<String, Value> = HashMap::new();
        for name in re.capture_names().flatten() {
            let v = caps.name(name).map(pair).unwrap_or(Value::Null);
            named_groups.insert(name.to_string(), v);
        }
        let groups_d = if named_groups.is_empty() { Value::Null } else { Value::object(named_groups) };
        indices_obj.insert("groups".to_string(), groups_d);
        map.insert("indices".to_string(), Value::object(indices_obj));
    }

    Value::object(map)
}

pub fn match_first(re: &Rc<regex::Regex>, s: &str) -> Value {
    match re.captures(s) {
        Some(caps) => build_match_object(&caps, s, re, false),
        None => Value::Null,
    }
}

fn char_to_byte(s: &str, char_idx: usize) -> Option<usize> {
    if char_idx == 0 {
        return Some(0);
    }
    let mut count = 0;
    for (b, _) in s.char_indices() {
        if count == char_idx {
            return Some(b);
        }
        count += 1;
    }
    if count == char_idx { Some(s.len()) } else { None }
}

pub fn exec_stateful(re: &Rc<regex::Regex>, flags: &str, last_index: &Rc<std::cell::RefCell<usize>>, s: &str) -> Value {
    let stateful = flags.contains('g') || flags.contains('y');
    let has_indices = flags.contains('d');
    if !stateful {
        return match re.captures(s) {
            Some(caps) => build_match_object(&caps, s, re, has_indices),
            None => Value::Null,
        };
    }
    let li = *last_index.borrow();
    let byte_pos = match char_to_byte(s, li) {
        Some(b) => b,
        None => {
            *last_index.borrow_mut() = 0;
            return Value::Null;
        }
    };
    let caps_opt = re.captures_at(s, byte_pos);
    match caps_opt {
        Some(caps) => {
            let whole = caps.get(0).expect("match group 0");
            if flags.contains('y') && whole.start() != byte_pos {
                *last_index.borrow_mut() = 0;
                return Value::Null;
            }
            let new_li = s[..whole.end()].chars().count();
            *last_index.borrow_mut() = new_li;
            build_match_object(&caps, s, re, has_indices)
        }
        None => {
            *last_index.borrow_mut() = 0;
            Value::Null
        }
    }
}

pub fn match_all_global(re: &Rc<regex::Regex>, s: &str) -> Vec<Value> {
    re.find_iter(s).map(|m| Value::String(m.as_str().to_string())).collect()
}

pub fn match_all_detailed(re: &Rc<regex::Regex>, s: &str) -> Vec<Value> {
    re.captures_iter(s).map(|caps| build_match_object(&caps, s, re, false)).collect()
}

pub fn split_string(re: &Rc<regex::Regex>, s: &str) -> Vec<String> {
    re.split(s).map(|p| p.to_string()).collect()
}

pub fn replace_string(re: &Rc<regex::Regex>, s: &str, replacement: &str, global: bool) -> String {
    let rep = ReplacementTemplate::new(replacement);
    if global { re.replace_all(s, &rep).into_owned() } else { re.replace(s, &rep).into_owned() }
}

pub fn replace_with_fn(
    interp: &mut Interpreter,
    re: &Rc<regex::Regex>,
    s: &str,
    fn_val: Value,
    global: bool,
    span: Span,
) -> Result<String, RuntimeError> {
    let mut out = String::new();
    let mut last_end = 0usize;
    for caps in re.captures_iter(s) {
        let whole = caps.get(0).expect("match group 0");
        out.push_str(&s[last_end..whole.start()]);
        let mut call_args: Vec<Value> = Vec::with_capacity(caps.len() + 2);
        call_args.push(Value::String(whole.as_str().to_string()));
        for i in 1..caps.len() {
            match caps.get(i) {
                Some(m) => call_args.push(Value::String(m.as_str().to_string())),
                None => call_args.push(Value::Null),
            }
        }
        let char_offset = char_index_at(s, whole.start());
        call_args.push(Value::Number(char_offset as f64));
        call_args.push(Value::String(s.to_string()));
        let returned = interp.call_function(fn_val.clone(), call_args, span)?;
        let rep = match returned {
            Value::String(rs) => rs,
            other => other.to_string(),
        };
        out.push_str(&rep);
        last_end = whole.end();
        if !global {
            break;
        }
    }
    out.push_str(&s[last_end..]);
    Ok(out)
}

pub fn search_index(re: &Rc<regex::Regex>, s: &str) -> i64 {
    match re.find(s) {
        Some(m) => char_index_at(s, m.start()),
        None => -1,
    }
}

struct ReplacementTemplate<'a> {
    src: &'a str,
}

impl<'a> ReplacementTemplate<'a> {
    fn new(src: &'a str) -> Self {
        Self { src }
    }
}

impl regex::Replacer for &ReplacementTemplate<'_> {
    fn replace_append(&mut self, caps: &regex::Captures<'_>, dst: &mut String) {
        let bytes = self.src.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            let b = bytes[i];
            if b == b'$' && i + 1 < bytes.len() {
                let nb = bytes[i + 1];
                match nb {
                    b'&' => {
                        if let Some(m) = caps.get(0) {
                            dst.push_str(m.as_str());
                        }
                        i += 2;
                        continue;
                    }
                    b'$' => {
                        dst.push('$');
                        i += 2;
                        continue;
                    }
                    b'0'..=b'9' => {
                        let d1 = (nb - b'0') as usize;
                        let total = caps.len();
                        let max_group = total.saturating_sub(1);
                        let two_digit = if i + 2 < bytes.len() {
                            let nb2 = bytes[i + 2];
                            if nb2.is_ascii_digit() { Some((nb2 - b'0') as usize) } else { None }
                        } else {
                            None
                        };
                        if let Some(d2) = two_digit {
                            let nn = d1 * 10 + d2;
                            if nn != 0 && nn <= max_group {
                                if let Some(m) = caps.get(nn) {
                                    dst.push_str(m.as_str());
                                }
                                i += 3;
                                continue;
                            }
                            if d1 != 0 && d1 <= max_group {
                                if let Some(m) = caps.get(d1) {
                                    dst.push_str(m.as_str());
                                }
                                dst.push(bytes[i + 2] as char);
                                i += 3;
                                continue;
                            }
                            dst.push('$');
                            dst.push(nb as char);
                            dst.push(bytes[i + 2] as char);
                            i += 3;
                            continue;
                        }
                        if d1 != 0 && d1 <= max_group {
                            if let Some(m) = caps.get(d1) {
                                dst.push_str(m.as_str());
                            }
                            i += 2;
                            continue;
                        }
                        dst.push('$');
                        dst.push(nb as char);
                        i += 2;
                        continue;
                    }
                    b'<' => {
                        if let Some(end_off) = bytes[i + 2..].iter().position(|&c| c == b'>') {
                            let name_start = i + 2;
                            let name_end = name_start + end_off;
                            let name = &self.src[name_start..name_end];
                            if let Some(m) = caps.name(name) {
                                dst.push_str(m.as_str());
                            }
                            i = name_end + 1;
                            continue;
                        }
                    }
                    _ => {}
                }
            }
            if b < 0x80 {
                dst.push(b as char);
                i += 1;
            } else {
                let rest = &self.src[i..];
                if let Some(ch) = rest.chars().next() {
                    dst.push(ch);
                    i += ch.len_utf8();
                } else {
                    i += 1;
                }
            }
        }
    }
}

pub fn call(
    _interp: &mut Interpreter,
    receiver: Value,
    method: &str,
    args: Vec<Value>,
    span: Span,
) -> Result<(Value, Option<Value>), RuntimeError> {
    let (pattern, flags, compiled, last_index) = match &receiver {
        Value::RegExp { pattern, flags, compiled, last_index } => {
            (pattern.clone(), flags.clone(), Rc::clone(compiled), Rc::clone(last_index))
        }
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
            exec_stateful(&compiled, &flags, &last_index, s)
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
    let (pattern, flags, last_index) = match receiver {
        Value::RegExp { pattern, flags, last_index, .. } => (pattern, flags, last_index),
        _ => return None,
    };
    match property {
        "источник" | "source" => Some(Value::String(pattern.clone())),
        "флаги" | "flags" => Some(Value::String(flags.clone())),
        "global" | "глобальный" => Some(Value::Boolean(flags.contains('g'))),
        "ignoreCase" | "игнорРегистр" => Some(Value::Boolean(flags.contains('i'))),
        "multiline" | "многострочный" => Some(Value::Boolean(flags.contains('m'))),
        "sticky" | "липкий" => Some(Value::Boolean(flags.contains('y'))),
        "hasIndices" | "имеетИндексы" => Some(Value::Boolean(flags.contains('d'))),
        "lastIndex" | "последнийИндекс" => Some(Value::Number(*last_index.borrow() as f64)),
        _ => None,
    }
}
