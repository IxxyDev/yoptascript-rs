use yps_lexer::Span;

use crate::error::RuntimeError;
use crate::value::Value;

use super::Interpreter;

impl Interpreter {
    pub(super) fn call_value_with_this(
        &mut self,
        func: Value,
        this: Option<Value>,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        match func {
            Value::Function { name, params, body, env, .. } => {
                self.call_method_with_this(name, &params, &body, &env, vec![], this, span)
            }
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
