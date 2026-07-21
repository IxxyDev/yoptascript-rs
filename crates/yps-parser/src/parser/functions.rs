use super::*;

impl<'a> Parser<'a> {
    fn paren_group_is_arrow_head(&self) -> bool {
        let mut depth = 0usize;
        let mut i = self.position;
        while let Some(tok) = self.tokens.get(i) {
            match tok.kind {
                TokenKind::Punctuation(PunctuationKind::LParen) => depth += 1,
                TokenKind::Punctuation(PunctuationKind::RParen) => {
                    if depth == 0 {
                        return false;
                    }
                    depth -= 1;
                    if depth == 0 {
                        return matches!(
                            self.tokens.get(i + 1).map(|t| &t.kind),
                            Some(TokenKind::Punctuation(PunctuationKind::Arrow))
                        );
                    }
                }
                _ => {}
            }
            i += 1;
        }
        false
    }

    pub(super) fn try_parse_arrow_function(&mut self) -> Result<Option<Expr>, ()> {
        if !self.paren_group_is_arrow_head() {
            return Ok(None);
        }
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

        let mut had_rest = false;
        loop {
            if had_rest {
                self.position = saved_pos;
                self.diagnostics.truncate(saved_diag_len);
                return Ok(None);
            }

            let is_rest = if matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::Spread)) {
                self.advance();
                had_rest = true;
                true
            } else {
                false
            };

            if matches!(
                self.current().kind,
                TokenKind::Punctuation(PunctuationKind::LBrace | PunctuationKind::LBracket)
            ) {
                let pat = match self.parse_pattern() {
                    Ok(p) => p,
                    Err(()) => {
                        self.position = saved_pos;
                        self.diagnostics.truncate(saved_diag_len);
                        return Ok(None);
                    }
                };
                let pat_span = pat.span();
                let synthetic = Identifier { name: "__пат__".to_string(), span: pat_span };
                let default = if matches!(self.current().kind, TokenKind::Operator(OperatorKind::Assign)) {
                    self.advance();
                    match self.parse_expr() {
                        Ok(expr) => Some(expr),
                        Err(()) => {
                            self.position = saved_pos;
                            self.diagnostics.truncate(saved_diag_len);
                            return Ok(None);
                        }
                    }
                } else {
                    None
                };
                params.push(Param { name: synthetic, default, is_rest, pattern: Some(pat) });
            } else {
                if !matches!(self.current().kind, TokenKind::Identifier) {
                    self.position = saved_pos;
                    self.diagnostics.truncate(saved_diag_len);
                    return Ok(None);
                }
                let span = self.current().span;
                let name_str = self.source.slice(span).to_string();
                self.advance();
                let name = Identifier { name: name_str, span };

                let default = if !is_rest && matches!(self.current().kind, TokenKind::Operator(OperatorKind::Assign)) {
                    self.advance();
                    match self.parse_expr() {
                        Ok(expr) => Some(expr),
                        Err(()) => {
                            self.position = saved_pos;
                            self.diagnostics.truncate(saved_diag_len);
                            return Ok(None);
                        }
                    }
                } else {
                    None
                };

                params.push(Param { name, default, is_rest, pattern: None });
            }

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

    pub(super) fn parse_single_param_arrow(&mut self) -> Result<Expr, ()> {
        let start = self.current().span.start;
        let ident = self.parse_identifier()?;
        let param = Param { name: ident, default: None, is_rest: false, pattern: None };
        self.advance();
        self.parse_arrow_body(vec![param], start)
    }

    pub(super) fn parse_arrow_body(&mut self, params: Vec<Param>, start: usize) -> Result<Expr, ()> {
        self.parse_arrow_body_with_async(params, start, false)
    }

