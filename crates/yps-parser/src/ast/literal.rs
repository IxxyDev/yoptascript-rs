use crate::ast::Expr;
use yps_lexer::Span;

#[derive(Debug, Clone)]
pub enum Literal {
    Number { raw: String, span: Span },
    String { value: String, span: Span },
    Array { elements: Vec<Expr>, span: Span },
}
