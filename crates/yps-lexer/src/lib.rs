#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub severity: Severity,
    pub message: String,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeywordKind {
    Pachan,
    Sliva,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperatorKind {
    Plus,
    Minus,
    Assign,
    Equals,
    StrictEquals,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PunctuationKind {
    LParen,
    RParen,
    LBrace,
    RBrace,
    Semicolon,
    Comma,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    Identifier,
    Number,
    StringLiteral,
    Keyword(KeywordKind),
    Operator(OperatorKind),
    Punctuation(PunctuationKind),
    Eof,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}
