use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::{Rc, Weak};

use crate::resolver::VarUse;
use crate::value::Value;

pub(crate) const MAX_SLOTS: usize = 64;

#[derive(Debug, Default)]
pub(crate) struct ScopeLayout {
    pub(crate) names: Vec<Rc<str>>,
    pub(crate) tdz_mask: u64,
    pub(crate) id: u32,
}

impl ScopeLayout {
    pub(crate) fn index_of(&self, name: &str) -> Option<usize> {
        self.names.iter().position(|n| &**n == name)
    }
}

#[derive(Debug)]
pub struct EnvFrame {
    layout: Option<Rc<ScopeLayout>>,
    slots: Vec<Value>,
    init_mask: u64,
    slot_tdz: u64,
    slot_const: u64,
    bindings: HashMap<String, Value>,
    constants: HashSet<String>,
    tdz: HashSet<String>,
    disposables: Vec<(Value, bool)>,
    parent: Option<Rc<RefCell<EnvFrame>>>,
}

pub(crate) enum Lookup {
    Found(Value),
    Tdz,
    Missing,
}

pub(crate) enum SlotWrite {
    Done,
    Const,
    Fallback,
}

impl EnvFrame {
    fn empty(parent: Option<Rc<RefCell<EnvFrame>>>) -> Self {
        Self {
            layout: None,
            slots: Vec::new(),
            init_mask: 0,
            slot_tdz: 0,
            slot_const: 0,
            bindings: HashMap::new(),
            constants: HashSet::new(),
            tdz: HashSet::new(),
            disposables: Vec::new(),
            parent,
        }
    }

    pub(crate) fn gc_values(&self) -> impl Iterator<Item = &Value> {
        self.slots
            .iter()
            .enumerate()
            .filter(|(i, _)| self.init_mask & (1u64 << i) != 0)
            .map(|(_, v)| v)
            .chain(self.bindings.values())
            .chain(self.disposables.iter().map(|(v, _)| v))
    }

    pub(crate) fn gc_parent(&self) -> Option<Rc<RefCell<EnvFrame>>> {
        self.parent.clone()
    }

    pub(crate) fn gc_clear(&mut self) {
        self.layout = None;
        self.slots.clear();
        self.init_mask = 0;
        self.slot_tdz = 0;
        self.slot_const = 0;
        self.bindings.clear();
        self.constants.clear();
        self.tdz.clear();
        self.disposables.clear();
        self.parent = None;
    }

    pub(crate) fn rebind(&mut self, name: String, value: Value) {
        if let Some(index) = self.slot_index(&name) {
            self.slots[index] = value;
            self.init_mask |= 1u64 << index;
            self.slot_tdz &= !(1u64 << index);
            return;
        }
        self.bindings.insert(name, value);
    }

    fn slot_index(&self, name: &str) -> Option<usize> {
        self.layout.as_ref()?.index_of(name)
    }

    fn layout_is(&self, expected: u32) -> bool {
        self.layout.as_ref().is_some_and(|l| l.id == expected)
    }

    fn slot_value(&self, index: usize) -> Option<&Value> {
        if self.init_mask & (1u64 << index) != 0 { Some(&self.slots[index]) } else { None }
    }

    #[inline]
    fn live_slot(&self, name: &str) -> Option<usize> {
        let index = self.slot_index(name)?;
        if self.init_mask & (1u64 << index) != 0 { Some(index) } else { None }
    }

    #[inline]
    fn slot_is_const(&self, index: usize) -> bool {
        self.slot_const & (1u64 << index) != 0
    }

    pub(crate) fn get_local(&self, name: &str) -> Option<Value> {
        if let Some(index) = self.live_slot(name) {
            return Some(self.slots[index].clone());
        }
        self.bindings.get(name).cloned()
    }

    fn local_read(&self, name: &str) -> Lookup {
        if let Some(index) = self.slot_index(name) {
            if let Some(value) = self.slot_value(index) {
                return Lookup::Found(value.clone());
            }
            if self.slot_tdz & (1u64 << index) != 0 {
                return Lookup::Tdz;
            }
        }
        if let Some(value) = self.bindings.get(name) {
            return Lookup::Found(value.clone());
        }
        if self.tdz.contains(name) {
            return Lookup::Tdz;
        }
        Lookup::Missing
    }

