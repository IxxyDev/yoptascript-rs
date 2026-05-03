use std::collections::HashMap;

use yps_lexer::Span;

use crate::error::RuntimeError;
use crate::interpreter::Interpreter;
use crate::stdlib::{as_string, builtin, object_of, require_args};
use crate::value::Value;

pub fn build_object() -> Value {
    object_of(&[("разобрать", builtin("Жсон.разобрать")), ("вСтроку", builtin("Жсон.вСтроку"))])
}

pub fn call_static(
    _interp: &mut Interpreter,
    method: &str,
    args: Vec<Value>,
    span: Span,
) -> Result<Value, RuntimeError> {
    match method {
        "разобрать" => {
            require_args(&args, 1, span, "Жсон.разобрать")?;
            let s = as_string(&args[0], span, "Жсон.разобрать")?;
            let mut parser = JsonParser { input: s.as_bytes(), pos: 0 };
            parser.skip_ws();
            let v = parser.parse_value(span)?;
            parser.skip_ws();
            if parser.pos != parser.input.len() {
                return Err(RuntimeError::new("Лишние символы после JSON", span));
            }
            Ok(v)
        }
        "вСтроку" => {
            require_args(&args, 1, span, "Жсон.вСтроку")?;
            stringify(&args[0], span)
        }
        _ => Err(RuntimeError::new(format!("У 'Жсон' нет метода '{method}'"), span)),
    }
}

fn stringify(v: &Value, span: Span) -> Result<Value, RuntimeError> {
    let mut out = String::new();
    stringify_into(v, &mut out, span)?;
    Ok(Value::String(out))
}

fn stringify_into(v: &Value, out: &mut String, span: Span) -> Result<(), RuntimeError> {
    match v {
        Value::Null | Value::Undefined => out.push_str("null"),
        Value::Boolean(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => {
            if n.is_finite() {
                if n.fract() == 0.0 {
                    out.push_str(&format!("{}", *n as i64));
                } else {
                    out.push_str(&format!("{n}"));
                }
            } else {
                out.push_str("null");
            }
        }
        Value::String(s) => {
            write_json_string(out, s);
        }
        Value::Array(arr) => {
            out.push('[');
            for (i, el) in arr.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                stringify_into(el, out, span)?;
            }
            out.push(']');
        }
        Value::Object(map) => {
            out.push('{');
            let mut first = true;
            for (k, val) in map.iter() {
                if k == "__class__" || k.starts_with("__get_") || k.starts_with("__set_") {
                    continue;
                }
                if matches!(val, Value::Function { .. } | Value::BuiltinFunction(_) | Value::Undefined) {
                    continue;
                }
                if !first {
                    out.push(',');
                }
                first = false;
                write_json_string(out, k);
                out.push(':');
                stringify_into(val, out, span)?;
            }
            out.push('}');
        }
        Value::Map(_) | Value::Set(_) => {
            return Err(RuntimeError::new(
                "Карту/Набор нельзя сериализовать в JSON напрямую — используйте записи()/значения()",
                span,
            ));
        }
        Value::Function { .. } | Value::BuiltinFunction(_) | Value::Class(_) => {
            return Err(RuntimeError::new("Функции/классы нельзя сериализовать в JSON", span));
        }
        Value::Symbol { .. } => {
            return Err(RuntimeError::new("Символы нельзя сериализовать в JSON", span));
        }
    }
    Ok(())
}

fn write_json_string(out: &mut String, s: &str) {
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

struct JsonParser<'a> {
    input: &'a [u8],
    pos: usize,
}

impl<'a> JsonParser<'a> {
    fn skip_ws(&mut self) {
        while self.pos < self.input.len() {
            let b = self.input[self.pos];
            if b == b' ' || b == b'\t' || b == b'\n' || b == b'\r' {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    fn peek(&self) -> Option<u8> {
        self.input.get(self.pos).copied()
    }

    fn parse_value(&mut self, span: Span) -> Result<Value, RuntimeError> {
        self.skip_ws();
        match self.peek() {
            Some(b'{') => self.parse_object(span),
            Some(b'[') => self.parse_array(span),
            Some(b'"') => self.parse_string(span).map(Value::String),
            Some(b't') | Some(b'f') => self.parse_bool(span),
            Some(b'n') => self.parse_null(span),
            Some(b) if b == b'-' || b.is_ascii_digit() => self.parse_number(span),
            _ => Err(RuntimeError::new("Ожидалось значение JSON", span)),
        }
    }

    fn parse_object(&mut self, span: Span) -> Result<Value, RuntimeError> {
        self.pos += 1;
        let mut map = HashMap::new();
        self.skip_ws();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            return Ok(Value::Object(map));
        }
        loop {
            self.skip_ws();
            let key = self.parse_string(span)?;
            self.skip_ws();
            if self.peek() != Some(b':') {
                return Err(RuntimeError::new("Ожидалось ':' в объекте JSON", span));
            }
            self.pos += 1;
            let v = self.parse_value(span)?;
            map.insert(key, v);
            self.skip_ws();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b'}') => {
                    self.pos += 1;
                    return Ok(Value::Object(map));
                }
                _ => return Err(RuntimeError::new("Ожидалось ',' или '}' в объекте JSON", span)),
            }
        }
    }

