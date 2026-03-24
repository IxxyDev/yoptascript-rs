#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Plus,
    Minus,
    Not,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Exp,
    Assign,
    PlusAssign,
    MinusAssign,
    MulAssign,
    DivAssign,
    ExpAssign,
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
    NullishCoalescing,
    NullishAssign,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostfixOp {
    Increment,
    Decrement,
}
