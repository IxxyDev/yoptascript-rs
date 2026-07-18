use std::cell::Cell;
use std::cell::RefCell;
use std::rc::Rc;
use std::rc::Weak;

use yps_interpreter::value::Value as IValue;
use yps_lexer::Span;

use crate::error::VmError;
use crate::value::{ObjMap, Value};
use crate::vm::Vm;

pub const NAMESPACES: &[&str] = &["Матан", "Кент", "Жсон", "Хуйня", "Помойка", "Отражение", "Строка"];

pub const ERROR_CTOR: &str = "Косяк";

const PURE_NAMESPACE_GLOBALS: &[&str] = &["Итератор", "ФС", "Процесс", "Сеть"];

const HOST_CONSTRUCTORS: &[&str] = &[
    "Карта",
    "Набор",
    "СлабаяКарта",
    "СлабыйНабор",
    "СлабаяСсылка",
    "РеестрФинализации",
    "Симбол",
    "Дата",
    "Посредник",
    "ОбластьБайтов",
    "ОбзорБайтов",
    "Ц8Массив",
    "Ц8ОграниченныйМассив",
    "Ч8Массив",
    "Ц16Массив",
    "Ч16Массив",
    "Ц32Массив",
    "Ч32Массив",
    "Др32Массив",
    "Др64Массив",
    "КонтроллёрОтмены",
    "СигналОтмены",
];

pub(crate) const IDENTITY_CACHE_PRUNE_THRESHOLD: usize = 1024;

enum CacheKeepalive {
    Array(Weak<RefCell<Vec<Value>>>),
    Object(Weak<RefCell<ObjMap>>),
}

impl CacheKeepalive {
    fn is_alive(&self) -> bool {
        match self {
            CacheKeepalive::Array(w) => w.strong_count() > 0,
            CacheKeepalive::Object(w) => w.strong_count() > 0,
        }
    }
}

thread_local! {
    static ACTIVE_VM: Cell<*mut Vm> = const { Cell::new(std::ptr::null_mut()) };
    static IDENTITY_CACHE: RefCell<std::collections::HashMap<usize, (CacheKeepalive, IValue)>> =
        RefCell::new(std::collections::HashMap::new());
    static IDENTITY_CACHE_PRUNE_AT: Cell<usize> = const { Cell::new(IDENTITY_CACHE_PRUNE_THRESHOLD) };
}

fn cached_identity(ptr: usize) -> Option<IValue> {
    IDENTITY_CACHE.with(|c| {
        let mut cache = c.borrow_mut();
        match cache.get(&ptr) {
            Some((keepalive, iv)) if keepalive.is_alive() => Some(iv.clone()),
            Some(_) => {
                cache.remove(&ptr);
                None
            }
            None => None,
        }
    })
}

fn store_identity(ptr: usize, keepalive: CacheKeepalive, value: IValue) {
    IDENTITY_CACHE.with(|c| {
        let mut cache = c.borrow_mut();
        let prune_at = IDENTITY_CACHE_PRUNE_AT.with(|t| t.get());
        if cache.len() >= prune_at {
            cache.retain(|_, (k, _)| k.is_alive());
            let live = cache.len();
            let new_threshold = std::cmp::max(IDENTITY_CACHE_PRUNE_THRESHOLD, 2 * live);
            IDENTITY_CACHE_PRUNE_AT.with(|t| t.set(new_threshold));
        }
        cache.insert(ptr, (keepalive, value));
    });
}

#[cfg(test)]
pub(crate) fn identity_cache_len() -> usize {
    IDENTITY_CACHE.with(|c| c.borrow().len())
}

#[cfg(test)]
pub(crate) fn identity_cache_prune_at() -> usize {
    IDENTITY_CACHE_PRUNE_AT.with(|t| t.get())
}

struct VmGuard {
    prev_vm: *mut Vm,
}

impl VmGuard {
    fn enter(vm: &mut Vm) -> Self {
        let prev_vm = ACTIVE_VM.with(|c| c.replace(vm as *mut Vm));
        VmGuard { prev_vm }
    }
}

impl Drop for VmGuard {
    fn drop(&mut self) {
        ACTIVE_VM.with(|c| c.set(self.prev_vm));
    }
}

fn with_interp<R>(
    vm: &mut Vm,
    f: impl FnOnce(&mut yps_interpreter::Interpreter) -> Result<R, VmError>,
) -> Result<R, VmError> {
    let _guard = VmGuard::enter(vm);
    let mut interp = yps_interpreter::Interpreter::new();
    f(&mut interp)
}

