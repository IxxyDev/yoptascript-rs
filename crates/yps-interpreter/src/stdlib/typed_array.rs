use std::cell::RefCell;
use std::rc::Rc;

use yps_lexer::Span;

use crate::error::RuntimeError;
use crate::value::{SharedBuffer, TypedArrayKind, Value};

fn checked_byte_len(length: usize, size: usize, span: Span) -> Result<usize, RuntimeError> {
    match length.checked_mul(size) {
        Some(bytes) if bytes <= isize::MAX as usize => Ok(bytes),
        _ => Err(RuntimeError::new("длина типизированного массива слишком велика", span)),
    }
}

pub fn kind_from_name(name: &str) -> Option<TypedArrayKind> {
    match name {
        "Ц8Массив" => Some(TypedArrayKind::U8),
        "Ц8ОграниченныйМассив" => Some(TypedArrayKind::U8Clamped),
        "Ч8Массив" => Some(TypedArrayKind::I8),
        "Ц16Массив" => Some(TypedArrayKind::U16),
        "Ч16Массив" => Some(TypedArrayKind::I16),
        "Ц32Массив" => Some(TypedArrayKind::U32),
        "Ч32Массив" => Some(TypedArrayKind::I32),
        "Др32Массив" => Some(TypedArrayKind::F32),
        "Др64Массив" => Some(TypedArrayKind::F64),
        _ => None,
    }
}

fn to_number_input(v: &Value, span: Span) -> Result<f64, RuntimeError> {
    match v {
        Value::Number(n) => Ok(*n),
        Value::Null => Ok(0.0),
        Value::Undefined => Ok(f64::NAN),
        Value::Boolean(b) => Ok(if *b { 1.0 } else { 0.0 }),
        Value::String(s) => {
            let t = s.trim();
            if t.is_empty() { Ok(0.0) } else { Ok(t.parse::<f64>().unwrap_or(f64::NAN)) }
        }
        other => Err(RuntimeError::new(
            format!("Нельзя записать значение типа '{}' в типизированный массив", other.type_name()),
            span,
        )),
    }
}

pub fn write_element(
    buffer: &SharedBuffer,
    kind: TypedArrayKind,
    byte_index: usize,
    v: &Value,
    span: Span,
) -> Result<(), RuntimeError> {
    let num = to_number_input(v, span)?;
    let mut bytes = buffer.borrow_mut();
    kind.write_le(&mut bytes, byte_index, num);
    Ok(())
}

pub fn construct(kind: TypedArrayKind, args: Vec<Value>, span: Span) -> Result<Value, RuntimeError> {
    let size = kind.element_size();
    let name = kind.type_name();
    let mut it = args.into_iter();
    let first = it.next();
    match first {
        None => Ok(Value::TypedArray { buffer: Rc::new(RefCell::new(Vec::new())), offset: 0, length: 0, kind }),
        Some(Value::Number(n)) => {
            if !n.is_finite() || n < 0.0 || n.fract() != 0.0 {
                return Err(RuntimeError::new(format!("'{name}' ожидает неотрицательную целую длину"), span));
            }
            let length = n as usize;
            let byte_len = checked_byte_len(length, size, span)?;
            let buffer = Rc::new(RefCell::new(vec![0u8; byte_len]));
            Ok(Value::TypedArray { buffer, offset: 0, length, kind })
        }
        Some(Value::ArrayBuffer(buffer)) => {
            let byte_len = buffer.borrow().len();
            let offset = match it.next() {
                None | Some(Value::Undefined) => 0,
                Some(Value::Number(o)) if o.is_finite() && o >= 0.0 && o.fract() == 0.0 => o as usize,
                Some(_) => {
                    return Err(RuntimeError::new(
                        format!("'{name}': смещение должно быть неотрицательным целым"),
                        span,
                    ));
                }
            };
            if offset % size != 0 {
                return Err(RuntimeError::new(
                    format!("'{name}': смещение {offset} не выровнено по размеру элемента {size}"),
                    span,
                ));
            }
            if offset > byte_len {
                return Err(RuntimeError::new(
                    format!("'{name}': смещение {offset} вне области ({byte_len} байт)"),
                    span,
                ));
            }
            let length = match it.next() {
                None | Some(Value::Undefined) => {
                    let remaining = byte_len - offset;
                    if remaining % size != 0 {
                        return Err(RuntimeError::new(
                            format!("'{name}': длина области ({byte_len}) не кратна размеру элемента {size}"),
                            span,
                        ));
                    }
                    remaining / size
                }
                Some(Value::Number(l)) if l.is_finite() && l >= 0.0 && l.fract() == 0.0 => l as usize,
                Some(_) => {
                    return Err(RuntimeError::new(format!("'{name}': длина должна быть неотрицательным целым"), span));
                }
            };
            let view_bytes = checked_byte_len(length, size, span)?;
            if offset.checked_add(view_bytes).is_none_or(|end| end > byte_len) {
                return Err(RuntimeError::new(
                    format!("'{name}': вьюха выходит за пределы области ({byte_len} байт)"),
                    span,
                ));
            }
            Ok(Value::TypedArray { buffer, offset, length, kind })
        }
        Some(Value::Array(arr)) => {
            let snapshot = arr.borrow().clone();
            let length = snapshot.len();
            let byte_len = checked_byte_len(length, size, span)?;
            let buffer = Rc::new(RefCell::new(vec![0u8; byte_len]));
            for (i, el) in snapshot.iter().enumerate() {
                write_element(&buffer, kind, i * size, el, span)?;
            }
            Ok(Value::TypedArray { buffer, offset: 0, length, kind })
        }
        Some(Value::TypedArray { buffer: src, offset: src_off, length, kind: src_kind }) => {
            let byte_len = checked_byte_len(length, size, span)?;
            let new_buffer = Rc::new(RefCell::new(vec![0u8; byte_len]));
            let src_size = src_kind.element_size();
            let src_bytes = src.borrow();
            let mut dst = new_buffer.borrow_mut();
            for i in 0..length {
                let num = src_kind.read_le(&src_bytes, src_off + i * src_size);
                kind.write_le(&mut dst, i * size, num);
            }
            drop(dst);
            Ok(Value::TypedArray { buffer: new_buffer, offset: 0, length, kind })
        }
        Some(other) => {
            Err(RuntimeError::new(format!("'{name}' нельзя построить из значения типа '{}'", other.type_name()), span))
        }
    }
}

