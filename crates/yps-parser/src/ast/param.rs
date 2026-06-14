use crate::ast::{Expr, Identifier, Pattern};

#[derive(Debug, Clone)]
pub struct Param {
    pub name: Identifier,
    pub default: Option<Expr>,
    pub is_rest: bool,
    pub pattern: Option<Pattern>,
}
