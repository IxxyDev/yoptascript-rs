use crate::ast::Stmt;

#[derive(Debug, Clone)]
pub struct Program {
    pub items: Vec<Stmt>,
}