    fn define_local(&mut self, name: &str, value: Value, is_const: bool) {
        if let Some(index) = self.slot_index(name) {
            let bit = 1u64 << index;
            self.slots[index] = value;
            self.init_mask |= bit;
            self.slot_tdz &= !bit;
            if is_const {
                self.slot_const |= bit;
            } else {
                self.slot_const &= !bit;
            }
            if !self.bindings.is_empty() {
                self.bindings.remove(name);
            }
            if !self.constants.is_empty() {
                self.constants.remove(name);
            }
            if !self.tdz.is_empty() {
                self.tdz.remove(name);
            }
            return;
        }
        if is_const {
            self.constants.insert(name.to_string());
        } else if !self.constants.is_empty() {
            self.constants.remove(name);
        }
        if !self.tdz.is_empty() {
            self.tdz.remove(name);
        }
        self.bindings.insert(name.to_string(), value);
    }

    fn spill_slots(&mut self) {
        let Some(layout) = self.layout.take() else { return };
        for (index, name) in layout.names.iter().enumerate() {
            let bit = 1u64 << index;
            if self.init_mask & bit != 0 {
                let value = std::mem::replace(&mut self.slots[index], Value::Undefined);
                self.bindings.insert(name.to_string(), value);
                if self.slot_const & bit != 0 {
                    self.constants.insert(name.to_string());
                }
            } else if self.slot_tdz & bit != 0 {
                self.tdz.insert(name.to_string());
            }
        }
        self.slots.clear();
        self.init_mask = 0;
        self.slot_tdz = 0;
        self.slot_const = 0;
    }

    pub fn debug_bindings(&self) -> Vec<(String, Value)> {
        let mut out: Vec<(String, Value)> = Vec::new();
        if let Some(layout) = &self.layout {
            for (index, name) in layout.names.iter().enumerate() {
                if let Some(value) = self.slot_value(index) {
                    out.push((name.to_string(), value.clone()));
                }
            }
        }
        out.extend(self.bindings.iter().map(|(k, v)| (k.clone(), v.clone())));
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
    }

    pub fn debug_parent(&self) -> Option<Rc<RefCell<EnvFrame>>> {
        self.parent.clone()
    }
}

#[derive(Debug)]
pub struct FrameRegistry {
    frames: RefCell<Vec<Weak<RefCell<EnvFrame>>>>,
    prune_at: Cell<usize>,
}

impl FrameRegistry {
    fn new() -> Rc<Self> {
        Rc::new(Self { frames: RefCell::new(Vec::new()), prune_at: Cell::new(1024) })
    }

    fn register(&self, frame: &Rc<RefCell<EnvFrame>>) {
        let mut frames = self.frames.borrow_mut();
        frames.push(Rc::downgrade(frame));
        if frames.len() >= self.prune_at.get() {
            frames.retain(|w| w.strong_count() > 0);
            self.prune_at.set((frames.len() * 2).max(1024));
        }
    }

    pub(crate) fn prune_and_count(&self) -> usize {
        let mut frames = self.frames.borrow_mut();
        frames.retain(|w| w.strong_count() > 0);
        self.prune_at.set((frames.len() * 2).max(1024));
        frames.len()
    }

    pub(crate) fn live_frames(&self) -> Vec<Rc<RefCell<EnvFrame>>> {
        self.frames.borrow().iter().filter_map(|w| w.upgrade()).collect()
    }
}

#[derive(Debug, Clone)]
pub struct Environment {
    current: Rc<RefCell<EnvFrame>>,
    registry: Rc<FrameRegistry>,
}

impl Default for Environment {
    fn default() -> Self {
        Self::new()
    }
}

impl Environment {
    pub fn new() -> Self {
        let registry = FrameRegistry::new();
        let current = Rc::new(RefCell::new(EnvFrame::empty(None)));
        registry.register(&current);
        Self { current, registry }
    }

    pub fn push_scope(&mut self) {
        self.push_frame(EnvFrame::empty(Some(Rc::clone(&self.current))));
    }

