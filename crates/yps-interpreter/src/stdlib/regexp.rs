use std::collections::HashMap;
use std::rc::Rc;

use yps_lexer::Span;

use crate::error::RuntimeError;
use crate::interpreter::Interpreter;
use crate::stdlib::{as_string, require_args};
use crate::value::Value;

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

fn fancy_err(e: fancy_regex::Error, span: Span) -> RuntimeError {
    RuntimeError::new(format!("Ошибка выполнения regex: {e}"), span)
}

impl YopRegex {
    pub fn is_match(&self, s: &str, span: Span) -> Result<bool, RuntimeError> {
        match self {
            YopRegex::Fast(re) => Ok(re.is_match(s)),
            YopRegex::Fancy(re) => re.is_match(s).map_err(|e| fancy_err(e, span)),
        }
    }

    pub fn captures(&self, s: &str, span: Span) -> Result<Option<MatchData>, RuntimeError> {
        match self {
            YopRegex::Fast(re) => Ok(re.captures(s).map(|c| MatchData::from_fast(&c, re))),
            YopRegex::Fancy(re) => {
                re.captures(s).map(|opt| opt.map(|c| MatchData::from_fancy(&c, re))).map_err(|e| fancy_err(e, span))
            }
        }
    }

    pub fn captures_from_pos(&self, s: &str, pos: usize, span: Span) -> Result<Option<MatchData>, RuntimeError> {
        match self {
            YopRegex::Fast(re) => Ok(re.captures_at(s, pos).map(|c| MatchData::from_fast(&c, re))),
            YopRegex::Fancy(re) => re
                .captures_from_pos(s, pos)
                .map(|opt| opt.map(|c| MatchData::from_fancy(&c, re)))
                .map_err(|e| fancy_err(e, span)),
        }
    }

    pub fn find_start(&self, s: &str, span: Span) -> Result<Option<usize>, RuntimeError> {
        match self {
            YopRegex::Fast(re) => Ok(re.find(s).map(|m| m.start())),
            YopRegex::Fancy(re) => re.find(s).map(|opt| opt.map(|m| m.start())).map_err(|e| fancy_err(e, span)),
        }
    }
}

