use yps_lexer::Span;

#[derive(Debug, Clone)]
pub enum Literal {
    Number { raw: String, span: Span },
    String { value: String, span: Span },
    // TODO: Other data types
}
