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
mod exec_stmt;
mod member;
mod module_loader;
mod promise_rt;
mod types;

pub(super) use types::{AccessSegment, ControlFlow};

pub struct Interpreter {
    pub(super) env: Environment,
    pub(super) pending_initializers: Vec<Value>,
    pub(super) base_path: Option<PathBuf>,
    pub(super) module_cache: Rc<RefCell<HashMap<PathBuf, HashMap<String, Value>>>>,
    pub(super) current_exports: HashMap<String, Value>,
    pub(super) generator_buffer: Option<Vec<Value>>,
    pub(super) microtasks: VecDeque<Microtask>,
}

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
            generator_buffer: None,
            microtasks: VecDeque::new(),
        }
    }

    pub fn set_base_path(&mut self, path: PathBuf) {
        self.base_path = Some(path);
    }

    pub fn get(&self, name: &str) -> Option<Value> {
        self.env.get(name)
    }

    pub fn run(&mut self, program: &Program) -> Result<(), RuntimeError> {
        for stmt in &program.items {
            let cf_opt = self.exec_stmt(stmt)?;
            self.drain_microtasks(Span { start: 0, end: 0 })?;
            if let Some(cf) = cf_opt {
                match cf {
                    ControlFlow::Return(_) => return Ok(()),
                    ControlFlow::Break => {
                        return Err(RuntimeError::new("'харэ' вне цикла", Span { start: 0, end: 0 }));
                    }
                    ControlFlow::Continue => {
                        return Err(RuntimeError::new("'двигай' вне цикла", Span { start: 0, end: 0 }));
                    }
                    ControlFlow::Throw(val) => {
                        return Err(RuntimeError::new(
                            format!("Необработанное исключение: {val}"),
                            Span { start: 0, end: 0 },
                        ));
                    }
                }
            }
        }
        self.drain_microtasks(Span { start: 0, end: 0 })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
