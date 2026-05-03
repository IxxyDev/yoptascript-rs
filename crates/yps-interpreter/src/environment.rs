use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use crate::value::Value;

#[derive(Debug)]
pub struct EnvFrame {
    bindings: HashMap<String, Value>,
    constants: HashSet<String>,
    disposables: Vec<Value>,
    parent: Option<Rc<RefCell<EnvFrame>>>,
}

#[derive(Debug, Clone)]
pub struct Environment {
    current: Rc<RefCell<EnvFrame>>,
}

impl Default for Environment {
    fn default() -> Self {
        Self::new()
    }
}

impl Environment {
    pub fn new() -> Self {
        Self {
            current: Rc::new(RefCell::new(EnvFrame {
                bindings: HashMap::new(),
                constants: HashSet::new(),
                disposables: Vec::new(),
                parent: None,
            })),
        }
    }

    pub fn push_scope(&mut self) {
        let new_frame = EnvFrame {
            bindings: HashMap::new(),
            constants: HashSet::new(),
            disposables: Vec::new(),
            parent: Some(Rc::clone(&self.current)),
        };
        self.current = Rc::new(RefCell::new(new_frame));
    }

    pub fn pop_scope(&mut self) {
        let parent = self.current.borrow().parent.clone();
        if let Some(parent) = parent {
            self.current = parent;
        }
    }

    pub fn snapshot(&self) -> Rc<RefCell<EnvFrame>> {
        Rc::clone(&self.current)
    }

    pub fn from_snapshot(frame: Rc<RefCell<EnvFrame>>) -> Self {
        Self { current: frame }
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
                if frame.bindings.contains_key(name) {
                    frame.bindings.insert(name.to_string(), value);
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
