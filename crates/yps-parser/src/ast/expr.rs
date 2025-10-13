use crate::ast::{BinaryOp, Identifier, Literal, UnaryOp};
use yps_lexer::Span;

#[derive(Debug, Clone)]
pub enum Expr {
    Identifier(Identifier),
    Literal(Literal),

    Unary { op: UnaryOp, expr: Box<Expr>, span: Span },

    Binary { op: BinaryOp, lhs: Box<Expr>, rhs: Box<Expr>, span: Span },

    Assignment { target: Identifier, value: Box<Expr>, span: Span },

    Grouping { expr: Box<Expr>, span: Span },
}
