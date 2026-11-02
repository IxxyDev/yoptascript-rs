use std::rc::Rc;

use indexmap::IndexMap;

use yps_lexer::Span;

use crate::error::RuntimeError;
use crate::symbols;
use crate::value::{ClassDef, FunctionData, MethodDef, Value};

use super::Interpreter;

pub(super) const MAX_PROTO_CHAIN: usize = 1_000_000;

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
                    return Ok(Value::BoundMethod { receiver: Box::new(obj.clone()), method: Rc::from(property) });
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
                    return Ok(Value::BoundMethod { receiver: Box::new(obj.clone()), method: Rc::from(property) });
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
            Value::RegExp(_) => {
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
                    Some(Value::Function(func)) => {
                        Some((func.params.clone(), Rc::clone(&func.body), Rc::clone(&func.env)))
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
                    if !symbols::is_internal_key(property)
                        && let Some(member) = Self::prototype_member(cls, property)
                    {
                        return Ok(member);
                    }
                }
                let proto = map.borrow().get(symbols::PROTO).cloned();
                if let Some(proto) = proto {
                    match proto {
                        Value::Class(_) | Value::WeakClass(_) | Value::Null => return Ok(Value::Undefined),
                        _ => {
                            if let Some((MethodDef { params, body, env }, super_class)) =
                                Self::inherited_getter(&proto, property, &self.env)
                            {
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
                            return stacker::maybe_grow(super::STACK_RED_ZONE, super::STACK_GROW_SIZE, || {
                                self.eval_member(proto, property, span)
                            });
                        }
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
                    return Ok(Value::Function(Rc::new(FunctionData {
                        name: Rc::from(property),
                        params: params.clone(),
                        body: Rc::clone(body),
                        env: Rc::clone(env),
                        is_generator: false,
                        is_async: false,
                    })));
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
            Value::TypedArray(ta) => match property {
                "length" | "длина" => Ok(Value::Number(ta.length as f64)),
                "byteLength" | "длинаБайт" => Ok(Value::Number((ta.length * ta.kind.element_size()) as f64)),
                "byteOffset" | "смещениеБайт" => Ok(Value::Number(ta.offset as f64)),
                "buffer" | "область" => Ok(Value::ArrayBuffer(std::rc::Rc::clone(&ta.buffer))),
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

    fn inherited_getter(
        proto: &Value,
        property: &str,
        env: &crate::environment::Environment,
    ) -> Option<(MethodDef, Option<Rc<crate::value::ClassDef>>)> {
        let getter_key = symbols::getter_key(property);
        let mut current = proto.clone();
        for _ in 0..MAX_PROTO_CHAIN {
            let Value::Object(map) = &current else {
                return None;
            };
            if let Some(Value::Function(func)) = map.borrow().get(&getter_key) {
                let getter =
                    MethodDef { params: func.params.clone(), body: Rc::clone(&func.body), env: Rc::clone(&func.env) };
                return Some((getter, None));
            }
            if map.borrow().contains_key(property) {
                return None;
            }
            if let Some(cls) = Self::resolve_class_for_object(map, env)
                && let Some(MethodDef { params, body, env }) = Self::find_getter_in_class(&cls, property)
            {
                let getter = MethodDef { params: params.clone(), body: Rc::clone(body), env: Rc::clone(env) };
                return Some((getter, Self::find_getter_owner_parent(&cls, property)));
            }
            let next = map.borrow().get(symbols::PROTO).cloned();
            current = next.unwrap_or(Value::Undefined);
        }
        None
    }

    pub(crate) fn class_along_chain(
        map: &Rc<std::cell::RefCell<crate::value::ObjectStore>>,
        env: &crate::environment::Environment,
    ) -> Option<Rc<crate::value::ClassDef>> {
        let mut current = Rc::clone(map);
        for _ in 0..MAX_PROTO_CHAIN {
            if let Some(cls) = Self::resolve_class_for_object(&current, env) {
                return Some(cls);
            }
            let next = current.borrow().get(symbols::PROTO).cloned();
            let Some(Value::Object(next)) = next else {
                return None;
            };
            current = next;
        }
        None
    }

    pub(crate) fn object_has_property(
        map: &Rc<std::cell::RefCell<crate::value::ObjectStore>>,
        key: &str,
        env: &crate::environment::Environment,
    ) -> bool {
        let getter_key = symbols::getter_key(key);
        let setter_key = symbols::setter_key(key);
        let mut current = Rc::clone(map);
        for _ in 0..MAX_PROTO_CHAIN {
            {
                let guard = current.borrow();
                if guard.contains_key(key) || guard.contains_key(&getter_key) || guard.contains_key(&setter_key) {
                    return true;
                }
            }
            if let Some(cls) = Self::resolve_class_for_object(&current, env)
                && (Self::find_method_in_class(&cls, key).is_some()
                    || Self::find_getter_in_class(&cls, key).is_some()
                    || Self::find_setter_in_class(&cls, key).is_some())
            {
                return true;
            }
            let next = current.borrow().get(symbols::PROTO).cloned();
            let Some(Value::Object(next)) = next else {
                return false;
            };
            current = next;
        }
        false
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
                for (name, MethodDef { params, body, env }) in &cls.methods {
                    map.insert(
                        name.clone(),
                        Value::Function(Rc::new(FunctionData {
                            name: Rc::from(name.as_str()),
                            params: params.clone(),
                            body: Rc::clone(body),
                            env: Rc::clone(env),
                            is_generator: false,
                            is_async: false,
                        })),
                    );
                }
                map.insert("конструктор".to_string(), Value::WeakClass(Rc::downgrade(cls)));
                map.insert(symbols::PROTO.to_string(), Value::WeakClass(Rc::downgrade(cls)));
                Value::object(map)
            })
            .clone()
    }

    pub(crate) fn visible_prototype(object: &Value, proto: Option<Value>) -> Value {
        let cls = match proto {
            Some(Value::Class(cls)) => cls,
            Some(Value::WeakClass(w)) => match w.upgrade() {
                Some(cls) => cls,
                None => return Value::Null,
            },
            Some(other) => return other,
            None => return Value::Null,
        };
        if cls.prototype_cache.get() == Some(object) {
            return match &cls.parent {
                Some(parent) => Self::class_prototype_object(parent),
                None => Value::Null,
            };
        }
        Self::class_prototype_object(&cls)
    }

    fn prototype_member(cls: &Rc<ClassDef>, property: &str) -> Option<Value> {
        let mut current = Some(cls);
        while let Some(class) = current {
            let Value::Object(proto) = Self::class_prototype_object(class) else {
                return None;
            };
            if let Some(value) = proto.borrow().get(property) {
                return Some(value.clone());
            }
            current = class.parent.as_ref();
        }
        None
    }
}
