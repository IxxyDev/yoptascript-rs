use super::{Expr, Identifier};
use yps_lexer::Span;

#[derive(Debug, Clone)]
pub enum Pattern {
    Identifier(Identifier),
    Array { elements: Vec<Option<Pattern>>, rest: Option<Box<Pattern>>, span: Span },
    Object { properties: Vec<ObjectPatternProp>, rest: Option<Box<Pattern>>, span: Span },
    Default { pattern: Box<Pattern>, default: Box<Expr>, span: Span },
}

impl Pattern {
    #[must_use]
    pub const fn span(&self) -> Span {
        match self {
            Self::Identifier(ident) => ident.span,
            Self::Array { span, .. } | Self::Object { span, .. } | Self::Default { span, .. } => *span,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ObjectPatternProp {
    pub key: Identifier,
    pub value: Option<Pattern>,
    pub span: Span,
}
