use yps_lexer::Span;

use crate::error::RuntimeError;
use crate::value::{IteratorState, Value};

use super::Interpreter;

impl Interpreter {
    pub(super) fn call_value_with_this(
        &mut self,
        func: Value,
        this: Option<Value>,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        match func {
            Value::Function(fdata) => self.call_method_with_this(
                fdata.name.clone(),
                &fdata.params,
                &fdata.body,
                &fdata.env,
                vec![],
                this,
                span,
            ),
            other => self.call_function(other, vec![], span),
        }
    }

    pub(super) fn get_user_iterator(&mut self, val: &Value, span: Span) -> Result<Option<Value>, RuntimeError> {
        let Value::Object(map) = val else {
            return Ok(None);
        };
        let iter_key = crate::symbols::symbol_key(crate::stdlib::symbol::ITERATOR_ID);
        let iter_method = map.borrow().get(&iter_key).cloned();
        let Some(method) = iter_method else {
            return Ok(None);
        };
        let iterator_obj = self.call_value_with_this(method, Some(val.clone()), span)?;
        Ok(Some(iterator_obj))
    }

    pub(super) fn get_async_iterator(&mut self, val: &Value, span: Span) -> Result<Option<Value>, RuntimeError> {
        match val {
            Value::Iterator(rc) => {
                let is_async_gen = matches!(&*rc.borrow(), IteratorState::Generator(g) if g.is_async);
                if is_async_gen { Ok(Some(val.clone())) } else { Ok(None) }
            }
            Value::Object(map) => {
                let key = crate::symbols::symbol_key(crate::stdlib::symbol::ASYNC_ITERATOR_ID);
                let method = map.borrow().get(&key).cloned();
                match method {
                    Some(method) => Ok(Some(self.call_value_with_this(method, Some(val.clone()), span)?)),
                    None => Ok(None),
                }
            }
            _ => Ok(None),
        }
    }

    fn call_async_iter_method(
        &mut self,
        aiter: &Value,
        method: &str,
        span: Span,
    ) -> Result<Option<Value>, RuntimeError> {
        if let Value::Iterator(_) = aiter {
            let (ret, _) = crate::stdlib::call_method(self, aiter.clone(), method, vec![], span)?;
            return Ok(Some(ret));
        }
        let func = self.eval_member(aiter.clone(), method, span)?;
        if matches!(func, Value::Undefined | Value::Null) {
            return Ok(None);
        }
        Ok(Some(self.call_value_with_this(func, Some(aiter.clone()), span)?))
    }

    pub(super) fn async_iter_next(&mut self, aiter: &Value, span: Span) -> Result<(bool, Value), RuntimeError> {
        let result = match self.call_async_iter_method(aiter, "следующий", span)? {
            Some(result) => result,
            None => return Err(RuntimeError::new("У асинхронного итератора нет метода 'следующий'", span)),
        };
        let result = self.do_await(result, span)?;
        match &result {
            Value::Object(r) => {
                let b = r.borrow();
                let done = matches!(b.get(crate::symbols::ITER_DONE), Some(Value::Boolean(true)));
                let value = b.get(crate::symbols::ITER_VALUE).cloned().unwrap_or(Value::Undefined);
                Ok((done, value))
            }
            _ => Ok((true, Value::Undefined)),
        }
    }

    pub(super) fn async_iter_close(&mut self, aiter: &Value, span: Span) -> Result<(), RuntimeError> {
        if let Some(result) = self.call_async_iter_method(aiter, "вернуть", span)? {
            self.do_await(result, span)?;
        }
        Ok(())
    }

    pub(super) fn collect_user_iterable(
        &mut self,
        iterator_obj: Value,
        span: Span,
    ) -> Result<Vec<Value>, RuntimeError> {
        let mut values = Vec::new();
        loop {
            let next_fn = self.eval_member(iterator_obj.clone(), "следующий", span)?;
            let result = self.call_value_with_this(next_fn, Some(iterator_obj.clone()), span)?;
            let done = match &result {
                Value::Object(r) => r.borrow().get(crate::symbols::ITER_DONE).cloned(),
                _ => None,
            };
            if matches!(done, Some(Value::Boolean(true))) {
                break;
            }
            let item = match &result {
                Value::Object(r) => r.borrow().get(crate::symbols::ITER_VALUE).cloned().unwrap_or(Value::Undefined),
                _ => Value::Undefined,
            };
            values.push(item);
        }
        Ok(values)
    }
}
