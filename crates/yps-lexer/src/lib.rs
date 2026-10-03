mod diagnostic;
mod lexer;
mod source;
mod sources;
mod span;
mod token;
mod trivia;

pub use diagnostic::{Diagnostic, Severity};
pub use lexer::{KEYWORDS, Lexer};
pub use source::SourceFile;
pub use sources::Sources;
pub use span::Span;
pub use token::{KeywordKind, OperatorKind, PunctuationKind, Token, TokenKind};
pub use trivia::{Trivia, TriviaKind};
