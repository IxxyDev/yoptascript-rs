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

    Call { callee: Box<Expr>, args: Vec<Expr>, span: Span },
}

impl Expr {
    #[allow(dead_code)]
    pub(crate) const fn span(&self) -> Span {
        match self {
            Self::Identifier(id) => id.span,
            Self::Literal(lit) => match lit {
                Literal::Number { span, .. } | Literal::String { span, .. } => *span,
            },
            Self::Unary { span, .. }
            | Self::Binary { span, .. }
            | Self::Assignment { span, .. }
            | Self::Grouping { span, .. }
            | Self::Call { span, .. } => *span,
        }
    }
}
