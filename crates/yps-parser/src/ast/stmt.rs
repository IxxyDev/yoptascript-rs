use crate::ast::{Expr, Indetifier};
use yps_lexer::Span;

#[derive(Debug, Clone)]
pub enum Stmt {
  VarDecl {
    name: Indetifier,
    init: Expr,
    span: Span,
  },
  Expr {
    expr: Expr,
    span: Span,
  },
  Block(Block),
  Empty {
    span: Span,
  }
}

#[derive(Debug, Clone)]
pub struct Block {
  pub stmts: Vec<Stmt>,
  pub span: Span,
}