pub fn compile(pattern: &str, flags: &str, span: Span) -> Result<Rc<YopRegex>, RuntimeError> {
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

    if needs_fancy(pattern) {
        let re = fancy_regex::RegexBuilder::new(&full)
            .backtrack_limit(FANCY_BACKTRACK_LIMIT)
            .build()
            .map_err(|e| RuntimeError::new(format!("Ошибка regex /{pattern}/: {e}"), span))?;
        Ok(Rc::new(YopRegex::Fancy(re)))
    } else {
        let re =
            regex::Regex::new(&full).map_err(|e| RuntimeError::new(format!("Ошибка regex /{pattern}/: {e}"), span))?;
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

pub fn build_match_object(md: &MatchData, s: &str, with_indices: bool) -> Value {
    let mut map: HashMap<String, Value> = HashMap::new();
    for (i, slot) in md.groups.iter().enumerate() {
        let key = i.to_string();
        match slot {
            Some(g) => map.insert(key, Value::String(g.text.clone())),
            None => map.insert(key, Value::Null),
        };
    }
    let whole = md.whole();
    map.insert("index".to_string(), Value::Number(char_index_at(s, whole.start) as f64));
    map.insert("input".to_string(), Value::String(s.to_string()));

    let mut groups: HashMap<String, Value> = HashMap::new();
    let mut has_named = false;
    for (name, slot) in &md.named {
        has_named = true;
        match slot {
            Some(g) => groups.insert(name.clone(), Value::String(g.text.clone())),
            None => groups.insert(name.clone(), Value::Null),
        };
    }
    let groups_val = if has_named { Value::object(groups) } else { Value::Null };
    map.insert("groups".to_string(), groups_val);

    if with_indices {
        let pair = |g: &GroupSlot| {
            Value::array(vec![
                Value::Number(char_index_at(s, g.start) as f64),
                Value::Number(char_index_at(s, g.end) as f64),
            ])
        };
        let mut indices_obj: HashMap<String, Value> = HashMap::new();
        for (i, slot) in md.groups.iter().enumerate() {
            let v = slot.as_ref().map(pair).unwrap_or(Value::Null);
            indices_obj.insert(i.to_string(), v);
        }
        let mut named_groups: HashMap<String, Value> = HashMap::new();
        for (name, slot) in &md.named {
            let v = slot.as_ref().map(pair).unwrap_or(Value::Null);
            named_groups.insert(name.clone(), v);
        }
        let groups_d = if named_groups.is_empty() { Value::Null } else { Value::object(named_groups) };
        indices_obj.insert("groups".to_string(), groups_d);
        map.insert("indices".to_string(), Value::object(indices_obj));
    }

    Value::object(map)
}

pub fn match_first(re: &Rc<YopRegex>, s: &str, span: Span) -> Result<Value, RuntimeError> {
    match re.captures(s, span)? {
        Some(md) => Ok(build_match_object(&md, s, false)),
        None => Ok(Value::Null),
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

pub fn exec_stateful(
    re: &Rc<YopRegex>,
    flags: &str,
    last_index: &Rc<std::cell::RefCell<usize>>,
    s: &str,
    span: Span,
) -> Result<Value, RuntimeError> {
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

pub fn match_all_global(re: &Rc<YopRegex>, s: &str, span: Span) -> Result<Vec<Value>, RuntimeError> {
    let mut out = Vec::new();
    let mut pos = 0usize;
    while pos <= s.len() {
        match re.captures_from_pos(s, pos, span)? {
            Some(md) => {
                let whole = md.whole();
                out.push(Value::String(whole.text.clone()));
                pos = if whole.end == whole.start { next_byte(s, whole.end) } else { whole.end };
            }
            None => break,
        }
    }
    Ok(out)
}

fn next_byte(s: &str, pos: usize) -> usize {
    if pos >= s.len() {
        return pos + 1;
    }
    match s[pos..].chars().next() {
        Some(ch) => pos + ch.len_utf8(),
        None => pos + 1,
    }
}

pub fn split_string(re: &Rc<YopRegex>, s: &str, span: Span) -> Result<Vec<Value>, RuntimeError> {
    if s.is_empty() {
        return match re.captures_from_pos(s, 0, span)? {
            Some(_) => Ok(Vec::new()),
            None => Ok(vec![Value::String(String::new())]),
        };
    }
    let mut out: Vec<Value> = Vec::new();
    let mut last = 0usize;
    let mut pos = 0usize;
    while pos < s.len() {
        match re.captures_from_pos(s, pos, span)? {
            Some(md) => {
                let whole = md.whole();
                if whole.end == last {
                    pos = next_byte(s, pos);
                    continue;
                }
                out.push(Value::String(s[last..whole.start].to_string()));
                for slot in md.groups.iter().skip(1) {
                    match slot {
                        Some(g) => out.push(Value::String(g.text.clone())),
                        None => out.push(Value::Null),
                    }
                }
                last = whole.end;
                pos = if whole.end == whole.start { next_byte(s, whole.end) } else { whole.end };
            }
            None => break,
        }
    }
    out.push(Value::String(s[last..].to_string()));
    Ok(out)
}

pub fn replace_string(
    re: &Rc<YopRegex>,
    s: &str,
    replacement: &str,
    global: bool,
    span: Span,
) -> Result<String, RuntimeError> {
    let template = ReplacementTemplate::new(replacement);
    let mut out = String::new();
    let mut last_end = 0usize;
    let mut pos = 0usize;
    while pos <= s.len() {
        match re.captures_from_pos(s, pos, span)? {
            Some(md) => {
                let whole = md.whole();
                out.push_str(&s[last_end..whole.start]);
                template.expand(&md, &mut out);
                last_end = whole.end;
                pos = if whole.end == whole.start { next_byte(s, whole.end) } else { whole.end };
                if !global {
                    break;
                }
            }
            None => break,
        }
    }
    out.push_str(&s[last_end..]);
    Ok(out)
}

pub fn replace_with_fn(
    interp: &mut Interpreter,
    re: &Rc<YopRegex>,
    s: &str,
    fn_val: Value,
    global: bool,
    span: Span,
) -> Result<String, RuntimeError> {
    let mut out = String::new();
    let mut last_end = 0usize;
    let mut pos = 0usize;
    while pos <= s.len() {
        let md = match re.captures_from_pos(s, pos, span)? {
            Some(md) => md,
            None => break,
        };
        let whole = md.whole();
        out.push_str(&s[last_end..whole.start]);
        let mut call_args: Vec<Value> = Vec::with_capacity(md.groups.len() + 2);
        call_args.push(Value::String(whole.text.clone()));
        for slot in md.groups.iter().skip(1) {
            match slot {
                Some(g) => call_args.push(Value::String(g.text.clone())),
                None => call_args.push(Value::Null),
            }
        }
        let char_offset = char_index_at(s, whole.start);
        call_args.push(Value::Number(char_offset as f64));
        call_args.push(Value::String(s.to_string()));
        let returned = interp.call_function(fn_val.clone(), call_args, span)?;
        let rep = match returned {
            Value::String(rs) => rs,
            other => other.to_string(),
        };
        out.push_str(&rep);
        last_end = whole.end;
        pos = if whole.end == whole.start { next_byte(s, whole.end) } else { whole.end };
        if !global {
            break;
        }
    }
    out.push_str(&s[last_end..]);
    Ok(out)
}

pub fn search_index(re: &Rc<YopRegex>, s: &str, span: Span) -> Result<i64, RuntimeError> {
    match re.find_start(s, span)? {
        Some(start) => Ok(char_index_at(s, start)),
        None => Ok(-1),
    }
}

struct ReplacementTemplate<'a> {
    src: &'a str,
}

impl<'a> ReplacementTemplate<'a> {
    fn new(src: &'a str) -> Self {
        Self { src }
    }

    fn group_text(md: &MatchData, idx: usize) -> Option<&str> {
        md.groups.get(idx).and_then(|s| s.as_ref()).map(|g| g.text.as_str())
    }

    fn named_text<'m>(md: &'m MatchData, name: &str) -> Option<&'m str> {
        md.named.iter().find(|(n, _)| n == name).and_then(|(_, s)| s.as_ref()).map(|g| g.text.as_str())
    }

    fn expand(&self, md: &MatchData, dst: &mut String) {
        let bytes = self.src.as_bytes();
        let mut i = 0;
        let max_group = md.groups.len().saturating_sub(1);
        while i < bytes.len() {
            let b = bytes[i];
            if b == b'$' && i + 1 < bytes.len() {
                let nb = bytes[i + 1];
                match nb {
                    b'&' => {
                        if let Some(t) = Self::group_text(md, 0) {
                            dst.push_str(t);
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
                        let two_digit = if i + 2 < bytes.len() {
                            let nb2 = bytes[i + 2];
                            if nb2.is_ascii_digit() { Some((nb2 - b'0') as usize) } else { None }
                        } else {
                            None
                        };
                        if let Some(d2) = two_digit {
                            let nn = d1 * 10 + d2;
                            if nn != 0 && nn <= max_group {
                                if let Some(t) = Self::group_text(md, nn) {
                                    dst.push_str(t);
                                }
                                i += 3;
                                continue;
                            }
                            if d1 != 0 && d1 <= max_group {
                                if let Some(t) = Self::group_text(md, d1) {
                                    dst.push_str(t);
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
                            if let Some(t) = Self::group_text(md, d1) {
                                dst.push_str(t);
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
                            if let Some(t) = Self::named_text(md, name) {
                                dst.push_str(t);
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
            Value::Boolean(compiled.is_match(s, span)?)
        }
        "найти" | "exec" => {
            require_args(&args, 1, span, "regex.найти")?;
            let s = as_string(&args[0], span, "regex.найти")?;
            exec_stateful(&compiled, &flags, &last_index, s, span)?
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
