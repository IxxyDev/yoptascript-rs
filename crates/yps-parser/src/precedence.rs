use crate::ast::BinaryOp;

pub const TERNARY_PRECEDENCE: u8 = 2;
pub const UNARY_PRECEDENCE: u8 = 12;

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
        | BinaryOp::OrAssign => 1,
        BinaryOp::Or => 3,
        BinaryOp::NullishCoalescing => 4,
        BinaryOp::And => 5,
        BinaryOp::Equals | BinaryOp::StrictEquals | BinaryOp::NotEquals | BinaryOp::StrictNotEquals => 6,
        BinaryOp::Less
        | BinaryOp::Greater
        | BinaryOp::LessOrEqual
        | BinaryOp::GreaterOrEqual
        | BinaryOp::Instanceof
        | BinaryOp::In => 7,
        BinaryOp::Pipeline => 8,
        BinaryOp::Add | BinaryOp::Sub => 9,
        BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod => 10,
        BinaryOp::Exp => 11,
    }
}

pub fn binary_is_right_assoc(op: BinaryOp) -> bool {
    matches!(op, BinaryOp::Exp)
}
