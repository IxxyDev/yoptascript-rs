use std::rc::Rc;

use yps_lexer::Span;

use crate::error::RuntimeError;
use crate::value::Value;

use super::Interpreter;

impl Interpreter {
    pub fn host_member_get(&mut self, obj: Value, property: &str, span: Span) -> Result<Value, RuntimeError> {
        self.eval_member(obj, property, span)
    }

    pub fn host_call(&mut self, callee: Value, args: Vec<Value>, span: Span) -> Result<Value, RuntimeError> {
        self.call_function(callee, args, span)
    }

    pub fn host_index_get(&mut self, obj: Value, index: Value, span: Span) -> Result<Value, RuntimeError> {
        self.eval_index(obj, index, span)
    }

    pub fn host_member_set(
        &mut self,
        obj: &Value,
        property: &str,
        value: Value,
        span: Span,
    ) -> Result<(), RuntimeError> {
        match obj {
            Value::Proxy { target, handler } => {
                let target = Rc::clone(target);
                let handler = Rc::clone(handler);
                self.proxy_set(&target, &handler, property, value, obj.clone(), span)
            }
            Value::Object(map) => {
                if !map.borrow().frozen {
                    map.borrow_mut().insert(property.to_string(), value);
                }
                Ok(())
            }
            _ => Err(RuntimeError::new(
                format!("Нельзя установить свойство '{property}' у типа '{}'", obj.type_name()),
                span,
            )),
        }
    }

    pub fn host_index_set(&mut self, obj: &Value, index: &Value, value: Value, span: Span) -> Result<(), RuntimeError> {
        match obj {
            Value::Proxy { target, handler } => {
                let target = Rc::clone(target);
                let handler = Rc::clone(handler);
                self.proxy_set(&target, &handler, &index.to_string(), value, obj.clone(), span)
            }
            Value::Object(map) => {
                if !map.borrow().frozen {
                    map.borrow_mut().insert(index.to_string(), value);
                }
                Ok(())
            }
            Value::Array(arr) => {
                let n = match index {
                    Value::Number(n) => *n,
                    other => crate::interpreter::coercion::to_number(other),
                };
                if n.is_finite() && n >= 0.0 && n.fract() == 0.0 {
                    let i = n as usize;
                    let mut guard = arr.borrow_mut();
                    let len = guard.len();
                    if let Some(slot) = guard.0.get_mut(i) {
                        *slot = value;
                    } else {
                        return Err(RuntimeError::new(format!("Индекс {i} вне диапазона (длина {len})"), span));
                    }
                }
                Ok(())
            }
            Value::TypedArray { buffer, offset, length, kind } => {
                let n = match index {
                    Value::Number(n) => *n,
                    other => crate::interpreter::coercion::to_number(other),
                };
                if n.is_finite() && n >= 0.0 && n.fract() == 0.0 {
                    let i = n as usize;
                    if i < *length {
                        crate::stdlib::typed_array::write_element(
                            buffer,
                            *kind,
                            *offset + i * kind.element_size(),
                            &value,
                            span,
                        )?;
                    }
                }
                Ok(())
            }
            _ => Err(RuntimeError::new(format!("Нельзя индексировать тип '{}' для записи", obj.type_name()), span)),
        }
    }

    pub fn host_in(&mut self, key: &Value, container: &Value, span: Span) -> Result<bool, RuntimeError> {
        match container {
            Value::Proxy { target, handler } => {
                let target = Rc::clone(target);
                let handler = Rc::clone(handler);
                let key = crate::interpreter::coercion::to_ecma_string(key);
                self.proxy_has(&target, &handler, &key, span)
            }
            Value::Object(map) => {
                let key = crate::interpreter::coercion::to_ecma_string(key);
                Ok(map.borrow().contains_key(&key))
            }
            Value::Array(arr) => {
                let len = arr.borrow().len();
                Ok(match key {
                    Value::Number(n) => n.fract() == 0.0 && *n >= 0.0 && (*n as usize) < len,
                    other => match crate::interpreter::coercion::to_ecma_string(other).parse::<usize>() {
                        Ok(idx) => idx < len,
                        Err(_) => false,
                    },
                })
            }
            other => Err(RuntimeError::new(
                format!("Правая сторона 'из' должна быть объектом или массивом, получено '{}'", other.type_name()),
                span,
            )),
        }
    }

    pub fn host_for_in_keys(&mut self, obj: &Value, span: Span) -> Result<Vec<Value>, RuntimeError> {
        match obj {
            Value::Array(elements) => Ok(elements.borrow().0.clone()),
            Value::TypedArray { buffer, offset, length, kind } => {
                Ok(crate::stdlib::typed_array::ta_elements(buffer, *offset, *length, *kind))
            }
            Value::Object(map) => Ok(map.borrow().keys().map(|k| Value::String(k.clone())).collect()),
            other => Err(RuntimeError::new(format!("Нельзя итерировать по типу '{}'", other.type_name()), span)),
        }
    }

    pub fn host_iterate(&mut self, obj: &Value, span: Span) -> Result<Vec<Value>, RuntimeError> {
        let obj = match obj.proxy_parts() {
            Some((target, _)) => (*target).clone(),
            None => obj.clone(),
        };
        match obj {
            Value::Array(arr) => Ok(arr.borrow().0.clone()),
            Value::String(s) => Ok(s.chars().map(|c| Value::String(c.to_string())).collect()),
            Value::Set(items) => Ok(items.borrow().iter().map(|k| k.as_value().clone()).collect()),
            Value::Map(entries) => {
                Ok(entries.borrow().iter().map(|(k, v)| Value::array(vec![k.as_value().clone(), v.clone()])).collect())
            }
            Value::TypedArray { buffer, offset, length, kind } => {
                Ok(crate::stdlib::typed_array::ta_elements(&buffer, offset, length, kind))
            }
            Value::Iterator(rc) => crate::stdlib::iterator::drain(self, &rc, span),
            other => Err(RuntimeError::new(format!("Нельзя итерировать по типу '{}'", other.type_name()), span)),
        }
    }
}