#[must_use]
pub fn namespace_value(name: &str) -> Option<Value> {
    let obj = match name {
        "Матан" => yps_interpreter::stdlib::math::build_object(),
        "Кент" => yps_interpreter::stdlib::object::build_object(),
        "Жсон" => yps_interpreter::stdlib::json::build_object(),
        "Хуйня" => yps_interpreter::stdlib::number::build_object(),
        "Помойка" => yps_interpreter::stdlib::array::build_object(),
        "Отражение" => yps_interpreter::stdlib::reflect::build_object(),
        "Строка" => yps_interpreter::stdlib::string_ns::build_object(),
        "Итератор" => yps_interpreter::stdlib::iterator::build_object(),
        "ФС" => yps_interpreter::stdlib::fs::build_object(),
        "Процесс" => yps_interpreter::stdlib::process::build_object(),
        "Сеть" => yps_interpreter::stdlib::network::build_object(),
        ERROR_CTOR => return Some(Value::Builtin(Rc::from(ERROR_CTOR))),
        _ if HOST_CONSTRUCTORS.contains(&name) => return Some(Value::Builtin(Rc::from(name))),
        _ => return None,
    };
    interp_to_vm(&obj).ok()
}

#[must_use]
pub fn is_bridged_call(name: &str) -> bool {
    if name == ERROR_CTOR || HOST_CONSTRUCTORS.contains(&name) {
        return true;
    }
    if let Some((ns, _)) = name.split_once('.') {
        return NAMESPACES.contains(&ns)
            || ns == ERROR_CTOR
            || PURE_NAMESPACE_GLOBALS.contains(&ns)
            || HOST_CONSTRUCTORS.contains(&ns);
    }
    false
}

pub fn call_bridged(vm: &mut Vm, name: &str, args: Vec<Value>, span: Span) -> Result<Value, VmError> {
    with_interp(vm, |interp| {
        let interp_args: Vec<IValue> = args.iter().map(|a| vm_to_interp(a, span)).collect::<Result<_, _>>()?;
        let value = match yps_interpreter::stdlib::call_static_namespaced(interp, name, interp_args.clone(), span) {
            Some(res) => res.map_err(map_err)?,
            None => yps_interpreter::builtins::call_builtin(name, interp_args, span).map_err(map_err)?,
        };
        interp_to_vm(&value).map_err(|m| VmError::new(m, span))
    })
}

pub fn call_host_method(
    vm: &mut Vm,
    receiver: &IValue,
    method: &str,
    args: Vec<Value>,
    span: Span,
) -> Result<Value, VmError> {
    with_interp(vm, |interp| {
        let interp_args: Vec<IValue> = args.iter().map(|a| vm_to_interp(a, span)).collect::<Result<_, _>>()?;
        let (value, _) = yps_interpreter::stdlib::call_method(interp, receiver.clone(), method, interp_args, span)
            .map_err(map_err)?;
        interp_to_vm(&value).map_err(|m| VmError::new(m, span))
    })
}

pub fn host_member_get(vm: &mut Vm, receiver: &IValue, prop: &str, span: Span) -> Result<Value, VmError> {
    with_interp(vm, |interp| {
        let value = interp.host_member_get(receiver.clone(), prop, span).map_err(map_err)?;
        interp_to_vm(&value).map_err(|m| VmError::new(m, span))
    })
}

pub fn host_index_get(vm: &mut Vm, receiver: &IValue, index: &Value, span: Span) -> Result<Value, VmError> {
    with_interp(vm, |interp| {
        let idx = vm_to_interp(index, span)?;
        let value = interp.host_index_get(receiver.clone(), idx, span).map_err(map_err)?;
        interp_to_vm(&value).map_err(|m| VmError::new(m, span))
    })
}

pub fn host_index_set(vm: &mut Vm, receiver: &IValue, index: &Value, value: &Value, span: Span) -> Result<(), VmError> {
    with_interp(vm, |interp| {
        let idx = vm_to_interp(index, span)?;
        let val = vm_to_interp(value, span)?;
        interp.host_index_set(receiver, &idx, val, span).map_err(map_err)
    })
}

pub fn host_member_set(vm: &mut Vm, receiver: &IValue, prop: &str, value: &Value, span: Span) -> Result<(), VmError> {
    with_interp(vm, |interp| {
        let val = vm_to_interp(value, span)?;
        interp.host_member_set(receiver, prop, val, span).map_err(map_err)
    })
}

