use std::cell::Cell;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::rc::Weak;

use yps_interpreter::value::Value as IValue;
use yps_lexer::Span;

use crate::abort::SignalState;
use crate::error::VmError;
use crate::value::{ClassDef, ObjMap, Value};
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
    Array(Weak<RefCell<crate::value::ArrayStore>>),
    Object(Weak<RefCell<ObjMap>>),
}

impl CacheKeepalive {
    fn is_alive(&self) -> bool {
        match self {
            CacheKeepalive::Array(w) => w.strong_count() > 0,
            CacheKeepalive::Object(w) => w.strong_count() > 0,
        }
    }

    fn upgrade(&self) -> Option<Value> {
        match self {
            CacheKeepalive::Array(w) => w.upgrade().map(Value::Array),
            CacheKeepalive::Object(w) => w.upgrade().map(Value::Object),
        }
    }
}

enum Snapshot {
    Array(Vec<IValue>),
    Object { map: indexmap::IndexMap<String, IValue>, frozen: bool, sealed: bool, extensible: bool },
}

struct Crossed {
    vm_value: Value,
    store: IValue,
    snapshot: Snapshot,
}

thread_local! {
    static ACTIVE_VM: Cell<*mut Vm> = const { Cell::new(std::ptr::null_mut()) };
    static IDENTITY_CACHE: RefCell<std::collections::HashMap<usize, (CacheKeepalive, IValue)>> =
        RefCell::new(std::collections::HashMap::new());
    static IDENTITY_CACHE_PRUNE_AT: Cell<usize> = const { Cell::new(IDENTITY_CACHE_PRUNE_THRESHOLD) };
    static VM_BY_STORE: RefCell<HashMap<usize, CacheKeepalive>> = RefCell::new(HashMap::new());
    static CROSSED: RefCell<HashMap<usize, Crossed>> = RefCell::new(HashMap::new());
    static CROSSED_LOG: RefCell<Vec<usize>> = const { RefCell::new(Vec::new()) };
    static ANCHORS: RefCell<HashMap<usize, (IValue, Value)>> = RefCell::new(HashMap::new());
    static SYNCED_AT: RefCell<HashMap<usize, u64>> = RefCell::new(HashMap::new());
    static RECORDING: Cell<bool> = const { Cell::new(true) };
    static GENERATOR_STORES: RefCell<HashMap<usize, IValue>> = RefCell::new(HashMap::new());
    static OPAQUE_STORES: RefCell<Vec<IValue>> = const { RefCell::new(Vec::new()) };
    static GENERATOR_BY_STORE: RefCell<HashMap<usize, Value>> = RefCell::new(HashMap::new());
    static CROSSING_DEPTH: Cell<usize> = const { Cell::new(0) };
    static CALLBACK_BY_PTR: RefCell<HashMap<usize, String>> = RefCell::new(HashMap::new());
    static VM_BY_CALLBACK: RefCell<HashMap<String, Value>> = RefCell::new(HashMap::new());
}

const STACK_RED_ZONE: usize = 256 * 1024;
const PAIRED_STORE_HOLDERS: usize = 2;
const STACK_GROW_SIZE: usize = 8 * 1024 * 1024;

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

fn store_children(store: &IValue) -> Vec<IValue> {
    match store {
        IValue::Array(rc) => rc.try_borrow().map(|a| a.0.clone()).unwrap_or_default(),
        IValue::Object(rc) => rc.try_borrow().map(|o| o.map.values().cloned().collect()).unwrap_or_default(),
        _ => Vec::new(),
    }
}

fn stores_held_from_outside(cache: &HashMap<usize, (CacheKeepalive, IValue)>) -> HashSet<usize> {
    let anchors: HashSet<usize> = ANCHORS.with(|a| a.borrow().keys().copied().collect());
    let known: HashMap<usize, &IValue> =
        cache.values().filter_map(|(_, store)| store_ptr(store).map(|ptr| (ptr, store))).collect();
    let mut inner_edges: HashMap<usize, usize> = HashMap::new();
    for store in known.values() {
        for child in store_children(store) {
            if let Some(ptr) = store_ptr(&child)
                && known.contains_key(&ptr)
            {
                *inner_edges.entry(ptr).or_default() += 1;
            }
        }
    }
    let mut pending: Vec<usize> = known
        .iter()
        .filter(|(ptr, store)| {
            let bridge_holders = 1 + usize::from(anchors.contains(*ptr));
            let inner = inner_edges.get(*ptr).copied().unwrap_or(0);
            store_holders(store) > bridge_holders + inner
        })
        .map(|(ptr, _)| *ptr)
        .collect();
    let mut held: HashSet<usize> = HashSet::new();
    while let Some(ptr) = pending.pop() {
        if held.insert(ptr)
            && let Some(store) = known.get(&ptr)
        {
            pending.extend(store_children(store).iter().filter_map(store_ptr));
        }
    }
    held
}

#[cfg(test)]
pub(crate) fn anchor_count() -> usize {
    ANCHORS.with(|a| a.borrow().len())
}

fn store_holders(store: &IValue) -> usize {
    match store {
        IValue::Array(rc) => Rc::strong_count(rc),
        IValue::Object(rc) => Rc::strong_count(rc),
        _ => 0,
    }
}

