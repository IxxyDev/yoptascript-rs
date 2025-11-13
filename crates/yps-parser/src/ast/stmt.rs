use crate::ast::{Expr, Identifier};
use yps_lexer::Span;

#[derive(Debug, Clone)]
pub enum Stmt {
    VarDecl { name: Identifier, init: Expr, span: Span },
    Expr { expr: Expr, span: Span },
    Block(Block),
    Empty { span: Span },
    If { condition: Expr, then_branch: Box<Stmt>, else_branch: Option<Box<Stmt>>, span: Span },
    While { condition: Expr, body: Box<Stmt>, span: Span },
    For { init: Option<Box<Stmt>>, condition: Option<Expr>, update: Option<Expr>, body: Box<Stmt>, span: Span },
    Break { span: Span },
    Continue { span: Span },
}

#[derive(Debug, Clone)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub span: Span,
}
