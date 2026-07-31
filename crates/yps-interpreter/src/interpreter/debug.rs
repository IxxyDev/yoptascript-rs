use std::rc::Rc;

use yps_lexer::Span;

use crate::error::{Frame, RuntimeError};
use crate::value::Value;

use super::Interpreter;

pub const DEBUG_TERMINATED: &str = "Отладка прервана";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugAction {
    Continue,
    StepIn,
    StepOver,
    StepOut,
    Terminate,
}

pub struct DebugEvent<'a> {
    pub span: Span,
    pub depth: usize,
    pub step_complete: bool,
    pub interp: &'a Interpreter,
}

pub trait DebugHook {
    /// `None` means "no decision taken, keep the pending mode and its depth baseline";
    /// `Some(action)` re-arms the mode relative to the depth of the current statement.
    fn on_statement(&mut self, event: DebugEvent<'_>) -> Option<DebugAction>;
}

impl Interpreter {
    pub fn set_debug_hook(&mut self, hook: Box<dyn DebugHook>) {
        self.debug_globals_baseline = self.global_root.borrow().debug_bindings().into_iter().map(|(n, _)| n).collect();
        self.debug_hook = Some(hook);
        self.debug_action = DebugAction::StepIn;
        self.debug_depth = 0;
    }

    pub fn set_debug_resume(&mut self, action: DebugAction) {
        self.debug_action = action;
        self.debug_depth = self.call_stack.len();
    }

    pub fn debug_call_stack(&self) -> &[Frame] {
        &self.call_stack
    }

    /// Bindings of the active scope and of every enclosing scope, innermost first.
    /// Top-level script variables live in the global frame next to every builtin, so that
    /// frame is filtered against the baseline captured when the hook was installed.
    pub fn debug_scope_chain(&self) -> Vec<Vec<(String, Value)>> {
        let mut out = Vec::new();
        let mut current = Some(self.env.snapshot());
        while let Some(frame) = current {
            let is_global = Rc::ptr_eq(&frame, &self.global_root);
            let borrowed = frame.borrow();
            let mut bindings = borrowed.debug_bindings();
            if is_global {
                bindings.retain(|(name, _)| !self.debug_globals_baseline.contains(name));
            }
            out.push(bindings);
            current = borrowed.debug_parent();
        }
        out
    }

    /// Visible bindings at the current execution point, innermost scope winning on shadowing.
    pub fn debug_visible_locals(&self) -> Vec<(String, Value)> {
        let mut seen = std::collections::HashSet::new();
        let mut out = Vec::new();
        for scope in self.debug_scope_chain() {
            for (name, value) in scope {
                if seen.insert(name.clone()) {
                    out.push((name, value));
                }
            }
        }
        out
    }

    pub(super) fn debug_before_stmt(&mut self, span: Span) -> Result<(), RuntimeError> {
        let depth = self.call_stack.len();
        let step_complete = match self.debug_action {
            DebugAction::Continue => false,
            DebugAction::StepIn => true,
            // Step granularity is expressed purely through call-stack depth: step-over resumes
            // at the same depth or shallower, step-out strictly shallower.
            DebugAction::StepOver => depth <= self.debug_depth,
            DebugAction::StepOut => depth < self.debug_depth,
            DebugAction::Terminate => return Err(RuntimeError::new(DEBUG_TERMINATED, span)),
        };

        let Some(mut hook) = self.debug_hook.take() else {
            return Ok(());
        };
        let decision = hook.on_statement(DebugEvent { span, depth, step_complete, interp: self });
        self.debug_hook = Some(hook);
        if let Some(action) = decision {
            self.debug_action = action;
            self.debug_depth = depth;
            if action == DebugAction::Terminate {
                return Err(RuntimeError::new(DEBUG_TERMINATED, span));
            }
        }
        Ok(())
    }
}
