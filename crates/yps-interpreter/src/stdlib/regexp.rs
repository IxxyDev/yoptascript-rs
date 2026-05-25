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
            'g' | 'u' => {}
            'y' => return Err(RuntimeError::new("флаг 'y' не поддерживается", span)),
            'd' => return Err(RuntimeError::new("флаг 'd' не поддерживается", span)),
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

fn build_match_object(caps: &regex::Captures<'_>, s: &str, re: &regex::Regex) -> Value {
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
    let groups_val = if has_named { Value::Object(groups) } else { Value::Null };
    map.insert("groups".to_string(), groups_val);
    Value::Object(map)
}

pub fn match_first(re: &Rc<regex::Regex>, s: &str) -> Value {
    match re.captures(s) {
        Some(caps) => build_match_object(&caps, s, re),
        None => Value::Null,
    }
}

pub fn match_all_global(re: &Rc<regex::Regex>, s: &str) -> Vec<Value> {
    re.find_iter(s).map(|m| Value::String(m.as_str().to_string())).collect()
}

pub fn match_all_detailed(re: &Rc<regex::Regex>, s: &str) -> Vec<Value> {
    re.captures_iter(s).map(|caps| build_match_object(&caps, s, re)).collect()
}

pub fn split_string(re: &Rc<regex::Regex>, s: &str) -> Vec<String> {
    re.split(s).map(|p| p.to_string()).collect()
}

pub fn replace_string(re: &Rc<regex::Regex>, s: &str, replacement: &str, global: bool) -> String {
    let rep = ReplacementTemplate::new(replacement);
    if global { re.replace_all(s, &rep).into_owned() } else { re.replace(s, &rep).into_owned() }
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
                        let idx = (nb - b'0') as usize;
                        if let Some(Some(m)) = caps.get(idx).map(Some) {
                            dst.push_str(m.as_str());
                        }
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
            if flags.contains('g') {
                return Err(RuntimeError::new("флаг g не поддержан в exec, используй match/matchAll", span));
            }
            let s = as_string(&args[0], span, "regex.найти")?;
            match_first(&compiled, s)
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
        "global" | "глобальный" => Some(Value::Boolean(flags.contains('g'))),
        "ignoreCase" | "игнорРегистр" => Some(Value::Boolean(flags.contains('i'))),
        "multiline" | "многострочный" => Some(Value::Boolean(flags.contains('m'))),
        _ => None,
    }
}
