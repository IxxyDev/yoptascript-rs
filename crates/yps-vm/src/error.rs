use std::fmt;

use yps_lexer::Span;

#[derive(Debug, Clone)]
pub struct CompileError {
    pub message: String,
    pub span: Span,
}

impl CompileError {
    pub fn new(message: impl Into<String>, span: Span) -> Self {
        Self { message: message.into(), span }
    }
}

impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for CompileError {}

#[derive(Debug, Clone)]
pub struct VmError {
    pub message: String,
    pub span: Span,
    pub thrown: Option<Box<crate::value::Value>>,
}

impl VmError {
    pub fn new(message: impl Into<String>, span: Span) -> Self {
        Self { message: message.into(), span, thrown: None }
    }

    pub fn with_thrown(mut self, value: crate::value::Value) -> Self {
        self.thrown = Some(Box::new(value));
        self
    }
}

impl fmt::Display for VmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for VmError {}

#[derive(Debug, Clone)]
pub enum ExecError {
    Compile(CompileError),
    Runtime(VmError),
}

impl ExecError {
    pub fn span(&self) -> Span {
        match self {
            ExecError::Compile(e) => e.span,
            ExecError::Runtime(e) => e.span,
        }
    }
}

impl fmt::Display for ExecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExecError::Compile(e) => write!(f, "{e}"),
            ExecError::Runtime(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for ExecError {}

impl From<CompileError> for ExecError {
    fn from(e: CompileError) -> Self {
        ExecError::Compile(e)
    }
}

impl From<VmError> for ExecError {
    fn from(e: VmError) -> Self {
        ExecError::Runtime(e)
    }
}
