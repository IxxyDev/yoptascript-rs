use std::cell::Cell;
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

use yps_lexer::Span;

use crate::error::RuntimeError;
use crate::interpreter::Interpreter;
use crate::value::Value;

const MAX_TIME: f64 = 8.64e15;

pub fn now_ms() -> f64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_millis() as f64,
        Err(e) => -(e.duration().as_millis() as f64),
    }
}

pub struct Components {
    pub year: i64,
    pub month: u32,
    pub day: u32,
    pub weekday: u32,
    pub hours: u32,
    pub minutes: u32,
    pub seconds: u32,
    pub millis: u32,
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

pub fn ms_to_components(ms: f64) -> Option<Components> {
    if !ms.is_finite() || ms.abs() > MAX_TIME {
        return None;
    }
    let days_f = (ms / 86_400_000.0).floor();
    let z = days_f as i64;
    let day_ms = ms - days_f * 86_400_000.0;
    let (year, month, day) = civil_from_days(z);
    let weekday = ((z.rem_euclid(7)) + 4).rem_euclid(7) as u32;
    let ms_in_day = day_ms as i64;
    let hours = (ms_in_day / 3_600_000) as u32;
    let minutes = ((ms_in_day / 60_000) % 60) as u32;
    let seconds = ((ms_in_day / 1000) % 60) as u32;
    let millis = (ms_in_day % 1000) as u32;
    Some(Components { year, month, day, weekday, hours, minutes, seconds, millis })
}

pub fn format_iso(ms: f64) -> String {
    match ms_to_components(ms) {
        Some(c) => format_components(&c),
        None => "Invalid Date".to_string(),
    }
}

fn format_components(c: &Components) -> String {
    let year_part = if (0..=9999).contains(&c.year) { format!("{:04}", c.year) } else { format!("{:+07}", c.year) };
    format!(
        "{}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
        year_part, c.month, c.day, c.hours, c.minutes, c.seconds, c.millis
    )
}

fn parse_iso(s: &str) -> f64 {
    match parse_iso_opt(s.trim()) {
        Some(ms) => ms,
        None => f64::NAN,
    }
}

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u64;
    let mp = if m > 2 { m - 3 } else { m + 9 } as u64;
    let doy = (153 * mp + 2) / 5 + d as u64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe as i64 - 719_468
}

fn parse_iso_opt(s: &str) -> Option<f64> {
    let bytes = s.as_bytes();
    let mut pos = 0;

    let mut sign = 1i64;
    let mut year_digits = 4;
    if bytes.first() == Some(&b'+') || bytes.first() == Some(&b'-') {
        sign = if bytes[0] == b'-' { -1 } else { 1 };
        pos += 1;
        year_digits = 6;
    }
    let year = read_uint(bytes, &mut pos, year_digits)? as i64 * sign;
    expect(bytes, &mut pos, b'-')?;
    let month = read_uint(bytes, &mut pos, 2)?;
    expect(bytes, &mut pos, b'-')?;
    let day = read_uint(bytes, &mut pos, 2)?;

    let (mut hour, mut minute, mut second, mut millis) = (0u64, 0u64, 0u64, 0u64);
    if pos < bytes.len() {
        if bytes[pos] != b'T' {
            return None;
        }
        pos += 1;
        hour = read_uint(bytes, &mut pos, 2)?;
        expect(bytes, &mut pos, b':')?;
        minute = read_uint(bytes, &mut pos, 2)?;
        if pos < bytes.len() && bytes[pos] == b':' {
            pos += 1;
            second = read_uint(bytes, &mut pos, 2)?;
            if pos < bytes.len() && bytes[pos] == b'.' {
                pos += 1;
                let start = pos;
                let frac = read_uint(bytes, &mut pos, 3)?;
                if pos - start != 3 {
                    return None;
                }
                millis = frac;
            }
        }
        expect(bytes, &mut pos, b'Z')?;
    }
    if pos != bytes.len() {
        return None;
    }
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    if hour > 23 || minute > 59 || second > 59 {
        return None;
    }

    let days = days_from_civil(year, month as i64, day as i64);
    let ms = days as f64 * 86_400_000.0
        + hour as f64 * 3_600_000.0
        + minute as f64 * 60_000.0
        + second as f64 * 1000.0
        + millis as f64;
    if !ms.is_finite() || ms.abs() > MAX_TIME {
        return None;
    }
    Some(ms)
}

fn read_uint(bytes: &[u8], pos: &mut usize, len: usize) -> Option<u64> {
    if *pos + len > bytes.len() {
        return None;
    }
    let mut value = 0u64;
    for &b in &bytes[*pos..*pos + len] {
        if !b.is_ascii_digit() {
            return None;
        }
        value = value * 10 + (b - b'0') as u64;
    }
    *pos += len;
    Some(value)
}

fn expect(bytes: &[u8], pos: &mut usize, ch: u8) -> Option<()> {
    if bytes.get(*pos) == Some(&ch) {
        *pos += 1;
        Some(())
    } else {
        None
    }
}

fn clamp_time(ms: f64) -> f64 {
    if !ms.is_finite() || ms.abs() > MAX_TIME { f64::NAN } else { ms }
}

