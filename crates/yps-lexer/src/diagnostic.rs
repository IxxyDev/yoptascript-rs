use std::fmt;

use crate::Span;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Error => "Ошибка",
            Self::Warning => "Предупреждение",
        })
    }
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub severity: Severity,
    pub message: String,
    pub span: Span,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_displays_in_russian() {
        assert_eq!(Severity::Error.to_string(), "Ошибка");
        assert_eq!(Severity::Warning.to_string(), "Предупреждение");
    }
}
