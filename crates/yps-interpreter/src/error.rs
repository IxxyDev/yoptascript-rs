use std::fmt;

use yps_lexer::Span;

#[derive(Debug)]
pub struct RuntimeError {
    pub message: String,
    pub span: Span,
    pub cause: Option<Box<RuntimeError>>,
}

impl RuntimeError {
    pub fn new(message: impl Into<String>, span: Span) -> Self {
        Self { message: message.into(), span, cause: None }
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
