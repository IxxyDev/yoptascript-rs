use yps_lexer::Span;
use yps_parser::ast::Expr;

use crate::error::RuntimeError;
use crate::value::Value;

use super::Interpreter;

impl Interpreter {
    pub(super) fn eval_delete(&mut self, expr: &Expr, span: Span) -> Result<Value, RuntimeError> {
        match expr {
            Expr::Member { object, property, .. } => {
                let mut path = Vec::new();
                let root_name = self.collect_access_path(
                    &Expr::Member { object: object.clone(), property: property.clone(), span },
                    &mut path,
                    span,
                )?;
                path.reverse();
                if path.len() == 1
                    && let Some(Value::Object(map)) = self.env.get(&root_name)
                {
                    map.borrow_mut().remove(&property.name);
                }
                Ok(Value::Boolean(true))
            }
            Expr::Index { object, index, .. } => {
                let idx = self.eval_expr(index)?;
                let mut path = Vec::new();
                let root_name = self.collect_access_path(object, &mut path, span)?;
                path.reverse();
                if path.is_empty() {
                    match self.env.get(&root_name) {
                        Some(Value::Object(map)) => {
                            let key = idx.to_string();
                            map.borrow_mut().remove(&key);
                        }
                        Some(Value::Array(arr)) => {
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
                        Some(Value::String(_)) => {
                            return Err(RuntimeError::new("Нельзя 'ёбнуть' символ строки — строки неизменяемы", span));
                        }
                        _ => {}
                    }
                }
                Ok(Value::Boolean(true))
            }
            _ => Ok(Value::Boolean(true)),
        }
    }
}