pub fn construct(args: Vec<Value>, _span: Span) -> Result<Value, RuntimeError> {
    let ms = match args.into_iter().next() {
        None => now_ms(),
        Some(Value::Number(n)) => clamp_time(n),
        Some(Value::String(s)) => parse_iso(&s),
        Some(Value::Date(cell)) => cell.get(),
        Some(Value::Boolean(b)) => clamp_time(if b { 1.0 } else { 0.0 }),
        Some(_) => f64::NAN,
    };
    Ok(Value::Date(Rc::new(Cell::new(ms))))
}

pub fn call_static(
    _interp: &mut Interpreter,
    method: &str,
    _args: Vec<Value>,
    span: Span,
) -> Result<Value, RuntimeError> {
    match method {
        "сейчас" => Ok(Value::Number(now_ms())),
        _ => Err(RuntimeError::new(format!("Неизвестный статический метод 'Дата.{method}'"), span)),
    }
}

pub fn call_instance(
    _interp: &mut Interpreter,
    receiver: Value,
    method: &str,
    _args: Vec<Value>,
    span: Span,
) -> Result<(Value, Option<Value>), RuntimeError> {
    let Value::Date(cell) = &receiver else {
        return Err(RuntimeError::new("Метод вызван не на дате", span));
    };
    let ms = cell.get();
    let comp = ms_to_components(ms);

    let result = match method {
        "времяМс" | "вЧисло" => Value::Number(ms),
        "год" => Value::Number(comp.as_ref().map_or(f64::NAN, |c| c.year as f64)),
        "месяц" => Value::Number(comp.as_ref().map_or(f64::NAN, |c| (c.month - 1) as f64)),
        "день" => Value::Number(comp.as_ref().map_or(f64::NAN, |c| c.day as f64)),
        "деньНедели" => Value::Number(comp.as_ref().map_or(f64::NAN, |c| c.weekday as f64)),
        "часы" => Value::Number(comp.as_ref().map_or(f64::NAN, |c| c.hours as f64)),
        "минуты" => Value::Number(comp.as_ref().map_or(f64::NAN, |c| c.minutes as f64)),
        "секунды" => Value::Number(comp.as_ref().map_or(f64::NAN, |c| c.seconds as f64)),
        "миллисекунды" => Value::Number(comp.as_ref().map_or(f64::NAN, |c| c.millis as f64)),
        "вИСО" | "вСтроку" => Value::String(match &comp {
            Some(c) => format_components(c),
            None => "Invalid Date".to_string(),
        }),
        _ => return Err(RuntimeError::new(format!("Дата не имеет метода '{method}'"), span)),
    };
    Ok((result, None))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_epoch() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
    }

    #[test]
    fn civil_leap_day_2000() {
        let days = days_from_civil(2000, 2, 29);
        assert_eq!(civil_from_days(days), (2000, 2, 29));
    }

    #[test]
    fn civil_before_1970() {
        assert_eq!(civil_from_days(-1), (1969, 12, 31));
        let days = days_from_civil(1900, 1, 1);
        assert_eq!(civil_from_days(days), (1900, 1, 1));
    }

    #[test]
    fn weekday_epoch_is_thursday() {
        let c = ms_to_components(0.0).unwrap();
        assert_eq!(c.weekday, 4);
    }

    #[test]
    fn weekday_before_epoch() {
        let c = ms_to_components(-86_400_000.0).unwrap();
        assert_eq!(c.weekday, 3);
    }

    #[test]
    fn clamp_out_of_range_is_none() {
        assert!(ms_to_components(8.64e15 + 1.0).is_none());
        assert!(ms_to_components(-8.64e15 - 1.0).is_none());
        assert!(ms_to_components(f64::NAN).is_none());
        assert!(ms_to_components(f64::INFINITY).is_none());
    }

    #[test]
    fn format_epoch() {
        assert_eq!(format_iso(0.0), "1970-01-01T00:00:00.000Z");
    }

    #[test]
    fn format_invalid() {
        assert_eq!(format_iso(f64::NAN), "Invalid Date");
    }

    #[test]
    fn format_extended_year() {
        let days = days_from_civil(275_760, 9, 13);
        let ms = days as f64 * 86_400_000.0;
        assert_eq!(format_iso(ms), "+275760-09-13T00:00:00.000Z");
    }

    #[test]
    fn negative_day_ms_nonnegative() {
        let c = ms_to_components(-1.0).unwrap();
        assert_eq!((c.year, c.month, c.day), (1969, 12, 31));
        assert_eq!(c.hours, 23);
        assert_eq!(c.minutes, 59);
        assert_eq!(c.seconds, 59);
        assert_eq!(c.millis, 999);
    }

    #[test]
    fn parse_roundtrip() {
        assert_eq!(parse_iso("1970-01-01T00:00:00.000Z"), 0.0);
        assert_eq!(parse_iso("2000-02-29"), days_from_civil(2000, 2, 29) as f64 * 86_400_000.0);
        assert!(parse_iso("не дата").is_nan());
    }

    #[test]
    fn mutability_via_cell() {
        let cell = Cell::new(0.0);
        assert_eq!(format_iso(cell.get()), "1970-01-01T00:00:00.000Z");
        cell.set(86_400_000.0);
        assert_eq!(format_iso(cell.get()), "1970-01-02T00:00:00.000Z");
    }
}
