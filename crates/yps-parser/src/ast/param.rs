use crate::ast::{Expr, Identifier};

#[derive(Debug, Clone)]
pub struct Param {
    pub name: Identifier,
    pub default: Option<Expr>,
    pub is_rest: bool,
}
