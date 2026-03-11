use crate::ast::stmt::Block;
use crate::ast::{BinaryOp, Identifier, Literal, PostfixOp, UnaryOp};
use yps_lexer::Span;

#[derive(Debug, Clone)]
pub enum Expr {
    Identifier(Identifier),
    Literal(Literal),

    Unary { op: UnaryOp, expr: Box<Expr>, span: Span },

    Binary { op: BinaryOp, lhs: Box<Expr>, rhs: Box<Expr>, span: Span },

    Assignment { target: Identifier, value: Box<Expr>, span: Span },

    Postfix { op: PostfixOp, expr: Box<Expr>, span: Span },

    Grouping { expr: Box<Expr>, span: Span },

    Call { callee: Box<Expr>, args: Vec<Expr>, span: Span },

    Index { object: Box<Expr>, index: Box<Expr>, span: Span },

    Member { object: Box<Expr>, property: Identifier, span: Span },

    Conditional { condition: Box<Expr>, then_expr: Box<Expr>, else_expr: Box<Expr>, span: Span },

    ArrowFunction { params: Vec<Identifier>, body: Block, span: Span },
}

impl Expr {
    #[allow(dead_code)]
    pub(crate) const fn span(&self) -> Span {
        match self {
            Self::Identifier(id) => id.span,
            Self::Literal(lit) => match lit {
                Literal::Number { span, .. }
                | Literal::String { span, .. }
                | Literal::Boolean { span, .. }
                | Literal::Null { span }
                | Literal::Array { span, .. }
                | Literal::Object { span, .. } => *span,
            },
            Self::Unary { span, .. }
            | Self::Binary { span, .. }
            | Self::Assignment { span, .. }
            | Self::Postfix { span, .. }
            | Self::Grouping { span, .. }
            | Self::Call { span, .. }
            | Self::Index { span, .. }
            | Self::Member { span, .. }
            | Self::Conditional { span, .. }
            | Self::ArrowFunction { span, .. } => *span,
        }
    }
}
