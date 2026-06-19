use yps_lexer::Span;

use crate::error::RuntimeError;
use crate::stdlib::proxy;
use crate::value::Value;

use super::Interpreter;

impl Interpreter {
    pub(crate) fn proxy_get(
        &mut self,
        target: &Value,
        handler: &Value,
        key: &str,
        proxy: Value,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if let Some(trap) = proxy::trap(handler, proxy::GET) {
            return self.call_function(trap, vec![target.clone(), Value::String(key.to_string()), proxy], span);
        }
        if let Value::Array(arr) = target
            && let Ok(idx) = key.parse::<usize>()
        {
            return Ok(arr.borrow().get(idx).cloned().unwrap_or(Value::Undefined));
        }
        self.eval_member(target.clone(), key, span)
    }

    pub(crate) fn proxy_set(
        &mut self,
        target: &Value,
        handler: &Value,
        key: &str,
        value: Value,
        proxy: Value,
        span: Span,
    ) -> Result<(), RuntimeError> {
        if let Some(trap) = proxy::trap(handler, proxy::SET) {
            let result =
                self.call_function(trap, vec![target.clone(), Value::String(key.to_string()), value, proxy], span)?;
            if !result.is_truthy() {
                return Err(RuntimeError::new(
                    format!("Ловушка 'установить' посредника отвергла запись свойства '{key}'"),
                    span,
                ));
            }
            return Ok(());
        }
        match target {
            Value::Object(map) => {
                if !map.borrow().frozen {
                    map.borrow_mut().insert(key.to_string(), value);
                }
                Ok(())
            }
            Value::Array(arr) => {
                if let Ok(idx) = key.parse::<usize>() {
                    let mut guard = arr.borrow_mut();
                    let len = guard.len();
                    let slot = guard
                        .get_mut(idx)
                        .ok_or_else(|| RuntimeError::new(format!("Индекс {idx} вне диапазона (длина {len})"), span))?;
                    *slot = value;
                }
                Ok(())
            }
            _ => Err(RuntimeError::new(
                format!("Нельзя установить свойство у цели посредника типа '{}'", target.type_name()),
                span,
            )),
        }
    }

    pub(crate) fn proxy_has(
        &mut self,
        target: &Value,
        handler: &Value,
        key: &str,
        span: Span,
    ) -> Result<bool, RuntimeError> {
        if let Some(trap) = proxy::trap(handler, proxy::HAS) {
            let result = self.call_function(trap, vec![target.clone(), Value::String(key.to_string())], span)?;
            return Ok(result.is_truthy());
        }
        Ok(match target {
            Value::Object(map) => map.borrow().contains_key(key),
            Value::Array(arr) => {
                key == "length" || key == "длина" || key.parse::<usize>().is_ok_and(|i| i < arr.borrow().len())
            }
            _ => false,
        })
    }

    pub(crate) fn proxy_delete(
        &mut self,
        target: &Value,
        handler: &Value,
        key: &str,
        span: Span,
    ) -> Result<bool, RuntimeError> {
        if let Some(trap) = proxy::trap(handler, proxy::DELETE) {
            let result = self.call_function(trap, vec![target.clone(), Value::String(key.to_string())], span)?;
            return Ok(result.is_truthy());
        }
        if let Value::Object(map) = target
            && !map.borrow().frozen
        {
            map.borrow_mut().shift_remove(key);
        }
        Ok(true)
    }

    pub(crate) fn proxy_apply(
        &mut self,
        target: &Value,
        handler: &Value,
        args: Vec<Value>,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if let Some(trap) = proxy::trap(handler, proxy::APPLY) {
            return self.call_function(trap, vec![target.clone(), Value::Undefined, Value::array(args)], span);
        }
        self.call_function(target.clone(), args, span)
    }

    pub(crate) fn proxy_construct(
        &mut self,
        target: &Value,
        handler: &Value,
        args: Vec<Value>,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if let Some(trap) = proxy::trap(handler, proxy::CONSTRUCT) {
            return self.call_function(trap, vec![target.clone(), Value::array(args)], span);
        }
        self.construct_instance(target.clone(), args, span)
    }
}