    fn parse_array(&mut self, span: Span) -> Result<Value, RuntimeError> {
        self.pos += 1;
        let mut arr = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b']') {
            self.pos += 1;
            return Ok(Value::Array(arr));
        }
        loop {
            let v = self.parse_value(span)?;
            arr.push(v);
            self.skip_ws();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b']') => {
                    self.pos += 1;
                    return Ok(Value::Array(arr));
                }
                _ => return Err(RuntimeError::new("Ожидалось ',' или ']' в массиве JSON", span)),
            }
        }
    }

    fn parse_string(&mut self, span: Span) -> Result<String, RuntimeError> {
        if self.peek() != Some(b'"') {
            return Err(RuntimeError::new("Ожидалась строка JSON", span));
        }
        self.pos += 1;
        let mut out = String::new();
        while self.pos < self.input.len() {
            let b = self.input[self.pos];
            match b {
                b'"' => {
                    self.pos += 1;
                    return Ok(out);
                }
                b'\\' => {
                    self.pos += 1;
                    let esc = self.input.get(self.pos).copied().ok_or_else(|| {
                        RuntimeError::new("Недописанная escape-последовательность в строке JSON", span)
                    })?;
                    self.pos += 1;
                    match esc {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'b' => out.push('\u{08}'),
                        b'f' => out.push('\u{0C}'),
                        b'u' => {
                            if self.pos + 4 > self.input.len() {
                                return Err(RuntimeError::new("Неполный \\u escape в JSON", span));
                            }
                            let hex = std::str::from_utf8(&self.input[self.pos..self.pos + 4])
                                .map_err(|_| RuntimeError::new("Невалидный \\u escape", span))?;
                            let code =
                                u32::from_str_radix(hex, 16).map_err(|_| RuntimeError::new("Невалидный \\u", span))?;
                            self.pos += 4;
                            if let Some(c) = char::from_u32(code) {
                                out.push(c);
                            }
                        }
                        _ => return Err(RuntimeError::new("Неизвестная escape-последовательность в JSON", span)),
                    }
                }
                _ => {
                    let ch_start = self.pos;
                    let utf8_len = utf8_char_len(b);
                    if self.pos + utf8_len > self.input.len() {
                        return Err(RuntimeError::new("Невалидная UTF-8 последовательность", span));
                    }
                    let slice = &self.input[ch_start..ch_start + utf8_len];
                    let s = std::str::from_utf8(slice)
                        .map_err(|_| RuntimeError::new("Невалидная UTF-8 последовательность", span))?;
                    out.push_str(s);
                    self.pos += utf8_len;
                }
            }
        }
        Err(RuntimeError::new("Недописанная строка JSON", span))
    }

    fn parse_bool(&mut self, span: Span) -> Result<Value, RuntimeError> {
        if self.input[self.pos..].starts_with(b"true") {
            self.pos += 4;
            Ok(Value::Boolean(true))
        } else if self.input[self.pos..].starts_with(b"false") {
            self.pos += 5;
            Ok(Value::Boolean(false))
        } else {
            Err(RuntimeError::new("Ожидалось true/false в JSON", span))
        }
    }

    fn parse_null(&mut self, span: Span) -> Result<Value, RuntimeError> {
        if self.input[self.pos..].starts_with(b"null") {
            self.pos += 4;
            Ok(Value::Null)
        } else {
            Err(RuntimeError::new("Ожидалось null в JSON", span))
        }
    }

    fn parse_number(&mut self, span: Span) -> Result<Value, RuntimeError> {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        while let Some(b) = self.peek() {
            if b.is_ascii_digit() || b == b'.' || b == b'e' || b == b'E' || b == b'+' || b == b'-' {
                self.pos += 1;
            } else {
                break;
            }
        }
        let s = std::str::from_utf8(&self.input[start..self.pos])
            .map_err(|_| RuntimeError::new("Невалидное число в JSON", span))?;
        s.parse::<f64>().map(Value::Number).map_err(|_| RuntimeError::new("Невалидное число в JSON", span))
    }
}

fn utf8_char_len(b: u8) -> usize {
    if b < 0xC0 {
        1
    } else if b < 0xE0 {
        2
    } else if b < 0xF0 {
        3
    } else {
        4
    }
}