pub fn host_iterate(vm: &mut Vm, receiver: &IValue, span: Span) -> Result<Vec<Value>, VmError> {
    with_interp(vm, |interp| {
        let values = interp.host_iterate(receiver, span).map_err(map_err)?;
        values.iter().map(|v| interp_to_vm(v).map_err(|m| VmError::new(m, span))).collect()
    })
}

#[must_use]
pub fn is_host_callback(name: &str) -> bool {
    yps_interpreter::host_callback::is_host_callback(name)
}

pub fn call_host_callback(vm: &mut Vm, name: &str, args: Vec<Value>, span: Span) -> Result<Value, VmError> {
    let _guard = VmGuard::enter(vm);
    let interp_args: Vec<IValue> = args.iter().map(|a| vm_to_interp(a, span)).collect::<Result<_, _>>()?;
    match yps_interpreter::host_callback::invoke(name, interp_args, span) {
        Some(res) => {
            let value = res.map_err(map_err)?;
            interp_to_vm(&value).map_err(|m| VmError::new(m, span))
        }
        None => Err(VmError::new(format!("хост-замыкание '{name}' не найдено"), span)),
    }
}

pub fn host_call(vm: &mut Vm, callee: &IValue, args: &[Value], span: Span) -> Result<Value, VmError> {
    with_interp(vm, |interp| {
        let interp_args: Vec<IValue> = args.iter().map(|a| vm_to_interp(a, span)).collect::<Result<_, _>>()?;
        let value = interp.host_call(callee.clone(), interp_args, span).map_err(map_err)?;
        interp_to_vm(&value).map_err(|m| VmError::new(m, span))
    })
}

pub fn host_in(vm: &mut Vm, key: &Value, container: &IValue, span: Span) -> Result<bool, VmError> {
    with_interp(vm, |interp| {
        let key = vm_to_interp(key, span)?;
        interp.host_in(&key, container, span).map_err(map_err)
    })
}

pub fn host_for_in_keys(vm: &mut Vm, receiver: &IValue, span: Span) -> Result<Vec<Value>, VmError> {
    with_interp(vm, |interp| {
        let values = interp.host_for_in_keys(receiver, span).map_err(map_err)?;
        values.iter().map(|v| interp_to_vm(v).map_err(|m| VmError::new(m, span))).collect()
    })
}

fn map_err(e: yps_interpreter::RuntimeError) -> VmError {
    if let Some(thrown) = &e.thrown {
        let vm_thrown = interp_to_vm(thrown).unwrap_or_else(|_| Value::Str(Rc::from(e.message.as_str())));
        return VmError::new(e.message, e.span).with_thrown(vm_thrown);
    }
    VmError::new(e.message, e.span)
}

pub fn vm_to_interp(value: &Value, span: Span) -> Result<IValue, VmError> {
    match value {
        Value::Number(n) => Ok(IValue::Number(*n)),
        Value::BigInt(n) => Ok(IValue::BigInt(*n)),
        Value::Str(s) => Ok(IValue::String(s.to_string())),
        Value::Bool(b) => Ok(IValue::Boolean(*b)),
        Value::Null => Ok(IValue::Null),
        Value::Undefined => Ok(IValue::Undefined),
        Value::Host(iv) => Ok(iv.clone()),
        Value::Array(items) => {
            let converted: Vec<IValue> =
                items.borrow().iter().map(|v| vm_to_interp(v, span)).collect::<Result<_, _>>()?;
            let ptr = Rc::as_ptr(items) as usize;
            if let Some(IValue::Array(store)) = cached_identity(ptr) {
                store.borrow_mut().0 = converted;
                return Ok(IValue::Array(store));
            }
            let value = IValue::array(converted);
            store_identity(ptr, CacheKeepalive::Array(Rc::downgrade(items)), value.clone());
            Ok(value)
        }
        Value::Object(map) => {
            let mut out = indexmap::IndexMap::new();
            for (k, v) in map.borrow().iter() {
                if crate::value::is_internal_key(k) {
                    continue;
                }
                out.insert(k.clone(), vm_to_interp(v, span)?);
            }
            let ptr = Rc::as_ptr(map) as usize;
            if let Some(IValue::Object(store)) = cached_identity(ptr) {
                store.borrow_mut().map = out;
                return Ok(IValue::Object(store));
            }
            let value = IValue::object(out);
            store_identity(ptr, CacheKeepalive::Object(Rc::downgrade(map)), value.clone());
            Ok(value)
        }
        Value::Function(_)
        | Value::Builtin(_)
        | Value::Class(_)
        | Value::PromiseCapability { .. }
        | Value::PromiseThenHandler { .. }
        | Value::PromiseFinallyHandler { .. }
        | Value::PromiseAggregateHandler { .. } => Ok(wrap_vm_callback(value.clone())),
        other => Err(VmError::new(
            format!("значение типа '{}' пока нельзя передать в stdlib интерпретатора", other.type_name()),
            span,
        )),
    }
}

