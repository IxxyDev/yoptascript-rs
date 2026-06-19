use std::cell::RefCell;
use std::rc::Rc;

use yps_lexer::Span;

use crate::error::VmError;
use crate::value::{ObjMap, Value};

const FANCY_BACKTRACK_LIMIT: usize = 1_000_000;

#[derive(Debug, Clone)]
pub enum YopRegex {
    Fast(regex::Regex),
    Fancy(fancy_regex::Regex),
}

pub struct GroupSlot {
    pub text: String,
    pub start: usize,
    pub end: usize,
}

pub struct MatchData {
    pub groups: Vec<Option<GroupSlot>>,
    pub named: Vec<(String, Option<GroupSlot>)>,
}

impl MatchData {
    fn whole(&self) -> &GroupSlot {
        self.groups[0].as_ref().expect("match group 0")
    }

    fn from_fast(caps: &regex::Captures<'_>, re: &regex::Regex) -> MatchData {
        let groups = (0..caps.len())
            .map(|i| caps.get(i).map(|m| GroupSlot { text: m.as_str().to_string(), start: m.start(), end: m.end() }))
            .collect();
        let named = re
            .capture_names()
            .flatten()
            .map(|name| {
                let slot =
                    caps.name(name).map(|m| GroupSlot { text: m.as_str().to_string(), start: m.start(), end: m.end() });
                (name.to_string(), slot)
            })
            .collect();
        MatchData { groups, named }
    }

    fn from_fancy(caps: &fancy_regex::Captures<'_>, re: &fancy_regex::Regex) -> MatchData {
        let groups = (0..caps.len())
            .map(|i| caps.get(i).map(|m| GroupSlot { text: m.as_str().to_string(), start: m.start(), end: m.end() }))
            .collect();
        let named = re
            .capture_names()
            .flatten()
            .map(|name| {
                let slot =
                    caps.name(name).map(|m| GroupSlot { text: m.as_str().to_string(), start: m.start(), end: m.end() });
                (name.to_string(), slot)
            })
            .collect();
        MatchData { groups, named }
    }
}

fn fancy_err(e: fancy_regex::Error, span: Span) -> VmError {
    VmError::new(format!("Ошибка выполнения regex: {e}"), span)
}

impl YopRegex {
    pub fn is_match(&self, s: &str, span: Span) -> Result<bool, VmError> {
        match self {
            YopRegex::Fast(re) => Ok(re.is_match(s)),
            YopRegex::Fancy(re) => re.is_match(s).map_err(|e| fancy_err(e, span)),
        }
    }

    pub fn captures(&self, s: &str, span: Span) -> Result<Option<MatchData>, VmError> {
        match self {
            YopRegex::Fast(re) => Ok(re.captures(s).map(|c| MatchData::from_fast(&c, re))),
            YopRegex::Fancy(re) => {
                re.captures(s).map(|opt| opt.map(|c| MatchData::from_fancy(&c, re))).map_err(|e| fancy_err(e, span))
            }
        }
    }

    pub fn captures_from_pos(&self, s: &str, pos: usize, span: Span) -> Result<Option<MatchData>, VmError> {
        match self {
            YopRegex::Fast(re) => Ok(re.captures_at(s, pos).map(|c| MatchData::from_fast(&c, re))),
            YopRegex::Fancy(re) => re
                .captures_from_pos(s, pos)
                .map(|opt| opt.map(|c| MatchData::from_fancy(&c, re)))
                .map_err(|e| fancy_err(e, span)),
        }
    }
}

pub fn compile(pattern: &str, flags: &str, span: Span) -> Result<Rc<YopRegex>, VmError> {
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
                return Err(VmError::new(format!("Неизвестный флаг regex: '{other}'"), span));
            }
        }
    }
    if has_inline {
        prefix.push(')');
    }
    let full = format!("{prefix}{transformed}");

    if needs_fancy(pattern) {
        let re = fancy_regex::RegexBuilder::new(&full)
            .backtrack_limit(FANCY_BACKTRACK_LIMIT)
            .build()
            .map_err(|e| VmError::new(format!("Ошибка regex /{pattern}/: {e}"), span))?;
        Ok(Rc::new(YopRegex::Fancy(re)))
    } else {
        let re = regex::Regex::new(&full).map_err(|e| VmError::new(format!("Ошибка regex /{pattern}/: {e}"), span))?;
        Ok(Rc::new(YopRegex::Fast(re)))
    }
}