fn store_ptr(store: &IValue) -> Option<usize> {
    match store {
        IValue::Array(rc) => Some(Rc::as_ptr(rc) as usize),
        IValue::Object(rc) => Some(Rc::as_ptr(rc) as usize),
        _ => None,
    }
}

fn vm_value_of_store(store: &IValue) -> Option<Value> {
    let ptr = store_ptr(store)?;
    VM_BY_STORE.with(|m| m.borrow().get(&ptr).and_then(CacheKeepalive::upgrade))
}

fn store_identity(ptr: usize, keepalive: CacheKeepalive, value: IValue) {
    if let Some(store) = store_ptr(&value) {
        let reverse = match &keepalive {
            CacheKeepalive::Array(w) => CacheKeepalive::Array(Weak::clone(w)),
            CacheKeepalive::Object(w) => CacheKeepalive::Object(Weak::clone(w)),
        };
        VM_BY_STORE.with(|m| m.borrow_mut().insert(store, reverse));
    }
    IDENTITY_CACHE.with(|c| {
        let mut cache = c.borrow_mut();
        let prune_at = IDENTITY_CACHE_PRUNE_AT.with(|t| t.get());
        if cache.len() >= prune_at {
            cache.retain(|_, (k, _)| k.is_alive());
            VM_BY_STORE.with(|m| m.borrow_mut().retain(|_, k| k.is_alive()));
            SYNCED_AT.with(|m| m.borrow_mut().retain(|ptr, _| cache.contains_key(ptr)));
            let held = stores_held_from_outside(&cache);
            ANCHORS.with(|a| a.borrow_mut().retain(|ptr, _| held.contains(ptr)));
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

pub(crate) fn with_interp<R>(
    vm: &mut Vm,
    f: impl FnOnce(&mut yps_interpreter::Interpreter) -> Result<R, VmError>,
) -> Result<R, VmError> {
    let _guard = VmGuard::enter(vm);
    let mark = CROSSED_LOG.with(|l| l.borrow().len());
    CROSSING_DEPTH.with(|d| d.set(d.get() + 1));
    let mut interp = vm.spare_interpreters.pop().unwrap_or_default();
    let result = f(&mut interp);
    if result.is_ok() {
        interp.reset_for_host_call();
        vm.spare_interpreters.push(interp);
    }
    let depth = CROSSING_DEPTH.with(|d| {
        d.set(d.get() - 1);
        d.get()
    });
    let touched = CROSSED_LOG.with(|l| l.borrow_mut().split_off(mark));
    for ptr in touched {
        let Some(mut entry) = CROSSED.with(|c| c.borrow_mut().remove(&ptr)) else {
            continue;
        };
        write_back(&mut entry);
        CROSSED.with(|c| c.borrow_mut().insert(ptr, entry));
    }
    if depth == 0 {
        anchor_held_values();
    }
    result
}

fn anchor_held_values() {
    let crossed = CROSSED.with(|c| std::mem::take(&mut *c.borrow_mut()));
    let pairs: Vec<(IValue, Value)> = crossed.into_values().map(|entry| (entry.store, entry.vm_value)).collect();
    for (store, vm_value) in pairs {
        if store_holders(&store) > PAIRED_STORE_HOLDERS
            && let Some(ptr) = store_ptr(&store)
        {
            ANCHORS.with(|a| {
                a.borrow_mut().entry(ptr).or_insert((store, vm_value));
            });
        }
    }
}

fn with_reader<R>(
    vm: &mut Vm,
    receiver: &IValue,
    span: Span,
    f: impl FnOnce(&mut yps_interpreter::Interpreter) -> Result<R, VmError>,
) -> Result<R, VmError> {
    with_interp(vm, |interp| {
        refresh_stale_receiver(receiver, span)?;
        f(interp)
    })
}

fn refresh_stale_receiver(receiver: &IValue, span: Span) -> Result<(), VmError> {
    match receiver {
        IValue::Proxy { target, handler } => {
            refresh_stale_receiver(target, span)?;
            refresh_stale_receiver(handler, span)
        }
        IValue::Array(_) | IValue::Object(_) => match vm_value_of_store(receiver) {
            Some(vm_value) if !is_synced(&vm_value) => vm_to_interp(&vm_value, span).map(|_| ()),
            _ => Ok(()),
        },
        _ => Ok(()),
    }
}

fn vm_ptr(value: &Value) -> Option<usize> {
    match value {
        Value::Array(rc) => Some(Rc::as_ptr(rc) as usize),
        Value::Object(rc) => Some(Rc::as_ptr(rc) as usize),
        _ => None,
    }
}

fn is_synced(value: &Value) -> bool {
    let epoch = crate::value::mutation_epoch();
    vm_ptr(value).is_some_and(|ptr| SYNCED_AT.with(|m| m.borrow().get(&ptr) == Some(&epoch)))
}

fn mark_synced(ptr: usize, epoch: u64) {
    SYNCED_AT.with(|m| m.borrow_mut().insert(ptr, epoch));
}

fn with_receiver<R>(
    vm: &mut Vm,
    receiver: &IValue,
    span: Span,
    f: impl FnOnce(&mut yps_interpreter::Interpreter) -> Result<R, VmError>,
) -> Result<R, VmError> {
    with_interp(vm, |interp| {
        refresh_receiver(receiver, span)?;
        f(interp)
    })
}

fn refresh_receiver(receiver: &IValue, span: Span) -> Result<(), VmError> {
    refresh_host_value(receiver, span, &mut HashSet::new())
}

fn refresh_host_value(value: &IValue, span: Span, seen: &mut HashSet<usize>) -> Result<(), VmError> {
    match value {
        IValue::Proxy { target, handler } => {
            refresh_host_value(target, span, seen)?;
            refresh_host_value(handler, span, seen)
        }
        IValue::Array(_) | IValue::Object(_) => match vm_value_of_store(value) {
            Some(vm_value) => convert_to_interp(&vm_value, span, seen).map(|_| ()),
            None => Ok(()),
        },
        _ => Ok(()),
    }
}

fn same_value(a: &IValue, b: &IValue) -> bool {
    match (a, b) {
        (IValue::Number(x), IValue::Number(y)) => x.to_bits() == y.to_bits(),
        _ => a == b,
    }
}

fn write_back(entry: &mut Crossed) {
    match (&entry.vm_value, &entry.store, &mut entry.snapshot) {
        (Value::Array(items), IValue::Array(store), Snapshot::Array(before)) => {
            let Ok(store) = store.try_borrow() else {
                return;
            };
            let unchanged =
                store.0.len() == before.len() && store.0.iter().zip(before.iter()).all(|(a, b)| same_value(a, b));
            if unchanged {
                return;
            }
            let mut items = items.borrow_mut();
            if store.0.len() != before.len() {
                items.truncate(store.0.len());
            }
            for (i, new) in store.0.iter().enumerate() {
                if before.get(i).is_some_and(|old| same_value(old, new)) && i < items.len() {
                    continue;
                }
                let Ok(converted) = interp_to_vm(new) else {
                    continue;
                };
                match items.get_mut(i) {
                    Some(slot) => *slot = converted,
                    None => items.push(converted),
                }
            }
            before.clone_from(&store.0);
        }
        (Value::Object(map), IValue::Object(store), Snapshot::Object { map: before, frozen, sealed, extensible }) => {
            let Ok(store) = store.try_borrow() else {
                return;
            };
            let unchanged = store.frozen == *frozen
                && store.sealed == *sealed
                && store.extensible == *extensible
                && store.map.len() == before.len()
                && store.map.iter().all(|(k, v)| before.get(k).is_some_and(|old| same_value(old, v)));
            if unchanged {
                return;
            }
            let mut map = map.borrow_mut();
            for key in before.keys() {
                if !store.map.contains_key(key) {
                    map.remove(key);
                }
            }
            for (key, new) in &store.map {
                if before.get(key).is_some_and(|old| same_value(old, new)) {
                    continue;
                }
                if let Ok(converted) = interp_to_vm(new) {
                    map.insert(key.clone(), converted);
                }
            }
            crate::value::note_mutation();
            if store.frozen != *frozen {
                map.frozen = store.frozen;
            }
            if store.sealed != *sealed {
                map.sealed = store.sealed;
            }
            if store.extensible != *extensible {
                map.extensible = store.extensible;
            }
            before.clone_from(&store.map);
            *frozen = store.frozen;
            *sealed = store.sealed;
            *extensible = store.extensible;
        }
        _ => {}
    }
}

pub(crate) fn mark_bridge_roots(marker: &mut crate::gc::Marker) {
    VM_BY_CALLBACK.with(|m| {
        for value in m.borrow().values() {
            marker.push_value(value);
        }
    });
    ANCHORS.with(|a| {
        for (_, value) in a.borrow().values() {
            marker.push_value(value);
        }
    });
    GENERATOR_BY_STORE.with(|m| {
        for value in m.borrow().values() {
            marker.push_value(value);
        }
    });
    CROSSED.with(|c| {
        for entry in c.borrow().values() {
            marker.push_value(&entry.vm_value);
        }
    });
}

fn active_vm(span: Span) -> Result<*mut Vm, yps_interpreter::RuntimeError> {
    let vm_ptr = ACTIVE_VM.with(Cell::get);
    if vm_ptr.is_null() {
        return Err(yps_interpreter::RuntimeError::new(
            "вызов VM-замыкания вне активного контекста VM".to_string(),
            span,
        ));
    }
    Ok(vm_ptr)
}

pub(crate) fn listen_for_abort(state: &SignalState, on_abort: impl Fn(&mut Vm, Span) + 'static) -> u64 {
    let marker = yps_interpreter::host_callback::register(Box::new(move |_, span| {
        let vm: &mut Vm = unsafe { &mut *active_vm(span)? };
        on_abort(vm, span);
        Ok(IValue::Undefined)
    }));
    let mut signal = state.borrow_mut();
    let token = signal.next_token;
    signal.next_token += 1;
    signal.listeners.push((token, IValue::BuiltinFunction(marker)));
    token
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
        "нихуя" => return Some(Value::Number(f64::NAN)),
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
    if let Some(res) = try_object_state_op(name, &args) {
        return res;
    }
    if let Some(res) = try_class_prototype_op(name, &args) {
        vm.class_prototypes_used = true;
        return res;
    }
    if name == "СигналОтмены.отВремени" {
        return crate::abort::timeout_signal(vm, &args, span);
    }
    with_interp(vm, |interp| {
        let interp_args = convert_args(name, &args, span)?;
        let value = match yps_interpreter::stdlib::call_static_namespaced(interp, name, interp_args.clone(), span) {
            Some(res) => res.map_err(map_err)?,
            None => yps_interpreter::builtins::call_builtin(name, interp_args, span).map_err(map_err)?,
        };
        interp_to_vm(&value).map_err(|m| VmError::new(m, span))
    })
}

fn prototype_chain_reaches(proto: &Value, target: &Rc<RefCell<crate::value::ObjMap>>) -> bool {
    let mut current = proto.clone();
    for _ in 0..crate::vm::MAX_PROTO_CHAIN {
        let Value::Object(map) = &current else {
            return false;
        };
        if Rc::ptr_eq(map, target) {
            return true;
        }
        let next = map.borrow().get(crate::value::PROTO_KEY).cloned();
        current = next.unwrap_or(Value::Null);
    }
    true
}

fn try_class_prototype_op(name: &str, args: &[Value]) -> Option<Result<Value, VmError>> {
    match (name, args) {
        ("Кент.создать", [Value::Class(_)]) => {
            let mut map = ObjMap::new();
            map.insert(crate::value::CLASS_TAG.to_string(), args[0].clone());
            Some(Ok(Value::Object(Rc::new(RefCell::new(map)))))
        }
        ("Кент.назначитьПрототип" | "Отражение.назначитьПрототип", [Value::Object(map), proto, ..]) =>
        {
            let tagged = map.borrow().get(crate::value::CLASS_TAG).is_some();
            if !tagged && !matches!(proto, Value::Class(_)) {
                return None;
            }
            if !matches!(proto, Value::Class(_) | Value::Object(_) | Value::Null) || prototype_chain_reaches(proto, map)
            {
                return None;
            }
            let succeeded = name == "Отражение.назначитьПрототип";
            let answer = || if succeeded { Value::Bool(true) } else { args[0].clone() };
            let mut target = map.borrow_mut();
            if target.frozen || !target.extensible {
                return None;
            }
            target.remove(crate::value::CLASS_TAG);
            target.remove(crate::value::PROTO_KEY);
            match proto {
                Value::Class(_) => target.insert(crate::value::CLASS_TAG.to_string(), proto.clone()),
                Value::Object(_) => target.insert(crate::value::PROTO_KEY.to_string(), proto.clone()),
                _ => {}
            }
            Some(Ok(answer()))
        }
        ("Кент.прототип" | "Отражение.прототипОт", [Value::Object(map), ..]) => {
            let class = match map.borrow().get(crate::value::CLASS_TAG) {
                Some(Value::Class(class)) => Rc::clone(class),
                _ => return None,
            };
            let is_own_prototype =
                matches!(class.prototype.get(), Some(Value::Object(proto)) if Rc::ptr_eq(proto, map));
            if !is_own_prototype {
                return Some(Ok(class.prototype_object()));
            }
            Some(Ok(class.parent.as_ref().map_or(Value::Null, ClassDef::prototype_object)))
        }
        _ => None,
    }
}

fn try_object_state_op(name: &str, args: &[Value]) -> Option<Result<Value, VmError>> {
    let Some(Value::Object(map)) = args.first() else {
        return None;
    };
    match name {
        "Кент.заморозить" => {
            map.borrow_mut().freeze();
            Some(Ok(Value::Object(Rc::clone(map))))
        }
        "Кент.запечатать" => {
            map.borrow_mut().seal();
            Some(Ok(Value::Object(Rc::clone(map))))
        }
        "Кент.запретитьРасширение" => {
            map.borrow_mut().prevent_extensions();
            Some(Ok(Value::Object(Rc::clone(map))))
        }
        "Кент.заморожен" => Some(Ok(Value::Bool(map.borrow().frozen))),
        "Кент.запечатан" => Some(Ok(Value::Bool(map.borrow().sealed))),
        "Кент.расширяем" => Some(Ok(Value::Bool(map.borrow().extensible))),
        _ => None,
    }
}

pub fn call_host_method(
    vm: &mut Vm,
    receiver: &IValue,
    method: &str,
    args: Vec<Value>,
    span: Span,
) -> Result<Value, VmError> {
    if method == "подписатьсяНаОтмену"
        && let IValue::AbortSignal { state } = receiver
        && let Some(callback) = args.first()
        && let Some(noop) = crate::abort::subscribe(vm, state, callback)
    {
        return Ok(noop);
    }
    with_receiver(vm, receiver, span, |interp| {
        let interp_args: Vec<IValue> = args.iter().map(|a| vm_to_interp(a, span)).collect::<Result<_, _>>()?;
        let (value, _) = yps_interpreter::stdlib::call_method(interp, receiver.clone(), method, interp_args, span)
            .map_err(map_err)?;
        interp_to_vm(&value).map_err(|m| VmError::new(m, span))
    })
}

pub fn host_member_get(vm: &mut Vm, receiver: &IValue, prop: &str, span: Span) -> Result<Value, VmError> {
    if prop == "обещание"
        && let IValue::AbortSignal { state } = receiver
    {
        return Ok(crate::abort::signal_promise(vm, state, span));
    }
    with_reader(vm, receiver, span, |interp| {
        let value = interp.host_member_get(receiver.clone(), prop, span).map_err(map_err)?;
        interp_to_vm(&value).map_err(|m| VmError::new(m, span))
    })
}

pub fn host_index_get(vm: &mut Vm, receiver: &IValue, index: &Value, span: Span) -> Result<Value, VmError> {
    with_reader(vm, receiver, span, |interp| {
        let idx = vm_to_interp(index, span)?;
        let value = interp.host_index_get(receiver.clone(), idx, span).map_err(map_err)?;
        interp_to_vm(&value).map_err(|m| VmError::new(m, span))
    })
}

pub fn host_index_set(vm: &mut Vm, receiver: &IValue, index: &Value, value: &Value, span: Span) -> Result<(), VmError> {
    with_receiver(vm, receiver, span, |interp| {
        let idx = vm_to_interp(index, span)?;
        let val = vm_to_interp(value, span)?;
        interp.host_index_set(receiver, &idx, val, span).map_err(map_err)
    })
}

pub fn host_member_set(vm: &mut Vm, receiver: &IValue, prop: &str, value: &Value, span: Span) -> Result<(), VmError> {
    with_receiver(vm, receiver, span, |interp| {
        let val = vm_to_interp(value, span)?;
        interp.host_member_set(receiver, prop, val, span).map_err(map_err)
    })
}

pub fn host_iterate(vm: &mut Vm, receiver: &IValue, span: Span) -> Result<Vec<Value>, VmError> {
    with_reader(vm, receiver, span, |interp| {
        let values = interp.host_iterate(receiver, span).map_err(map_err)?;
        values.iter().map(|v| interp_to_vm(v).map_err(|m| VmError::new(m, span))).collect()
    })
}

#[must_use]
pub fn is_host_callback(name: &str) -> bool {
    yps_interpreter::host_callback::is_host_callback(name)
}

pub fn call_host_callback(vm: &mut Vm, name: &str, args: Vec<Value>, span: Span) -> Result<Value, VmError> {
    with_interp(vm, |_| {
        let interp_args: Vec<IValue> = args.iter().map(|a| vm_to_interp(a, span)).collect::<Result<_, _>>()?;
        match yps_interpreter::host_callback::invoke(name, interp_args, span) {
            Some(res) => {
                let value = res.map_err(map_err)?;
                interp_to_vm(&value).map_err(|m| VmError::new(m, span))
            }
            None => Err(VmError::new(format!("хост-замыкание '{name}' не найдено"), span)),
        }
    })
}

pub fn host_call(vm: &mut Vm, callee: &IValue, args: &[Value], span: Span) -> Result<Value, VmError> {
    with_receiver(vm, callee, span, |interp| {
        let interp_args: Vec<IValue> = args.iter().map(|a| vm_to_interp(a, span)).collect::<Result<_, _>>()?;
        let value = interp.host_call(callee.clone(), interp_args, span).map_err(map_err)?;
        interp_to_vm(&value).map_err(|m| VmError::new(m, span))
    })
}

pub fn host_in(vm: &mut Vm, key: &Value, container: &IValue, span: Span) -> Result<bool, VmError> {
    with_reader(vm, container, span, |interp| {
        let key = vm_to_interp(key, span)?;
        interp.host_in(&key, container, span).map_err(map_err)
    })
}

pub fn host_for_in_keys(vm: &mut Vm, receiver: &IValue, span: Span) -> Result<Vec<Value>, VmError> {
    with_reader(vm, receiver, span, |interp| {
        let values = interp.host_for_in_keys(receiver, span).map_err(map_err)?;
        values.iter().map(|v| interp_to_vm(v).map_err(|m| VmError::new(m, span))).collect()
    })
}

pub fn host_delete(vm: &mut Vm, receiver: &IValue, key: &Value, span: Span) -> Result<bool, VmError> {
    with_receiver(vm, receiver, span, |interp| {
        let key = vm_to_interp(key, span)?;
        interp.host_delete(receiver, &key, span).map_err(map_err)
    })
}

pub(crate) fn map_err(e: yps_interpreter::RuntimeError) -> VmError {
    if let Some(thrown) = &e.thrown {
        let vm_thrown = interp_to_vm(thrown).unwrap_or_else(|_| Value::Str(Rc::from(e.message.as_str())));
        return VmError::new(e.message, e.span).with_thrown(vm_thrown);
    }
    VmError::new(e.message, e.span)
}

pub fn vm_to_interp(value: &Value, span: Span) -> Result<IValue, VmError> {
    convert_to_interp(value, span, &mut HashSet::new())
}

fn is_recording() -> bool {
    CROSSING_DEPTH.with(Cell::get) > 0 && RECORDING.with(Cell::get)
}

fn is_read_only_call(name: &str) -> bool {
    let Some((namespace, method)) = name.split_once('.') else {
        return false;
    };
    match namespace {
        "Матан" | "Хуйня" | "Строка" | "Жсон" => true,
        "Кент" => matches!(
            method,
            "ключи"
                | "значения"
                | "записи"
                | "имеетСвоё"
                | "есть"
                | "описатьСвойство"
                | "описатьСвойства"
                | "прототип"
        ),
        "Отражение" => {
            matches!(
                method,
                "получить"
                    | "есть"
                    | "собственныеКлючи"
                    | "описатьСвойство"
                    | "прототипОт"
                    | "расширяем"
            )
        }
        _ => false,
    }
}

fn convert_args(name: &str, args: &[Value], span: Span) -> Result<Vec<IValue>, VmError> {
    let previous = RECORDING.with(|r| r.replace(!is_read_only_call(name)));
    let converted = args.iter().map(|a| vm_to_interp(a, span)).collect();
    RECORDING.with(|r| r.set(previous));
    converted
}

fn record_crossing(vm_value: Value, store: IValue, snapshot: Snapshot) {
    let Some(ptr) = store_ptr(&store) else {
        return;
    };
    if !is_recording() {
        CROSSED.with(|c| {
            if let Some(entry) = c.borrow_mut().get_mut(&ptr) {
                entry.snapshot = snapshot;
            }
        });
        return;
    }
    CROSSED.with(|c| c.borrow_mut().insert(ptr, Crossed { vm_value, store, snapshot }));
    CROSSED_LOG.with(|l| l.borrow_mut().push(ptr));
}

fn convert_to_interp(value: &Value, span: Span, seen: &mut HashSet<usize>) -> Result<IValue, VmError> {
    match value {
        Value::Number(n) => Ok(IValue::Number(*n)),
        Value::BigInt(n) => Ok(IValue::BigInt(*n)),
        Value::Str(s) => Ok(IValue::String(Rc::clone(s))),
        Value::Bool(b) => Ok(IValue::Boolean(*b)),
        Value::Null => Ok(IValue::Null),
        Value::Undefined => Ok(IValue::Undefined),
        Value::Host(iv) => {
            refresh_host_value(iv, span, seen)?;
            Ok(iv.clone())
        }
        Value::RegExp { pattern, flags, compiled, last_index } => {
            Ok(IValue::RegExp(Rc::new(yps_interpreter::value::RegExpData {
                pattern: pattern.to_string(),
                flags: flags.to_string(),
                compiled: Rc::clone(compiled),
                last_index: Rc::clone(last_index),
            })))
        }
        Value::Array(items) => stacker::maybe_grow(STACK_RED_ZONE, STACK_GROW_SIZE, || {
            let ptr = Rc::as_ptr(items) as usize;
            let store = match cached_identity(ptr) {
                Some(store @ IValue::Array(_)) => store,
                _ => {
                    let store = IValue::array(Vec::new());
                    store_identity(ptr, CacheKeepalive::Array(Rc::downgrade(items)), store.clone());
                    store
                }
            };
            if !seen.insert(ptr) {
                return Ok(store);
            }
            let epoch = crate::value::mutation_epoch();
            if is_synced(value) {
                if is_recording()
                    && let IValue::Array(rc) = &store
                {
                    record_crossing(value.clone(), store.clone(), Snapshot::Array(rc.borrow().0.clone()));
                }
                return Ok(store);
            }
            let converted: Vec<IValue> =
                items.borrow().iter().map(|v| convert_to_interp(v, span, seen)).collect::<Result<_, _>>()?;
            if let IValue::Array(rc) = &store
                && let Ok(mut guard) = rc.try_borrow_mut()
            {
                guard.0.clone_from(&converted);
            }
            items.borrow_mut().1 = true;
            record_crossing(value.clone(), store.clone(), Snapshot::Array(converted));
            mark_synced(ptr, epoch);
            Ok(store)
        }),
        Value::Object(map) => stacker::maybe_grow(STACK_RED_ZONE, STACK_GROW_SIZE, || {
            let ptr = Rc::as_ptr(map) as usize;
            let store = match cached_identity(ptr) {
                Some(store @ IValue::Object(_)) => store,
                _ => {
                    let store = IValue::object(indexmap::IndexMap::new());
                    store_identity(ptr, CacheKeepalive::Object(Rc::downgrade(map)), store.clone());
                    store
                }
            };
            if !seen.insert(ptr) {
                return Ok(store);
            }
            let epoch = crate::value::mutation_epoch();
            if is_synced(value) {
                if is_recording()
                    && let IValue::Object(rc) = &store
                {
                    let current = rc.borrow();
                    record_crossing(
                        value.clone(),
                        store.clone(),
                        Snapshot::Object {
                            map: current.map.clone(),
                            frozen: current.frozen,
                            sealed: current.sealed,
                            extensible: current.extensible,
                        },
                    );
                }
                return Ok(store);
            }
            let mut out = indexmap::IndexMap::new();
            let (frozen, sealed, extensible) = {
                let source = map.borrow();
                for (k, v) in source.iter() {
                    if k != crate::value::CLASS_TAG {
                        out.insert(k.clone(), convert_to_interp(v, span, seen)?);
                    }
                }
                if let Some(Value::Class(class)) = source.get(crate::value::CLASS_TAG) {
                    let own_prototype =
                        matches!(class.prototype.get(), Some(Value::Object(proto)) if Rc::ptr_eq(proto, map));
                    let inherited = if own_prototype {
                        class.parent.as_ref().map(ClassDef::prototype_object)
                    } else {
                        Some(class.prototype_object())
                    };
                    if let Some(inherited) = inherited {
                        out.insert(crate::value::PROTO_KEY.to_string(), convert_to_interp(&inherited, span, seen)?);
                    }
                }
                (source.frozen, source.sealed, source.extensible)
            };
            if let IValue::Object(rc) = &store
                && let Ok(mut guard) = rc.try_borrow_mut()
            {
                guard.map.clone_from(&out);
                guard.frozen = frozen;
                guard.sealed = sealed;
                guard.extensible = extensible;
            }
            map.borrow_mut().crossed = true;
            record_crossing(value.clone(), store.clone(), Snapshot::Object { map: out, frozen, sealed, extensible });
            mark_synced(ptr, epoch);
            Ok(store)
        }),
        Value::Generator(generator) => Ok(generator_to_interp(generator)),
        Value::Function(_)
        | Value::Builtin(_)
        | Value::Class(_)
        | Value::BoundMethod { .. }
        | Value::PromiseCapability { .. }
        | Value::PromiseThenHandler { .. }
        | Value::PromiseFinallyHandler { .. }
        | Value::PromiseAggregateHandler { .. } => Ok(wrap_vm_callback(value.clone())),
        other => {
            let store = IValue::object(indexmap::IndexMap::new());
            if let Some(ptr) = store_ptr(&store) {
                GENERATOR_BY_STORE.with(|m| m.borrow_mut().insert(ptr, other.clone()));
                OPAQUE_STORES.with(|m| m.borrow_mut().push(store.clone()));
            }
            Ok(store)
        }
    }
}

fn to_runtime_error(e: &VmError, span: Span) -> yps_interpreter::RuntimeError {
    match &e.thrown {
        Some(t) => {
            let it = vm_to_interp(t, span).unwrap_or(IValue::String(e.message.clone().into()));
            yps_interpreter::RuntimeError::thrown(it, e.span)
        }
        None => yps_interpreter::RuntimeError::new(e.message.clone(), e.span),
    }
}

type GeneratorStep = fn(&mut Vm, &Rc<RefCell<crate::value::GenState>>, Value, Span) -> Result<Value, VmError>;

fn generator_method(generator: &Rc<RefCell<crate::value::GenState>>, step: GeneratorStep) -> IValue {
    let held = Rc::clone(generator);
    let marker = yps_interpreter::host_callback::register(Box::new(move |iargs, span| {
        let vm: &mut Vm = unsafe { &mut *active_vm(span)? };
        let sent = match iargs.first() {
            Some(arg) => interp_to_vm(arg).map_err(|m| yps_interpreter::RuntimeError::new(m, span))?,
            None => Value::Undefined,
        };
        let result = step(vm, &held, sent, span).map_err(|e| to_runtime_error(&e, span))?;
        vm_to_interp(&result, span).map_err(|e| yps_interpreter::RuntimeError::new(e.message, span))
    }));
    IValue::BuiltinFunction(marker)
}

fn generator_to_interp(generator: &Rc<RefCell<crate::value::GenState>>) -> IValue {
    let key = Rc::as_ptr(generator) as usize;
    if let Some(store) = GENERATOR_STORES.with(|m| m.borrow().get(&key).cloned()) {
        return store;
    }
    let mut protocol = indexmap::IndexMap::new();
    protocol.insert("следующий".to_string(), generator_method(generator, Vm::gen_next));
    protocol.insert("вернуть".to_string(), generator_method(generator, Vm::gen_return));
    let store = IValue::object(protocol);
    GENERATOR_STORES.with(|m| m.borrow_mut().insert(key, store.clone()));
    if let Some(ptr) = store_ptr(&store) {
        GENERATOR_BY_STORE.with(|m| m.borrow_mut().insert(ptr, Value::Generator(Rc::clone(generator))));
    }
    store
}

fn wrap_vm_callback(callee: Value) -> IValue {
    let identity = match &callee {
        Value::Function(closure) => Some(Rc::as_ptr(closure) as *const () as usize),
        Value::Class(class) => Some(Rc::as_ptr(class) as *const () as usize),
        _ => None,
    };
    if let Some(ptr) = identity
        && let Some(marker) = CALLBACK_BY_PTR.with(|m| m.borrow().get(&ptr).cloned())
    {
        return IValue::BuiltinFunction(marker);
    }
    let original = callee.clone();
    let marker = yps_interpreter::host_callback::register(Box::new(move |iargs, span| {
        let vm: &mut Vm = unsafe { &mut *active_vm(span)? };
        let mut vm_args: Vec<Value> = Vec::with_capacity(iargs.len());
        for a in &iargs {
            match interp_to_vm(a) {
                Ok(v) => vm_args.push(v),
                Err(m) => return Err(yps_interpreter::RuntimeError::new(m, span)),
            }
        }
        let this = match yps_interpreter::host_callback::take_call_this() {
            Some(this) => Some(interp_to_vm(&this).map_err(|m| yps_interpreter::RuntimeError::new(m, span))?),
            None => None,
        };
        let result = match (&callee, &this) {
            (Value::Function(closure), Some(Value::Object(map)))
                if !closure.proto.is_generator && !closure.proto.is_async =>
            {
                let owner = Vm::class_along_chain(map)
                    .and_then(|class| class.find_method_with_owner(&closure.proto.name))
                    .filter(|(method, _)| Rc::ptr_eq(method, closure))
                    .map(|(_, owner)| owner);
                vm.call_closure_with_owner(Rc::clone(closure), this.clone(), owner, &vm_args, span)
            }
            _ => vm.call_value(callee.clone(), this, &vm_args, span),
        };
        for arg in vm_args.iter().filter(|a| matches!(a, Value::Array(_) | Value::Object(_))) {
            vm_to_interp(arg, span).map_err(|e| yps_interpreter::RuntimeError::new(e.message, span))?;
        }
        match result {
            Ok(result) => vm_to_interp(&result, span).map_err(|e| yps_interpreter::RuntimeError::new(e.message, span)),
            Err(e) => Err(to_runtime_error(&e, span)),
        }
    }));
    if let Some(ptr) = identity {
        CALLBACK_BY_PTR.with(|m| m.borrow_mut().insert(ptr, marker.clone()));
    }
    VM_BY_CALLBACK.with(|m| m.borrow_mut().insert(marker.clone(), original));
    IValue::BuiltinFunction(marker)
}

pub fn interp_to_vm(value: &IValue) -> Result<Value, String> {
    match value {
        IValue::Number(n) => Ok(Value::Number(*n)),
        IValue::BigInt(n) => Ok(Value::BigInt(*n)),
        IValue::String(s) => Ok(Value::Str(Rc::clone(s))),
        IValue::Boolean(b) => Ok(Value::Bool(*b)),
        IValue::Null => Ok(Value::Null),
        IValue::Undefined => Ok(Value::Undefined),
        IValue::BuiltinFunction(name) => Ok(VM_BY_CALLBACK
            .with(|m| m.borrow().get(name).cloned())
            .unwrap_or_else(|| Value::Builtin(Rc::from(name.as_str())))),
        IValue::RegExp(re) => Ok(Value::RegExp {
            pattern: Rc::from(re.pattern.as_str()),
            flags: Rc::from(re.flags.as_str()),
            compiled: Rc::clone(&re.compiled),
            last_index: Rc::clone(&re.last_index),
        }),
        IValue::Array(_) | IValue::Object(_) => stacker::maybe_grow(STACK_RED_ZONE, STACK_GROW_SIZE, || {
            if let Some(known) = vm_value_of_store(value) {
                return Ok(known);
            }
            let generator = store_ptr(value).and_then(|ptr| GENERATOR_BY_STORE.with(|m| m.borrow().get(&ptr).cloned()));
            if let Some(generator) = generator {
                return Ok(generator);
            }
            let filled = new_vm_value_for_store(value);
            if filled.is_err()
                && let Some(ptr) = store_ptr(value)
            {
                VM_BY_STORE.with(|m| m.borrow_mut().remove(&ptr));
            }
            filled
        }),
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
        | IValue::AbortUnsubscribe { .. }
        | IValue::Iterator(_) => Ok(Value::Host(value.clone())),
        other => Err(format!("значение типа '{}' из stdlib пока нельзя вернуть в VM", other.type_name())),
    }
}

fn new_vm_value_for_store(store: &IValue) -> Result<Value, String> {
    match store {
        IValue::Array(items) => {
            let rc = Rc::new(RefCell::new(crate::value::ArrayStore::default()));
            store_identity(Rc::as_ptr(&rc) as usize, CacheKeepalive::Array(Rc::downgrade(&rc)), store.clone());
            let converted: Vec<Value> = items.borrow().0.iter().map(interp_to_vm).collect::<Result<_, _>>()?;
            {
                let mut array = rc.borrow_mut();
                array.0 = converted;
                array.1 = true;
            }
            let vm_value = Value::Array(rc);
            record_crossing(vm_value.clone(), store.clone(), Snapshot::Array(items.borrow().0.clone()));
            Ok(vm_value)
        }
        IValue::Object(obj) => {
            let rc = Rc::new(RefCell::new(ObjMap::new()));
            store_identity(Rc::as_ptr(&rc) as usize, CacheKeepalive::Object(Rc::downgrade(&rc)), store.clone());
            let source = obj.borrow();
            for (k, v) in &source.map {
                let converted = interp_to_vm(v)?;
                rc.borrow_mut().insert(k.clone(), converted);
            }
            {
                let mut map = rc.borrow_mut();
                map.frozen = source.frozen;
                map.sealed = source.sealed;
                map.extensible = source.extensible;
                map.crossed = true;
            }
            let vm_value = Value::Object(rc);
            record_crossing(
                vm_value.clone(),
                store.clone(),
                Snapshot::Object {
                    map: source.map.clone(),
                    frozen: source.frozen,
                    sealed: source.sealed,
                    extensible: source.extensible,
                },
            );
            Ok(vm_value)
        }
        other => interp_to_vm(other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cached_identity_ignores_stale_dead_entry() {
        let ptr = 0x1234_usize;
        let items = Rc::new(RefCell::new(crate::value::ArrayStore::default()));
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
