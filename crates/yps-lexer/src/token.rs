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
    Gyy,
    Uchastkoviy,
    YasenHuy,
    Vilkoyvglaz,
    Ilivzhopuraz,
    Potreshchim,
    Go,
    Hare,
    Dvigay,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperatorKind {
    Plus,
    Minus,
    Multiply,
    Divide,
    Modulo,
    Assign,
    Equals,
    StrictEquals,
    NotEquals,
    StrictNotEquals,
    Less,
    Greater,
    LessOrEqual,
    GreaterOrEqual,
    And,
    Or,
    Not,
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
