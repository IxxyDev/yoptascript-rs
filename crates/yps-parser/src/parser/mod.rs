#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(clippy::missing_const_for_fn)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::match_same_arms)]

use crate::ast::{
    BinaryOp, Block, Expr, Identifier, Literal, ObjectPatternProp, Pattern, PostfixOp, Program, Stmt, SwitchCase,
    TemplatePart, UnaryOp,
};
use yps_lexer::{Diagnostic, KeywordKind, OperatorKind, PunctuationKind, Severity, SourceFile, Span, Token, TokenKind};

const TERNARY_PRECEDENCE: u8 = 2;
const UNARY_PRECEDENCE: u8 = 9;

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

    pub fn parse_program(mut self) -> (Program, Vec<Diagnostic>) {
        let mut items = Vec::new();

        while !self.is_at_end() {
            match self.parse_statement() {
                Ok(stmt) => items.push(stmt),
                Err(()) => {
                    self.synchronize();
                }
            }
        }

        let program = Program { items };
        (program, self.diagnostics)
    }

    fn parse_primary(&mut self) -> Result<Expr, ()> {
        match &self.current().kind {
            TokenKind::Number => Ok(self.parse_number()),
            TokenKind::StringLiteral => Ok(self.parse_string()),
            TokenKind::Keyword(KeywordKind::Pravda) => {
                let span = self.current().span;
                self.advance();
                Ok(Expr::Literal(Literal::Boolean { value: true, span }))
            }
            TokenKind::Keyword(KeywordKind::Lozh) => {
                let span = self.current().span;
                self.advance();
                Ok(Expr::Literal(Literal::Boolean { value: false, span }))
            }
            TokenKind::Keyword(KeywordKind::Nol) => {
                let span = self.current().span;
                self.advance();
                Ok(Expr::Literal(Literal::Null { span }))
            }
            TokenKind::Keyword(KeywordKind::Undefined) => {
                let span = self.current().span;
                self.advance();
                Ok(Expr::Literal(Literal::Undefined { span }))
            }
            TokenKind::Identifier => {
                if self.position + 1 < self.tokens.len()
                    && matches!(self.tokens[self.position + 1].kind, TokenKind::Punctuation(PunctuationKind::Arrow))
                {
                    self.parse_single_param_arrow()
                } else {
                    self.parse_identifier().map(Expr::Identifier)
                }
            }
            TokenKind::Punctuation(PunctuationKind::LParen) => {
                if let Some(arrow) = self.try_parse_arrow_function()? {
                    Ok(arrow)
                } else {
                    self.parse_grouping()
                }
            }
            TokenKind::TemplateNoSub => Ok(self.parse_template_nosub()),
            TokenKind::TemplateHead => self.parse_template_literal(),
            TokenKind::Punctuation(PunctuationKind::LBracket) => self.parse_array(),
            TokenKind::Punctuation(PunctuationKind::LBrace) => self.parse_object(),
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
        let inner = &raw[1..raw.len() - 1];
        let value = Self::unescape_string(inner);
        self.advance();
        Expr::Literal(Literal::String { value, span })
    }

    fn unescape_string(s: &str) -> String {
        let mut result = String::with_capacity(s.len());
        let mut chars = s.chars();
        while let Some(ch) = chars.next() {
            if ch == '\\' {
                match chars.next() {
                    Some('n') => result.push('\n'),
                    Some('t') => result.push('\t'),
                    Some('r') => result.push('\r'),
                    Some('0') => result.push('\0'),
                    Some('\\') => result.push('\\'),
                    Some('\'') => result.push('\''),
                    Some('"') => result.push('"'),
                    Some('`') => result.push('`'),
                    Some('$') => result.push('$'),
                    Some(other) => {
                        result.push('\\');
                        result.push(other);
                    }
                    None => result.push('\\'),
                }
            } else {
                result.push(ch);
            }
        }
        result
    }

    fn parse_template_nosub(&mut self) -> Expr {
        let span = self.current().span;
        let raw = self.source.slice(span);
        let inner = &raw[1..raw.len() - 1];
        let value = Self::unescape_string(inner);
        self.advance();
        Expr::Literal(Literal::String { value, span })
    }

    fn parse_template_literal(&mut self) -> Result<Expr, ()> {
        let start = self.current().span.start;
        let mut parts = Vec::new();

        let head_span = self.current().span;
        let head_raw = self.source.slice(head_span);
        let head_text = &head_raw[1..head_raw.len() - 2];
        parts.push(TemplatePart::Str(Self::unescape_string(head_text)));
        self.advance();

        let end;
        loop {
            let expr = self.parse_expr()?;
            parts.push(TemplatePart::Expr(Box::new(expr)));

            match &self.current().kind {
                TokenKind::TemplateMiddle => {
                    let mid_span = self.current().span;
                    let mid_raw = self.source.slice(mid_span);
                    let mid_text = &mid_raw[1..mid_raw.len() - 2];
                    parts.push(TemplatePart::Str(Self::unescape_string(mid_text)));
                    self.advance();
                }
                TokenKind::TemplateTail => {
                    let tail_span = self.current().span;
                    let tail_raw = self.source.slice(tail_span);
                    let tail_text = &tail_raw[1..tail_raw.len() - 1];
                    parts.push(TemplatePart::Str(Self::unescape_string(tail_text)));
                    end = self.current().span.end;
                    self.advance();
                    break;
                }
                _ => {
                    let span = self.current().span;
                    self.push_error(span, "Ожидалось продолжение шаблонной строки");
                    return Err(());
                }
            }
        }

        Ok(Expr::TemplateLiteral { parts, span: Span { start, end } })
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

    fn try_parse_arrow_function(&mut self) -> Result<Option<Expr>, ()> {
        let saved_pos = self.position;
        let saved_diag_len = self.diagnostics.len();

        self.advance();

        let mut params = Vec::new();

        if matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::RParen)) {
            self.advance();
            if matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::Arrow)) {
                self.advance();
                return Ok(Some(self.parse_arrow_body(params, saved_pos)?));
            }
            self.position = saved_pos;
            self.diagnostics.truncate(saved_diag_len);
            return Ok(None);
        }

        loop {
            if !matches!(self.current().kind, TokenKind::Identifier) {
                self.position = saved_pos;
                self.diagnostics.truncate(saved_diag_len);
                return Ok(None);
            }
            let span = self.current().span;
            let name = self.source.slice(span).to_string();
            self.advance();
            params.push(Identifier { name, span });

            if matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::Comma)) {
                self.advance();
            } else {
                break;
            }
        }

        if !matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::RParen)) {
            self.position = saved_pos;
            self.diagnostics.truncate(saved_diag_len);
            return Ok(None);
        }
        self.advance();

        if !matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::Arrow)) {
            self.position = saved_pos;
            self.diagnostics.truncate(saved_diag_len);
            return Ok(None);
        }
        self.advance();

        Ok(Some(self.parse_arrow_body(params, saved_pos)?))
    }

    fn parse_single_param_arrow(&mut self) -> Result<Expr, ()> {
        let start = self.current().span.start;
        let param = self.parse_identifier()?;
        self.advance();
        self.parse_arrow_body(vec![param], start)
    }

    fn parse_arrow_body(&mut self, params: Vec<Identifier>, start: usize) -> Result<Expr, ()> {
        if matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::LBrace)) {
            let body = self.parse_block()?;
            let end = body.span.end;
            Ok(Expr::ArrowFunction { params, body, span: Span { start, end } })
        } else {
            let expr = self.parse_expr()?;
            let end = expr.span().end;
            let body = Block {
                stmts: vec![Stmt::Return { value: Some(expr), span: Span { start, end } }],
                span: Span { start, end },
            };
            Ok(Expr::ArrowFunction { params, body, span: Span { start, end } })
        }
    }

    fn parse_array(&mut self) -> Result<Expr, ()> {
        let start = self.current().span.start;
        self.advance(); // consume '['

        let mut elements = Vec::new();
        if !matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::RBracket)) {
            loop {
                elements.push(self.parse_expr()?);

                if matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::Comma)) {
                    self.advance();
                } else {
                    break;
                }
            }
        }

        if !matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::RBracket)) {
            let span = self.current().span;
            self.push_error(span, "Ожидался ']'");
            return Err(());
        }
        let end = self.current().span.end;
        self.advance();

        Ok(Expr::Literal(Literal::Array { elements, span: Span { start, end } }))
    }

    fn parse_object(&mut self) -> Result<Expr, ()> {
        let start = self.current().span.start;
        self.advance();

        let mut properties = Vec::new();
        if !matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::RBrace)) {
            loop {
                let key = self.parse_identifier()?;

                if !matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::Colon)) {
                    let span = self.current().span;
                    self.push_error(span, "Ожидалось ':' после ключа объекта");
                    return Err(());
                }
                self.advance();

                let value = self.parse_expr()?;

                properties.push(crate::ast::ObjectProperty { key, value });

                if matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::Comma)) {
                    self.advance();
                } else {
                    break;
                }
            }
        }

        if !matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::RBrace)) {
            let span = self.current().span;
            self.push_error(span, "Ожидался '}'");
            return Err(());
        }
        let end = self.current().span.end;
        self.advance();

        Ok(Expr::Literal(Literal::Object { properties, span: Span { start, end } }))
    }

    fn parse_expr(&mut self) -> Result<Expr, ()> {
        self.parse_expression_with_precedence(0)
    }

    fn parse_statement(&mut self) -> Result<Stmt, ()> {
        match &self.current().kind {
            TokenKind::Keyword(KeywordKind::Gyy | KeywordKind::Uchastkoviy | KeywordKind::YasenHuy) => {
                self.parse_var_decl()
            }
            TokenKind::Keyword(KeywordKind::Vilkoyvglaz) => self.parse_if_stmt(),
            TokenKind::Keyword(KeywordKind::Potreshchim) => self.parse_while_stmt(),
            TokenKind::Keyword(KeywordKind::Go) => self.parse_for_stmt(),
            TokenKind::Keyword(KeywordKind::Hare) => self.parse_break_stmt(),
            TokenKind::Keyword(KeywordKind::Dvigay) => self.parse_continue_stmt(),
            TokenKind::Keyword(KeywordKind::Yopta) => self.parse_function_decl(),
            TokenKind::Keyword(KeywordKind::Otvechayu) => self.parse_return_stmt(),
            TokenKind::Keyword(KeywordKind::Try) => self.parse_try_stmt(),
            TokenKind::Keyword(KeywordKind::Throw) => self.parse_throw_stmt(),
            TokenKind::Keyword(KeywordKind::Switch) => self.parse_switch_stmt(),
            TokenKind::Keyword(KeywordKind::DoWhile) => self.parse_do_while_stmt(),
            TokenKind::Punctuation(PunctuationKind::LBrace) => self.parse_block().map(Stmt::Block),
            TokenKind::Punctuation(PunctuationKind::Semicolon) => {
                let span = self.current().span;
                self.advance();
                Ok(Stmt::Empty { span })
            }
            _ => self.parse_expr_stmt(),
        }
    }

    fn parse_var_decl(&mut self) -> Result<Stmt, ()> {
        let start = self.current().span.start;
        let is_const = matches!(self.current().kind, TokenKind::Keyword(KeywordKind::Uchastkoviy));
        self.advance();

        let pattern = self.parse_pattern()?;
        if !matches!(self.current().kind, TokenKind::Operator(OperatorKind::Assign)) {
            let span = self.current().span;
            self.push_error(span, "Ожидался '=' после имени переменной");
            return Err(());
        }
        self.advance();

        let init = self.parse_expr()?;
        if !matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::Semicolon)) {
            let span = self.current().span;
            self.push_error(span, "Ожидалась ';' после объявления переменной");
            return Err(());
        }
        let end = self.current().span.end;
        self.advance();

        Ok(Stmt::VarDecl { pattern, init, is_const, span: Span { start, end } })
    }

    fn parse_pattern(&mut self) -> Result<Pattern, ()> {
        match &self.current().kind {
            TokenKind::Punctuation(PunctuationKind::LBracket) => self.parse_array_pattern(),
            TokenKind::Punctuation(PunctuationKind::LBrace) => self.parse_object_pattern(),
            TokenKind::Identifier => {
                let ident = self.parse_identifier()?;
                Ok(Pattern::Identifier(ident))
            }
            _ => {
                let span = self.current().span;
                self.push_error(span, "Ожидался идентификатор или паттерн деструктуризации");
                Err(())
            }
        }
    }

    fn parse_array_pattern(&mut self) -> Result<Pattern, ()> {
        let start = self.current().span.start;
        self.advance();

        let mut elements: Vec<Option<Pattern>> = Vec::new();
        let mut rest: Option<Box<Pattern>> = None;

        if !matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::RBracket)) {
            loop {
                if matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::Spread)) {
                    self.advance();
                    let pat = self.parse_pattern()?;
                    rest = Some(Box::new(pat));
                    break;
                }

                if matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::Comma)) {
                    elements.push(None);
                } else {
                    elements.push(Some(self.parse_pattern()?));
                }

                if matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::Comma)) {
                    self.advance();
                } else {
                    break;
                }
            }
        }

        if !matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::RBracket)) {
            let span = self.current().span;
            self.push_error(span, "Ожидался ']'");
            return Err(());
        }
        let end = self.current().span.end;
        self.advance();

        Ok(Pattern::Array { elements, rest, span: Span { start, end } })
    }

    fn parse_object_pattern(&mut self) -> Result<Pattern, ()> {
        let start = self.current().span.start;
        self.advance();

        let mut properties: Vec<ObjectPatternProp> = Vec::new();
        let mut rest: Option<Box<Pattern>> = None;

        if !matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::RBrace)) {
            loop {
                if matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::Spread)) {
                    self.advance();
                    let pat = self.parse_pattern()?;
                    rest = Some(Box::new(pat));
                    break;
                }

                let key = self.parse_identifier()?;
                let prop_start = key.span.start;

                let value = if matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::Colon)) {
                    self.advance();
                    Some(self.parse_pattern()?)
                } else {
                    None
                };

                let prop_end = if let Some(ref v) = value {
                    match v {
                        Pattern::Identifier(id) => id.span.end,
                        Pattern::Array { span, .. } | Pattern::Object { span, .. } => span.end,
                    }
                } else {
                    key.span.end
                };

                properties.push(ObjectPatternProp { key, value, span: Span { start: prop_start, end: prop_end } });

                if matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::Comma)) {
                    self.advance();
                } else {
                    break;
                }
            }
        }

        if !matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::RBrace)) {
            let span = self.current().span;
            self.push_error(span, "Ожидался '}'");
            return Err(());
        }
        let end = self.current().span.end;
        self.advance();

        Ok(Pattern::Object { properties, rest, span: Span { start, end } })
    }

    fn parse_block(&mut self) -> Result<Block, ()> {
        let start = self.current().span.start;

        if !matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::LBrace)) {
            let span = self.current().span;
            self.push_error(span, "Ожидалась '{'");
            return Err(());
        }
        self.advance();

        let mut stmts = Vec::new();

        while !matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::RBrace)) && !self.is_at_end() {
            match self.parse_statement() {
                Ok(stmt) => stmts.push(stmt),
                Err(()) => {
                    self.synchronize();
                }
            }
        }

        if !matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::RBrace)) {
            let span = self.current().span;
            self.push_error(span, "Ожидалась '}'");
            return Err(());
        }
        let end = self.current().span.end;
        self.advance();

        Ok(Block { stmts, span: Span { start, end } })
    }

    fn parse_expr_stmt(&mut self) -> Result<Stmt, ()> {
        let expr = self.parse_expr()?;

        if !matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::Semicolon)) {
            let span = self.current().span;
            self.push_error(span, "Ожидалась ';' после выражения");
            return Err(());
        }
        let end = self.current().span.end;
        self.advance();

        let span = Span { start: expr.span().start, end };

        Ok(Stmt::Expr { expr, span })
    }

    fn parse_if_stmt(&mut self) -> Result<Stmt, ()> {
        let start = self.current().span.start;
        self.advance();

        if !matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::LParen)) {
            let span = self.current().span;
            self.push_error(span, "Ожидалась '(' после 'вилкойвглаз'");
            return Err(());
        }
        self.advance();

        let condition = self.parse_expr()?;

        if !matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::RParen)) {
            let span = self.current().span;
            self.push_error(span, "Ожидалась ')' после условия");
            return Err(());
        }
        self.advance();

        let then_branch = Box::new(self.parse_statement()?);

        let else_branch = if matches!(self.current().kind, TokenKind::Keyword(KeywordKind::Ilivzhopuraz)) {
            self.advance();
            Some(Box::new(self.parse_statement()?))
        } else {
            None
        };

        let end = else_branch.as_ref().map_or_else(
            || match then_branch.as_ref() {
                Stmt::VarDecl { span, .. }
                | Stmt::Expr { span, .. }
                | Stmt::Block(Block { span, .. })
                | Stmt::If { span, .. }
                | Stmt::While { span, .. }
                | Stmt::For { span, .. }
                | Stmt::Break { span }
                | Stmt::Continue { span }
                | Stmt::FunctionDecl { span, .. }
                | Stmt::Return { span, .. }
                | Stmt::TryCatch { span, .. }
                | Stmt::Throw { span, .. }
                | Stmt::Switch { span, .. }
                | Stmt::DoWhile { span, .. }
                | Stmt::ForIn { span, .. }
                | Stmt::Empty { span } => span.end,
            },
            |else_stmt| match else_stmt.as_ref() {
                Stmt::VarDecl { span, .. }
                | Stmt::Expr { span, .. }
                | Stmt::Block(Block { span, .. })
                | Stmt::If { span, .. }
                | Stmt::While { span, .. }
                | Stmt::For { span, .. }
                | Stmt::Break { span }
                | Stmt::Continue { span }
                | Stmt::FunctionDecl { span, .. }
                | Stmt::Return { span, .. }
                | Stmt::TryCatch { span, .. }
                | Stmt::Throw { span, .. }
                | Stmt::Switch { span, .. }
                | Stmt::DoWhile { span, .. }
                | Stmt::ForIn { span, .. }
                | Stmt::Empty { span } => span.end,
            },
        );

        Ok(Stmt::If { condition, then_branch, else_branch, span: Span { start, end } })
    }

    fn parse_while_stmt(&mut self) -> Result<Stmt, ()> {
        let start = self.current().span.start;
        self.advance();

        if !matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::LParen)) {
            let span = self.current().span;
            self.push_error(span, "Ожидалась '(' после 'потрещим'");
            return Err(());
        }
        self.advance();

        let condition = self.parse_expr()?;

        if !matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::RParen)) {
            let span = self.current().span;
            self.push_error(span, "Ожидалась ')' после условия");
            return Err(());
        }
        self.advance();

        let body = Box::new(self.parse_statement()?);

        let end = match body.as_ref() {
            Stmt::VarDecl { span, .. }
            | Stmt::Expr { span, .. }
            | Stmt::Block(Block { span, .. })
            | Stmt::If { span, .. }
            | Stmt::While { span, .. }
            | Stmt::For { span, .. }
            | Stmt::Break { span }
            | Stmt::Continue { span }
            | Stmt::FunctionDecl { span, .. }
            | Stmt::Return { span, .. }
            | Stmt::TryCatch { span, .. }
            | Stmt::Throw { span, .. }
            | Stmt::Switch { span, .. }
            | Stmt::DoWhile { span, .. }
            | Stmt::ForIn { span, .. }
            | Stmt::Empty { span } => span.end,
        };

        Ok(Stmt::While { condition, body, span: Span { start, end } })
    }

    fn parse_for_stmt(&mut self) -> Result<Stmt, ()> {
        let start = self.current().span.start;
        self.advance();

        if !matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::LParen)) {
            let span = self.current().span;
            self.push_error(span, "Ожидалась '(' после 'го'");
            return Err(());
        }
        self.advance();

        if matches!(self.current().kind, TokenKind::Identifier)
            && matches!(self.peek(1).kind, TokenKind::Keyword(KeywordKind::In))
        {
            return self.parse_for_in_rest(start);
        }

        let init = if matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::Semicolon)) {
            self.advance(); // пропускаем ';'
            None
        } else if matches!(
            self.current().kind,
            TokenKind::Keyword(KeywordKind::Gyy | KeywordKind::Uchastkoviy | KeywordKind::YasenHuy)
        ) {
            Some(Box::new(self.parse_var_decl()?))
        } else {
            let expr = self.parse_expr()?;
            if !matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::Semicolon)) {
                let span = self.current().span;
                self.push_error(span, "Ожидалась ';' после инициализации");
                return Err(());
            }
            self.advance();
            Some(Box::new(Stmt::Expr { span: expr.span(), expr }))
        };

        let condition = if matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::Semicolon)) {
            None
        } else {
            Some(self.parse_expr()?)
        };

        if !matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::Semicolon)) {
            let span = self.current().span;
            self.push_error(span, "Ожидалась ';' после условия");
            return Err(());
        }
        self.advance();

        let update = if matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::RParen)) {
            None
        } else {
            Some(self.parse_expr()?)
        };

        if !matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::RParen)) {
            let span = self.current().span;
            self.push_error(span, "Ожидалась ')' после 'го'");
            return Err(());
        }
        self.advance();

        let body = Box::new(self.parse_statement()?);

        let end = match body.as_ref() {
            Stmt::VarDecl { span, .. }
            | Stmt::Expr { span, .. }
            | Stmt::Block(Block { span, .. })
            | Stmt::If { span, .. }
            | Stmt::While { span, .. }
            | Stmt::For { span, .. }
            | Stmt::Break { span }
            | Stmt::Continue { span }
            | Stmt::FunctionDecl { span, .. }
            | Stmt::Return { span, .. }
            | Stmt::TryCatch { span, .. }
            | Stmt::Throw { span, .. }
            | Stmt::Switch { span, .. }
            | Stmt::DoWhile { span, .. }
            | Stmt::ForIn { span, .. }
            | Stmt::Empty { span } => span.end,
        };

        Ok(Stmt::For { init, condition, update, body, span: Span { start, end } })
    }

    fn parse_break_stmt(&mut self) -> Result<Stmt, ()> {
        let start = self.current().span.start;
        self.advance();

        if !matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::Semicolon)) {
            let span = self.current().span;
            self.push_error(span, "Ожидалась ';' после 'харэ'");
            return Err(());
        }
        let end = self.current().span.end;
        self.advance();

        Ok(Stmt::Break { span: Span { start, end } })
    }

    fn parse_continue_stmt(&mut self) -> Result<Stmt, ()> {
        let start = self.current().span.start;
        self.advance();

        if !matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::Semicolon)) {
            let span = self.current().span;
            self.push_error(span, "Ожидалась ';' после 'двигай'");
            return Err(());
        }
        let end = self.current().span.end;
        self.advance();

        Ok(Stmt::Continue { span: Span { start, end } })
    }

    fn parse_function_decl(&mut self) -> Result<Stmt, ()> {
        let start = self.current().span.start;
        self.advance();

        let name = self.parse_identifier()?;

        if !matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::LParen)) {
            let span = self.current().span;
            self.push_error(span, "Ожидалась '(' после имени функции");
            return Err(());
        }
        self.advance();

        let mut params = Vec::new();
        if !matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::RParen)) {
            loop {
                params.push(self.parse_identifier()?);

                if matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::Comma)) {
                    self.advance();
                } else {
                    break;
                }
            }
        }

        if !matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::RParen)) {
            let span = self.current().span;
            self.push_error(span, "Ожидалась ')' после параметров функции");
            return Err(());
        }
        self.advance();

        let body = self.parse_block()?;
        let end = body.span.end;

        Ok(Stmt::FunctionDecl { name, params, body, span: Span { start, end } })
    }

    fn parse_return_stmt(&mut self) -> Result<Stmt, ()> {
        let start = self.current().span.start;
        self.advance();

        let value = if matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::Semicolon)) {
            None
        } else {
            Some(self.parse_expr()?)
        };

        if !matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::Semicolon)) {
            let span = self.current().span;
            self.push_error(span, "Ожидалась ';' после 'отвечаю'");
            return Err(());
        }
        let end = self.current().span.end;
        self.advance();

        Ok(Stmt::Return { value, span: Span { start, end } })
    }

    fn parse_try_stmt(&mut self) -> Result<Stmt, ()> {
        let start = self.current().span.start;
        self.advance(); // consume 'хапнуть'

        let try_block = self.parse_block()?;

        let (catch_param, catch_block) = if matches!(self.current().kind, TokenKind::Keyword(KeywordKind::Catch)) {
            self.advance(); // consume 'гоп'

            let param = if matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::LParen)) {
                self.advance();
                let ident = self.parse_identifier()?;
                if !matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::RParen)) {
                    let span = self.current().span;
                    self.push_error(span, "Ожидалась ')' после параметра 'гоп'");
                    return Err(());
                }
                self.advance();
                Some(ident)
            } else {
                None
            };

            let block = self.parse_block()?;
            (param, Some(block))
        } else {
            (None, None)
        };

        let finally_block = if matches!(self.current().kind, TokenKind::Keyword(KeywordKind::Finally)) {
            self.advance(); // consume 'тюряжка'
            Some(self.parse_block()?)
        } else {
            None
        };

        if catch_block.is_none() && finally_block.is_none() {
            let span = self.current().span;
            self.push_error(span, "Ожидался 'гоп' или 'тюряжка' после 'хапнуть'");
            return Err(());
        }

        let end = finally_block
            .as_ref()
            .map(|b| b.span.end)
            .or_else(|| catch_block.as_ref().map(|b| b.span.end))
            .unwrap_or(try_block.span.end);

        Ok(Stmt::TryCatch { try_block, catch_param, catch_block, finally_block, span: Span { start, end } })
    }

    fn parse_throw_stmt(&mut self) -> Result<Stmt, ()> {
        let start = self.current().span.start;
        self.advance(); // consume 'кидай'

        let value = self.parse_expr()?;

        if !matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::Semicolon)) {
            let span = self.current().span;
            self.push_error(span, "Ожидалась ';' после 'кидай'");
            return Err(());
        }
        let end = self.current().span.end;
        self.advance();

        Ok(Stmt::Throw { value, span: Span { start, end } })
    }

    fn parse_for_in_rest(&mut self, start: usize) -> Result<Stmt, ()> {
        let variable = self.parse_identifier()?;
        self.advance();

        let iterable = self.parse_expr()?;

        if !matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::RParen)) {
            let span = self.current().span;
            self.push_error(span, "Ожидалась ')' после 'го'");
            return Err(());
        }
        self.advance();

        let body = Box::new(self.parse_statement()?);

        let end = match body.as_ref() {
            Stmt::VarDecl { span, .. }
            | Stmt::Expr { span, .. }
            | Stmt::Block(Block { span, .. })
            | Stmt::If { span, .. }
            | Stmt::While { span, .. }
            | Stmt::For { span, .. }
            | Stmt::Break { span }
            | Stmt::Continue { span }
            | Stmt::FunctionDecl { span, .. }
            | Stmt::Return { span, .. }
            | Stmt::TryCatch { span, .. }
            | Stmt::Throw { span, .. }
            | Stmt::Switch { span, .. }
            | Stmt::DoWhile { span, .. }
            | Stmt::ForIn { span, .. }
            | Stmt::Empty { span } => span.end,
        };

        Ok(Stmt::ForIn { variable, iterable, body, span: Span { start, end } })
    }

    fn parse_do_while_stmt(&mut self) -> Result<Stmt, ()> {
        let start = self.current().span.start;
        self.advance();

        let body = Box::new(self.parse_statement()?);

        if !matches!(self.current().kind, TokenKind::Keyword(KeywordKind::Potreshchim)) {
            let span = self.current().span;
            self.push_error(span, "Ожидалось 'потрещим' после тела 'крутани'");
            return Err(());
        }
        self.advance();

        if !matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::LParen)) {
            let span = self.current().span;
            self.push_error(span, "Ожидалась '(' после 'потрещим'");
            return Err(());
        }
        self.advance();

        let condition = self.parse_expr()?;

        if !matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::RParen)) {
            let span = self.current().span;
            self.push_error(span, "Ожидалась ')' после условия");
            return Err(());
        }
        self.advance();

        if !matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::Semicolon)) {
            let span = self.current().span;
            self.push_error(span, "Ожидалась ';' после 'крутани...потрещим'");
            return Err(());
        }
        let end = self.current().span.end;
        self.advance();

        Ok(Stmt::DoWhile { body, condition, span: Span { start, end } })
    }

    fn parse_switch_stmt(&mut self) -> Result<Stmt, ()> {
        let start = self.current().span.start;
        self.advance();

        if !matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::LParen)) {
            let span = self.current().span;
            self.push_error(span, "Ожидалась '(' после 'базарпо'");
            return Err(());
        }
        self.advance();

        let expr = self.parse_expr()?;

        if !matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::RParen)) {
            let span = self.current().span;
            self.push_error(span, "Ожидалась ')' после выражения");
            return Err(());
        }
        self.advance();

        if !matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::LBrace)) {
            let span = self.current().span;
            self.push_error(span, "Ожидалась '{' после 'базарпо'");
            return Err(());
        }
        self.advance();

        let mut cases = Vec::new();
        let mut default = None;

        while !matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::RBrace) | TokenKind::Eof) {
            if matches!(self.current().kind, TokenKind::Keyword(KeywordKind::Case)) {
                let case_start = self.current().span.start;
                self.advance();

                let value = self.parse_expr()?;

                if !matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::Colon)) {
                    let span = self.current().span;
                    self.push_error(span, "Ожидалось ':' после значения 'тема'");
                    return Err(());
                }
                self.advance();

                let body = self.parse_block()?;
                let case_end = body.span.end;

                cases.push(SwitchCase { value, body, span: Span { start: case_start, end: case_end } });
            } else if matches!(self.current().kind, TokenKind::Keyword(KeywordKind::Default)) {
                self.advance();

                let body = self.parse_block()?;
                default = Some(body);
            } else {
                let span = self.current().span;
                self.push_error(span, "Ожидалось 'тема' или 'нуичо' внутри 'базарпо'");
                return Err(());
            }
        }

        if !matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::RBrace)) {
            let span = self.current().span;
            self.push_error(span, "Ожидалась '}' после 'базарпо'");
            return Err(());
        }
        let end = self.current().span.end;
        self.advance();

        Ok(Stmt::Switch { expr, cases, default, span: Span { start, end } })
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
            if matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::Question))
                && min_precedence <= TERNARY_PRECEDENCE
            {
                let start = lhs.span().start;
                self.advance();
                let then_expr = self.parse_expression_with_precedence(0)?;
                if !matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::Colon)) {
                    let span = self.current().span;
                    self.push_error(span, "Ожидалось ':' в тернарном операторе");
                    return Err(());
                }
                self.advance();
                let else_expr = self.parse_expression_with_precedence(TERNARY_PRECEDENCE)?;
                let end = else_expr.span().end;
                lhs = Expr::Conditional {
                    condition: Box::new(lhs),
                    then_expr: Box::new(then_expr),
                    else_expr: Box::new(else_expr),
                    span: Span { start, end },
                };
                continue;
            }

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
        let mut expr = match &self.current().kind {
            TokenKind::Operator(OperatorKind::Plus) => {
                let start = self.current().span.start;
                self.advance();
                let expr = self.parse_expression_with_precedence(UNARY_PRECEDENCE)?;
                let end = expr.span().end;
                Expr::Unary { op: UnaryOp::Plus, expr: Box::new(expr), span: Span { start, end } }
            }
            TokenKind::Operator(OperatorKind::Minus) => {
                let start = self.current().span.start;
                self.advance();
                let expr = self.parse_expression_with_precedence(UNARY_PRECEDENCE)?;
                let end = expr.span().end;
                Expr::Unary { op: UnaryOp::Minus, expr: Box::new(expr), span: Span { start, end } }
            }
            TokenKind::Operator(OperatorKind::Not) => {
                let start = self.current().span.start;
                self.advance();
                let expr = self.parse_expression_with_precedence(UNARY_PRECEDENCE)?;
                let end = expr.span().end;
                Expr::Unary { op: UnaryOp::Not, expr: Box::new(expr), span: Span { start, end } }
            }
            _ => self.parse_primary()?,
        };

        loop {
            if matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::LParen)) {
                expr = self.parse_call(expr)?;
            } else if matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::LBracket)) {
                expr = self.parse_index(expr)?;
            } else if matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::Dot)) {
                expr = self.parse_member(expr)?;
            } else if matches!(self.current().kind, TokenKind::Operator(OperatorKind::Increment)) {
                let start = expr.span().start;
                let end = self.current().span.end;
                self.advance();
                expr = Expr::Postfix { op: PostfixOp::Increment, expr: Box::new(expr), span: Span { start, end } };
            } else if matches!(self.current().kind, TokenKind::Operator(OperatorKind::Decrement)) {
                let start = expr.span().start;
                let end = self.current().span.end;
                self.advance();
                expr = Expr::Postfix { op: PostfixOp::Decrement, expr: Box::new(expr), span: Span { start, end } };
            } else {
                break;
            }
        }

        Ok(expr)
    }

    fn parse_call(&mut self, callee: Expr) -> Result<Expr, ()> {
        let start = callee.span().start;
        self.advance();

        let mut args = Vec::new();
        if !matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::RParen)) {
            loop {
                args.push(self.parse_expr()?);

                if matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::Comma)) {
                    self.advance();
                } else {
                    break;
                }
            }
        }

        if !matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::RParen)) {
            let span = self.current().span;
            self.push_error(span, "Ожидалась ')' после аргументов функции");
            return Err(());
        }
        let end = self.current().span.end;
        self.advance();

        Ok(Expr::Call { callee: Box::new(callee), args, span: Span { start, end } })
    }

    fn parse_index(&mut self, object: Expr) -> Result<Expr, ()> {
        let start = object.span().start;
        self.advance();

        let index = self.parse_expr()?;

        if !matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::RBracket)) {
            let span = self.current().span;
            self.push_error(span, "Ожидался ']'");
            return Err(());
        }
        let end = self.current().span.end;
        self.advance();

        Ok(Expr::Index { object: Box::new(object), index: Box::new(index), span: Span { start, end } })
    }

    fn parse_member(&mut self, object: Expr) -> Result<Expr, ()> {
        let start = object.span().start;
        self.advance();

        let property = self.parse_identifier()?;

        let end = property.span.end;

        Ok(Expr::Member { object: Box::new(object), property, span: Span { start, end } })
    }

    fn try_parse_binary_op(&self) -> Option<(BinaryOp, u8)> {
        let TokenKind::Operator(op_kind) = &self.current().kind else {
            return None;
        };

        match op_kind {
            OperatorKind::Assign => Some((BinaryOp::Assign, 1)),
            OperatorKind::PlusAssign => Some((BinaryOp::PlusAssign, 1)),
            OperatorKind::MinusAssign => Some((BinaryOp::MinusAssign, 1)),
            OperatorKind::MulAssign => Some((BinaryOp::MulAssign, 1)),
            OperatorKind::DivAssign => Some((BinaryOp::DivAssign, 1)),
            OperatorKind::Or => Some((BinaryOp::Or, 3)),
            OperatorKind::And => Some((BinaryOp::And, 4)),
            OperatorKind::Equals => Some((BinaryOp::Equals, 5)),
            OperatorKind::StrictEquals => Some((BinaryOp::StrictEquals, 5)),
            OperatorKind::NotEquals => Some((BinaryOp::NotEquals, 5)),
            OperatorKind::StrictNotEquals => Some((BinaryOp::StrictNotEquals, 5)),
            OperatorKind::Less => Some((BinaryOp::Less, 6)),
            OperatorKind::Greater => Some((BinaryOp::Greater, 6)),
            OperatorKind::LessOrEqual => Some((BinaryOp::LessOrEqual, 6)),
            OperatorKind::GreaterOrEqual => Some((BinaryOp::GreaterOrEqual, 6)),
            OperatorKind::Plus => Some((BinaryOp::Add, 7)),
            OperatorKind::Minus => Some((BinaryOp::Sub, 7)),
            OperatorKind::Multiply => Some((BinaryOp::Mul, 8)),
            OperatorKind::Divide => Some((BinaryOp::Div, 8)),
            OperatorKind::Modulo => Some((BinaryOp::Mod, 8)),
            OperatorKind::Not | OperatorKind::Increment | OperatorKind::Decrement => None,
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
    fn test_parse_string_escape_newline() {
        let expr = parse_expr_from_source(r#""hello\nworld""#).unwrap();
        match expr {
            Expr::Literal(Literal::String { value, .. }) => assert_eq!(value, "hello\nworld"),
            _ => panic!("expected string literal"),
        }
    }

    #[test]
    fn test_parse_string_escape_tab() {
        let expr = parse_expr_from_source(r#""a\tb""#).unwrap();
        match expr {
            Expr::Literal(Literal::String { value, .. }) => assert_eq!(value, "a\tb"),
            _ => panic!("expected string literal"),
        }
    }

    #[test]
    fn test_parse_string_escape_backslash() {
        let expr = parse_expr_from_source(r#""a\\b""#).unwrap();
        match expr {
            Expr::Literal(Literal::String { value, .. }) => assert_eq!(value, "a\\b"),
            _ => panic!("expected string literal"),
        }
    }

    #[test]
    fn test_parse_string_escape_quote() {
        let expr = parse_expr_from_source(r#""say \"yo\"""#).unwrap();
        match expr {
            Expr::Literal(Literal::String { value, .. }) => assert_eq!(value, "say \"yo\""),
            _ => panic!("expected string literal"),
        }
    }

    #[test]
    fn test_parse_string_escape_multiple() {
        let expr = parse_expr_from_source(r#""a\nb\tc\r\0""#).unwrap();
        match expr {
            Expr::Literal(Literal::String { value, .. }) => assert_eq!(value, "a\nb\tc\r\0"),
            _ => panic!("expected string literal"),
        }
    }

    #[test]
    fn test_parse_string_unknown_escape_preserved() {
        let expr = parse_expr_from_source(r#""a\xb""#).unwrap();
        match expr {
            Expr::Literal(Literal::String { value, .. }) => assert_eq!(value, "a\\xb"),
            _ => panic!("expected string literal"),
        }
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

    #[test]
    fn test_parse_var_decl_gyy() {
        let source = SourceFile::new("test.yop".to_string(), "гыы x = 5;".to_string());
        let lexer = yps_lexer::Lexer::new(&source);
        let (tokens, lex_diags) = lexer.tokenize();
        assert!(lex_diags.is_empty());

        let parser = Parser::new(&tokens, &source);
        let (program, diags) = parser.parse_program();

        assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
        assert_eq!(program.items.len(), 1);

        match &program.items[0] {
            Stmt::VarDecl { pattern: Pattern::Identifier(name), init, .. } => {
                assert_eq!(name.name, "x");
                assert!(matches!(init, Expr::Literal(Literal::Number { .. })));
            }
            _ => panic!("Expected VarDecl, got: {:?}", program.items[0]),
        }
    }

    #[test]
    fn test_parse_var_decl_yasen_huy() {
        let source = SourceFile::new("test.yop".to_string(), "ясенХуй y = \"hello\";".to_string());
        let lexer = yps_lexer::Lexer::new(&source);
        let (tokens, lex_diags) = lexer.tokenize();
        assert!(lex_diags.is_empty());

        let parser = Parser::new(&tokens, &source);
        let (program, diags) = parser.parse_program();

        assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
        assert_eq!(program.items.len(), 1);

        match &program.items[0] {
            Stmt::VarDecl { pattern: Pattern::Identifier(name), init, .. } => {
                assert_eq!(name.name, "y");
                assert!(matches!(init, Expr::Literal(Literal::String { .. })));
            }
            _ => panic!("Expected VarDecl"),
        }
    }

    #[test]
    fn test_parse_expr_stmt() {
        let source = SourceFile::new("test.yop".to_string(), "x + 5;".to_string());
        let lexer = yps_lexer::Lexer::new(&source);
        let (tokens, lex_diags) = lexer.tokenize();
        assert!(lex_diags.is_empty());

        let parser = Parser::new(&tokens, &source);
        let (program, diags) = parser.parse_program();

        assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
        assert_eq!(program.items.len(), 1);

        match &program.items[0] {
            Stmt::Expr { expr, .. } => {
                assert!(matches!(expr, Expr::Binary { op: BinaryOp::Add, .. }));
            }
            _ => panic!("Expected Expr statement"),
        }
    }

    #[test]
    fn test_parse_empty_stmt() {
        let source = SourceFile::new("test.yop".to_string(), ";".to_string());
        let lexer = yps_lexer::Lexer::new(&source);
        let (tokens, lex_diags) = lexer.tokenize();
        assert!(lex_diags.is_empty());

        let parser = Parser::new(&tokens, &source);
        let (program, diags) = parser.parse_program();

        assert!(diags.is_empty());
        assert_eq!(program.items.len(), 1);
        assert!(matches!(program.items[0], Stmt::Empty { .. }));
    }

    #[test]
    fn test_parse_multiple_statements() {
        let source = SourceFile::new("test.yop".to_string(), "гыы x = 5;\nясенХуй y = 10;\nx + y;".to_string());
        let lexer = yps_lexer::Lexer::new(&source);
        let (tokens, lex_diags) = lexer.tokenize();
        assert!(lex_diags.is_empty());

        let parser = Parser::new(&tokens, &source);
        let (program, diags) = parser.parse_program();

        assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
        assert_eq!(program.items.len(), 3);

        assert!(matches!(program.items[0], Stmt::VarDecl { .. }));
        assert!(matches!(program.items[1], Stmt::VarDecl { .. }));
        assert!(matches!(program.items[2], Stmt::Expr { .. }));
    }

    #[test]
    fn test_parse_if_stmt() {
        let source = SourceFile::new("test.yop".to_string(), "вилкойвглаз (x > 5) x = 10;".to_string());
        let lexer = yps_lexer::Lexer::new(&source);
        let (tokens, lex_diags) = lexer.tokenize();
        assert!(lex_diags.is_empty());

        let parser = Parser::new(&tokens, &source);
        let (program, diags) = parser.parse_program();

        assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
        assert_eq!(program.items.len(), 1);

        match &program.items[0] {
            Stmt::If { condition, then_branch, else_branch, .. } => {
                assert!(matches!(condition, Expr::Binary { op: BinaryOp::Greater, .. }));
                assert!(matches!(then_branch.as_ref(), Stmt::Expr { .. }));
                assert!(else_branch.is_none());
            }
            _ => panic!("Expected If statement"),
        }
    }

    #[test]
    fn test_parse_if_else_stmt() {
        let source =
            SourceFile::new("test.yop".to_string(), "вилкойвглаз (x > 5) x = 10; иливжопураз x = 0;".to_string());
        let lexer = yps_lexer::Lexer::new(&source);
        let (tokens, lex_diags) = lexer.tokenize();
        assert!(lex_diags.is_empty());

        let parser = Parser::new(&tokens, &source);
        let (program, diags) = parser.parse_program();

        assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
        assert_eq!(program.items.len(), 1);

        match &program.items[0] {
            Stmt::If { condition, then_branch, else_branch, .. } => {
                assert!(matches!(condition, Expr::Binary { op: BinaryOp::Greater, .. }));
                assert!(matches!(then_branch.as_ref(), Stmt::Expr { .. }));
                assert!(else_branch.is_some());
                assert!(matches!(else_branch.as_ref().unwrap().as_ref(), Stmt::Expr { .. }));
            }
            _ => panic!("Expected If statement"),
        }
    }

    #[test]
    fn test_parse_if_with_block() {
        let source = SourceFile::new("test.yop".to_string(), "вилкойвглаз (x > 5) { x = 10; }".to_string());
        let lexer = yps_lexer::Lexer::new(&source);
        let (tokens, lex_diags) = lexer.tokenize();
        assert!(lex_diags.is_empty());

        let parser = Parser::new(&tokens, &source);
        let (program, diags) = parser.parse_program();

        assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
        assert_eq!(program.items.len(), 1);

        match &program.items[0] {
            Stmt::If { then_branch, .. } => {
                assert!(matches!(then_branch.as_ref(), Stmt::Block(_)));
            }
            _ => panic!("Expected If statement"),
        }
    }

    #[test]
    fn test_parse_while_stmt() {
        let source = SourceFile::new("test.yop".to_string(), "потрещим (x > 0) x = x - 1;".to_string());
        let lexer = yps_lexer::Lexer::new(&source);
        let (tokens, lex_diags) = lexer.tokenize();
        assert!(lex_diags.is_empty());
        let parser = Parser::new(&tokens, &source);

        let (program, diags) = parser.parse_program();

        assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
        assert_eq!(program.items.len(), 1);
        match &program.items[0] {
            Stmt::While { condition, body, .. } => {
                assert!(matches!(condition, Expr::Binary { op: BinaryOp::Greater, .. }));
                assert!(matches!(body.as_ref(), Stmt::Expr { .. }));
            }
            _ => panic!("Expected While statement"),
        }
    }

    #[test]
    fn test_parse_while_with_block() {
        let source = SourceFile::new("test.yop".to_string(), "потрещим (x > 0) { x = x - 1; }".to_string());
        let lexer = yps_lexer::Lexer::new(&source);
        let (tokens, lex_diags) = lexer.tokenize();
        assert!(lex_diags.is_empty());
        let parser = Parser::new(&tokens, &source);

        let (program, diags) = parser.parse_program();

        assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
        assert_eq!(program.items.len(), 1);
        match &program.items[0] {
            Stmt::While { body, .. } => {
                assert!(matches!(body.as_ref(), Stmt::Block(_)));
            }
            _ => panic!("Expected While statement"),
        }
    }

    #[test]
    fn test_parse_nested_while() {
        let source =
            SourceFile::new("test.yop".to_string(), "потрещим (x > 0) потрещим (y > 0) y = y - 1;".to_string());
        let lexer = yps_lexer::Lexer::new(&source);
        let (tokens, lex_diags) = lexer.tokenize();
        assert!(lex_diags.is_empty());
        let parser = Parser::new(&tokens, &source);

        let (program, diags) = parser.parse_program();

        assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
        assert_eq!(program.items.len(), 1);
        match &program.items[0] {
            Stmt::While { body, .. } => {
                assert!(matches!(body.as_ref(), Stmt::While { .. }));
            }
            _ => panic!("Expected While statement"),
        }
    }

    #[test]
    fn test_parse_for_stmt() {
        let source =
            SourceFile::new("test.yop".to_string(), "го (гыы i = 0; i < 10; i = i + 1) x = x + i;".to_string());
        let lexer = yps_lexer::Lexer::new(&source);
        let (tokens, lex_diags) = lexer.tokenize();
        assert!(lex_diags.is_empty());
        let parser = Parser::new(&tokens, &source);

        let (program, diags) = parser.parse_program();

        assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
        assert_eq!(program.items.len(), 1);
        match &program.items[0] {
            Stmt::For { init, condition, update, body, .. } => {
                assert!(init.is_some());
                assert!(condition.is_some());
                assert!(update.is_some());
                assert!(matches!(body.as_ref(), Stmt::Expr { .. }));
            }
            _ => panic!("Expected For statement"),
        }
    }

    #[test]
    fn test_parse_for_with_block() {
        let source =
            SourceFile::new("test.yop".to_string(), "го (гыы i = 0; i < 10; i = i + 1) { x = x + i; }".to_string());
        let lexer = yps_lexer::Lexer::new(&source);
        let (tokens, lex_diags) = lexer.tokenize();
        assert!(lex_diags.is_empty());
        let parser = Parser::new(&tokens, &source);

        let (program, diags) = parser.parse_program();

        assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
        assert_eq!(program.items.len(), 1);
        match &program.items[0] {
            Stmt::For { body, .. } => {
                assert!(matches!(body.as_ref(), Stmt::Block(_)));
            }
            _ => panic!("Expected For statement"),
        }
    }

    #[test]
    fn test_parse_for_without_init() {
        let source = SourceFile::new("test.yop".to_string(), "го (; i < 10; i = i + 1) x = x + i;".to_string());
        let lexer = yps_lexer::Lexer::new(&source);
        let (tokens, lex_diags) = lexer.tokenize();
        assert!(lex_diags.is_empty());
        let parser = Parser::new(&tokens, &source);

        let (program, diags) = parser.parse_program();

        assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
        assert_eq!(program.items.len(), 1);
        match &program.items[0] {
            Stmt::For { init, condition, update, .. } => {
                assert!(init.is_none());
                assert!(condition.is_some());
                assert!(update.is_some());
            }
            _ => panic!("Expected For statement"),
        }
    }

    #[test]
    fn test_parse_for_without_condition() {
        let source = SourceFile::new("test.yop".to_string(), "го (гыы i = 0; ; i = i + 1) x = x + i;".to_string());
        let lexer = yps_lexer::Lexer::new(&source);
        let (tokens, lex_diags) = lexer.tokenize();
        assert!(lex_diags.is_empty());
        let parser = Parser::new(&tokens, &source);

        let (program, diags) = parser.parse_program();

        assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
        assert_eq!(program.items.len(), 1);
        match &program.items[0] {
            Stmt::For { init, condition, update, .. } => {
                assert!(init.is_some());
                assert!(condition.is_none());
                assert!(update.is_some());
            }
            _ => panic!("Expected For statement"),
        }
    }

    #[test]
    fn test_parse_for_without_update() {
        let source = SourceFile::new("test.yop".to_string(), "го (гыы i = 0; i < 10;) x = x + i;".to_string());
        let lexer = yps_lexer::Lexer::new(&source);
        let (tokens, lex_diags) = lexer.tokenize();
        assert!(lex_diags.is_empty());
        let parser = Parser::new(&tokens, &source);

        let (program, diags) = parser.parse_program();

        assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
        assert_eq!(program.items.len(), 1);
        match &program.items[0] {
            Stmt::For { init, condition, update, .. } => {
                assert!(init.is_some());
                assert!(condition.is_some());
                assert!(update.is_none());
            }
            _ => panic!("Expected For statement"),
        }
    }

    #[test]
    fn test_parse_for_infinite_loop() {
        let source = SourceFile::new("test.yop".to_string(), "го (;;) x = x + 1;".to_string());
        let lexer = yps_lexer::Lexer::new(&source);
        let (tokens, lex_diags) = lexer.tokenize();
        assert!(lex_diags.is_empty());
        let parser = Parser::new(&tokens, &source);

        let (program, diags) = parser.parse_program();

        assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
        assert_eq!(program.items.len(), 1);
        match &program.items[0] {
            Stmt::For { init, condition, update, .. } => {
                assert!(init.is_none());
                assert!(condition.is_none());
                assert!(update.is_none());
            }
            _ => panic!("Expected For statement"),
        }
    }

    #[test]
    fn test_parse_nested_for() {
        let source = SourceFile::new(
            "test.yop".to_string(),
            "го (гыы i = 0; i < 10; i = i + 1) го (гыы j = 0; j < 5; j = j + 1) x = x + 1;".to_string(),
        );
        let lexer = yps_lexer::Lexer::new(&source);
        let (tokens, lex_diags) = lexer.tokenize();
        assert!(lex_diags.is_empty());
        let parser = Parser::new(&tokens, &source);

        let (program, diags) = parser.parse_program();

        assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
        assert_eq!(program.items.len(), 1);
        match &program.items[0] {
            Stmt::For { body, .. } => {
                assert!(matches!(body.as_ref(), Stmt::For { .. }));
            }
            _ => panic!("Expected For statement"),
        }
    }

    #[test]
    fn test_parse_break_stmt() {
        let source = SourceFile::new("test.yop".to_string(), "харэ;".to_string());
        let lexer = yps_lexer::Lexer::new(&source);
        let (tokens, lex_diags) = lexer.tokenize();
        assert!(lex_diags.is_empty());
        let parser = Parser::new(&tokens, &source);

        let (program, diags) = parser.parse_program();

        assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
        assert_eq!(program.items.len(), 1);
        assert!(matches!(program.items[0], Stmt::Break { .. }));
    }

    #[test]
    fn test_parse_continue_stmt() {
        let source = SourceFile::new("test.yop".to_string(), "двигай;".to_string());
        let lexer = yps_lexer::Lexer::new(&source);
        let (tokens, lex_diags) = lexer.tokenize();
        assert!(lex_diags.is_empty());
        let parser = Parser::new(&tokens, &source);

        let (program, diags) = parser.parse_program();

        assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
        assert_eq!(program.items.len(), 1);
        assert!(matches!(program.items[0], Stmt::Continue { .. }));
    }

    #[test]
    fn test_parse_break_in_while() {
        let source = SourceFile::new("test.yop".to_string(), "потрещим (x > 0) { харэ; }".to_string());
        let lexer = yps_lexer::Lexer::new(&source);
        let (tokens, lex_diags) = lexer.tokenize();
        assert!(lex_diags.is_empty());
        let parser = Parser::new(&tokens, &source);

        let (program, diags) = parser.parse_program();

        assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
        assert_eq!(program.items.len(), 1);
        match &program.items[0] {
            Stmt::While { body, .. } => match body.as_ref() {
                Stmt::Block(Block { stmts, .. }) => {
                    assert_eq!(stmts.len(), 1);
                    assert!(matches!(stmts[0], Stmt::Break { .. }));
                }
                _ => panic!("Expected Block in While body"),
            },
            _ => panic!("Expected While statement"),
        }
    }

    #[test]
    fn test_parse_continue_in_for() {
        let source =
            SourceFile::new("test.yop".to_string(), "го (гыы i = 0; i < 10; i = i + 1) { двигай; }".to_string());
        let lexer = yps_lexer::Lexer::new(&source);
        let (tokens, lex_diags) = lexer.tokenize();
        assert!(lex_diags.is_empty());
        let parser = Parser::new(&tokens, &source);

        let (program, diags) = parser.parse_program();

        assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
        assert_eq!(program.items.len(), 1);
        match &program.items[0] {
            Stmt::For { body, .. } => match body.as_ref() {
                Stmt::Block(Block { stmts, .. }) => {
                    assert_eq!(stmts.len(), 1);
                    assert!(matches!(stmts[0], Stmt::Continue { .. }));
                }
                _ => panic!("Expected Block in For body"),
            },
            _ => panic!("Expected For statement"),
        }
    }

    #[test]
    fn test_parse_function_decl() {
        let source = SourceFile::new("test.yop".to_string(), "йопта foo(x, y) { x + y; }".to_string());
        let lexer = yps_lexer::Lexer::new(&source);
        let (tokens, lex_diags) = lexer.tokenize();
        assert!(lex_diags.is_empty());
        let parser = Parser::new(&tokens, &source);

        let (program, diags) = parser.parse_program();

        assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
        assert_eq!(program.items.len(), 1);
        match &program.items[0] {
            Stmt::FunctionDecl { name, params, body, .. } => {
                assert_eq!(name.name, "foo");
                assert_eq!(params.len(), 2);
                assert_eq!(params[0].name, "x");
                assert_eq!(params[1].name, "y");
                assert_eq!(body.stmts.len(), 1);
            }
            _ => panic!("Expected FunctionDecl statement"),
        }
    }

    #[test]
    fn test_parse_function_decl_no_params() {
        let source = SourceFile::new("test.yop".to_string(), "йопта bar() { отвечаю 42; }".to_string());
        let lexer = yps_lexer::Lexer::new(&source);
        let (tokens, lex_diags) = lexer.tokenize();
        assert!(lex_diags.is_empty());
        let parser = Parser::new(&tokens, &source);

        let (program, diags) = parser.parse_program();

        assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
        assert_eq!(program.items.len(), 1);
        match &program.items[0] {
            Stmt::FunctionDecl { name, params, body, .. } => {
                assert_eq!(name.name, "bar");
                assert_eq!(params.len(), 0);
                assert_eq!(body.stmts.len(), 1);
            }
            _ => panic!("Expected FunctionDecl statement"),
        }
    }

    #[test]
    fn test_parse_return_stmt() {
        let source = SourceFile::new("test.yop".to_string(), "отвечаю 42;".to_string());
        let lexer = yps_lexer::Lexer::new(&source);
        let (tokens, lex_diags) = lexer.tokenize();
        assert!(lex_diags.is_empty());
        let parser = Parser::new(&tokens, &source);

        let (program, diags) = parser.parse_program();

        assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
        assert_eq!(program.items.len(), 1);
        match &program.items[0] {
            Stmt::Return { value, .. } => {
                assert!(value.is_some());
            }
            _ => panic!("Expected Return statement"),
        }
    }

    #[test]
    fn test_parse_return_stmt_no_value() {
        let source = SourceFile::new("test.yop".to_string(), "отвечаю;".to_string());
        let lexer = yps_lexer::Lexer::new(&source);
        let (tokens, lex_diags) = lexer.tokenize();
        assert!(lex_diags.is_empty());
        let parser = Parser::new(&tokens, &source);

        let (program, diags) = parser.parse_program();

        assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
        assert_eq!(program.items.len(), 1);
        match &program.items[0] {
            Stmt::Return { value, .. } => {
                assert!(value.is_none());
            }
            _ => panic!("Expected Return statement"),
        }
    }

    #[test]
    fn test_parse_function_call() {
        let source = SourceFile::new("test.yop".to_string(), "foo(1, 2);".to_string());
        let lexer = yps_lexer::Lexer::new(&source);
        let (tokens, lex_diags) = lexer.tokenize();
        assert!(lex_diags.is_empty());
        let parser = Parser::new(&tokens, &source);

        let (program, diags) = parser.parse_program();

        assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
        assert_eq!(program.items.len(), 1);
        match &program.items[0] {
            Stmt::Expr { expr, .. } => match expr {
                Expr::Call { args, .. } => {
                    assert_eq!(args.len(), 2);
                }
                _ => panic!("Expected Call expression"),
            },
            _ => panic!("Expected Expr statement"),
        }
    }

    #[test]
    fn test_parse_function_call_no_args() {
        let source = SourceFile::new("test.yop".to_string(), "bar();".to_string());
        let lexer = yps_lexer::Lexer::new(&source);
        let (tokens, lex_diags) = lexer.tokenize();
        assert!(lex_diags.is_empty());
        let parser = Parser::new(&tokens, &source);

        let (program, diags) = parser.parse_program();

        assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
        assert_eq!(program.items.len(), 1);
        match &program.items[0] {
            Stmt::Expr { expr, .. } => match expr {
                Expr::Call { args, .. } => {
                    assert_eq!(args.len(), 0);
                }
                _ => panic!("Expected Call expression"),
            },
            _ => panic!("Expected Expr statement"),
        }
    }

    #[test]
    fn test_parse_nested_function_call() {
        let source = SourceFile::new("test.yop".to_string(), "foo(bar(1), 2);".to_string());
        let lexer = yps_lexer::Lexer::new(&source);
        let (tokens, lex_diags) = lexer.tokenize();
        assert!(lex_diags.is_empty());
        let parser = Parser::new(&tokens, &source);

        let (program, diags) = parser.parse_program();

        assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
        assert_eq!(program.items.len(), 1);
        match &program.items[0] {
            Stmt::Expr { expr, .. } => match expr {
                Expr::Call { args, .. } => {
                    assert_eq!(args.len(), 2);
                    assert!(matches!(args[0], Expr::Call { .. }));
                }
                _ => panic!("Expected Call expression"),
            },
            _ => panic!("Expected Expr statement"),
        }
    }

    #[test]
    fn test_parse_array_literal() {
        let source = SourceFile::new("test.yop".to_string(), "[1, 2, 3];".to_string());
        let lexer = yps_lexer::Lexer::new(&source);
        let (tokens, lex_diags) = lexer.tokenize();
        assert!(lex_diags.is_empty());
        let parser = Parser::new(&tokens, &source);

        let (program, diags) = parser.parse_program();

        assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
        assert_eq!(program.items.len(), 1);
        match &program.items[0] {
            Stmt::Expr { expr, .. } => match expr {
                Expr::Literal(Literal::Array { elements, .. }) => {
                    assert_eq!(elements.len(), 3);
                }
                _ => panic!("Expected Array literal"),
            },
            _ => panic!("Expected Expr statement"),
        }
    }

    #[test]
    fn test_parse_empty_array() {
        let source = SourceFile::new("test.yop".to_string(), "[];".to_string());
        let lexer = yps_lexer::Lexer::new(&source);
        let (tokens, lex_diags) = lexer.tokenize();
        assert!(lex_diags.is_empty());
        let parser = Parser::new(&tokens, &source);

        let (program, diags) = parser.parse_program();

        assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
        assert_eq!(program.items.len(), 1);
        match &program.items[0] {
            Stmt::Expr { expr, .. } => match expr {
                Expr::Literal(Literal::Array { elements, .. }) => {
                    assert_eq!(elements.len(), 0);
                }
                _ => panic!("Expected Array literal"),
            },
            _ => panic!("Expected Expr statement"),
        }
    }

    #[test]
    fn test_parse_array_index() {
        let source = SourceFile::new("test.yop".to_string(), "arr[0];".to_string());
        let lexer = yps_lexer::Lexer::new(&source);
        let (tokens, lex_diags) = lexer.tokenize();
        assert!(lex_diags.is_empty());
        let parser = Parser::new(&tokens, &source);

        let (program, diags) = parser.parse_program();

        assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
        assert_eq!(program.items.len(), 1);
        match &program.items[0] {
            Stmt::Expr { expr, .. } => {
                assert!(matches!(expr, Expr::Index { .. }));
            }
            _ => panic!("Expected Expr statement"),
        }
    }

    #[test]
    fn test_parse_nested_array_index() {
        let source = SourceFile::new("test.yop".to_string(), "arr[i][j];".to_string());
        let lexer = yps_lexer::Lexer::new(&source);
        let (tokens, lex_diags) = lexer.tokenize();
        assert!(lex_diags.is_empty());
        let parser = Parser::new(&tokens, &source);

        let (program, diags) = parser.parse_program();

        assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
        assert_eq!(program.items.len(), 1);
        match &program.items[0] {
            Stmt::Expr { expr, .. } => match expr {
                Expr::Index { object, .. } => {
                    assert!(matches!(object.as_ref(), Expr::Index { .. }));
                }
                _ => panic!("Expected Index expression"),
            },
            _ => panic!("Expected Expr statement"),
        }
    }

    #[test]
    fn test_parse_nested_array_literal() {
        let source = SourceFile::new("test.yop".to_string(), "[[1, 2], [3, 4]];".to_string());
        let lexer = yps_lexer::Lexer::new(&source);
        let (tokens, lex_diags) = lexer.tokenize();
        assert!(lex_diags.is_empty());
        let parser = Parser::new(&tokens, &source);

        let (program, diags) = parser.parse_program();

        assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
        assert_eq!(program.items.len(), 1);
        match &program.items[0] {
            Stmt::Expr { expr, .. } => match expr {
                Expr::Literal(Literal::Array { elements, .. }) => {
                    assert_eq!(elements.len(), 2);
                    assert!(matches!(elements[0], Expr::Literal(Literal::Array { .. })));
                }
                _ => panic!("Expected Array literal"),
            },
            _ => panic!("Expected Expr statement"),
        }
    }

    #[test]
    fn test_parse_object_literal() {
        let source = SourceFile::new("test.yop".to_string(), "гыы obj = {x: 1, y: 2};".to_string());
        let lexer = yps_lexer::Lexer::new(&source);
        let (tokens, lex_diags) = lexer.tokenize();
        assert!(lex_diags.is_empty());
        let parser = Parser::new(&tokens, &source);

        let (program, diags) = parser.parse_program();

        assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
        assert_eq!(program.items.len(), 1);
        match &program.items[0] {
            Stmt::VarDecl { init, .. } => match init {
                Expr::Literal(Literal::Object { properties, .. }) => {
                    assert_eq!(properties.len(), 2);
                    assert_eq!(properties[0].key.name, "x");
                    assert_eq!(properties[1].key.name, "y");
                }
                _ => panic!("Expected Object literal"),
            },
            _ => panic!("Expected VarDecl statement"),
        }
    }

    #[test]
    fn test_parse_empty_object() {
        let source = SourceFile::new("test.yop".to_string(), "гыы obj = {};".to_string());
        let lexer = yps_lexer::Lexer::new(&source);
        let (tokens, lex_diags) = lexer.tokenize();
        assert!(lex_diags.is_empty());
        let parser = Parser::new(&tokens, &source);

        let (program, diags) = parser.parse_program();

        assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
        assert_eq!(program.items.len(), 1);
        match &program.items[0] {
            Stmt::VarDecl { init, .. } => match init {
                Expr::Literal(Literal::Object { properties, .. }) => {
                    assert_eq!(properties.len(), 0);
                }
                _ => panic!("Expected Object literal"),
            },
            _ => panic!("Expected VarDecl statement"),
        }
    }

    #[test]
    fn test_parse_member_access() {
        let source = SourceFile::new("test.yop".to_string(), "obj.prop;".to_string());
        let lexer = yps_lexer::Lexer::new(&source);
        let (tokens, lex_diags) = lexer.tokenize();
        assert!(lex_diags.is_empty());
        let parser = Parser::new(&tokens, &source);

        let (program, diags) = parser.parse_program();

        assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
        assert_eq!(program.items.len(), 1);
        match &program.items[0] {
            Stmt::Expr { expr, .. } => {
                assert!(matches!(expr, Expr::Member { .. }));
            }
            _ => panic!("Expected Expr statement"),
        }
    }

    #[test]
    fn test_parse_nested_member_access() {
        let source = SourceFile::new("test.yop".to_string(), "obj.prop.nested;".to_string());
        let lexer = yps_lexer::Lexer::new(&source);
        let (tokens, lex_diags) = lexer.tokenize();
        assert!(lex_diags.is_empty());
        let parser = Parser::new(&tokens, &source);

        let (program, diags) = parser.parse_program();

        assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
        assert_eq!(program.items.len(), 1);
        match &program.items[0] {
            Stmt::Expr { expr, .. } => match expr {
                Expr::Member { object, .. } => {
                    assert!(matches!(object.as_ref(), Expr::Member { .. }));
                }
                _ => panic!("Expected Member expression"),
            },
            _ => panic!("Expected Expr statement"),
        }
    }

    #[test]
    fn test_parse_nested_object_literal() {
        let source = SourceFile::new("test.yop".to_string(), "гыы obj = {x: {y: 1}};".to_string());
        let lexer = yps_lexer::Lexer::new(&source);
        let (tokens, lex_diags) = lexer.tokenize();
        assert!(lex_diags.is_empty());
        let parser = Parser::new(&tokens, &source);

        let (program, diags) = parser.parse_program();

        assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
        assert_eq!(program.items.len(), 1);
        match &program.items[0] {
            Stmt::VarDecl { init, .. } => match init {
                Expr::Literal(Literal::Object { properties, .. }) => {
                    assert_eq!(properties.len(), 1);
                    assert!(matches!(properties[0].value, Expr::Literal(Literal::Object { .. })));
                }
                _ => panic!("Expected Object literal"),
            },
            _ => panic!("Expected VarDecl statement"),
        }
    }

    #[test]
    fn test_parse_method_call() {
        let source = SourceFile::new("test.yop".to_string(), "obj.method();".to_string());
        let lexer = yps_lexer::Lexer::new(&source);
        let (tokens, lex_diags) = lexer.tokenize();
        assert!(lex_diags.is_empty());
        let parser = Parser::new(&tokens, &source);

        let (program, diags) = parser.parse_program();

        assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
        assert_eq!(program.items.len(), 1);
        match &program.items[0] {
            Stmt::Expr { expr, .. } => match expr {
                Expr::Call { callee, .. } => {
                    assert!(matches!(callee.as_ref(), Expr::Member { .. }));
                }
                _ => panic!("Expected Call expression"),
            },
            _ => panic!("Expected Expr statement"),
        }
    }

    #[test]
    fn test_parse_array_of_objects() {
        let source = SourceFile::new("test.yop".to_string(), "[{x: 1}, {y: 2}];".to_string());
        let lexer = yps_lexer::Lexer::new(&source);
        let (tokens, lex_diags) = lexer.tokenize();
        assert!(lex_diags.is_empty());
        let parser = Parser::new(&tokens, &source);

        let (program, diags) = parser.parse_program();

        assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
        assert_eq!(program.items.len(), 1);
        match &program.items[0] {
            Stmt::Expr { expr, .. } => match expr {
                Expr::Literal(Literal::Array { elements, .. }) => {
                    assert_eq!(elements.len(), 2);
                    assert!(matches!(elements[0], Expr::Literal(Literal::Object { .. })));
                    assert!(matches!(elements[1], Expr::Literal(Literal::Object { .. })));
                }
                _ => panic!("Expected Array literal"),
            },
            _ => panic!("Expected Expr statement"),
        }
    }

    #[test]
    fn test_parse_ternary_simple() {
        let expr = parse_expr_from_source("правда ? 1 : 2").unwrap();
        assert!(matches!(expr, Expr::Conditional { .. }));
    }

    #[test]
    fn test_parse_ternary_with_comparison() {
        let expr = parse_expr_from_source("x > 5 ? 10 : 20").unwrap();
        match &expr {
            Expr::Conditional { condition, .. } => {
                assert!(matches!(condition.as_ref(), Expr::Binary { .. }));
            }
            _ => panic!("Expected Conditional"),
        }
    }

    #[test]
    fn test_parse_ternary_nested_else() {
        let expr = parse_expr_from_source("a ? 1 : b ? 2 : 3").unwrap();
        match &expr {
            Expr::Conditional { else_expr, .. } => {
                assert!(matches!(else_expr.as_ref(), Expr::Conditional { .. }));
            }
            _ => panic!("Expected nested Conditional"),
        }
    }

    #[test]
    fn test_parse_ternary_missing_colon() {
        let result = parse_expr_from_source("правда ? 1 2");
        assert!(result.is_err());
    }
}
