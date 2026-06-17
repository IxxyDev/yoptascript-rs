use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::{Rc, Weak};

use crate::value::Value;

#[derive(Debug)]
pub struct EnvFrame {
    bindings: HashMap<String, Value>,
    constants: HashSet<String>,
    disposables: Vec<Value>,
    parent: Option<Rc<RefCell<EnvFrame>>>,
}

impl EnvFrame {
    pub(crate) fn gc_values(&self) -> impl Iterator<Item = &Value> {
        self.bindings.values().chain(self.disposables.iter())
    }

    pub(crate) fn gc_parent(&self) -> Option<Rc<RefCell<EnvFrame>>> {
        self.parent.clone()
    }

    pub(crate) fn gc_clear(&mut self) {
        self.bindings.clear();
        self.constants.clear();
        self.disposables.clear();
        self.parent = None;
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
        let current = Rc::new(RefCell::new(EnvFrame {
            bindings: HashMap::new(),
            constants: HashSet::new(),
            disposables: Vec::new(),
            parent: None,
        }));
        registry.register(&current);
        Self { current, registry }
    }

    pub fn push_scope(&mut self) {
        let new_frame = EnvFrame {
            bindings: HashMap::new(),
            constants: HashSet::new(),
            disposables: Vec::new(),
            parent: Some(Rc::clone(&self.current)),
        };
        self.current = Rc::new(RefCell::new(new_frame));
        self.registry.register(&self.current);
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
                bindings: frame.bindings.clone(),
                constants: frame.constants.clone(),
                disposables: Vec::new(),
                parent: frame.parent.clone(),
            }
        };
        self.current = Rc::new(RefCell::new(new_frame));
        self.registry.register(&self.current);
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

    pub fn define(&mut self, name: String, value: Value, is_const: bool) {
        let mut frame = self.current.borrow_mut();
        if is_const {
            frame.constants.insert(name.clone());
        }
        frame.bindings.insert(name, value);
    }

    pub fn is_const(&self, name: &str) -> bool {
        let mut frame_rc = Rc::clone(&self.current);
        loop {
            let parent = {
                let frame = frame_rc.borrow();
                if frame.constants.contains(name) {
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

    pub fn get(&self, name: &str) -> Option<Value> {
        let mut frame_rc = Rc::clone(&self.current);
        loop {
            let parent = {
                let frame = frame_rc.borrow();
                if let Some(value) = frame.bindings.get(name) {
                    return Some(value.clone());
                }
                frame.parent.clone()
            };
            match parent {
                Some(p) => frame_rc = p,
                None => return None,
            }
        }
    }

    pub fn add_disposable(&mut self, value: Value) {
        self.current.borrow_mut().disposables.push(value);
    }

    pub fn take_disposables(&mut self) -> Vec<Value> {
        std::mem::take(&mut self.current.borrow_mut().disposables)
    }

    pub fn set(&self, name: &str, value: Value) -> bool {
        let mut frame_rc = Rc::clone(&self.current);
        loop {
            let parent = {
                let mut frame = frame_rc.borrow_mut();
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
