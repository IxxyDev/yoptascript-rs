use crate::ast::{Expr, Identifier};
use yps_lexer::Span;

#[derive(Debug, Clone)]
pub struct ObjectProperty {
    pub key: Identifier,
    pub value: Expr,
}

#[derive(Debug, Clone)]
pub enum Literal {
    Number { raw: String, span: Span },
    String { value: String, span: Span },
    Array { elements: Vec<Expr>, span: Span },
    Object { properties: Vec<ObjectProperty>, span: Span },
}
