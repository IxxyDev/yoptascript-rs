use std::cell::RefCell;
use std::rc::Rc;

use yps_lexer::Span;

use yps_interpreter::RuntimeError;
pub use yps_interpreter::stdlib::regexp::YopRegex;
use yps_interpreter::stdlib::regexp::{GroupSlot, MatchData, compile as compile_engine};

use crate::error::VmError;
use crate::value::{ObjMap, Value};

fn to_vm(e: RuntimeError) -> VmError {
    VmError::new(e.message, e.span)
}

pub fn compile(pattern: &str, flags: &str, span: Span) -> Result<Rc<YopRegex>, VmError> {
    compile_engine(pattern, flags, span).map_err(to_vm)
}

fn match_whole(md: &MatchData) -> &GroupSlot {
    md.groups[0].as_ref().expect("match group 0")
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
    let whole = match_whole(md);
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
        return Ok(match re.captures(s, span).map_err(to_vm)? {
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
    match re.captures_from_pos(s, byte_pos, span).map_err(to_vm)? {
        Some(md) => {
            let whole = match_whole(&md);
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
            Value::Bool(compiled.is_match(&s, span).map_err(to_vm)?)
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
