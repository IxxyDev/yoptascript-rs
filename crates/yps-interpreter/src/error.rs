use std::fmt;
use std::rc::Rc;

use yps_lexer::Span;

use crate::value::Value;

pub const MAX_STACK_DEPTH: usize = 64;

#[derive(Debug, Clone)]
pub struct Frame {
    pub name: Rc<str>,
    pub span: Span,
}

#[derive(Debug)]
pub struct RuntimeError {
    pub message: String,
    pub span: Span,
    pub cause: Option<Box<RuntimeError>>,
    pub thrown: Option<Box<Value>>,
    pub stack: Vec<Frame>,
}

impl RuntimeError {
    pub fn new(message: impl Into<String>, span: Span) -> Self {
        Self { message: message.into(), span, cause: None, thrown: None, stack: Vec::new() }
    }

    pub fn thrown(value: Value, span: Span) -> Self {
        Self {
            message: format!("Необработанное исключение: {value}"),
            span,
            cause: None,
            thrown: Some(Box::new(value)),
            stack: Vec::new(),
        }
    }

    pub fn attach_stack(&mut self, stack: Vec<Frame>) {
        if self.stack.is_empty() {
            self.stack = stack;
        }
    }

    pub fn thrown_with_stack(value: Value, span: Span, stack: Vec<Frame>) -> Self {
        let mut err = Self::thrown(value, span);
        err.attach_stack(stack);
        err
    }

    #[must_use]
    pub fn with_cause(mut self, cause: RuntimeError) -> Self {
        self.cause = Some(Box::new(cause));
        self
    }
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Ошибка: {}", self.message)?;
        let mut current = self.cause.as_deref();
        while let Some(c) = current {
            write!(f, "\n  причина: {}", c.message)?;
            current = c.cause.as_deref();
        }
        Ok(())
    }
}
