use std::io::{self, BufRead, Read, Write};

use serde_json::Value;

const CONTENT_LENGTH: &str = "content-length:";

const MAX_MESSAGE_LEN: usize = 64 * 1024 * 1024;

const MAX_HEADER_LINE_LEN: usize = 8 * 1024;

/// Reads one `Content-Length`-framed JSON message. `Ok(None)` means clean end of stream.
pub fn read_message<R: BufRead>(reader: &mut R) -> io::Result<Option<Value>> {
    let mut length: Option<usize> = None;
    let mut saw_header = false;
    loop {
        let mut line = String::new();
        let read = reader.by_ref().take(MAX_HEADER_LINE_LEN as u64).read_line(&mut line)?;
        if read >= MAX_HEADER_LINE_LEN && !line.ends_with('\n') {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "слишком длинная строка заголовка"));
        }
        if read == 0 {
            if saw_header {
                return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "поток оборван посреди заголовков"));
            }
            return Ok(None);
        }
        saw_header = true;
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            break;
        }
        let lower = trimmed.to_ascii_lowercase();
        if let Some(rest) = lower.strip_prefix(CONTENT_LENGTH) {
            let value = rest.trim();
            length = Some(value.parse::<usize>().map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidData, format!("некорректное значение Content-Length: '{value}'"))
            })?);
        }
    }

    let Some(length) = length else {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "нет заголовка Content-Length"));
    };
    if length > MAX_MESSAGE_LEN {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Content-Length {length} превышает предел {MAX_MESSAGE_LEN}"),
        ));
    }
    let mut body = Vec::new();
    reader.by_ref().take(length as u64).read_to_end(&mut body)?;
    if body.len() != length {
        return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "тело сообщения короче Content-Length"));
    }
    serde_json::from_slice(&body).map(Some).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