pub fn construct_array_buffer(args: Vec<Value>, span: Span) -> Result<Value, RuntimeError> {
    let n = match args.into_iter().next() {
        None | Some(Value::Undefined) => 0.0,
        Some(Value::Number(n)) => n,
        Some(other) => {
            return Err(RuntimeError::new(
                format!("'ОбластьБайтов' ожидает число байт, получено '{}'", other.type_name()),
                span,
            ));
        }
    };
    if !n.is_finite() || n < 0.0 || n.fract() != 0.0 {
        return Err(RuntimeError::new("'ОбластьБайтов' ожидает неотрицательное целое число байт", span));
    }
    Ok(Value::ArrayBuffer(Rc::new(RefCell::new(vec![0u8; n as usize]))))
}

pub fn ta_elements(buffer: &SharedBuffer, offset: usize, length: usize, kind: TypedArrayKind) -> Vec<Value> {
    let bytes = buffer.borrow();
    let size = kind.element_size();
    (0..length).map(|i| Value::Number(kind.read_le(&bytes, offset + i * size))).collect()
}

fn relative_index(arg: Option<&Value>, length: usize, default: usize) -> usize {
    match arg {
        None | Some(Value::Undefined) => default,
        Some(Value::Number(n)) => {
            if !n.is_finite() {
                return if *n > 0.0 { length } else { 0 };
            }
            let i = n.trunc();
            if i < 0.0 {
                let from_end = length as f64 + i;
                if from_end < 0.0 { 0 } else { from_end as usize }
            } else if i as usize > length {
                length
            } else {
                i as usize
            }
        }
        _ => default,
    }
}

pub fn call(
    _interp: &mut crate::interpreter::Interpreter,
    receiver: Value,
    method: &str,
    args: Vec<Value>,
    span: Span,
) -> Result<(Value, Option<Value>), RuntimeError> {
    let Value::TypedArray { buffer, offset, length, kind } = &receiver else {
        return Err(RuntimeError::new("Метод вызван не на типизированном массиве", span));
    };
    let buffer = buffer.clone();
    let offset = *offset;
    let length = *length;
    let kind = *kind;
    let size = kind.element_size();
    match method {
        "набор" | "set" => {
            let source = args.first().cloned().unwrap_or(Value::Undefined);
            let target_offset = match args.get(1) {
                None | Some(Value::Undefined) => 0,
                Some(Value::Number(n)) if n.is_finite() && *n >= 0.0 && n.fract() == 0.0 => *n as usize,
                Some(_) => return Err(RuntimeError::new("'набор': смещение должно быть неотрицательным целым", span)),
            };
            let items: Vec<Value> = match source {
                Value::Array(a) => a.borrow().clone(),
                Value::TypedArray { buffer: sb, offset: so, length: sl, kind: sk } => ta_elements(&sb, so, sl, sk),
                other => {
                    return Err(RuntimeError::new(
                        format!("'набор' ожидает массив или типизированный массив, получено '{}'", other.type_name()),
                        span,
                    ));
                }
            };
            if target_offset.checked_add(items.len()).is_none_or(|end| end > length) {
                return Err(RuntimeError::new("'набор': источник не помещается в целевой массив", span));
            }
            for (i, el) in items.iter().enumerate() {
                write_element(&buffer, kind, offset + (target_offset + i) * size, el, span)?;
            }
            Ok((Value::Undefined, None))
        }
        "подмассив" | "subarray" => {
            let begin = relative_index(args.first(), length, 0);
            let end = relative_index(args.get(1), length, length);
            let new_length = end.saturating_sub(begin);
            Ok((Value::TypedArray { buffer, offset: offset + begin * size, length: new_length, kind }, None))
        }
        "срез" | "slice" => {
            let begin = relative_index(args.first(), length, 0);
            let end = relative_index(args.get(1), length, length);
            let new_length = end.saturating_sub(begin);
            let new_buffer = {
                let src = buffer.borrow();
                let start_byte = offset + begin * size;
                let end_byte = start_byte + new_length * size;
                Rc::new(RefCell::new(src[start_byte..end_byte].to_vec()))
            };
            Ok((Value::TypedArray { buffer: new_buffer, offset: 0, length: new_length, kind }, None))
        }
        _ => Err(RuntimeError::new(format!("У типизированного массива нет метода '{method}'"), span)),
    }
}
