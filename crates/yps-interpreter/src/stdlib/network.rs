use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use yps_lexer::Span;

use crate::error::RuntimeError;
use crate::interpreter::Interpreter;
use crate::stdlib::{as_string, builtin, object_of, require_args};
use crate::value::Value;

pub fn build_object() -> Value {
    object_of(&[("достать", builtin("Сеть.достать"))])
}

pub fn call_static(
    _interp: &mut Interpreter,
    method: &str,
    args: Vec<Value>,
    span: Span,
) -> Result<Value, RuntimeError> {
    match method {
        "достать" => {
            require_args(&args, 1, span, "Сеть.достать")?;
            let url = as_string(&args[0], span, "Сеть.достать")?.to_string();
            let opts = args.get(1).cloned().unwrap_or(Value::Undefined);
            fetch(&url, opts, span)
        }
        _ => Err(RuntimeError::new(format!("У 'Сеть' нет метода '{method}'"), span)),
    }
}

#[derive(Debug, PartialEq)]
struct ParsedUrl {
    host: String,
    port: u16,
    path: String,
}

fn parse_http_url(url: &str, span: Span) -> Result<ParsedUrl, RuntimeError> {
    if url.starts_with("https://") {
        return Err(RuntimeError::new(
            "HTTPS не поддерживается без внешних зависимостей. Используй http:// URL.",
            span,
        ));
    }
    let rest = url.strip_prefix("http://").ok_or_else(|| {
        RuntimeError::new(format!("URL должен начинаться с http:// или https://, получено '{url}'"), span)
    })?;
    let (authority, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    if authority.is_empty() {
        return Err(RuntimeError::new(format!("URL без хоста: '{url}'"), span));
    }
    let (host, port) = match authority.rsplit_once(':') {
        Some((h, p)) => {
            let port: u16 =
                p.parse().map_err(|_| RuntimeError::new(format!("Не разобрать порт '{p}' в URL '{url}'"), span))?;
            (h.to_string(), port)
        }
        None => (authority.to_string(), 80),
    };
    if !header_safe(&host) || !header_safe(path) {
        return Err(RuntimeError::new("URL содержит управляющие символы (CR/LF) в хосте или пути".to_string(), span));
    }
    Ok(ParsedUrl { host, port, path: path.to_string() })
}

fn header_safe(s: &str) -> bool {
    !s.contains('\r') && !s.contains('\n')
}

fn build_request(method: &str, parsed: &ParsedUrl, headers: &[(String, String)], body: &str) -> String {
    let mut s = String::new();
    s.push_str(&format!("{} {} HTTP/1.1\r\n", method, parsed.path));
    s.push_str(&format!("Host: {}\r\n", parsed.host));
    s.push_str("Connection: close\r\n");
    s.push_str("User-Agent: YoptaScript/0.1\r\n");
    let mut has_len = false;
    for (k, v) in headers {
        if !header_safe(k) || !header_safe(v) {
            continue;
        }
        if k.eq_ignore_ascii_case("content-length") {
            has_len = true;
        }
        s.push_str(&format!("{k}: {v}\r\n"));
    }
    if !body.is_empty() && !has_len {
        s.push_str(&format!("Content-Length: {}\r\n", body.len()));
    }
    s.push_str("\r\n");
    s.push_str(body);
    s
}

fn parse_response(raw: &str) -> Result<(u16, HashMap<String, Value>, String), String> {
    let split = raw.find("\r\n\r\n").ok_or_else(|| "ответ без разделителя заголовков".to_string())?;
    let head = &raw[..split];
    let body = &raw[split + 4..];
    let mut lines = head.split("\r\n");
    let status_line = lines.next().ok_or_else(|| "пустой ответ".to_string())?;
    let mut parts = status_line.splitn(3, ' ');
    let _ = parts.next();
    let code: u16 =
        parts.next().and_then(|s| s.parse().ok()).ok_or_else(|| format!("не разобрать статус: '{status_line}'"))?;
    let mut headers = HashMap::new();
    for line in lines {
        if let Some((k, v)) = line.split_once(':') {
            headers.insert(k.trim().to_ascii_lowercase(), Value::String(v.trim().to_string()));
        }
    }
    Ok((code, headers, body.to_string()))
}

fn fetch(url: &str, opts: Value, span: Span) -> Result<Value, RuntimeError> {
    let parsed = parse_http_url(url, span)?;
    let (method, headers, body) = extract_opts(opts, span)?;
    let req = build_request(&method, &parsed, &headers, &body);

    let mut stream = TcpStream::connect((parsed.host.as_str(), parsed.port))
        .map_err(|e| RuntimeError::new(format!("Не подключиться к '{}': {e}", parsed.host), span))?;
    stream.set_read_timeout(Some(Duration::from_secs(30))).ok();
    stream.set_write_timeout(Some(Duration::from_secs(30))).ok();
    stream.write_all(req.as_bytes()).map_err(|e| RuntimeError::new(format!("Ошибка отправки: {e}"), span))?;
    let mut raw = String::new();
    stream.read_to_string(&mut raw).map_err(|e| RuntimeError::new(format!("Ошибка чтения ответа: {e}"), span))?;
    let (code, headers, body) =
        parse_response(&raw).map_err(|e| RuntimeError::new(format!("Сеть.достать: {e}"), span))?;
    let mut out = HashMap::new();
    out.insert("статус".to_string(), Value::Number(code as f64));
    out.insert("тело".to_string(), Value::String(body));
    out.insert("заголовки".to_string(), Value::object(headers));
    Ok(Value::object(out))
}

type FetchOpts = (String, Vec<(String, String)>, String);

fn extract_opts(opts: Value, span: Span) -> Result<FetchOpts, RuntimeError> {
    let mut method = "GET".to_string();
    let mut headers: Vec<(String, String)> = Vec::new();
    let mut body = String::new();
    if let Value::Object(map) = opts {
        let method_val = map.borrow().get("метод").cloned();
        if let Some(v) = method_val {
            match v {
                Value::String(s) => method = s.to_ascii_uppercase(),
                other => {
                    return Err(RuntimeError::new(
                        format!("'метод' должен быть строкой, получено '{}'", other.type_name()),
                        span,
                    ));
                }
            }
        }
        let body_val = map.borrow().get("тело").cloned();
        if let Some(v) = body_val {
            match v {
                Value::String(s) => body = s.clone(),
                Value::Undefined | Value::Null => {}
                other => {
                    return Err(RuntimeError::new(
                        format!("'тело' должен быть строкой, получено '{}'", other.type_name()),
                        span,
                    ));
                }
            }
        }
        let headers_val = map.borrow().get("заголовки").cloned();
        if let Some(Value::Object(h)) = headers_val {
            for (k, v) in h.borrow().iter() {
                let vs = match v {
                    Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                headers.push((k.clone(), vs));
            }
        }
    }
    Ok((method, headers, body))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_http_simple() {
        let p = parse_http_url("http://example.com/foo", Span { start: 0, end: 0 }).unwrap();
        assert_eq!(p, ParsedUrl { host: "example.com".into(), port: 80, path: "/foo".into() });
    }

    #[test]
    fn parse_http_default_path() {
        let p = parse_http_url("http://example.com", Span { start: 0, end: 0 }).unwrap();
        assert_eq!(p.path, "/");
    }

    #[test]
    fn parse_http_custom_port() {
        let p = parse_http_url("http://localhost:8080/api", Span { start: 0, end: 0 }).unwrap();
        assert_eq!(p.port, 8080);
        assert_eq!(p.host, "localhost");
    }

    #[test]
    fn crlf_in_path_rejected() {
        let err = parse_http_url("http://host/foo\r\nX-Injected: 1", Span { start: 0, end: 0 }).unwrap_err();
        assert!(err.message.contains("управляющие символы"));
    }

    #[test]
    fn crlf_in_host_rejected() {
        let err = parse_http_url("http://ho\r\nst/foo", Span { start: 0, end: 0 }).unwrap_err();
        assert!(err.message.contains("управляющие символы"));
    }

    #[test]
    fn https_rejected() {
        let err = parse_http_url("https://example.com/", Span { start: 0, end: 0 }).unwrap_err();
        assert!(err.message.contains("HTTPS"));
    }

    #[test]
    fn unknown_scheme_rejected() {
        let err = parse_http_url("ftp://x/", Span { start: 0, end: 0 }).unwrap_err();
        assert!(err.message.contains("http://"));
    }

    #[test]
    fn request_includes_host_and_method() {
        let p = ParsedUrl { host: "h".into(), port: 80, path: "/p".into() };
        let req = build_request("POST", &p, &[("X-Foo".into(), "bar".into())], "тело");
        assert!(req.starts_with("POST /p HTTP/1.1\r\n"));
        assert!(req.contains("Host: h\r\n"));
        assert!(req.contains("X-Foo: bar\r\n"));
        assert!(req.contains("Content-Length:"));
        assert!(req.ends_with("тело"));
    }

    #[test]
    fn parse_response_basic() {
        let raw = "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\n\r\nпривет";
        let (code, headers, body) = parse_response(raw).unwrap();
        assert_eq!(code, 200);
        assert_eq!(body, "привет");
        assert_eq!(headers.get("content-type"), Some(&Value::String("text/plain".to_string())));
    }
}
