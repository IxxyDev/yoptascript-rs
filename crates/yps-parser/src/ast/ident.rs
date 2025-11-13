use yps_lexer::Span;

#[derive(Debug, Clone)]
pub struct Identifier {
    pub name: String,
    pub span: Span,
}
