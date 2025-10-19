#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(clippy::missing_const_for_fn)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::match_same_arms)]

use crate::ast::{BinaryOp, Block, Expr, Identifier, Literal, Program, Stmt, UnaryOp};
use yps_lexer::{Diagnostic, KeywordKind, OperatorKind, PunctuationKind, Severity, SourceFile, Span, Token, TokenKind};

pub struct Parser<'a> {
    tokens: &'a [Token],
    source: &'a SourceFile,
    position: usize,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> Parser<'a> {
    pub fn new(tokens: &'a [Token], source: &'a SourceFile) -> Self {
        Self { tokens, source, position: 0, diagnostics: Vec::new() }
    }

    pub fn parse_program(self) -> (Program, Vec<Diagnostic>) {
        let program = Program { items: Vec::new() };
        (program, self.diagnostics)
    }

    fn current(&self) -> &Token {
        self.tokens.get(self.position).or_else(|| self.tokens.last()).expect("Парсеру нужен хотя бы один токен (EOF)")
    }

    fn peek(&self, offset: usize) -> &Token {
        let idx = self.position + offset;
        self.tokens.get(idx).or_else(|| self.tokens.last()).expect("Парсеру нужен хотя бы один токен (EOF)")
    }

    fn previous(&self) -> Option<&Token> {
        if self.position == 0 { None } else { self.tokens.get(self.position - 1) }
    }

    fn advance(&mut self) -> &Token {
        if !self.is_at_end() {
            self.position += 1;
        }
        self.previous().or_else(|| self.tokens.last()).expect("Парсеру нужен хотя бы один токен (EOF)")
    }

    fn is_at_end(&self) -> bool {
        matches!(self.current().kind, TokenKind::Eof)
    }

    fn push_error(&mut self, span: Span, message: impl Into<String>) {
        self.diagnostics.push(Diagnostic { severity: Severity::Error, message: message.into(), span });
    }

    fn synchronize(&mut self) {
        while !self.is_at_end() {
            if matches!(
                self.previous().map(|t| &t.kind),
                Some(TokenKind::Punctuation(
                    yps_lexer::PunctuationKind::Semicolon | yps_lexer::PunctuationKind::RBrace
                ))
            ) {
                return;
            }

            match &self.current().kind {
                TokenKind::Keyword(_) | TokenKind::Punctuation(yps_lexer::PunctuationKind::LBrace) => return,
                _ => {
                    self.advance();
                }
            }
        }
    }
}
