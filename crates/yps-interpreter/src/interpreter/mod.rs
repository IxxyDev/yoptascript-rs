use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::rc::Rc;

use yps_lexer::Span;
use yps_parser::ast::Program;

use crate::builtins::builtin_names;
use crate::environment::{EnvFrame, Environment};
use crate::error::{Frame, MAX_STACK_DEPTH, RuntimeError};
use crate::resolver::{self, RootResolution};
use crate::value::{FinRegState, Value};
use yps_parser::ast::Identifier;

pub(crate) type MicrotaskFn = Box<dyn FnOnce(&mut Interpreter, Span) -> Result<(), RuntimeError>>;

pub(crate) struct Microtask {
    pub(crate) roots: Vec<GcRoot>,
    pub(crate) run: MicrotaskFn,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RunMode {
    Script,
    Repl,
}

mod assign;
mod call;
mod class;
pub mod coercion;
mod delete;
mod eval_expr;
mod event_loop;
mod exec_stmt;
mod gc;
pub(crate) mod generator;
mod host_api;
mod member;
mod module_loader;
mod promise_rt;
mod proxy;
mod types;
mod user_iter;

use event_loop::MacrotaskQueue;

pub(crate) use gc::GcRoot;
pub(super) use types::{AccessSegment, ControlFlow, LoopOp};

pub struct Interpreter {
    pub(super) env: Environment,
    pub(super) pending_initializers: Vec<Value>,
    pub(super) base_path: Option<PathBuf>,
    pub(super) module_cache: Rc<RefCell<HashMap<PathBuf, module_loader::ModuleState>>>,
    pub(super) module_links: Rc<RefCell<Vec<module_loader::DeferredLink>>>,
    pub(super) current_exports: HashMap<String, Value>,
    pub(super) export_cell: Option<module_loader::ExportCell>,
    pub(super) microtasks: VecDeque<Microtask>,
    pub(super) macrotasks: MacrotaskQueue,
    pub(super) await_depth: usize,
    pub(super) pending_label: Option<String>,
    pub(super) call_stack: Vec<Frame>,
    pub(super) coercion_depth: usize,
    pub(super) finalization_registries: Vec<std::rc::Weak<RefCell<FinRegState>>>,
    pub(super) resolution: RootResolution,
    pub(super) global_root: Rc<RefCell<EnvFrame>>,
}

pub(super) const MAX_AWAIT_DEPTH: usize = 16;
pub(super) const MAX_COERCION_DEPTH: usize = 100;
pub(super) const MAX_CALL_DEPTH: usize = 1000;
pub(super) const GC_THRESHOLD: usize = 256;
pub(super) const SCRIPT_GC_INTERVAL: usize = 128;
pub(super) const LOOP_GC_INTERVAL: usize = 64;
pub(super) const STACK_RED_ZONE: usize = 256 * 1024;
pub(super) const STACK_GROW_SIZE: usize = 8 * 1024 * 1024;

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
        let global_root = env.snapshot();
        Self {
            env,
            pending_initializers: Vec::new(),
            base_path: None,
            module_cache: Rc::new(RefCell::new(HashMap::new())),
            module_links: Rc::new(RefCell::new(Vec::new())),
            current_exports: HashMap::new(),
            export_cell: None,
            microtasks: VecDeque::new(),
            macrotasks: MacrotaskQueue::new(),
            await_depth: 0,
            pending_label: None,
            call_stack: Vec::new(),
            coercion_depth: 0,
            finalization_registries: Vec::new(),
            resolution: RootResolution::default(),
            global_root,
        }
    }

    pub(super) fn lookup_read(&self, ident: &Identifier) -> Option<Value> {
        if !self.resolution.is_empty()
            && self.resolution.is_root_read(ident.span.start)
            && let Some(value) = self.global_root.borrow().get_local(&ident.name)
        {
            return Some(value);
        }
        self.env.get(&ident.name)
    }

    pub(crate) fn register_finalization_registry(&mut self, state: &Rc<RefCell<FinRegState>>) {
        self.finalization_registries.push(Rc::downgrade(state));
    }

    pub(super) fn push_frame(&mut self, name: Rc<str>, span: Span) {
        self.call_stack.push(Frame { name, span });
    }

    pub(super) fn pop_frame(&mut self) {
        self.call_stack.pop();
    }

    pub(super) fn snapshot_stack(&self) -> Vec<Frame> {
        let start = self.call_stack.len().saturating_sub(MAX_STACK_DEPTH);
        let mut frames = self.call_stack[start..].to_vec();
        frames.reverse();
        frames
    }

    pub fn set_base_path(&mut self, path: PathBuf) {
        self.base_path = Some(path);
    }

    pub fn get(&self, name: &str) -> Option<Value> {
        self.env.get(name)
    }

    pub fn run(&mut self, program: &Program) -> Result<(), RuntimeError> {
        self.run_internal(program, RunMode::Script).map(|_| ())
    }

    pub fn run_repl(&mut self, program: &Program) -> Result<Option<Value>, RuntimeError> {
        let result = self.run_internal(program, RunMode::Repl);
        self.clear_pending_tasks();
        if self.live_frames() > GC_THRESHOLD {
            let extra_roots: Vec<Value> = result.iter().flatten().cloned().collect();
            self.collect_cycles_with_roots(&extra_roots);
        }
        result
    }

    pub fn live_frames(&self) -> usize {
        self.env.registry().prune_and_count()
    }

    pub(super) fn maybe_collect_garbage(&mut self) {
        if self.live_frames() > GC_THRESHOLD {
            self.collect_cycles_with_roots(&[]);
        }
    }

    fn clear_pending_tasks(&mut self) {
        self.microtasks.clear();
        self.macrotasks.clear();
    }

    fn run_internal(&mut self, program: &Program, mode: RunMode) -> Result<Option<Value>, RuntimeError> {
        self.call_stack.clear();
        self.resolution = if mode == RunMode::Script { resolver::resolve(program) } else { RootResolution::default() };
        self.hoist_functions(&program.items);
        let mut last: Option<Value> = None;
        let mut since_gc = 0usize;
        for stmt in &program.items {
            if mode == RunMode::Repl {
                if let yps_parser::ast::Stmt::Expr { expr, .. } = stmt {
                    self.pending_label.take();
                    let val = self.eval_expr(expr)?;
                    self.drain_microtasks(Span { start: 0, end: 0 })?;
                    last = Some(val);
                    continue;
                } else {
                    last = None;
                }
            }
            let cf_opt = self.exec_stmt(stmt)?;
            if mode == RunMode::Repl {
                self.drain_microtasks(Span { start: 0, end: 0 })?;
            }
            if let Some(cf) = cf_opt {
                match cf {
                    ControlFlow::Return(_) => return Ok(last),
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
            if mode == RunMode::Script {
                since_gc += 1;
                if since_gc >= SCRIPT_GC_INTERVAL {
                    since_gc = 0;
                    self.maybe_collect_garbage();
                }
            }
        }
        self.drive_event_loop(Span { start: 0, end: 0 })?;
        Ok(last)
    }
}

#[cfg(test)]
mod tests;