#[must_use]
pub fn is_recoverable(err: &io::Error) -> bool {
    err.get_ref().is_some_and(|inner| inner.is::<serde_json::Error>())
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
    use crate::test_support::Lcg;
    use serde_json::json;

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
        let text = std::str::from_utf8(&buf).unwrap();
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
    fn missing_header_is_an_unrecoverable_error_that_says_so() {
        for data in [&b"\r\n{}"[..], &b"X-Other: 1\r\n\r\n{}"[..]] {
            let err = read_message(&mut io::BufReader::new(data)).unwrap_err();
            assert!(err.to_string().contains("нет заголовка"), "{err}");
            assert!(!is_recoverable(&err));
        }
    }

    #[test]
    fn unparsable_content_length_is_reported_accurately() {
        let mut reader = io::BufReader::new(&b"Content-Length: abc\r\n\r\n{}"[..]);
        let err = read_message(&mut reader).unwrap_err();
        assert!(err.to_string().contains("некорректное значение Content-Length"), "{err}");
        assert!(err.to_string().contains("abc"), "{err}");
        assert!(!err.to_string().contains("нет заголовка"), "{err}");
        assert!(!is_recoverable(&err));
    }

    #[test]
    fn invalid_json_of_correct_length_is_recoverable_and_stream_stays_in_sync() {
        let mut buf = b"Content-Length: 5\r\n\r\n{nope".to_vec();
        write_message(&mut buf, &json!({"seq": 2})).unwrap();
        let mut reader = io::BufReader::new(buf.as_slice());
        let err = read_message(&mut reader).unwrap_err();
        assert!(is_recoverable(&err));
        let next = read_message(&mut reader).unwrap().unwrap();
        assert_eq!(next["seq"], 2);
    }

    #[test]
    fn truncated_body_is_unrecoverable() {
        let mut reader = io::BufReader::new(&b"Content-Length: 50\r\n\r\n{}"[..]);
        let err = read_message(&mut reader).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::UnexpectedEof);
        assert!(!is_recoverable(&err));
    }

    #[test]
    fn clean_eof_is_none() {
        let mut reader = io::BufReader::new(&b""[..]);
        assert!(read_message(&mut reader).unwrap().is_none());
    }

    #[test]
    fn huge_declared_length_is_rejected_without_reading_the_body() {
        let mut reader = io::BufReader::new(&b"Content-Length: 18446744073709551615\r\n\r\n{}"[..]);
        let err = read_message(&mut reader).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        assert!(err.to_string().contains("превышает"), "{err}");
        assert!(!is_recoverable(&err));
    }

    #[test]
    fn length_just_above_the_cap_is_rejected_and_short_stream_below_it_is_not_preallocated() {
        let over = format!("Content-Length: {}\r\n\r\n", MAX_MESSAGE_LEN + 1);
        let mut reader = io::BufReader::new(over.as_bytes());
        assert_eq!(read_message(&mut reader).unwrap_err().kind(), io::ErrorKind::InvalidData);

        let under = format!("Content-Length: {MAX_MESSAGE_LEN}\r\n\r\n{{}}");
        let mut reader = io::BufReader::new(under.as_bytes());
        assert_eq!(read_message(&mut reader).unwrap_err().kind(), io::ErrorKind::UnexpectedEof);
    }

    fn fuzz_case(rng: &mut Lcg, index: usize) -> Vec<u8> {
        let valid_body = br#"{"seq":1,"type":"request","command":"threads"}"#;
        let weird_lengths: [&[u8]; 12] = [
            b"-5",
            b"0x10",
            b"99999999999999999999999999",
            b"+3",
            b" 7 ",
            b"",
            b"1e3",
            b"18446744073709551616",
            b"-0",
            b"\xef\xbc\x91\xef\xbc\x92",
            b"1_0",
            b"4294967296",
        ];
        let mut out = Vec::new();
        match index % 8 {
            0 => out = rng.bytes(300),
            1 => {
                out.extend_from_slice(format!("Content-Length: {}\r\n\r\n", valid_body.len()).as_bytes());
                out.extend_from_slice(valid_body);
            }
            2 => {
                let declared = valid_body.len() + 1 + rng.below(1000);
                out.extend_from_slice(format!("Content-Length: {declared}\r\n\r\n").as_bytes());
                let keep = rng.below(valid_body.len());
                out.extend_from_slice(&valid_body[..keep]);
            }
            3 => {
                let declared = match rng.below(3) {
                    0 => MAX_MESSAGE_LEN as u64 + 1 + rng.below(1 << 20) as u64,
                    1 => u64::MAX - rng.below(1000) as u64,
                    _ => (MAX_MESSAGE_LEN as u64) - rng.below(1 << 20) as u64,
                };
                out.extend_from_slice(format!("Content-Length: {declared}\r\n\r\n").as_bytes());
                out.extend_from_slice(&rng.bytes(64));
            }
            4 => {
                let body = rng.bytes(200);
                out.extend_from_slice(format!("Content-Length: {}\r\n\r\n", body.len()).as_bytes());
                out.extend_from_slice(&body);
            }
            5 => {
                out.extend_from_slice(b"Content-Length: ");
                out.extend_from_slice(weird_lengths[rng.below(weird_lengths.len())]);
                out.extend_from_slice(b"\r\n\r\n");
                out.extend_from_slice(&rng.bytes(40));
            }
            6 => {
                for _ in 0..=rng.below(5) {
                    out.extend_from_slice(b"Content-Length: 10\r\n");
                    out.extend_from_slice(&rng.bytes(20));
                    out.push(b'\n');
                }
            }
            _ => {
                let separators: [&[u8]; 5] = [b"\r\n", b"\n", b"\r", b"\r\r\n", b"\n\r\n"];
                let pick = |rng: &mut Lcg| separators[rng.below(separators.len())];
                out.extend_from_slice(format!("Content-Length: {}", valid_body.len()).as_bytes());
                out.extend_from_slice(pick(rng));
                if rng.below(2) == 0 {
                    out.extend_from_slice(b"Content-Type: x");
                    out.extend_from_slice(pick(rng));
                }
                out.extend_from_slice(pick(rng));
                out.extend_from_slice(valid_body);
            }
        }
        out
    }

    #[test]
    fn read_message_survives_pseudo_random_streams() {
        let started = std::time::Instant::now();
        let mut rng = Lcg(0x9e37_79b9_7f4a_7c15);
        for index in 0..2000 {
            let mut data = fuzz_case(&mut rng, index);
            if rng.below(4) == 0 {
                let tail = fuzz_case(&mut rng, index + 1);
                data.extend_from_slice(&tail);
            }
            let mut reader = io::BufReader::new(data.as_slice());
            let mut calls = 0;
            loop {
                calls += 1;
                assert!(calls <= data.len() + 2, "case {index}: reader does not make progress");
                match read_message(&mut reader) {
                    Ok(Some(_)) => {}
                    Err(err) if is_recoverable(&err) => {}
                    Ok(None) | Err(_) => break,
                }
            }
        }
        assert!(started.elapsed() < std::time::Duration::from_secs(2), "fuzz took {:?}", started.elapsed());
    }

    #[test]
    fn endless_header_line_is_rejected_instead_of_buffered() {
        let mut reader = io::BufReader::new(io::repeat(b'a').take(1 << 30));
        let err = read_message(&mut reader).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        assert!(!is_recoverable(&err));
    }

    #[test]
    fn header_line_at_the_limit_with_newline_is_still_accepted() {
        let padding = "x".repeat(MAX_HEADER_LINE_LEN - "X-Pad: \n".len());
        let data = format!("X-Pad: {padding}\nContent-Length: 2\r\n\r\n{{}}");
        let mut reader = io::BufReader::new(data.as_bytes());
        assert!(read_message(&mut reader).unwrap().is_some());
    }
}
