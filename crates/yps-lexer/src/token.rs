use crate::Span;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeywordKind {
    Pachan,
    Sliva,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperatorKind {
    Plus,
    Minus,
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

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}
