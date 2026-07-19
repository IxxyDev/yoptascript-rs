use yps_lexer::Span;
use yps_parser::ast::Expr;

use crate::error::RuntimeError;
use crate::value::Value;

use super::Interpreter;

impl Interpreter {
    pub(super) fn eval_delete(&mut self, expr: &Expr, span: Span) -> Result<Value, RuntimeError> {
        match expr {
            Expr::Member { object, property, .. } => {
                let obj = self.eval_expr(object)?;
                if let Some((target, handler)) = obj.proxy_parts() {
                    let removed = self.proxy_delete(&target, &handler, &property.name, span)?;
                    return Ok(Value::Boolean(removed));
                }
                if let Value::Object(map) = &obj
                    && map.borrow().can_delete()
                {
                    map.borrow_mut().shift_remove(&property.name);
                }
                Ok(Value::Boolean(true))
            }
            Expr::Index { object, index, .. } => {
                let idx = self.eval_expr(index)?;
                let obj = self.eval_expr(object)?;
                if let Some((target, handler)) = obj.proxy_parts() {
                    let key = idx.to_string();
                    let removed = self.proxy_delete(&target, &handler, &key, span)?;
                    return Ok(Value::Boolean(removed));
                }
                match &obj {
                    Value::Object(map) => {
                        if map.borrow().can_delete() {
                            let key = idx.to_string();
                            map.borrow_mut().shift_remove(&key);
                        }
                    }
                    Value::Array(arr) => {
                        if let Value::Number(n) = idx
                            && n.is_finite()
                            && n >= 0.0
                            && n.fract() == 0.0
                        {
                            let i = n as usize;
                            let mut guard = arr.borrow_mut();
                            if i < guard.len() {
                                guard[i] = Value::Undefined;
                            }
                        }
                    }
                    Value::String(_) => {
                        return Err(RuntimeError::new("Нельзя 'ёбнуть' символ строки — строки неизменяемы", span));
                    }
                    _ => {}
                }
                Ok(Value::Boolean(true))
            }
            _ => Ok(Value::Boolean(true)),
        }
    }
}
