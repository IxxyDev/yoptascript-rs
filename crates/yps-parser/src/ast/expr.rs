use crate::ast::stmt::Block;
use crate::ast::{BinaryOp, Identifier, Literal, Param, PostfixOp, UnaryOp};
use yps_lexer::Span;

#[derive(Debug, Clone)]
pub enum TemplatePart {
    Str(String),
    Expr(Box<Expr>),
}

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

    OptionalMember { object: Box<Expr>, property: Identifier, span: Span },

    OptionalIndex { object: Box<Expr>, index: Box<Expr>, span: Span },

    OptionalCall { callee: Box<Expr>, args: Vec<Expr>, span: Span },

    Conditional { condition: Box<Expr>, then_expr: Box<Expr>, else_expr: Box<Expr>, span: Span },

    ArrowFunction { params: Vec<Param>, body: Block, is_async: bool, span: Span },

    TemplateLiteral { parts: Vec<TemplatePart>, span: Span },

    Spread { expr: Box<Expr>, span: Span },

    This { span: Span },

    New { callee: Box<Expr>, args: Vec<Expr>, span: Span },

    Super { span: Span },

    Yield { argument: Option<Box<Expr>>, delegate: bool, span: Span },

    Await { argument: Box<Expr>, span: Span },
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
                | Literal::Undefined { span }
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
            | Self::OptionalMember { span, .. }
            | Self::OptionalIndex { span, .. }
            | Self::OptionalCall { span, .. }
            | Self::Conditional { span, .. }
            | Self::ArrowFunction { span, .. }
            | Self::TemplateLiteral { span, .. }
            | Self::Spread { span, .. }
            | Self::This { span, .. }
            | Self::New { span, .. }
            | Self::Super { span, .. }
            | Self::Yield { span, .. }
            | Self::Await { span, .. } => *span,
        }
    }
}