fn wrap_vm_callback(callee: Value) -> IValue {
    let cell = RefCell::new(callee);
    let marker = yps_interpreter::host_callback::register(Box::new(move |iargs, span| {
        let vm_ptr = ACTIVE_VM.with(Cell::get);
        if vm_ptr.is_null() {
            return Err(yps_interpreter::RuntimeError::new(
                "вызов VM-замыкания вне активного контекста VM".to_string(),
                span,
            ));
        }
        let vm: &mut Vm = unsafe { &mut *vm_ptr };
        let mut vm_args: Vec<Value> = Vec::with_capacity(iargs.len());
        for a in &iargs {
            match interp_to_vm_arg(a) {
                Ok(v) => vm_args.push(v),
                Err(m) => return Err(yps_interpreter::RuntimeError::new(m, span)),
            }
        }
        let callee = cell.borrow().clone();
        match vm.call_value(callee, None, &vm_args, span) {
            Ok(result) => vm_to_interp(&result, span).map_err(|e| yps_interpreter::RuntimeError::new(e.message, span)),
            Err(e) => match &e.thrown {
                Some(t) => {
                    let it = vm_to_interp(t, span).unwrap_or(IValue::String(e.message.clone()));
                    Err(yps_interpreter::RuntimeError::thrown(it, span))
                }
                None => Err(yps_interpreter::RuntimeError::new(e.message, span)),
            },
        }
    }));
    IValue::BuiltinFunction(marker)
}

pub fn interp_to_vm(value: &IValue) -> Result<Value, String> {
    match value {
        IValue::Number(n) => Ok(Value::Number(*n)),
        IValue::BigInt(n) => Ok(Value::BigInt(*n)),
        IValue::String(s) => Ok(Value::Str(Rc::from(s.as_str()))),
        IValue::Boolean(b) => Ok(Value::Bool(*b)),
        IValue::Null => Ok(Value::Null),
        IValue::Undefined => Ok(Value::Undefined),
        IValue::BuiltinFunction(name) => Ok(Value::Builtin(Rc::from(name.as_str()))),
        IValue::Array(items) => {
            let converted: Vec<Value> = items.borrow().0.iter().map(interp_to_vm).collect::<Result<_, _>>()?;
            Ok(Value::Array(Rc::new(RefCell::new(converted))))
        }
        IValue::Object(obj) => {
            let mut map = ObjMap::new();
            for (k, v) in &obj.borrow().map {
                map.insert(k.clone(), interp_to_vm(v)?);
            }
            Ok(Value::Object(Rc::new(RefCell::new(map))))
        }
        IValue::Map(_)
        | IValue::Set(_)
        | IValue::Date(_)
        | IValue::Symbol { .. }
        | IValue::Proxy { .. }
        | IValue::ArrayBuffer(_)
        | IValue::TypedArray { .. }
        | IValue::DataView { .. }
        | IValue::WeakMap(_)
        | IValue::WeakSet(_)
        | IValue::WeakRef(_)
        | IValue::FinalizationRegistry(_)
        | IValue::AbortController { .. }
        | IValue::AbortSignal { .. }
        | IValue::Iterator(_) => Ok(Value::Host(value.clone())),
        other => Err(format!("значение типа '{}' из stdlib пока нельзя вернуть в VM", other.type_name())),
    }
}

fn interp_to_vm_arg(value: &IValue) -> Result<Value, String> {
    match value {
        IValue::Object(_) | IValue::Array(_) => Ok(Value::Host(value.clone())),
        _ => interp_to_vm(value),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cached_identity_ignores_stale_dead_entry() {
        let ptr = 0x1234_usize;
        let items = Rc::new(RefCell::new(Vec::<Value>::new()));
        let weak = Rc::downgrade(&items);
        drop(items);
        assert_eq!(weak.strong_count(), 0);
        let stale_value = IValue::array(vec![IValue::Number(1.0)]);
        IDENTITY_CACHE.with(|c| {
            c.borrow_mut().insert(ptr, (CacheKeepalive::Array(weak), stale_value));
        });
        assert!(cached_identity(ptr).is_none());
        assert!(!IDENTITY_CACHE.with(|c| c.borrow().contains_key(&ptr)));
    }
}
