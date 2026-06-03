use crate::Span;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriviaKind {
    LineComment,
    BlockComment,
}

#[derive(Debug, Clone)]
pub struct Trivia {
    pub kind: TriviaKind,
    pub text: String,
    pub span: Span,
}
