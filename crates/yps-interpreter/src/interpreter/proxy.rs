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
            return self.call_function(trap, vec![target.clone(), Value::String(key.to_string().into()), proxy], span);
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
            let result = self.call_function(
                trap,
                vec![target.clone(), Value::String(key.to_string().into()), value, proxy],
                span,
            )?;
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
                if map.borrow().can_write_key(key) {
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
            let result = self.call_function(trap, vec![target.clone(), Value::String(key.to_string().into())], span)?;
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
            let result = self.call_function(trap, vec![target.clone(), Value::String(key.to_string().into())], span)?;
            return Ok(result.is_truthy());
        }
        if let Value::Object(map) = target
            && map.borrow().can_delete()
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

    pub(crate) fn proxy_own_keys(
        &mut self,
        target: &Value,
        handler: &Value,
        span: Span,
    ) -> Result<Vec<Value>, RuntimeError> {
        if let Some(trap) = proxy::trap(handler, proxy::OWN_KEYS) {
            let result = self.call_function(trap, vec![target.clone()], span)?;
            return match result {
                Value::Array(arr) => Ok(arr.borrow().0.iter().map(|k| Value::String(k.to_string().into())).collect()),
                other => Err(RuntimeError::new(
                    format!(
                        "Ловушка 'собственныеКлючи' посредника должна вернуть массив, получено '{}'",
                        other.type_name()
                    ),
                    span,
                )),
            };
        }
        Ok(default_own_keys(target))
    }

    pub(crate) fn proxy_get_prototype_of(
        &mut self,
        target: &Value,
        handler: &Value,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if let Some(trap) = proxy::trap(handler, proxy::GET_PROTOTYPE_OF) {
            return self.call_function(trap, vec![target.clone()], span);
        }
        Ok(default_prototype_of(target))
    }

    pub(crate) fn proxy_set_prototype_of(
        &mut self,
        target: &Value,
        handler: &Value,
        proto: Value,
        span: Span,
    ) -> Result<bool, RuntimeError> {
        if let Some(trap) = proxy::trap(handler, proxy::SET_PROTOTYPE_OF) {
            let result = self.call_function(trap, vec![target.clone(), proto], span)?;
            return Ok(result.is_truthy());
        }
        Ok(default_set_prototype_of(target, proto))
    }

    pub(crate) fn proxy_define_property(
        &mut self,
        target: &Value,
        handler: &Value,
        key: &str,
        descriptor: Value,
        span: Span,
    ) -> Result<bool, RuntimeError> {
        if let Some(trap) = proxy::trap(handler, proxy::DEFINE_PROPERTY) {
            let result = self.call_function(
                trap,
                vec![target.clone(), Value::String(key.to_string().into()), descriptor],
                span,
            )?;
            return Ok(result.is_truthy());
        }
        if let Value::Object(map) = target {
            crate::stdlib::object::define_property_impl(map, key, &descriptor, "определитьСвойство", span)?;
            return Ok(true);
        }
        Ok(false)
    }

    pub(crate) fn proxy_get_own_property_descriptor(
        &mut self,
        target: &Value,
        handler: &Value,
        key: &str,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if let Some(trap) = proxy::trap(handler, proxy::GET_OWN_PROPERTY_DESCRIPTOR) {
            return self.call_function(trap, vec![target.clone(), Value::String(key.to_string().into())], span);
        }
        if let Value::Object(map) = target {
            return Ok(crate::stdlib::object::describe_property_impl(&map.borrow(), key).unwrap_or(Value::Undefined));
        }
        Ok(Value::Undefined)
    }

    pub(crate) fn proxy_is_extensible(
        &mut self,
        target: &Value,
        handler: &Value,
        span: Span,
    ) -> Result<bool, RuntimeError> {
        if let Some(trap) = proxy::trap(handler, proxy::IS_EXTENSIBLE) {
            let result = self.call_function(trap, vec![target.clone()], span)?;
            return Ok(result.is_truthy());
        }
        Ok(match target {
            Value::Object(map) => map.borrow().extensible,
            _ => false,
        })
    }

    pub(crate) fn proxy_prevent_extensions(
        &mut self,
        target: &Value,
        handler: &Value,
        span: Span,
    ) -> Result<bool, RuntimeError> {
        if let Some(trap) = proxy::trap(handler, proxy::PREVENT_EXTENSIONS) {
            let result = self.call_function(trap, vec![target.clone()], span)?;
            return Ok(result.is_truthy());
        }
        if let Value::Object(map) = target {
            map.borrow_mut().prevent_extensions();
        }
        Ok(true)
    }
}

fn default_own_keys(target: &Value) -> Vec<Value> {
    match target {
        Value::Object(map) => map
            .borrow()
            .keys()
            .filter(|k| !crate::symbols::is_internal_key(k) && !k.starts_with('#'))
            .map(|k| Value::String(k.clone().into()))
            .collect(),
        Value::Array(arr) => {
            let mut keys: Vec<Value> = (0..arr.borrow().len()).map(|i| Value::String(i.to_string().into())).collect();
            keys.push(Value::String("length".into()));
            keys
        }
        _ => Vec::new(),
    }
}

fn default_prototype_of(target: &Value) -> Value {
    match target {
        Value::Object(map) => map.borrow().get(crate::symbols::PROTO).cloned().unwrap_or(Value::Null),
        _ => Value::Null,
    }
}

fn default_set_prototype_of(target: &Value, proto: Value) -> bool {
    let Value::Object(map) = target else {
        return false;
    };
    let current = map.borrow().get(crate::symbols::PROTO).cloned().unwrap_or(Value::Null);
    let unchanged = crate::value::same_value(&current, &proto);
    let allowed = {
        let guard = map.borrow();
        !guard.frozen && (unchanged || guard.extensible)
    };
    if allowed {
        map.borrow_mut().insert(crate::symbols::PROTO.to_string(), proto);
    }
    allowed
}
