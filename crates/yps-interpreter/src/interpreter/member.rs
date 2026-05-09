use std::rc::Rc;

use yps_lexer::Span;

use crate::error::RuntimeError;
use crate::symbols;
use crate::value::Value;

use super::Interpreter;

impl Interpreter {
    pub(super) fn eval_member(&mut self, obj: Value, property: &str, span: Span) -> Result<Value, RuntimeError> {
        match &obj {
            Value::Array(arr) => {
                if property == "length" || property == "длина" {
                    return Ok(Value::Number(arr.len() as f64));
                }
                Ok(Value::Undefined)
            }
            Value::Map(entries) => {
                if property == "size" || property == "размер" {
                    return Ok(Value::Number(entries.len() as f64));
                }
                Ok(Value::Undefined)
            }
            Value::Set(items) => {
                if property == "size" || property == "размер" {
                    return Ok(Value::Number(items.len() as f64));
                }
                Ok(Value::Undefined)
            }
            Value::String(s) => {
                if property == "length" || property == "длина" {
                    return Ok(Value::Number(s.chars().count() as f64));
                }
                Ok(Value::Undefined)
            }
            Value::BuiltinFunction(name) => {
                if name == "Симбол"
                    && let Some(sym) = crate::stdlib::symbol::well_known(property)
                {
                    return Ok(sym);
                }
                Ok(Value::BuiltinFunction(format!("{name}.{property}")))
            }
            Value::Symbol { .. } => {
                if let Some(v) = crate::stdlib::symbol::member(&obj, property) {
                    return Ok(v);
                }
                Ok(Value::Undefined)
            }
            Value::Object(map) => {
                if property.starts_with('#') {
                    let in_class = if let Some(Value::String(class_name)) = map.get(symbols::CLASS_TAG) {
                        self.env.get(symbols::THIS).is_some()
                            && self
                                .env
                                .get(symbols::THIS)
                                .and_then(|this| {
                                    if let Value::Object(m) = &this { m.get(symbols::CLASS_TAG).cloned() } else { None }
                                })
                                .is_some_and(|c| if let Value::String(cn) = c { cn == *class_name } else { false })
                    } else {
                        false
                    };
                    if !in_class {
                        return Err(RuntimeError::new(
                            format!("Нельзя обращаться к приватному полю '{property}' за пределами класса"),
                            span,
                        ));
                    }
                }

                let getter_key = symbols::getter_key(property);
                if let Some(Value::Function { params, body, env, .. }) = map.get(&getter_key) {
                    let params = params.clone();
                    let body = Rc::clone(body);
                    let env = Rc::clone(env);
                    return self.call_method_with_this(&params, &body, &env, vec![], Some(obj), span);
                }
                if let Some(val) = map.get(property) {
                    return Ok(val.clone());
                }
                if let Some(Value::String(class_name)) = map.get(symbols::CLASS_TAG)
                    && let Some(Value::Class(cls)) = self.env.get(class_name).as_ref()
                {
                    if let Some((params, body, env)) = Self::find_getter_in_class(cls, property) {
                        let params = params.clone();
                        let body = Rc::clone(body);
                        let env = Rc::clone(env);
                        return self.call_method_with_this(&params, &body, &env, vec![], Some(obj), span);
                    }
                    if let Some((params, body, env)) = Self::find_method_in_class(cls, property) {
                        return Ok(Value::Function {
                            name: Rc::from(property),
                            params: params.clone(),
                            body: Rc::clone(body),
                            env: Rc::clone(env),
                            is_generator: false,
                            is_async: false,
                        });
                    }
                }
                Ok(Value::Undefined)
            }
            Value::Class(cls) => {
                if let Some((params, body, env)) = cls.static_getters.get(property) {
                    return self.call_method_with_this(params, body, env, vec![], None, span);
                }
                if let Some(val) = cls.static_fields.get(property) {
                    return Ok(val.clone());
                }
                if let Some((params, body, env)) = cls.static_methods.get(property) {
                    return Ok(Value::Function {
                        name: Rc::from(property),
                        params: params.clone(),
                        body: Rc::clone(body),
                        env: Rc::clone(env),
                        is_generator: false,
                        is_async: false,
                    });
                }
                Ok(Value::Undefined)
            }
            _ => Err(RuntimeError::new(format!("Нельзя получить свойство у типа '{}'", obj.type_name()), span)),
        }
    }
}
