use crate::ast::BinaryOp;

pub const TERNARY_PRECEDENCE: u8 = 2;
pub const UNARY_PRECEDENCE: u8 = 16;

pub fn binary_precedence(op: BinaryOp) -> u8 {
    match op {
        BinaryOp::Assign
        | BinaryOp::PlusAssign
        | BinaryOp::MinusAssign
        | BinaryOp::MulAssign
        | BinaryOp::DivAssign
        | BinaryOp::ExpAssign
        | BinaryOp::NullishAssign
        | BinaryOp::AndAssign
        | BinaryOp::OrAssign
        | BinaryOp::ModAssign
        | BinaryOp::BitAndAssign
        | BinaryOp::BitOrAssign
        | BinaryOp::BitXorAssign
        | BinaryOp::ShlAssign
        | BinaryOp::ShrAssign
        | BinaryOp::UshrAssign => 1,
        BinaryOp::Or => 3,
        BinaryOp::NullishCoalescing => 4,
        BinaryOp::And => 5,
        BinaryOp::BitOr => 6,
        BinaryOp::BitXor => 7,
        BinaryOp::BitAnd => 8,
        BinaryOp::Equals | BinaryOp::StrictEquals | BinaryOp::NotEquals | BinaryOp::StrictNotEquals => 9,
        BinaryOp::Less
        | BinaryOp::Greater
        | BinaryOp::LessOrEqual
        | BinaryOp::GreaterOrEqual
        | BinaryOp::Instanceof
        | BinaryOp::In => 10,
        BinaryOp::Pipeline => 11,
        BinaryOp::LeftShift | BinaryOp::RightShift | BinaryOp::UnsignedRightShift => 12,
        BinaryOp::Add | BinaryOp::Sub => 13,
        BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod => 14,
        BinaryOp::Exp => 15,
    }
}

pub fn binary_is_right_assoc(op: BinaryOp) -> bool {
    matches!(
        op,
        BinaryOp::Exp
            | BinaryOp::Assign
            | BinaryOp::PlusAssign
            | BinaryOp::MinusAssign
            | BinaryOp::MulAssign
            | BinaryOp::DivAssign
            | BinaryOp::ExpAssign
            | BinaryOp::NullishAssign
            | BinaryOp::AndAssign
            | BinaryOp::OrAssign
            | BinaryOp::ModAssign
            | BinaryOp::BitAndAssign
            | BinaryOp::BitOrAssign
            | BinaryOp::BitXorAssign
            | BinaryOp::ShlAssign
            | BinaryOp::ShrAssign
            | BinaryOp::UshrAssign
    )
}
