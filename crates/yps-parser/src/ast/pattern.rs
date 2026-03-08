use super::Identifier;
use yps_lexer::Span;

#[derive(Debug, Clone)]
pub enum Pattern {
    Identifier(Identifier),
    Array { elements: Vec<Option<Pattern>>, rest: Option<Box<Pattern>>, span: Span },
    Object { properties: Vec<ObjectPatternProp>, rest: Option<Box<Pattern>>, span: Span },
}

#[derive(Debug, Clone)]
pub struct ObjectPatternProp {
    pub key: Identifier,
    pub value: Option<Pattern>,
    pub span: Span,
}
