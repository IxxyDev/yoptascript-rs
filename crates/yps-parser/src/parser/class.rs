use super::*;

impl<'a> Parser<'a> {
    pub(super) fn parse_decorators(&mut self) -> Result<Vec<Expr>, ()> {
        let mut decorators = Vec::new();
        while matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::At)) {
            self.advance();
            let expr = self.parse_prefix()?;
            decorators.push(expr);
        }
        Ok(decorators)
    }

    pub(super) fn parse_class_decl(&mut self) -> Result<Stmt, ()> {
        self.parse_class_decl_with_decorators(vec![])
    }

    pub(super) fn parse_class_decl_with_decorators(&mut self, decorators: Vec<Expr>) -> Result<Stmt, ()> {
        let start = self.current().span.start;
        self.advance();

        let name = self.parse_identifier()?;

        let super_class = if matches!(self.current().kind, TokenKind::Keyword(KeywordKind::Extends)) {
            self.advance();
            Some(self.parse_expr()?)
        } else {
            None
        };

        self.expect_punct(PunctuationKind::LBrace, "Ожидалась '{' после имени класса")?;

        let mut members = Vec::new();
        while !matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::RBrace) | TokenKind::Eof) {
            members.push(self.parse_class_member(&name.name)?);
        }

        let end = self.expect_punct(PunctuationKind::RBrace, "Ожидалась '}' в конце класса")?.end;

        Ok(Stmt::ClassDecl { name, super_class, members, decorators, span: Span { start, end } })
    }

    pub(super) fn parse_class_member(&mut self, class_name: &str) -> Result<ClassMember, ()> {
        let decorators = self.parse_decorators()?;

        let start = self.current().span.start;

        let is_static = if matches!(self.current().kind, TokenKind::Keyword(KeywordKind::Static)) {
            self.advance();
            true
        } else {
            false
        };

        let modifier_private = self.parse_visibility_modifier();

        if matches!(self.current().kind, TokenKind::Identifier)
            && self.source.slice(self.current().span) == "get"
            && !matches!(self.peek(1).kind, TokenKind::Punctuation(PunctuationKind::LParen))
        {
            self.advance();
            let (member_name, is_private) = self.parse_member_name(modifier_private)?;
            self.expect_punct(PunctuationKind::LParen, "Ожидалась '(' после имени геттера")?;
            self.expect_punct(PunctuationKind::RParen, "Геттер не принимает параметров")?;
            let body = self.parse_block()?;
            let end = body.span.end;
            return Ok(ClassMember::Getter {
                name: member_name,
                body: Rc::new(body),
                is_static,
                is_private,
                decorators,
                span: Span { start, end },
            });
        }

        if matches!(self.current().kind, TokenKind::Identifier)
            && self.source.slice(self.current().span) == "set"
            && !matches!(self.peek(1).kind, TokenKind::Punctuation(PunctuationKind::LParen))
        {
            self.advance();
            let (member_name, is_private) = self.parse_member_name(modifier_private)?;
            self.expect_punct(PunctuationKind::LParen, "Ожидалась '(' после имени сеттера")?;
            let params = self.parse_function_params()?;
            if params.len() != 1 {
                let span = self.current().span;
                self.push_error(span, "Сеттер принимает ровно один параметр");
                return Err(());
            }
            self.expect_punct(PunctuationKind::RParen, "Ожидалась ')' после параметра сеттера")?;
            let body = self.parse_block()?;
            let end = body.span.end;
            let param = params.into_iter().next().unwrap();
            return Ok(ClassMember::Setter {
                name: member_name,
                param,
                body: Rc::new(body),
                is_static,
                is_private,
                decorators,
                span: Span { start, end },
            });
        }

        let (member_name, is_private) = self.parse_member_name(modifier_private)?;

        if matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::LParen)) {
            self.advance();
            let params = self.parse_function_params()?;

            self.expect_punct(PunctuationKind::RParen, "Ожидалась ')' после параметров метода")?;

            let body = self.parse_block()?;
            let end = body.span.end;

            if !is_static && !is_private && member_name.name == class_name {
                if !decorators.is_empty() {
                    self.push_error(Span { start, end }, "Декораторы нельзя применять к конструктору");
                    return Err(());
                }
                Ok(ClassMember::Constructor { params: params.into(), body: Rc::new(body), span: Span { start, end } })
            } else {
                Ok(ClassMember::Method {
                    name: member_name,
                    params: params.into(),
                    body: Rc::new(body),
                    is_static,
                    is_private,
                    decorators,
                    span: Span { start, end },
                })
            }
        } else {
            let init = if matches!(self.current().kind, TokenKind::Operator(OperatorKind::Assign)) {
                self.advance();
                Some(self.parse_expr()?)
            } else {
                None
            };

            if matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::Semicolon)) {
                self.advance();
            }

            let end = self.current().span.start;
            Ok(ClassMember::Field {
                name: member_name,
                init,
                is_static,
                is_private,
                decorators,
                span: Span { start, end },
            })
        }
    }

    pub(super) fn parse_visibility_modifier(&mut self) -> bool {
        match self.current().kind {
            TokenKind::Keyword(KeywordKind::Private) => {
                self.advance();
                true
            }
            TokenKind::Keyword(KeywordKind::Public) | TokenKind::Keyword(KeywordKind::Protected) => {
                self.advance();
                false
            }
            _ => false,
        }
    }

    pub(super) fn parse_member_name(&mut self, modifier_private: bool) -> Result<(Identifier, bool), ()> {
        if matches!(self.current().kind, TokenKind::PrivateIdentifier) {
            let span = self.current().span;
            let name = self.source.slice(span).to_string();
            self.advance();
            Ok((Identifier { name, span }, true))
        } else if modifier_private {
            let ident = self.parse_identifier()?;
            let private_name = format!("#{}", ident.name);
            Ok((Identifier { name: private_name, span: ident.span }, true))
        } else {
            let ident = self.parse_identifier()?;
            Ok((ident, false))
        }
    }
}