    pub(super) fn parse_arrow_body_with_async(
        &mut self,
        params: Vec<Param>,
        start: usize,
        is_async: bool,
    ) -> Result<Expr, ()> {
        if matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::LBrace)) {
            let body = self.parse_block()?;
            let end = body.span.end;
            Ok(Expr::ArrowFunction { params: params.into(), body: Rc::new(body), is_async, span: Span { start, end } })
        } else {
            let expr = self.parse_expr()?;
            let end = expr.span().end;
            let body = Block {
                stmts: vec![Stmt::Return { value: Some(expr), span: Span { start, end } }],
                span: Span { start, end },
            };
            Ok(Expr::ArrowFunction { params: params.into(), body: Rc::new(body), is_async, span: Span { start, end } })
        }
    }

    pub(super) fn parse_function_params(&mut self) -> Result<Vec<Param>, ()> {
        let mut params = Vec::new();
        let mut had_rest = false;
        if !matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::RParen)) {
            loop {
                if had_rest {
                    let span = self.current().span;
                    self.push_error(span, "Rest-параметр должен быть последним");
                    return Err(());
                }

                let is_rest = if matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::Spread)) {
                    self.advance();
                    had_rest = true;
                    true
                } else {
                    false
                };

                if matches!(
                    self.current().kind,
                    TokenKind::Punctuation(PunctuationKind::LBrace | PunctuationKind::LBracket)
                ) {
                    let pat = self.parse_pattern()?;
                    let pat_span = pat.span();
                    let synthetic = Identifier { name: "__пат__".to_string(), span: pat_span };
                    let default = if matches!(self.current().kind, TokenKind::Operator(OperatorKind::Assign)) {
                        self.advance();
                        Some(self.parse_expr()?)
                    } else {
                        None
                    };
                    params.push(Param { name: synthetic, default, is_rest, pattern: Some(pat) });
                } else {
                    let name = self.parse_identifier()?;
                    let default =
                        if !is_rest && matches!(self.current().kind, TokenKind::Operator(OperatorKind::Assign)) {
                            self.advance();
                            Some(self.parse_expr()?)
                        } else {
                            None
                        };
                    params.push(Param { name, default, is_rest, pattern: None });
                }

                if matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::Comma)) {
                    self.advance();
                } else {
                    break;
                }
            }
        }
        Ok(params)
    }

    pub(super) fn parse_function_decl(&mut self) -> Result<Stmt, ()> {
        self.parse_function_decl_inner(false, false)
    }

    pub(super) fn parse_generator_decl(&mut self) -> Result<Stmt, ()> {
        self.parse_function_decl_inner(true, false)
    }

    pub(super) fn parse_function_decl_inner(&mut self, is_generator: bool, is_async: bool) -> Result<Stmt, ()> {
        let start = self.current().span.start;
        self.advance();

        let name = self.parse_identifier()?;
        let (params, body) = self.parse_function_params_and_body()?;
        let end = body.span.end;

        Ok(Stmt::FunctionDecl {
            name,
            params: params.into(),
            body: Rc::new(body),
            is_generator,
            is_async,
            span: Span { start, end },
        })
    }

    pub(super) fn parse_function_params_and_body(&mut self) -> Result<(Vec<Param>, Block), ()> {
        self.expect_punct(PunctuationKind::LParen, "Ожидалась '(' после имени функции")?;

        let params = self.parse_function_params()?;

        self.expect_punct(PunctuationKind::RParen, "Ожидалась ')' после параметров функции")?;

        let body = self.parse_block()?;
        Ok((params, body))
    }

    pub(super) fn parse_function_expr(&mut self) -> Result<Expr, ()> {
        let start = self.current().span.start;
        self.parse_function_expr_inner(start, false, false)
    }

    pub(super) fn parse_generator_expr(&mut self) -> Result<Expr, ()> {
        let start = self.current().span.start;
        self.parse_function_expr_inner(start, true, false)
    }

    pub(super) fn parse_function_expr_inner(
        &mut self,
        start: usize,
        is_generator: bool,
        is_async: bool,
    ) -> Result<Expr, ()> {
        self.advance();

        let name =
            if matches!(self.current().kind, TokenKind::Identifier) { Some(self.parse_identifier()?) } else { None };
        let (params, body) = self.parse_function_params_and_body()?;
        let end = body.span.end;

        Ok(Expr::FunctionExpr {
            name,
            params: params.into(),
            body: Rc::new(body),
            is_generator,
            is_async,
            span: Span { start, end },
        })
    }

    pub(super) fn parse_async_stmt(&mut self) -> Result<Stmt, ()> {
        let async_span = self.current().span;
        self.advance();
        match self.current().kind {
            TokenKind::Keyword(KeywordKind::Yopta) => self.parse_function_decl_inner(false, true),
            TokenKind::Keyword(KeywordKind::GeneratorFn) => self.parse_function_decl_inner(true, true),
            _ => {
                self.push_error(async_span, "После 'ассо' ожидалась 'йопта' или 'пиздюли' для объявления функции");
                Err(())
            }
        }
    }

    pub(super) fn parse_async_expr(&mut self) -> Result<Expr, ()> {
        let start = self.current().span.start;
        self.advance();
        if matches!(self.current().kind, TokenKind::Keyword(KeywordKind::Yopta)) {
            self.parse_function_expr_inner(start, false, true)
        } else if matches!(self.current().kind, TokenKind::Keyword(KeywordKind::GeneratorFn)) {
            self.parse_function_expr_inner(start, true, true)
        } else if matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::LParen)) {
            let saved_pos = self.position;
            let saved_diag_len = self.diagnostics.len();
            self.advance();
            let mut params = Vec::new();
            let mut had_rest = false;
            let mut malformed = false;
            if !matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::RParen)) {
                loop {
                    if had_rest {
                        malformed = true;
                        break;
                    }
                    let is_rest = if matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::Spread)) {
                        self.advance();
                        had_rest = true;
                        true
                    } else {
                        false
                    };
                    if !matches!(self.current().kind, TokenKind::Identifier) {
                        malformed = true;
                        break;
                    }
                    let name = match self.parse_identifier() {
                        Ok(n) => n,
                        Err(_) => {
                            malformed = true;
                            break;
                        }
                    };
                    let default =
                        if !is_rest && matches!(self.current().kind, TokenKind::Operator(OperatorKind::Assign)) {
                            self.advance();
                            match self.parse_expr() {
                                Ok(e) => Some(e),
                                Err(_) => {
                                    malformed = true;
                                    break;
                                }
                            }
                        } else {
                            None
                        };
                    params.push(Param { name, default, is_rest, pattern: None });
                    if matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::Comma)) {
                        self.advance();
                    } else {
                        break;
                    }
                }
            }
            if malformed || !matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::RParen)) {
                self.position = saved_pos;
                self.diagnostics.truncate(saved_diag_len);
                self.push_error(self.current().span, "После 'ассо' ожидалась стрелочная или 'йопта' функция");
                return Err(());
            }
            self.advance();
            if !matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::Arrow)) {
                self.position = saved_pos;
                self.diagnostics.truncate(saved_diag_len);
                self.push_error(self.current().span, "После 'ассо (...)' ожидалась '=>'");
                return Err(());
            }
            self.advance();
            self.parse_arrow_body_with_async(params, start, true)
        } else if matches!(self.current().kind, TokenKind::Identifier)
            && matches!(self.peek(1).kind, TokenKind::Punctuation(PunctuationKind::Arrow))
        {
            let ident = self.parse_identifier()?;
            let param = Param { name: ident, default: None, is_rest: false, pattern: None };
            self.advance();
            self.parse_arrow_body_with_async(vec![param], start, true)
        } else {
            self.push_error(self.current().span, "После 'ассо' ожидалась 'йопта', 'пиздюли' или стрелочная функция");
            Err(())
        }
    }
}