fn needs_fancy(pattern: &str) -> bool {
    let bytes = pattern.as_bytes();
    let mut i = 0;
    let mut in_class = false;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'\\' {
            if i + 1 < bytes.len() {
                let nxt = bytes[i + 1];
                if !in_class && ((b'1'..=b'9').contains(&nxt) || nxt == b'k') {
                    return true;
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
            if b == b'(' && i + 2 < bytes.len() && bytes[i + 1] == b'?' {
                let c2 = bytes[i + 2];
                if c2 == b'=' || c2 == b'!' {
                    return true;
                }
                if c2 == b'<' && i + 3 < bytes.len() {
                    let c3 = bytes[i + 3];
                    if c3 == b'=' || c3 == b'!' {
                        return true;
                    }
                }
            }
        } else if b == b']' {
            in_class = false;
        }
        i += 1;
    }
    false
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

fn obj_value(map: ObjMap) -> Value {
    Value::Object(Rc::new(RefCell::new(map)))
}

pub fn build_match_object(md: &MatchData, s: &str, with_indices: bool) -> Value {
    let mut map = ObjMap::new();
    for (i, slot) in md.groups.iter().enumerate() {
        let key = i.to_string();
        match slot {
            Some(g) => map.insert(key, Value::string(g.text.as_str())),
            None => map.insert(key, Value::Null),
        };
    }
    let whole = md.whole();
    map.insert("index".to_string(), Value::Number(char_index_at(s, whole.start) as f64));
    map.insert("input".to_string(), Value::string(s));

    let mut groups = ObjMap::new();
    let mut has_named = false;
    for (name, slot) in &md.named {
        has_named = true;
        match slot {
            Some(g) => groups.insert(name.clone(), Value::string(g.text.as_str())),
            None => groups.insert(name.clone(), Value::Null),
        };
    }
    let groups_val = if has_named { obj_value(groups) } else { Value::Null };
    map.insert("groups".to_string(), groups_val);

    if with_indices {
        let pair = |g: &GroupSlot| {
            Value::Array(Rc::new(RefCell::new(vec![
                Value::Number(char_index_at(s, g.start) as f64),
                Value::Number(char_index_at(s, g.end) as f64),
            ])))
        };
        let mut indices_obj = ObjMap::new();
        for (i, slot) in md.groups.iter().enumerate() {
            let v = slot.as_ref().map(pair).unwrap_or(Value::Null);
            indices_obj.insert(i.to_string(), v);
        }
        let mut named_groups = ObjMap::new();
        let mut has_named_d = false;
        for (name, slot) in &md.named {
            has_named_d = true;
            let v = slot.as_ref().map(pair).unwrap_or(Value::Null);
            named_groups.insert(name.clone(), v);
        }
        let groups_d = if has_named_d { obj_value(named_groups) } else { Value::Null };
        indices_obj.insert("groups".to_string(), groups_d);
        map.insert("indices".to_string(), obj_value(indices_obj));
    }

    obj_value(map)
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

pub fn exec_stateful(
    re: &Rc<YopRegex>,
    flags: &str,
    last_index: &Rc<RefCell<usize>>,
    s: &str,
    span: Span,
) -> Result<Value, VmError> {
    let stateful = flags.contains('g') || flags.contains('y');
    let has_indices = flags.contains('d');
    if !stateful {
        return Ok(match re.captures(s, span)? {
            Some(md) => build_match_object(&md, s, has_indices),
            None => Value::Null,
        });
    }
    let li = *last_index.borrow();
    let byte_pos = match char_to_byte(s, li) {
        Some(b) => b,
        None => {
            *last_index.borrow_mut() = 0;
            return Ok(Value::Null);
        }
    };
    match re.captures_from_pos(s, byte_pos, span)? {
        Some(md) => {
            let whole = md.whole();
            if flags.contains('y') && whole.start != byte_pos {
                *last_index.borrow_mut() = 0;
                return Ok(Value::Null);
            }
            let new_li = s[..whole.end].chars().count();
            *last_index.borrow_mut() = new_li;
            Ok(build_match_object(&md, s, has_indices))
        }
        None => {
            *last_index.borrow_mut() = 0;
            Ok(Value::Null)
        }
    }
}

pub fn member(receiver: &Value, property: &str) -> Option<Value> {
    let (pattern, flags, last_index) = match receiver {
        Value::RegExp { pattern, flags, last_index, .. } => (pattern, flags, last_index),
        _ => return None,
    };
    match property {
        "источник" | "source" => Some(Value::string(Rc::clone(pattern))),
        "флаги" | "flags" => Some(Value::string(Rc::clone(flags))),
        "global" | "глобальный" => Some(Value::Bool(flags.contains('g'))),
        "ignoreCase" | "игнорРегистр" => Some(Value::Bool(flags.contains('i'))),
        "multiline" | "многострочный" => Some(Value::Bool(flags.contains('m'))),
        "sticky" | "липкий" => Some(Value::Bool(flags.contains('y'))),
        "hasIndices" | "имеетИндексы" => Some(Value::Bool(flags.contains('d'))),
        "lastIndex" | "последнийИндекс" => Some(Value::Number(*last_index.borrow() as f64)),
        _ => None,
    }
}

pub fn call(receiver: &Value, method: &str, args: &[Value], span: Span) -> Result<Value, VmError> {
    let (pattern, flags, compiled, last_index) = match receiver {
        Value::RegExp { pattern, flags, compiled, last_index } => {
            (pattern.clone(), flags.clone(), Rc::clone(compiled), Rc::clone(last_index))
        }
        _ => return Err(VmError::new("Ожидался regex", span)),
    };

    let result = match method {
        "проверить" | "test" => {
            let s = require_str(args, span, "regex.проверить")?;
            Value::Bool(compiled.is_match(&s, span)?)
        }
        "найти" | "exec" => {
            let s = require_str(args, span, "regex.найти")?;
            exec_stateful(&compiled, &flags, &last_index, &s, span)?
        }
        "вСтроку" | "toString" => Value::string(format!("/{pattern}/{flags}")),
        "источник" | "source" => Value::string(pattern),
        "флаги" | "flags" => Value::string(flags),
        other => {
            return Err(VmError::new(format!("У regex нет метода '{other}'"), span));
        }
    };
    Ok(result)
}

fn require_str(args: &[Value], span: Span, who: &str) -> Result<String, VmError> {
    let Some(first) = args.first() else {
        return Err(VmError::new(format!("'{who}' ожидает минимум 1 аргумент(ов), получено {}", args.len()), span));
    };
    match first {
        Value::Str(s) => Ok(s.to_string()),
        other => Err(VmError::new(format!("'{who}' ожидает строку, получено '{}'", other.type_name()), span)),
    }
}
