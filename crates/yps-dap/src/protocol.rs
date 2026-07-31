use std::io::{self, BufRead, Write};

use serde_json::Value;

const CONTENT_LENGTH: &str = "content-length:";

/// Reads one `Content-Length`-framed JSON message. `Ok(None)` means clean end of stream.
pub fn read_message<R: BufRead>(reader: &mut R) -> io::Result<Option<Value>> {
    let mut length: Option<usize> = None;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            return Ok(None);
        }
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            break;
        }
        let lower = trimmed.to_ascii_lowercase();
        if let Some(rest) = lower.strip_prefix(CONTENT_LENGTH) {
            length = rest.trim().parse::<usize>().ok();
        }
    }

    let Some(length) = length else {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "нет заголовка Content-Length"));
    };
    let mut body = vec![0u8; length];
    reader.read_exact(&mut body)?;
    serde_json::from_slice(&body).map(Some).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

pub fn write_message<W: Write>(writer: &mut W, message: &Value) -> io::Result<()> {
    let body = serde_json::to_vec(message)?;
    write!(writer, "Content-Length: {}\r\n\r\n", body.len())?;
    writer.write_all(&body)?;
    writer.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::Read;

    /// Reads every framed message available in `reader`, ignoring anything after a malformed frame.
    fn read_all<R: Read>(reader: R) -> Vec<Value> {
        let mut reader = io::BufReader::new(reader);
        let mut out = Vec::new();
        while let Ok(Some(message)) = read_message(&mut reader) {
            out.push(message);
        }
        out
    }

    #[test]
    fn roundtrip_single_message() {
        let mut buf = Vec::new();
        write_message(&mut buf, &json!({"seq": 1, "type": "request", "command": "initialize"})).unwrap();
        let text = String::from_utf8(buf.clone()).unwrap();
        assert!(text.starts_with("Content-Length: "));
        assert!(text.contains("\r\n\r\n"));
        let back = read_all(buf.as_slice());
        assert_eq!(back.len(), 1);
        assert_eq!(back[0]["command"], "initialize");
    }

    #[test]
    fn reads_several_messages_back_to_back() {
        let mut buf = Vec::new();
        write_message(&mut buf, &json!({"seq": 1})).unwrap();
        write_message(&mut buf, &json!({"seq": 2})).unwrap();
        let back = read_all(buf.as_slice());
        assert_eq!(back.len(), 2);
        assert_eq!(back[1]["seq"], 2);
    }

    #[test]
    fn header_name_is_case_insensitive() {
        let payload = b"content-length: 8\r\n\r\n{\"a\": 1}";
        let back = read_all(&payload[..]);
        assert_eq!(back.len(), 1);
        assert_eq!(back[0]["a"], 1);
    }

    #[test]
    fn missing_header_is_an_error() {
        let mut reader = io::BufReader::new(&b"\r\n{}"[..]);
        assert!(read_message(&mut reader).is_err());
    }
}
