mod span;
mod diagnostic;
mod token;
mod lexer;

pub use span::Span;
pub use diagnostic::{Diagnostic, Severity};
pub use token::{
    KeywordKind,
    OperatorKind,
    PunctuationKind,
    Token,
    TokenKind,
};
pub use lexer::Lexer;

#[cfg(test)]
mod tests;
