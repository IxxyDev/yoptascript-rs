use yps_lexer::Span;

use crate::error::RuntimeError;
use crate::interpreter::Interpreter;
use crate::value::{Value, to_int_n, to_uint_n};

use super::require_args;

pub fn construct(args: Vec<Value>, span: Span) -> Result<Value, RuntimeError> {
    let mut it = args.into_iter();
    let buffer = match it.next() {
        Some(Value::ArrayBuffer(b)) => b,
        Some(other) => {
            return Err(RuntimeError::new(
                format!("'ОбзорБайтов' ожидает ОбластьБайтов, получено '{}'", other.type_name()),
                span,
            ));
        }
        None => {
            return Err(RuntimeError::new("'ОбзорБайтов' ожидает аргумент ОбластьБайтов", span));
        }
    };
    let byte_len = buffer.borrow().len();
    let offset = match it.next() {
        None | Some(Value::Undefined) => 0,
        Some(Value::Number(o)) if o.is_finite() && o >= 0.0 && o.fract() == 0.0 => o as usize,
        Some(_) => {
            return Err(RuntimeError::new("'ОбзорБайтов': смещение должно быть неотрицательным целым", span));
        }
    };
    if offset > byte_len {
        return Err(RuntimeError::new(format!("'ОбзорБайтов': смещение {offset} вне области ({byte_len} байт)"), span));
    }
    let length = match it.next() {
        None | Some(Value::Undefined) => byte_len - offset,
        Some(Value::Number(l)) if l.is_finite() && l >= 0.0 && l.fract() == 0.0 => l as usize,
        Some(_) => {
            return Err(RuntimeError::new("'ОбзорБайтов': длина должна быть неотрицательным целым", span));
        }
    };
    if offset.checked_add(length).is_none_or(|end| end > byte_len) {
        return Err(RuntimeError::new(
            format!("'ОбзорБайтов': вьюха выходит за пределы области ({byte_len} байт)"),
            span,
        ));
    }
    Ok(Value::DataView { buffer, offset, length })
}

pub fn call(
    _interp: &mut Interpreter,
    receiver: Value,
    method: &str,
    args: Vec<Value>,
    span: Span,
) -> Result<(Value, Option<Value>), RuntimeError> {
    let Value::DataView { buffer, offset, length } = receiver else {
        return Err(RuntimeError::new(format!("Тип '{}' не является ОбзорБайтов", receiver.type_name()), span));
    };
    match method {
        "взятьЦ8" | "getUint8" => {
            let raw = read_byte(&buffer, offset, &args, length, method, span)?;
            Ok((Value::Number(raw as f64), None))
        }
        "взятьЧ8" | "getInt8" => {
            let raw = read_byte(&buffer, offset, &args, length, method, span)?;
            Ok((Value::Number(raw as i8 as f64), None))
        }
        "взятьЦ16" | "getUint16" => {
            let b = read_bytes::<2>(&buffer, offset, &args, length, method, span)?;
            let v = if is_little_endian(args.get(1)) { u16::from_le_bytes(b) } else { u16::from_be_bytes(b) };
            Ok((Value::Number(v as f64), None))
        }
        "взятьЧ16" | "getInt16" => {
            let b = read_bytes::<2>(&buffer, offset, &args, length, method, span)?;
            let v = if is_little_endian(args.get(1)) { i16::from_le_bytes(b) } else { i16::from_be_bytes(b) };
            Ok((Value::Number(v as f64), None))
        }
        "взятьЦ32" | "getUint32" => {
            let b = read_bytes::<4>(&buffer, offset, &args, length, method, span)?;
            let v = if is_little_endian(args.get(1)) { u32::from_le_bytes(b) } else { u32::from_be_bytes(b) };
            Ok((Value::Number(v as f64), None))
        }
        "взятьЧ32" | "getInt32" => {
            let b = read_bytes::<4>(&buffer, offset, &args, length, method, span)?;
            let v = if is_little_endian(args.get(1)) { i32::from_le_bytes(b) } else { i32::from_be_bytes(b) };
            Ok((Value::Number(v as f64), None))
        }
        "взятьДр32" | "getFloat32" => {
            let b = read_bytes::<4>(&buffer, offset, &args, length, method, span)?;
            let v = if is_little_endian(args.get(1)) { f32::from_le_bytes(b) } else { f32::from_be_bytes(b) };
            Ok((Value::Number(v as f64), None))
        }
        "взятьДр64" | "getFloat64" => {
            let b = read_bytes::<8>(&buffer, offset, &args, length, method, span)?;
            let v = if is_little_endian(args.get(1)) { f64::from_le_bytes(b) } else { f64::from_be_bytes(b) };
            Ok((Value::Number(v), None))
        }
        "задатьЦ8" | "setUint8" => {
            let (byte_off, num) = prepare_set(&args, 1, length, method, span)?;
            write_byte(&buffer, offset, byte_off, to_uint_n(num, 8) as u8);
            Ok((Value::Undefined, None))
        }
        "задатьЧ8" | "setInt8" => {
            let (byte_off, num) = prepare_set(&args, 1, length, method, span)?;
            write_byte(&buffer, offset, byte_off, to_int_n(num, 8) as i8 as u8);
            Ok((Value::Undefined, None))
        }
        "задатьЦ16" | "setUint16" => {
            let (byte_off, num) = prepare_set(&args, 2, length, method, span)?;
            let v = to_uint_n(num, 16) as u16;
            let encoded = if is_little_endian(args.get(2)) { v.to_le_bytes() } else { v.to_be_bytes() };
            write_bytes(&buffer, offset, byte_off, &encoded);
            Ok((Value::Undefined, None))
        }
        "задатьЧ16" | "setInt16" => {
            let (byte_off, num) = prepare_set(&args, 2, length, method, span)?;
            let v = to_int_n(num, 16) as i16;
            let encoded = if is_little_endian(args.get(2)) { v.to_le_bytes() } else { v.to_be_bytes() };
            write_bytes(&buffer, offset, byte_off, &encoded);
            Ok((Value::Undefined, None))
        }
        "задатьЦ32" | "setUint32" => {
            let (byte_off, num) = prepare_set(&args, 4, length, method, span)?;
            let v = to_uint_n(num, 32) as u32;
            let encoded = if is_little_endian(args.get(2)) { v.to_le_bytes() } else { v.to_be_bytes() };
            write_bytes(&buffer, offset, byte_off, &encoded);
            Ok((Value::Undefined, None))
        }
        "задатьЧ32" | "setInt32" => {
            let (byte_off, num) = prepare_set(&args, 4, length, method, span)?;
            let v = to_int_n(num, 32) as i32;
            let encoded = if is_little_endian(args.get(2)) { v.to_le_bytes() } else { v.to_be_bytes() };
            write_bytes(&buffer, offset, byte_off, &encoded);
            Ok((Value::Undefined, None))
        }
        "задатьДр32" | "setFloat32" => {
            let (byte_off, num) = prepare_set(&args, 4, length, method, span)?;
            let v = num as f32;
            let encoded = if is_little_endian(args.get(2)) { v.to_le_bytes() } else { v.to_be_bytes() };
            write_bytes(&buffer, offset, byte_off, &encoded);
            Ok((Value::Undefined, None))
        }
        "задатьДр64" | "setFloat64" => {
            let (byte_off, num) = prepare_set(&args, 8, length, method, span)?;
            let encoded = if is_little_endian(args.get(2)) { num.to_le_bytes() } else { num.to_be_bytes() };
            write_bytes(&buffer, offset, byte_off, &encoded);
            Ok((Value::Undefined, None))
        }
        _ => Err(RuntimeError::new(format!("'ОбзорБайтов' не имеет метода '{method}'"), span)),
    }
}

