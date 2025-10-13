use crate::ast::Program;
use yps_lexer::{Diagnostic, Token, TokenKind};

pub struct Parser<'a> {
  tokens: &'a [Token],
  position: usize,
  diagnostics: Vec<Diagnostic>,
}

impl<'a> Parser<'a> {
  pub fn new(tokens: &'a [Token]) -> Self {
    Self {
      tokens,
      position: 0,
      diagnostics: Vec::new(),
    }
  }

  pub fn parse_program(mut self) -> (Program, Vec<Diagnostic>) {
    let program = Program { items: Vec::new() };
    (program, self.diagnostics)
  }
}