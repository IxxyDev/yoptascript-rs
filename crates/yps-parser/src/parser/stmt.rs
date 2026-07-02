use super::*;

impl<'a> Parser<'a> {
    pub(super) fn parse_statement(&mut self) -> Result<Stmt, ()> {
        self.enter_depth()?;
        let result = stacker::maybe_grow(STACK_RED_ZONE, STACK_GROW_SIZE, || self.parse_statement_inner());
        self.depth -= 1;
        result
    }

    pub(super) fn parse_statement_inner(&mut self) -> Result<Stmt, ()> {
        if matches!(self.current().kind, TokenKind::Identifier)
            && matches!(self.peek(1).kind, TokenKind::Punctuation(PunctuationKind::Colon))
        {
            return self.parse_labeled_stmt();
        }

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
            TokenKind::Keyword(KeywordKind::GeneratorFn) => self.parse_generator_decl(),
            TokenKind::Keyword(KeywordKind::Async) => self.parse_async_stmt(),
            TokenKind::Keyword(KeywordKind::Otvechayu) => self.parse_return_stmt(),
            TokenKind::Keyword(KeywordKind::Try) => self.parse_try_stmt(),
            TokenKind::Keyword(KeywordKind::Throw) => self.parse_throw_stmt(),
            TokenKind::Keyword(KeywordKind::Switch) => self.parse_switch_stmt(),
            TokenKind::Keyword(KeywordKind::DoWhile) => self.parse_do_while_stmt(),
            TokenKind::Keyword(KeywordKind::Class) => self.parse_class_decl(),
            TokenKind::Keyword(KeywordKind::Using) => self.parse_using_stmt(),
            TokenKind::Keyword(KeywordKind::Debugger) => {
                let span = self.current().span;
                self.advance();
                if matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::Semicolon)) {
                    self.advance();
                }
                Ok(Stmt::Debugger { span })
            }
            TokenKind::Keyword(KeywordKind::Import) => {
                if matches!(self.peek(1).kind, TokenKind::Punctuation(PunctuationKind::LParen)) {
                    self.parse_expr_stmt()
                } else {
                    self.parse_import_stmt()
                }
            }
            TokenKind::Keyword(KeywordKind::Export) => self.parse_export_stmt(),
            TokenKind::Punctuation(PunctuationKind::At) => {
                let decorators = self.parse_decorators()?;
                if matches!(self.current().kind, TokenKind::Keyword(KeywordKind::Class)) {
                    self.parse_class_decl_with_decorators(decorators)
                } else {
                    let span = self.current().span;
                    self.push_error(span, "Декораторы можно применять только к классам");
                    Err(())
                }
            }
            TokenKind::Punctuation(PunctuationKind::LBrace) => self.parse_block().map(Stmt::Block),
            TokenKind::Punctuation(PunctuationKind::Semicolon) => {
                let span = self.current().span;
                self.advance();
                Ok(Stmt::Empty { span })
            }
            _ => self.parse_expr_stmt(),
        }
    }

    pub(super) fn parse_var_decl(&mut self) -> Result<Stmt, ()> {
        let start = self.current().span.start;
        let is_const = matches!(self.current().kind, TokenKind::Keyword(KeywordKind::YasenHuy));
        self.advance();

        let pattern = self.parse_pattern()?;
        if !matches!(self.current().kind, TokenKind::Operator(OperatorKind::Assign)) {
            let span = self.current().span;
            self.push_error(span, "Ожидался '=' после имени переменной");
            return Err(());
        }
        self.advance();

        let init = self.parse_expr()?;
        let end = self.expect_punct(PunctuationKind::Semicolon, "Ожидалась ';' после объявления переменной")?.end;

        Ok(Stmt::VarDecl { pattern, init, is_const, span: Span { start, end } })
    }

    pub(super) fn parse_using_stmt(&mut self) -> Result<Stmt, ()> {
        let start = self.current().span.start;
        self.advance();

        let name = self.parse_identifier()?;
        if !matches!(self.current().kind, TokenKind::Operator(OperatorKind::Assign)) {
            let span = self.current().span;
            self.push_error(span, "Ожидался '=' после имени ресурса в 'юзай'");
            return Err(());
        }
        self.advance();

        let init = self.parse_expr()?;
        let end = self.expect_punct(PunctuationKind::Semicolon, "Ожидалась ';' после объявления 'юзай'")?.end;

        Ok(Stmt::Using { name, init, span: Span { start, end } })
    }

    pub(super) fn parse_block(&mut self) -> Result<Block, ()> {
        let start = self.current().span.start;

        self.expect_punct(PunctuationKind::LBrace, "Ожидалась '{'")?;

        let mut stmts = Vec::new();

        while !matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::RBrace)) && !self.is_at_end() {
            let before = self.position;
            match self.parse_statement() {
                Ok(stmt) => stmts.push(stmt),
                Err(()) => {
                    self.synchronize();
                    if self.position == before && !self.is_at_end() {
                        self.advance();
                    }
                }
            }
        }

        let end = self.expect_punct(PunctuationKind::RBrace, "Ожидалась '}'")?.end;

        Ok(Block { stmts, span: Span { start, end } })
    }

    pub(super) fn parse_expr_stmt(&mut self) -> Result<Stmt, ()> {
        let expr = self.parse_expr()?;

        let end = self.expect_punct(PunctuationKind::Semicolon, "Ожидалась ';' после выражения")?.end;

        let span = Span { start: expr.span().start, end };

        Ok(Stmt::Expr { expr, span })
    }

    pub(super) fn parse_if_stmt(&mut self) -> Result<Stmt, ()> {
        let start = self.current().span.start;
        self.advance();

        self.expect_punct(PunctuationKind::LParen, "Ожидалась '(' после 'вилкойвглаз'")?;

        let condition = self.parse_expr()?;

        self.expect_punct(PunctuationKind::RParen, "Ожидалась ')' после условия")?;

        let then_branch = Box::new(self.parse_statement()?);

        let else_branch = if matches!(self.current().kind, TokenKind::Keyword(KeywordKind::Ilivzhopuraz)) {
            self.advance();
            Some(Box::new(self.parse_statement()?))
        } else {
            None
        };

        let end = else_branch.as_ref().map_or_else(|| then_branch.span().end, |else_stmt| else_stmt.span().end);

        Ok(Stmt::If { condition, then_branch, else_branch, span: Span { start, end } })
    }

    pub(super) fn parse_while_stmt(&mut self) -> Result<Stmt, ()> {
        let start = self.current().span.start;
        self.advance();

        self.expect_punct(PunctuationKind::LParen, "Ожидалась '(' после 'потрещим'")?;

        let condition = self.parse_expr()?;

        self.expect_punct(PunctuationKind::RParen, "Ожидалась ')' после условия")?;

        let body = Box::new(self.parse_statement()?);

        let end = body.span().end;

        Ok(Stmt::While { condition, body, span: Span { start, end } })
    }

    pub(super) fn parse_for_stmt(&mut self) -> Result<Stmt, ()> {
        let start = self.current().span.start;
        self.advance();

        let is_await = matches!(self.current().kind, TokenKind::Keyword(KeywordKind::Await));
        if is_await {
            self.advance();
        }

        self.expect_punct(PunctuationKind::LParen, "Ожидалась '(' после 'го'")?;

        let decl_offset = if matches!(
            self.current().kind,
            TokenKind::Keyword(KeywordKind::Gyy | KeywordKind::Uchastkoviy | KeywordKind::YasenHuy)
        ) {
            1
        } else {
            0
        };

        if matches!(self.peek(decl_offset).kind, TokenKind::Identifier)
            && matches!(self.peek(decl_offset + 1).kind, TokenKind::Keyword(KeywordKind::In))
        {
            if is_await {
                let span = self.current().span;
                self.push_error(span, "'сидетьНахуй' допустим только с 'сашаГрей' (for-await-of)");
                self.skip_to_for_recovery();
                return Err(());
            }
            if decl_offset == 1 {
                self.advance();
            }
            return self.parse_for_in_rest(start);
        }

        if matches!(self.peek(decl_offset).kind, TokenKind::Identifier)
            && matches!(self.peek(decl_offset + 1).kind, TokenKind::Keyword(KeywordKind::Of))
        {
            if decl_offset == 1 {
                self.advance();
            }
            if is_await {
                return self.parse_for_await_of_rest(start);
            }
            return self.parse_for_of_rest(start);
        }

        if is_await {
            let span = self.current().span;
            self.push_error(span, "'сидетьНахуй' допустим только с 'сашаГрей' (for-await-of)");
            self.skip_to_for_recovery();
            return Err(());
        }

        let init = if matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::Semicolon)) {
            self.advance();
            None
        } else if matches!(
            self.current().kind,
            TokenKind::Keyword(KeywordKind::Gyy | KeywordKind::Uchastkoviy | KeywordKind::YasenHuy)
        ) {
            Some(Box::new(self.parse_var_decl()?))
        } else {
            let expr = self.parse_expr()?;
            self.expect_punct(PunctuationKind::Semicolon, "Ожидалась ';' после инициализации")?;
            Some(Box::new(Stmt::Expr { span: expr.span(), expr }))
        };

        let condition = if matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::Semicolon)) {
            None
        } else {
            Some(self.parse_expr()?)
        };

        self.expect_punct(PunctuationKind::Semicolon, "Ожидалась ';' после условия")?;

        let update = if matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::RParen)) {
            None
        } else {
            Some(self.parse_expr()?)
        };

        self.expect_punct(PunctuationKind::RParen, "Ожидалась ')' после 'го'")?;

        let body = Box::new(self.parse_statement()?);

        let end = body.span().end;

        Ok(Stmt::For { init, condition, update, body, span: Span { start, end } })
    }

    pub(super) fn parse_break_stmt(&mut self) -> Result<Stmt, ()> {
        let start = self.current().span.start;
        self.advance();

        let label =
            if matches!(self.current().kind, TokenKind::Identifier) { Some(self.parse_identifier()?) } else { None };

        let end = self.expect_punct(PunctuationKind::Semicolon, "Ожидалась ';' после 'харэ'")?.end;

        Ok(Stmt::Break { label, span: Span { start, end } })
    }

    pub(super) fn parse_continue_stmt(&mut self) -> Result<Stmt, ()> {
        let start = self.current().span.start;
        self.advance();

        let label =
            if matches!(self.current().kind, TokenKind::Identifier) { Some(self.parse_identifier()?) } else { None };

        let end = self.expect_punct(PunctuationKind::Semicolon, "Ожидалась ';' после 'двигай'")?.end;

        Ok(Stmt::Continue { label, span: Span { start, end } })
    }

    pub(super) fn parse_labeled_stmt(&mut self) -> Result<Stmt, ()> {
        let label = self.parse_identifier()?;
        self.advance();
        let body = self.parse_statement()?;
        let span = Span { start: label.span.start, end: body.span().end };
        Ok(Stmt::Labeled { label, body: Box::new(body), span })
    }

    pub(super) fn parse_return_stmt(&mut self) -> Result<Stmt, ()> {
        let start = self.current().span.start;
        self.advance();

        let value = if matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::Semicolon)) {
            None
        } else {
            Some(self.parse_expr()?)
        };

        let end = self.expect_punct(PunctuationKind::Semicolon, "Ожидалась ';' после 'отвечаю'")?.end;

        Ok(Stmt::Return { value, span: Span { start, end } })
    }

    pub(super) fn parse_try_stmt(&mut self) -> Result<Stmt, ()> {
        let start = self.current().span.start;
        self.advance();

        let try_block = self.parse_block()?;

        let (catch_param, catch_block) = if matches!(self.current().kind, TokenKind::Keyword(KeywordKind::Catch)) {
            self.advance();

            let param = if matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::LParen)) {
                self.advance();
                let ident = self.parse_identifier()?;
                self.expect_punct(PunctuationKind::RParen, "Ожидалась ')' после параметра 'гоп'")?;
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
            self.advance();
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

    pub(super) fn parse_throw_stmt(&mut self) -> Result<Stmt, ()> {
        let start = self.current().span.start;
        self.advance();

        let value = self.parse_expr()?;

        let end = self.expect_punct(PunctuationKind::Semicolon, "Ожидалась ';' после 'кидай'")?.end;

        Ok(Stmt::Throw { value, span: Span { start, end } })
    }

    pub(super) fn parse_for_in_rest(&mut self, start: usize) -> Result<Stmt, ()> {
        let variable = self.parse_identifier()?;
        self.advance();

        let iterable = self.parse_expr()?;

        self.expect_punct(PunctuationKind::RParen, "Ожидалась ')' после 'го'")?;

        let body = Box::new(self.parse_statement()?);

        let end = body.span().end;

        Ok(Stmt::ForIn { variable, iterable, body, span: Span { start, end } })
    }

    pub(super) fn parse_for_of_rest(&mut self, start: usize) -> Result<Stmt, ()> {
        let variable = self.parse_identifier()?;
        self.advance();

        let iterable = self.parse_expr()?;

        self.expect_punct(PunctuationKind::RParen, "Ожидалась ')' после 'го'")?;

        let body = Box::new(self.parse_statement()?);
        let end = body.span().end;

        Ok(Stmt::ForOf { variable, iterable, body, span: Span { start, end } })
    }

    pub(super) fn parse_for_await_of_rest(&mut self, start: usize) -> Result<Stmt, ()> {
        let variable = self.parse_identifier()?;
        self.advance();

        let iterable = self.parse_expr()?;

        self.expect_punct(PunctuationKind::RParen, "Ожидалась ')' после 'го сидетьНахуй'")?;

        let body = Box::new(self.parse_statement()?);
        let end = body.span().end;

        Ok(Stmt::ForAwaitOf { variable, iterable, body, span: Span { start, end } })
    }

    pub(super) fn parse_do_while_stmt(&mut self) -> Result<Stmt, ()> {
        let start = self.current().span.start;
        self.advance();

        let body = Box::new(self.parse_statement()?);

        if !matches!(self.current().kind, TokenKind::Keyword(KeywordKind::Potreshchim)) {
            let span = self.current().span;
            self.push_error(span, "Ожидалось 'потрещим' после тела 'крутани'");
            return Err(());
        }
        self.advance();

        self.expect_punct(PunctuationKind::LParen, "Ожидалась '(' после 'потрещим'")?;

        let condition = self.parse_expr()?;

        self.expect_punct(PunctuationKind::RParen, "Ожидалась ')' после условия")?;

        let end = self.expect_punct(PunctuationKind::Semicolon, "Ожидалась ';' после 'крутани...потрещим'")?.end;

        Ok(Stmt::DoWhile { body, condition, span: Span { start, end } })
    }

    pub(super) fn parse_switch_stmt(&mut self) -> Result<Stmt, ()> {
        let start = self.current().span.start;
        self.advance();

        self.expect_punct(PunctuationKind::LParen, "Ожидалась '(' после 'базарпо'")?;

        let expr = self.parse_expr()?;

        self.expect_punct(PunctuationKind::RParen, "Ожидалась ')' после выражения")?;

        self.expect_punct(PunctuationKind::LBrace, "Ожидалась '{' после 'базарпо'")?;

        let mut cases = Vec::new();
        let mut default = None;

        while !matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::RBrace) | TokenKind::Eof) {
            if matches!(self.current().kind, TokenKind::Keyword(KeywordKind::Case)) {
                let case_start = self.current().span.start;
                self.advance();

                let value = self.parse_expr()?;

                self.expect_punct(PunctuationKind::Colon, "Ожидалось ':' после значения 'тема'")?;

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

        let end = self.expect_punct(PunctuationKind::RBrace, "Ожидалась '}' после 'базарпо'")?.end;

        Ok(Stmt::Switch { expr, cases, default, span: Span { start, end } })
    }
}