fn read_byte(
    buffer: &crate::value::SharedBuffer,
    offset: usize,
    args: &[Value],
    length: usize,
    method: &str,
    span: Span,
) -> Result<u8, RuntimeError> {
    require_args(args, 1, span, method)?;
    let byte_off = require_offset(&args[0], span, method)?;
    check_bounds(byte_off, 1, length, method, span)?;
    Ok(buffer.borrow()[offset + byte_off])
}

fn read_bytes<const N: usize>(
    buffer: &crate::value::SharedBuffer,
    offset: usize,
    args: &[Value],
    length: usize,
    method: &str,
    span: Span,
) -> Result<[u8; N], RuntimeError> {
    require_args(args, 1, span, method)?;
    let byte_off = require_offset(&args[0], span, method)?;
    check_bounds(byte_off, N, length, method, span)?;
    let bytes = buffer.borrow();
    let mut out = [0u8; N];
    out.copy_from_slice(&bytes[offset + byte_off..offset + byte_off + N]);
    Ok(out)
}

fn prepare_set(
    args: &[Value],
    size: usize,
    length: usize,
    method: &str,
    span: Span,
) -> Result<(usize, f64), RuntimeError> {
    require_args(args, 2, span, method)?;
    let byte_off = require_offset(&args[0], span, method)?;
    check_bounds(byte_off, size, length, method, span)?;
    let num = to_number_for_set(&args[1], span, method)?;
    Ok((byte_off, num))
}

fn write_byte(buffer: &crate::value::SharedBuffer, offset: usize, byte_off: usize, byte: u8) {
    buffer.borrow_mut()[offset + byte_off] = byte;
}

fn write_bytes(buffer: &crate::value::SharedBuffer, offset: usize, byte_off: usize, encoded: &[u8]) {
    let mut bytes = buffer.borrow_mut();
    bytes[offset + byte_off..offset + byte_off + encoded.len()].copy_from_slice(encoded);
}

fn require_offset(v: &Value, span: Span, method: &str) -> Result<usize, RuntimeError> {
    match v {
        Value::Number(n) if n.is_finite() && *n >= 0.0 && n.fract() == 0.0 => Ok(*n as usize),
        _ => Err(RuntimeError::new(format!("'{method}': смещение должно быть неотрицательным целым"), span)),
    }
}

fn check_bounds(byte_off: usize, size: usize, length: usize, method: &str, span: Span) -> Result<(), RuntimeError> {
    if byte_off.checked_add(size).is_none_or(|end| end > length) {
        return Err(RuntimeError::new(
            format!("'{method}': смещение {byte_off} + размер {size} выходит за пределы вьюхи ({length} байт)"),
            span,
        ));
    }
    Ok(())
}

fn is_little_endian(v: Option<&Value>) -> bool {
    matches!(v, Some(Value::Boolean(true)))
}

fn to_number_for_set(v: &Value, span: Span, method: &str) -> Result<f64, RuntimeError> {
    match v {
        Value::Number(n) => Ok(*n),
        Value::Boolean(b) => Ok(if *b { 1.0 } else { 0.0 }),
        Value::Null => Ok(0.0),
        Value::Undefined => Ok(f64::NAN),
        _ => Err(RuntimeError::new(
            format!("'{method}': значение должно быть числом, получено '{}'", v.type_name()),
            span,
        )),
    }
}
