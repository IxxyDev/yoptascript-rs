use crate::Span;

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
    Yopta,
    Otvechayu,
    Pravda,
    Lozh,
    Nol,
    Try,
    Catch,
    Finally,
    Throw,
    Switch,
    Case,
    Default,
    DoWhile,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperatorKind {
    Plus,
    Minus,
    Multiply,
    Divide,
    Modulo,
    Assign,
    PlusAssign,
    MinusAssign,
    MulAssign,
    DivAssign,
    Increment,
    Decrement,
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
    LBracket,
    RBracket,
    Semicolon,
    Comma,
    Colon,
    Dot,
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
