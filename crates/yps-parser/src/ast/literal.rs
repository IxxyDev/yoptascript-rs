use crate::ast::{Expr, Identifier};
use yps_lexer::Span;

#[derive(Debug, Clone)]
pub enum PropKey {
    Identifier(Identifier),
    Computed(Expr),
}

#[derive(Debug, Clone)]
pub enum ObjectEntry {
    Property { key: PropKey, value: Expr },
    Spread(Expr),
}

#[derive(Debug, Clone)]
pub enum Literal {
    Number { raw: String, span: Span },
    String { value: String, span: Span },
    Boolean { value: bool, span: Span },
    Null { span: Span },
    Undefined { span: Span },
    Array { elements: Vec<Expr>, span: Span },
    Object { entries: Vec<ObjectEntry>, span: Span },
}
