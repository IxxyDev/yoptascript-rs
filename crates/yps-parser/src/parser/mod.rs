use std::rc::Rc;

use crate::ast::{
    BinaryOp, Block, ClassMember, Expr, Identifier, Literal, ObjectEntry, ObjectPatternProp, Param, Pattern, PostfixOp,
    Program, PropKey, Stmt, SwitchCase, TemplatePart, TemplateQuasi, UnaryOp,
};
use yps_lexer::{Diagnostic, KeywordKind, OperatorKind, PunctuationKind, Severity, SourceFile, Span, Token, TokenKind};

use crate::precedence::{TERNARY_PRECEDENCE, UNARY_PRECEDENCE};

const MAX_PARSE_DEPTH: usize = 200;
const MAX_CHAIN_LEN: usize = 10_000;
const STACK_RED_ZONE: usize = 128 * 1024;
const STACK_GROW_SIZE: usize = 4 * 1024 * 1024;

pub struct Parser<'a> {
    tokens: &'a [Token],
    source: &'a SourceFile,
    position: usize,
    diagnostics: Vec<Diagnostic>,
    unexpected_eof: bool,
    depth: usize,
}

impl<'a> Parser<'a> {
    pub fn new(tokens: &'a [Token], source: &'a SourceFile) -> Self {
        assert!(
            matches!(tokens.last().map(|t| &t.kind), Some(TokenKind::Eof)),
            "Parser::new требует, чтобы tokens заканчивался TokenKind::Eof"
        );
        Self { tokens, source, position: 0, diagnostics: Vec::new(), unexpected_eof: false, depth: 0 }
    }

    fn expect_punct(&mut self, kind: PunctuationKind, msg: &str) -> Result<Span, ()> {
        if !matches!(&self.current().kind, TokenKind::Punctuation(k) if *k == kind) {
            let span = self.current().span;
            self.push_error(span, msg);
            return Err(());
        }
        let span = self.current().span;
        self.advance();
        Ok(span)
    }

    fn enter_depth(&mut self) -> Result<(), ()> {
        if self.depth >= MAX_PARSE_DEPTH {
            let span = self.current().span;
            self.push_error(span, "Слишком глубокая вложенность конструкций");
            return Err(());
        }
        self.depth += 1;
        Ok(())
    }

    pub fn parse_program_extended(mut self) -> (Program, Vec<Diagnostic>, bool) {
        let mut items = Vec::new();

        while !self.is_at_end() {
            let pos_before = self.position;
            match self.parse_statement() {
                Ok(stmt) => items.push(stmt),
                Err(()) => {
                    self.synchronize();
                    if self.position == pos_before && !self.is_at_end() {
                        self.advance();
                    }
                }
            }
        }

        let program = Program { items };
        let unexpected_eof = self.unexpected_eof;
        (program, self.diagnostics, unexpected_eof)
    }

    pub fn parse_program(self) -> (Program, Vec<Diagnostic>) {
        let (program, diagnostics, _) = self.parse_program_extended();
        (program, diagnostics)
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
        if self.diagnostics.is_empty()
            && let Some(eof_tok) = self.tokens.last()
            && span.start >= eof_tok.span.start
        {
            self.unexpected_eof = true;
        }
        self.diagnostics.push(Diagnostic { severity: Severity::Error, message: message.into(), span });
    }

    fn skip_to_for_recovery(&mut self) {
        let mut depth = 1i32;
        while !self.is_at_end() && depth > 0 {
            match self.current().kind {
                TokenKind::Punctuation(PunctuationKind::LParen) => depth += 1,
                TokenKind::Punctuation(PunctuationKind::RParen) => depth -= 1,
                _ => {}
            }
            self.advance();
        }
        if matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::LBrace)) {
            let mut brace_depth = 0i32;
            loop {
                match self.current().kind {
                    TokenKind::Punctuation(PunctuationKind::LBrace) => brace_depth += 1,
                    TokenKind::Punctuation(PunctuationKind::RBrace) => {
                        brace_depth -= 1;
                        if brace_depth == 0 {
                            self.advance();
                            break;
                        }
                    }
                    _ => {}
                }
                if self.is_at_end() {
                    break;
                }
                self.advance();
            }
        }
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
                TokenKind::Keyword(_)
                | TokenKind::Punctuation(
                    yps_lexer::PunctuationKind::LBrace
                    | yps_lexer::PunctuationKind::RBrace
                    | yps_lexer::PunctuationKind::RParen
                    | yps_lexer::PunctuationKind::RBracket,
                ) => return,
                _ => {
                    self.advance();
                }
            }
        }
    }
}

mod class;
mod expr;
mod functions;
mod literals;
mod modules;
mod patterns;
mod stmt;

#[cfg(test)]
mod tests;