    pub(crate) fn push_scope_with(&mut self, layout: Rc<ScopeLayout>) {
        if layout.names.len() > MAX_SLOTS {
            self.push_scope();
            return;
        }
        let mut frame = EnvFrame::empty(Some(Rc::clone(&self.current)));
        frame.slots = vec![Value::Undefined; layout.names.len()];
        frame.layout = Some(layout);
        self.push_frame(frame);
    }

    fn push_frame(&mut self, frame: EnvFrame) {
        self.current = Rc::new(RefCell::new(frame));
        self.registry.register(&self.current);
    }

    pub(crate) fn apply_layout_tdz(&mut self) {
        let mut frame = self.current.borrow_mut();
        let Some(layout) = frame.layout.clone() else { return };
        frame.slot_tdz |= layout.tdz_mask & !frame.init_mask;
    }

    pub(crate) fn install_root_layout(&mut self, layout: Rc<ScopeLayout>) {
        if layout.names.len() > MAX_SLOTS {
            return;
        }
        let mut frame = self.current.borrow_mut();
        frame.spill_slots();
        frame.slots = vec![Value::Undefined; layout.names.len()];
        frame.layout = Some(layout);
    }

    pub fn pop_scope(&mut self) {
        let parent = self.current.borrow().parent.clone();
        if let Some(parent) = parent {
            self.current = parent;
        }
    }

    pub fn fork_current(&mut self) {
        let new_frame = {
            let frame = self.current.borrow();
            EnvFrame {
                layout: frame.layout.clone(),
                slots: frame.slots.clone(),
                init_mask: frame.init_mask,
                slot_tdz: frame.slot_tdz,
                slot_const: frame.slot_const,
                bindings: frame.bindings.clone(),
                constants: frame.constants.clone(),
                tdz: frame.tdz.clone(),
                disposables: Vec::new(),
                parent: frame.parent.clone(),
            }
        };
        self.push_frame(new_frame);
    }

    pub fn snapshot(&self) -> Rc<RefCell<EnvFrame>> {
        Rc::clone(&self.current)
    }

    pub(crate) fn registry(&self) -> Rc<FrameRegistry> {
        Rc::clone(&self.registry)
    }

    pub(crate) fn from_snapshot(frame: Rc<RefCell<EnvFrame>>, registry: Rc<FrameRegistry>) -> Self {
        Self { current: frame, registry }
    }

    pub fn define(&mut self, name: &str, value: Value, is_const: bool) {
        self.current.borrow_mut().define_local(name, value, is_const);
    }

    pub(crate) fn mark_tdz(&mut self, names: impl IntoIterator<Item = String>) {
        let mut frame = self.current.borrow_mut();
        for name in names {
            match frame.slot_index(&name) {
                Some(index) => {
                    let bit = 1u64 << index;
                    if frame.init_mask & bit == 0 {
                        frame.slot_tdz |= bit;
                    }
                }
                None => {
                    if !frame.bindings.contains_key(&name) {
                        frame.tdz.insert(name);
                    }
                }
            }
        }
    }

    fn ancestor(&self, hops: u16) -> Option<Rc<RefCell<EnvFrame>>> {
        let mut frame = Rc::clone(&self.current);
        for _ in 0..hops {
            let parent = frame.borrow().parent.clone();
            frame = parent?;
        }
        Some(frame)
    }

    pub(crate) fn read_slot(&self, var: VarUse, name: &str) -> Lookup {
        let (hops, slot) = (var.hops, var.slot);
        let Some(frame_rc) = self.ancestor(hops) else {
            debug_assert!(false, "слот-резолюция вышла за пределы цепочки кадров");
            return self.lookup_read(name);
        };
        let frame = frame_rc.borrow();
        if !frame.layout_is(var.layout) {
            debug_assert!(false, "слот-резолюция указывает на чужой слот");
            drop(frame);
            return self.lookup_read(name);
        }
        let index = slot as usize;
        let bit = 1u64 << index;
        if frame.init_mask & bit != 0 {
            return Lookup::Found(frame.slots[index].clone());
        }
        if frame.slot_tdz & bit != 0 {
            return Lookup::Tdz;
        }
        drop(frame);
        self.lookup_read(name)
    }

