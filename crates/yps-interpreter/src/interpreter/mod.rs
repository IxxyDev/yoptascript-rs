use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::rc::Rc;

use yps_lexer::Span;
use yps_parser::ast::Program;

use crate::builtins::builtin_names;
use crate::environment::Environment;
use crate::error::RuntimeError;
use crate::value::Value;

pub(crate) type Microtask = Box<dyn FnOnce(&mut Interpreter, Span) -> Result<(), RuntimeError>>;

mod assign;
mod call;
mod class;
mod delete;
mod eval_expr;
mod event_loop;
mod exec_stmt;
pub(crate) mod generator;
mod member;
mod module_loader;
mod promise_rt;
mod types;

use event_loop::MacrotaskQueue;

pub(super) use types::{AccessSegment, ControlFlow, LoopOp};

pub struct Interpreter {
    pub(super) env: Environment,
    pub(super) pending_initializers: Vec<Value>,
    pub(super) base_path: Option<PathBuf>,
    pub(super) module_cache: Rc<RefCell<HashMap<PathBuf, HashMap<String, Value>>>>,
    pub(super) current_exports: HashMap<String, Value>,
    pub(super) microtasks: VecDeque<Microtask>,
    pub(super) macrotasks: MacrotaskQueue,
    pub(super) await_depth: usize,
    pub(super) pending_label: Option<String>,
}

pub(super) const MAX_AWAIT_DEPTH: usize = 16;

impl Default for Interpreter {
    fn default() -> Self {
        Self::new()
    }
}

impl Interpreter {
    pub fn new() -> Self {
        let mut env = Environment::new();
        for name in builtin_names() {
            env.define(name.to_string(), Value::BuiltinFunction(name.to_string()), true);
        }
        for (name, value) in crate::stdlib::build_globals() {
            env.define(name, value, true);
        }
        env.define("нихуя".to_string(), Value::Number(f64::NAN), true);
        Self {
            env,
            pending_initializers: Vec::new(),
            base_path: None,
            module_cache: Rc::new(RefCell::new(HashMap::new())),
            current_exports: HashMap::new(),
            microtasks: VecDeque::new(),
            macrotasks: MacrotaskQueue::new(),
            await_depth: 0,
            pending_label: None,
        }
    }

    pub fn set_base_path(&mut self, path: PathBuf) {
        self.base_path = Some(path);
    }

    pub fn get(&self, name: &str) -> Option<Value> {
        self.env.get(name)
    }

    pub fn run(&mut self, program: &Program) -> Result<(), RuntimeError> {
        self.hoist_functions(&program.items);
        for stmt in &program.items {
            let cf_opt = self.exec_stmt(stmt)?;
            self.drain_microtasks(Span { start: 0, end: 0 })?;
            if let Some(cf) = cf_opt {
                match cf {
                    ControlFlow::Return(_) => return Ok(()),
                    ControlFlow::Break(label) => {
                        return Err(RuntimeError::new(
                            label.map_or_else(|| "'харэ' вне цикла".to_string(), |l| format!("Метка '{l}' не найдена")),
                            Span { start: 0, end: 0 },
                        ));
                    }
                    ControlFlow::Continue(label) => {
                        return Err(RuntimeError::new(
                            label.map_or_else(
                                || "'двигай' вне цикла".to_string(),
                                |l| format!("Метка '{l}' не найдена"),
                            ),
                            Span { start: 0, end: 0 },
                        ));
                    }
                    ControlFlow::Throw(val) => {
                        return Err(RuntimeError::thrown(val, Span { start: 0, end: 0 }));
                    }
                }
            }
        }
        self.drive_event_loop(Span { start: 0, end: 0 })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
