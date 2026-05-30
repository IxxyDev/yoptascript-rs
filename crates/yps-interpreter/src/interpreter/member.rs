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
            Value::RegExp { .. } => {
                if let Some(v) = crate::stdlib::regexp::member(&obj, property) {
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
                    return self.call_method_with_this(
                        Rc::from(property),
                        &params,
                        &body,
                        &env,
                        vec![],
                        Some(obj),
                        span,
                    );
                }
                if let Some(val) = map.get(property) {
                    return Ok(val.clone());
                }
                let effective_class = Self::resolve_class_for_object(map, &self.env);
                if (property == "конструктор" || property == "constructor")
                    && let Some(cls) = &effective_class
                {
                    return Ok(Value::Class(std::rc::Rc::clone(cls)));
                }
                if let Some(cls) = &effective_class {
                    if let Some((params, body, env)) = Self::find_getter_in_class(cls, property) {
                        let params = params.clone();
                        let body = Rc::clone(body);
                        let env = Rc::clone(env);
                        return self.call_method_with_this(
                            Rc::from(property),
                            &params,
                            &body,
                            &env,
                            vec![],
                            Some(obj),
                            span,
                        );
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
                if let Some(proto) = map.get(symbols::PROTO).cloned() {
                    match proto {
                        Value::Class(_) | Value::Null => return Ok(Value::Undefined),
                        _ => return self.eval_member(proto, property, span),
                    }
                }
                Ok(Value::Undefined)
            }
            Value::Class(cls) => {
                if property == "прототип" || property == "prototype" {
                    return Ok(Self::class_prototype_object(cls));
                }
                if let Some((params, body, env)) = cls.static_getters.get(property) {
                    return self.call_method_with_this(Rc::from(property), params, body, env, vec![], None, span);
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
            Value::AbortController { .. } | Value::AbortSignal { .. } => {
                if let Some(val) = crate::stdlib::abort::get_property(&obj, property) {
                    return Ok(val);
                }
                Ok(Value::Undefined)
            }
            _ => Err(RuntimeError::new(format!("Нельзя получить свойство у типа '{}'", obj.type_name()), span)),
        }
    }

    pub(crate) fn resolve_class_for_object(
        map: &std::collections::HashMap<String, Value>,
        env: &crate::environment::Environment,
    ) -> Option<std::rc::Rc<crate::value::ClassDef>> {
        match map.get(symbols::PROTO) {
            Some(Value::Class(cls)) => Some(std::rc::Rc::clone(cls)),
            Some(_) => None,
            None => {
                if let Some(Value::String(class_name)) = map.get(symbols::CLASS_TAG)
                    && let Some(Value::Class(cls)) = env.get(class_name)
                {
                    return Some(cls);
                }
                None
            }
        }
    }

    pub(crate) fn class_prototype_object(cls: &std::rc::Rc<crate::value::ClassDef>) -> Value {
        let mut map = std::collections::HashMap::new();
        let mut current: Option<&crate::value::ClassDef> = Some(cls);
        while let Some(c) = current {
            for (name, (params, body, env)) in &c.methods {
                map.entry(name.clone()).or_insert_with(|| Value::Function {
                    name: Rc::from(name.as_str()),
                    params: params.clone(),
                    body: Rc::clone(body),
                    env: Rc::clone(env),
                    is_generator: false,
                    is_async: false,
                });
            }
            current = c.parent.as_deref();
        }
        map.insert("конструктор".to_string(), Value::Class(std::rc::Rc::clone(cls)));
        map.insert(symbols::PROTO.to_string(), Value::Class(std::rc::Rc::clone(cls)));
        Value::Object(map)
    }
}