    pub(crate) fn get_slot(&self, var: VarUse) -> Option<Value> {
        let frame_rc = self.ancestor(var.hops)?;
        let frame = frame_rc.borrow();
        if !frame.layout_is(var.layout) {
            debug_assert!(false, "слот-резолюция указывает на чужой слот");
            return None;
        }
        frame.slot_value(var.slot as usize).cloned()
    }

    pub(crate) fn write_slot(&self, var: VarUse, value: &Value) -> SlotWrite {
        let (hops, slot) = (var.hops, var.slot);
        let Some(frame_rc) = self.ancestor(hops) else {
            debug_assert!(false, "слот-резолюция вышла за пределы цепочки кадров");
            return SlotWrite::Fallback;
        };
        let mut frame = frame_rc.borrow_mut();
        if !frame.layout_is(var.layout) {
            debug_assert!(false, "слот-резолюция указывает на чужой слот");
            return SlotWrite::Fallback;
        }
        let index = slot as usize;
        let bit = 1u64 << index;
        if frame.init_mask & bit == 0 {
            return SlotWrite::Fallback;
        }
        if frame.slot_const & bit != 0 {
            return SlotWrite::Const;
        }
        frame.slots[index] = value.clone();
        SlotWrite::Done
    }

    pub(crate) fn lookup_read(&self, name: &str) -> Lookup {
        let mut frame_rc = Rc::clone(&self.current);
        loop {
            let parent = {
                let frame = frame_rc.borrow();
                match frame.local_read(name) {
                    Lookup::Found(value) => return Lookup::Found(value),
                    Lookup::Tdz => return Lookup::Tdz,
                    Lookup::Missing => frame.parent.clone(),
                }
            };
            match parent {
                Some(p) => frame_rc = p,
                None => return Lookup::Missing,
            }
        }
    }

    pub fn is_const(&self, name: &str) -> bool {
        let mut frame_rc = Rc::clone(&self.current);
        loop {
            let parent = {
                let frame = frame_rc.borrow();
                if let Some(index) = frame.live_slot(name) {
                    return frame.slot_is_const(index);
                }
                if frame.constants.contains(name) {
                    return true;
                }
                if frame.bindings.contains_key(name) {
                    return false;
                }
                frame.parent.clone()
            };
            match parent {
                Some(p) => frame_rc = p,
                None => return false,
            }
        }
    }

    #[inline]
    pub(crate) fn get_shallow(&self, name: &str) -> Option<Value> {
        self.current.borrow().get_local(name)
    }

    pub fn get(&self, name: &str) -> Option<Value> {
        let mut frame_rc = Rc::clone(&self.current);
        loop {
            let parent = {
                let frame = frame_rc.borrow();
                if let Some(value) = frame.get_local(name) {
                    return Some(value);
                }
                frame.parent.clone()
            };
            frame_rc = parent?;
        }
    }

    pub fn lookup(&self, name: &str) -> (bool, Option<Value>) {
        let mut frame_rc = Rc::clone(&self.current);
        loop {
            let parent = {
                let frame = frame_rc.borrow();
                if let Some(index) = frame.live_slot(name) {
                    return (frame.slot_is_const(index), Some(frame.slots[index].clone()));
                }
                if let Some(v) = frame.bindings.get(name) {
                    return (frame.constants.contains(name), Some(v.clone()));
                }
                if frame.constants.contains(name) {
                    return (true, None);
                }
                frame.parent.clone()
            };
            match parent {
                Some(p) => frame_rc = p,
                None => return (false, None),
            }
        }
    }

    pub fn add_disposable(&mut self, value: Value, is_await: bool) {
        self.current.borrow_mut().disposables.push((value, is_await));
    }

    pub fn take_disposables(&mut self) -> Vec<(Value, bool)> {
        std::mem::take(&mut self.current.borrow_mut().disposables)
    }

    pub fn set(&self, name: &str, value: Value) -> bool {
        let mut frame_rc = Rc::clone(&self.current);
        loop {
            let parent = {
                let mut frame = frame_rc.borrow_mut();
                if let Some(index) = frame.live_slot(name) {
                    frame.slots[index] = value;
                    return true;
                }
                if let Some(slot) = frame.bindings.get_mut(name) {
                    *slot = value;
                    return true;
                }
                frame.parent.clone()
            };
            match parent {
                Some(p) => frame_rc = p,
                None => return false,
            }
        }
    }
}
