use std::rc::Rc;

use indexmap::IndexMap;

use yps_lexer::Span;

use crate::error::RuntimeError;
use crate::symbols;
use crate::value::{ClassDef, MethodDef, Value};

use super::Interpreter;

impl Interpreter {
    pub(crate) fn eval_member(&mut self, obj: Value, property: &str, span: Span) -> Result<Value, RuntimeError> {
        match &obj {
            Value::Proxy { target, handler } => {
                let target = Rc::clone(target);
                let handler = Rc::clone(handler);
                self.proxy_get(&target, &handler, property, obj.clone(), span)
            }
            Value::Array(arr) => {
                if property == "length" || property == "длина" {
                    return Ok(Value::Number(arr.borrow().len() as f64));
                }
                if crate::stdlib::array::method_exists(property) {
                    return Ok(Value::BoundMethod { receiver: Box::new(obj.clone()), method: property.to_string() });
                }
                Ok(Value::Undefined)
            }
            Value::Map(entries) => {
                if property == "size" || property == "размер" {
                    return Ok(Value::Number(entries.borrow().len() as f64));
                }
                Ok(Value::Undefined)
            }
            Value::Set(items) => {
                if property == "size" || property == "размер" {
                    return Ok(Value::Number(items.borrow().len() as f64));
                }
                Ok(Value::Undefined)
            }
            Value::String(s) => {
                if property == "length" || property == "длина" {
                    return Ok(Value::Number(s.encode_utf16().count() as f64));
                }
                if crate::stdlib::string::method_exists(property) {
                    return Ok(Value::BoundMethod { receiver: Box::new(obj.clone()), method: property.to_string() });
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
                    let class_name = match map.borrow().get(symbols::CLASS_TAG) {
                        Some(Value::String(cn)) => Some(cn.clone()),
                        _ => None,
                    };
                    let in_class = if let Some(class_name) = class_name {
                        self.env.get(symbols::THIS).is_some()
                            && self
                                .env
                                .get(symbols::THIS)
                                .and_then(|this| {
                                    if let Value::Object(m) = &this {
                                        m.borrow().get(symbols::CLASS_TAG).cloned()
                                    } else {
                                        None
                                    }
                                })
                                .is_some_and(|c| if let Value::String(cn) = c { cn == class_name } else { false })
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
                let getter = match map.borrow().get(&getter_key) {
                    Some(Value::Function { params, body, env, .. }) => {
                        Some((params.clone(), Rc::clone(body), Rc::clone(env)))
                    }
                    _ => None,
                };
                if let Some((params, body, env)) = getter {
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
                if let Some(val) = map.borrow().get(property) {
                    let val = match val.clone() {
                        Value::WeakClass(w) => w.upgrade().map(Value::Class).unwrap_or(Value::Undefined),
                        other => other,
                    };
                    return Ok(val);
                }
                let effective_class = Self::resolve_class_for_object(map, &self.env);
                if (property == "конструктор" || property == "constructor")
                    && let Some(cls) = &effective_class
                {
                    return Ok(Value::Class(std::rc::Rc::clone(cls)));
                }
                if let Some(cls) = &effective_class {
                    if let Some(MethodDef { params, body, env }) = Self::find_getter_in_class(cls, property) {
                        let params = params.clone();
                        let body = Rc::clone(body);
                        let env = Rc::clone(env);
                        let super_class = Self::find_getter_owner_parent(cls, property);
                        return self.call_method_with_this_super(
                            Rc::from(property),
                            &params,
                            &body,
                            &env,
                            vec![],
                            Some(obj),
                            super_class,
                            span,
                        );
                    }
                    if let Some(MethodDef { params, body, env }) = Self::find_method_in_class(cls, property) {
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
                let proto = map.borrow().get(symbols::PROTO).cloned();
                if let Some(proto) = proto {
                    match proto {
                        Value::Class(_) | Value::WeakClass(_) | Value::Null => return Ok(Value::Undefined),
                        _ => return self.eval_member(proto, property, span),
                    }
                }
                Ok(Value::Undefined)
            }
            Value::Class(cls) => {
                if property == "прототип" || property == "prototype" {
                    return Ok(Self::class_prototype_object(cls));
                }
                if let Some(MethodDef { params, body, env }) = Self::find_static_getter_in_class(cls, property) {
                    return self.call_method_with_this(Rc::from(property), params, body, env, vec![], None, span);
                }
                if let Some(val) = Self::find_static_field_in_class(cls, property) {
                    return Ok(val);
                }
                if let Some(MethodDef { params, body, env }) = Self::find_static_method_in_class(cls, property) {
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
            Value::Date(_) => Ok(Value::Undefined),
            Value::TypedArray { buffer, offset, length, kind } => match property {
                "length" | "длина" => Ok(Value::Number(*length as f64)),
                "byteLength" | "длинаБайт" => Ok(Value::Number((length * kind.element_size()) as f64)),
                "byteOffset" | "смещениеБайт" => Ok(Value::Number(*offset as f64)),
                "buffer" | "область" => Ok(Value::ArrayBuffer(std::rc::Rc::clone(buffer))),
                _ => Ok(Value::Undefined),
            },
            Value::ArrayBuffer(buffer) => match property {
                "byteLength" | "длинаБайт" => Ok(Value::Number(buffer.borrow().len() as f64)),
                _ => Ok(Value::Undefined),
            },
            Value::DataView { buffer, offset, length } => match property {
                "byteLength" | "длинаБайт" => Ok(Value::Number(*length as f64)),
                "byteOffset" | "смещениеБайт" => Ok(Value::Number(*offset as f64)),
                "buffer" | "область" => Ok(Value::ArrayBuffer(std::rc::Rc::clone(buffer))),
                _ => Ok(Value::Undefined),
            },
            _ => Err(RuntimeError::new(format!("Нельзя получить свойство у типа '{}'", obj.type_name()), span)),
        }
    }

    pub(crate) fn resolve_class_for_object(
        map: &Rc<std::cell::RefCell<crate::value::ObjectStore>>,
        env: &crate::environment::Environment,
    ) -> Option<std::rc::Rc<crate::value::ClassDef>> {
        let proto = map.borrow().get(symbols::PROTO).cloned();
        match proto {
            Some(Value::Class(cls)) => Some(cls),
            Some(Value::WeakClass(w)) => w.upgrade(),
            Some(_) => None,
            None => {
                let class_name = match map.borrow().get(symbols::CLASS_TAG) {
                    Some(Value::String(cn)) => Some(cn.clone()),
                    _ => None,
                };
                if let Some(class_name) = class_name
                    && let Some(Value::Class(cls)) = env.get(&class_name)
                {
                    return Some(cls);
                }
                None
            }
        }
    }

    pub(crate) fn class_prototype_object(cls: &Rc<ClassDef>) -> Value {
        cls.prototype_cache
            .get_or_init(|| {
                let mut map = IndexMap::new();
                let mut current: Option<&ClassDef> = Some(cls);
                while let Some(c) = current {
                    for (name, MethodDef { params, body, env }) in &c.methods {
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
                map.insert("конструктор".to_string(), Value::WeakClass(Rc::downgrade(cls)));
                map.insert(symbols::PROTO.to_string(), Value::WeakClass(Rc::downgrade(cls)));
                Value::object(map)
            })
            .clone()
    }
}
