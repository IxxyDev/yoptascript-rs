#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(clippy::missing_const_for_fn)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::match_same_arms)]

use crate::ast::{BinaryOp, Block, Expr, Identifier, Literal, Program, Stmt, UnaryOp};
use yps_lexer::{Diagnostic, KeywordKind, OperatorKind, PunctuationKind, Severity, SourceFile, Span, Token, TokenKind};

const UNARY_PRECEDENCE: u8 = 8;

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

    fn parse_primary(&mut self) -> Result<Expr, ()> {
        match &self.current().kind {
            TokenKind::Number => Ok(self.parse_number()),
            TokenKind::StringLiteral => Ok(self.parse_string()),
            TokenKind::Identifier => self.parse_identifier().map(Expr::Identifier),
            TokenKind::Punctuation(PunctuationKind::LParen) => self.parse_grouping(),
            _ => {
                let span = self.current().span;
                self.push_error(span, format!("Неожиданный токен: {:?}", self.current().kind));
                Err(())
            }
        }
    }

    fn parse_number(&mut self) -> Expr {
        let span = self.current().span;
        let raw = self.source.slice(span).to_string();
        self.advance();
        Expr::Literal(Literal::Number { raw, span })
    }

    fn parse_string(&mut self) -> Expr {
        let span = self.current().span;
        let raw = self.source.slice(span);
        let value = raw[1..raw.len() - 1].to_string();
        self.advance();
        Expr::Literal(Literal::String { value, span })
    }

    fn parse_identifier(&mut self) -> Result<Identifier, ()> {
        if !matches!(self.current().kind, TokenKind::Identifier) {
            let span = self.current().span;
            self.push_error(span, "Ожидался идентификатор");
            return Err(());
        }

        let span = self.current().span;
        let name = self.source.slice(span).to_string();
        self.advance();
        Ok(Identifier { name, span })
    }

    fn parse_grouping(&mut self) -> Result<Expr, ()> {
        let start = self.current().span.start;
        self.advance();

        let expr = self.parse_expr()?;

        if !matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::RParen)) {
            let span = self.current().span;
            self.push_error(span, "Ожидался ')'");
            return Err(());
        }
        let end = self.current().span.end;
        self.advance();

        Ok(Expr::Grouping { expr: Box::new(expr), span: Span { start, end } })
    }

    fn parse_expr(&mut self) -> Result<Expr, ()> {
        self.parse_expression_with_precedence(0)
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

    fn parse_expression_with_precedence(&mut self, min_precedence: u8) -> Result<Expr, ()> {
        let mut lhs = self.parse_prefix()?;

        loop {
            let Some((op, precedence)) = self.try_parse_binary_op() else {
                break;
            };

            if precedence < min_precedence {
                break;
            }

            self.advance();

            let rhs = self.parse_expression_with_precedence(precedence + 1)?;

            let start = lhs.span().start;
            let end = rhs.span().end;
            lhs = Expr::Binary { op, lhs: Box::new(lhs), rhs: Box::new(rhs), span: Span { start, end } };
        }

        Ok(lhs)
    }

    fn parse_prefix(&mut self) -> Result<Expr, ()> {
        match &self.current().kind {
            TokenKind::Operator(OperatorKind::Plus) => {
                let start = self.current().span.start;
                self.advance();
                let expr = self.parse_expression_with_precedence(UNARY_PRECEDENCE)?;
                let end = expr.span().end;
                Ok(Expr::Unary { op: UnaryOp::Plus, expr: Box::new(expr), span: Span { start, end } })
            }
            TokenKind::Operator(OperatorKind::Minus) => {
                let start = self.current().span.start;
                self.advance();
                let expr = self.parse_expression_with_precedence(UNARY_PRECEDENCE)?;
                let end = expr.span().end;
                Ok(Expr::Unary { op: UnaryOp::Minus, expr: Box::new(expr), span: Span { start, end } })
            }
            TokenKind::Operator(OperatorKind::Not) => {
                let start = self.current().span.start;
                self.advance();
                let expr = self.parse_expression_with_precedence(UNARY_PRECEDENCE)?;
                let end = expr.span().end;
                Ok(Expr::Unary { op: UnaryOp::Not, expr: Box::new(expr), span: Span { start, end } })
            }
            _ => self.parse_primary(),
        }
    }

    fn try_parse_binary_op(&self) -> Option<(BinaryOp, u8)> {
        let TokenKind::Operator(op_kind) = &self.current().kind else {
            return None;
        };

        match op_kind {
            OperatorKind::Assign => Some((BinaryOp::Assign, 1)),
            OperatorKind::Or => Some((BinaryOp::Or, 2)),
            OperatorKind::And => Some((BinaryOp::And, 3)),
            OperatorKind::Equals => Some((BinaryOp::Equals, 4)),
            OperatorKind::StrictEquals => Some((BinaryOp::StrictEquals, 4)),
            OperatorKind::NotEquals => Some((BinaryOp::NotEquals, 4)),
            OperatorKind::StrictNotEquals => Some((BinaryOp::StrictNotEquals, 4)),
            OperatorKind::Less => Some((BinaryOp::Less, 5)),
            OperatorKind::Greater => Some((BinaryOp::Greater, 5)),
            OperatorKind::LessOrEqual => Some((BinaryOp::LessOrEqual, 5)),
            OperatorKind::GreaterOrEqual => Some((BinaryOp::GreaterOrEqual, 5)),
            OperatorKind::Plus => Some((BinaryOp::Add, 6)),
            OperatorKind::Minus => Some((BinaryOp::Sub, 6)),
            OperatorKind::Multiply => Some((BinaryOp::Mul, 7)),
            OperatorKind::Divide => Some((BinaryOp::Div, 7)),
            OperatorKind::Modulo => Some((BinaryOp::Mod, 7)),
            OperatorKind::Not => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_expr_from_source(src: &str) -> Result<Expr, Vec<Diagnostic>> {
        let source = SourceFile::new("test.yop".to_string(), src.to_string());
        let lexer = yps_lexer::Lexer::new(&source);
        let (tokens, lex_diags) = lexer.tokenize();

        if !lex_diags.is_empty() {
            return Err(lex_diags);
        }

        let mut parser = Parser::new(&tokens, &source);
        match parser.parse_expr() {
            Ok(expr) => Ok(expr),
            Err(()) => Err(parser.diagnostics),
        }
    }

    #[test]
    fn test_parse_number() {
        let expr = parse_expr_from_source("42").unwrap();
        assert!(matches!(expr, Expr::Literal(Literal::Number { .. })));
    }

    #[test]
    fn test_parse_string() {
        let expr = parse_expr_from_source("\"hello\"").unwrap();
        assert!(matches!(expr, Expr::Literal(Literal::String { .. })));
    }

    #[test]
    fn test_parse_identifier() {
        let expr = parse_expr_from_source("foo").unwrap();
        assert!(matches!(expr, Expr::Identifier(_)));
    }

    #[test]
    fn test_parse_grouping() {
        let expr = parse_expr_from_source("(5)").unwrap();
        assert!(matches!(expr, Expr::Grouping { .. }));
    }

    #[test]
    fn test_parse_unary_minus() {
        let expr = parse_expr_from_source("-5").unwrap();
        match expr {
            Expr::Unary { op, .. } => assert_eq!(op, UnaryOp::Minus),
            _ => panic!("Expected Unary expression"),
        }
    }

    #[test]
    fn test_parse_unary_plus() {
        let expr = parse_expr_from_source("+5").unwrap();
        match expr {
            Expr::Unary { op, .. } => assert_eq!(op, UnaryOp::Plus),
            _ => panic!("Expected Unary expression"),
        }
    }

    #[test]
    fn test_parse_unary_not() {
        let expr = parse_expr_from_source("!true").unwrap();
        match expr {
            Expr::Unary { op, .. } => assert_eq!(op, UnaryOp::Not),
            _ => panic!("Expected Unary expression"),
        }
    }

    #[test]
    fn test_parse_binary_add() {
        let expr = parse_expr_from_source("2 + 3").unwrap();
        match expr {
            Expr::Binary { op, .. } => assert_eq!(op, BinaryOp::Add),
            _ => panic!("Expected Binary expression"),
        }
    }

    #[test]
    fn test_parse_binary_multiply() {
        let expr = parse_expr_from_source("2 * 3").unwrap();
        match expr {
            Expr::Binary { op, .. } => assert_eq!(op, BinaryOp::Mul),
            _ => panic!("Expected Binary expression"),
        }
    }

    #[test]
    fn test_precedence_mul_over_add() {
        let expr = parse_expr_from_source("2 + 3 * 4").unwrap();
        match expr {
            Expr::Binary { op: BinaryOp::Add, lhs, rhs, .. } => {
                assert!(matches!(*lhs, Expr::Literal(Literal::Number { .. })));
                assert!(matches!(*rhs, Expr::Binary { op: BinaryOp::Mul, .. }));
            }
            _ => panic!("Expected Add at top level with Mul on right"),
        }
    }

    #[test]
    fn test_precedence_parentheses() {
        let expr = parse_expr_from_source("(2 + 3) * 4").unwrap();
        match expr {
            Expr::Binary { op: BinaryOp::Mul, lhs, rhs, .. } => {
                assert!(matches!(*lhs, Expr::Grouping { .. }));
                assert!(matches!(*rhs, Expr::Literal(Literal::Number { .. })));
            }
            _ => panic!("Expected Mul at top level with Grouping on left"),
        }
    }

    #[test]
    fn test_comparison_less() {
        let expr = parse_expr_from_source("x < 5").unwrap();
        match expr {
            Expr::Binary { op, .. } => assert_eq!(op, BinaryOp::Less),
            _ => panic!("Expected Binary expression"),
        }
    }

    #[test]
    fn test_comparison_greater_or_equal() {
        let expr = parse_expr_from_source("x >= 10").unwrap();
        match expr {
            Expr::Binary { op, .. } => assert_eq!(op, BinaryOp::GreaterOrEqual),
            _ => panic!("Expected Binary expression"),
        }
    }

    #[test]
    fn test_logical_and() {
        let expr = parse_expr_from_source("x && y").unwrap();
        match expr {
            Expr::Binary { op, .. } => assert_eq!(op, BinaryOp::And),
            _ => panic!("Expected Binary expression"),
        }
    }

    #[test]
    fn test_logical_or() {
        let expr = parse_expr_from_source("x || y").unwrap();
        match expr {
            Expr::Binary { op, .. } => assert_eq!(op, BinaryOp::Or),
            _ => panic!("Expected Binary expression"),
        }
    }

    #[test]
    fn test_equality() {
        let expr = parse_expr_from_source("x == 5").unwrap();
        match expr {
            Expr::Binary { op, .. } => assert_eq!(op, BinaryOp::Equals),
            _ => panic!("Expected Binary expression"),
        }
    }

    #[test]
    fn test_strict_equality() {
        let expr = parse_expr_from_source("x === 5").unwrap();
        match expr {
            Expr::Binary { op, .. } => assert_eq!(op, BinaryOp::StrictEquals),
            _ => panic!("Expected Binary expression"),
        }
    }

    #[test]
    fn test_complex_expression() {
        let expr = parse_expr_from_source("2 + 3 * 4 - 5 / 2").unwrap();
        assert!(matches!(expr, Expr::Binary { op: BinaryOp::Sub, .. }));
    }

    #[test]
    fn test_precedence_logical_over_comparison() {
        let expr = parse_expr_from_source("x > 5 && y < 10").unwrap();
        match expr {
            Expr::Binary { op: BinaryOp::And, lhs, rhs, .. } => {
                assert!(matches!(*lhs, Expr::Binary { op: BinaryOp::Greater, .. }));
                assert!(matches!(*rhs, Expr::Binary { op: BinaryOp::Less, .. }));
            }
            _ => panic!("Expected And at top level with comparisons as operands"),
        }
    }
}
