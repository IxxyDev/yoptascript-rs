#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Plus,
    Minus,
    Not,
    BitwiseNot,
    Typeof,
    Delete,
    Void,
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
    AndAssign,
    OrAssign,
    Pipeline,
    Instanceof,
    In,
    BitAnd,
    BitOr,
    BitXor,
    LeftShift,
    RightShift,
    UnsignedRightShift,
    ModAssign,
    BitAndAssign,
    BitOrAssign,
    BitXorAssign,
    ShlAssign,
    ShrAssign,
    UshrAssign,
}

impl BinaryOp {
    #[must_use]
    pub const fn is_compound_assign(self) -> bool {
        matches!(
            self,
            Self::PlusAssign
                | Self::MinusAssign
                | Self::MulAssign
                | Self::DivAssign
                | Self::ExpAssign
                | Self::ModAssign
                | Self::NullishAssign
                | Self::AndAssign
                | Self::OrAssign
                | Self::BitAndAssign
                | Self::BitOrAssign
                | Self::BitXorAssign
                | Self::ShlAssign
                | Self::ShrAssign
                | Self::UshrAssign
        )
    }

    #[must_use]
    pub const fn is_assign(self) -> bool {
        matches!(self, Self::Assign) || self.is_compound_assign()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostfixOp {
    Increment,
    Decrement,
}

#[cfg(test)]
mod tests {
    use super::BinaryOp;
    use crate::precedence::{ASSIGN_PRECEDENCE, binary_precedence};

    const ALL: [BinaryOp; 42] = [
        BinaryOp::Add,
        BinaryOp::Sub,
        BinaryOp::Mul,
        BinaryOp::Div,
        BinaryOp::Mod,
        BinaryOp::Exp,
        BinaryOp::Assign,
        BinaryOp::PlusAssign,
        BinaryOp::MinusAssign,
        BinaryOp::MulAssign,
        BinaryOp::DivAssign,
        BinaryOp::ExpAssign,
        BinaryOp::Equals,
        BinaryOp::StrictEquals,
        BinaryOp::NotEquals,
        BinaryOp::StrictNotEquals,
        BinaryOp::Less,
        BinaryOp::Greater,
        BinaryOp::LessOrEqual,
        BinaryOp::GreaterOrEqual,
        BinaryOp::And,
        BinaryOp::Or,
        BinaryOp::NullishCoalescing,
        BinaryOp::NullishAssign,
        BinaryOp::AndAssign,
        BinaryOp::OrAssign,
        BinaryOp::Pipeline,
        BinaryOp::Instanceof,
        BinaryOp::In,
        BinaryOp::BitAnd,
        BinaryOp::BitOr,
        BinaryOp::BitXor,
        BinaryOp::LeftShift,
        BinaryOp::RightShift,
        BinaryOp::UnsignedRightShift,
        BinaryOp::ModAssign,
        BinaryOp::BitAndAssign,
        BinaryOp::BitOrAssign,
        BinaryOp::BitXorAssign,
        BinaryOp::ShlAssign,
        BinaryOp::ShrAssign,
        BinaryOp::UshrAssign,
    ];

    #[test]
    fn assign_predicates_agree_with_precedence() {
        for op in ALL {
            assert_eq!(op.is_assign(), binary_precedence(op) == ASSIGN_PRECEDENCE, "{op:?}");
            assert_eq!(op.is_compound_assign(), op.is_assign() && op != BinaryOp::Assign, "{op:?}");
        }
    }
}
