use std::io::{self, BufRead, Read, Write};

use yps_lexer::Span;

use crate::error::RuntimeError;
use crate::value::Value;

pub fn read_line(span: Span) -> Result<Value, RuntimeError> {
    io::stdout().flush().ok();
    let stdin = io::stdin();
    let mut locked = stdin.lock();
    read_line_from(&mut locked, span)
}

pub fn read_all(span: Span) -> Result<Value, RuntimeError> {
    let stdin = io::stdin();
    let mut locked = stdin.lock();
    read_all_from(&mut locked, span)
}

fn read_line_from<R: BufRead>(reader: &mut R, span: Span) -> Result<Value, RuntimeError> {
    let mut line = String::new();
    let n =
        reader.read_line(&mut line).map_err(|e| RuntimeError::new(format!("'прочестьСтроку' не смогла: {e}"), span))?;
    if n == 0 {
        return Ok(Value::Null);
    }
    if line.ends_with('\n') {
        line.pop();
        if line.ends_with('\r') {
            line.pop();
        }
    }
    Ok(Value::String(line))
}

fn read_all_from<R: Read>(reader: &mut R, span: Span) -> Result<Value, RuntimeError> {
    let mut buf = String::new();
    reader.read_to_string(&mut buf).map_err(|e| RuntimeError::new(format!("'прочестьВсё' не смогла: {e}"), span))?;
    Ok(Value::String(buf))
}

#[cfg(test)]
mod tests {
    use super::*;

    const NO_SPAN: Span = Span { start: 0, end: 0 };

    #[test]
    fn read_line_simple() {
        let mut cursor = "привет\nещё\n".as_bytes();
        assert_eq!(read_line_from(&mut cursor, NO_SPAN).unwrap(), Value::String("привет".into()));
    }

    #[test]
    fn read_line_eof_returns_null() {
        let mut cursor: &[u8] = b"";
        assert_eq!(read_line_from(&mut cursor, NO_SPAN).unwrap(), Value::Null);
    }

    #[test]
    fn read_line_strips_crlf() {
        let mut cursor = "строка\r\n".as_bytes();
        assert_eq!(read_line_from(&mut cursor, NO_SPAN).unwrap(), Value::String("строка".into()));
    }

    #[test]
    fn read_all_collects_everything() {
        let mut cursor = "один\nдва\nтри".as_bytes();
        assert_eq!(read_all_from(&mut cursor, NO_SPAN).unwrap(), Value::String("один\nдва\nтри".into()));
    }
}
