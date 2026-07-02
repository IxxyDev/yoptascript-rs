use super::*;

impl<'a> Parser<'a> {
    pub(super) fn parse_dynamic_import(&mut self) -> Result<Expr, ()> {
        let start = self.current().span.start;
        self.advance();
        self.expect_punct(PunctuationKind::LParen, "Ожидалась '(' после 'спиздить' в динамическом импорте")?;
        let source = self.parse_expr()?;
        let end = self.expect_punct(PunctuationKind::RParen, "Ожидалась ')' в динамическом импорте")?.end;
        Ok(Expr::DynamicImport { source: Box::new(source), span: Span { start, end } })
    }

    pub(super) fn parse_import_stmt(&mut self) -> Result<Stmt, ()> {
        let start = self.current().span.start;
        self.advance();

        let mut specifiers: Vec<crate::ast::ImportSpec> = Vec::new();

        if matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::LBrace)) {
            self.advance();
            if !matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::RBrace)) {
                loop {
                    let imported = self.parse_identifier()?;
                    let local = imported.clone();
                    specifiers.push(crate::ast::ImportSpec::Named { imported, local });
                    if matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::Comma)) {
                        self.advance();
                    } else {
                        break;
                    }
                }
            }
            self.expect_punct(PunctuationKind::RBrace, "Ожидалась '}' в списке импортов")?;
        } else if matches!(self.current().kind, TokenKind::Identifier) {
            let local = self.parse_identifier()?;
            specifiers.push(crate::ast::ImportSpec::Default { local });
        } else {
            let span = self.current().span;
            self.push_error(span, "Ожидался идентификатор или '{' после 'спиздить'");
            return Err(());
        }

        self.expect_keyword(KeywordKind::In, "Ожидалось 'из' в импорте")?;

        let source = if matches!(self.current().kind, TokenKind::StringLiteral) {
            let span = self.current().span;
            let raw = self.source.slice(span);
            let inner = Self::strip_delimiters(raw, 1);
            let value = Self::unescape_string(inner);
            self.advance();
            value
        } else {
            let span = self.current().span;
            self.push_error(span, "Ожидалась строка-путь модуля");
            return Err(());
        };

        let attributes = if matches!(self.current().kind, TokenKind::Identifier) && {
            let raw = self.source.slice(self.current().span);
            raw == "with" || raw == "сатр"
        } {
            self.advance();
            self.parse_import_attributes()?
        } else {
            Vec::new()
        };

        let end = self.expect_punct(PunctuationKind::Semicolon, "Ожидалась ';' после импорта")?.end;

        Ok(Stmt::Import { specifiers, source, attributes, span: Span { start, end } })
    }

    pub(super) fn parse_import_attributes(&mut self) -> Result<Vec<(String, String)>, ()> {
        self.expect_punct(PunctuationKind::LBrace, "Ожидалась '{' после 'with' в импорте")?;

        let mut attrs: Vec<(String, String)> = Vec::new();
        if !matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::RBrace)) {
            loop {
                let key = match &self.current().kind {
                    TokenKind::Identifier => {
                        let s = self.source.slice(self.current().span).to_string();
                        self.advance();
                        s
                    }
                    TokenKind::StringLiteral => {
                        let raw = self.source.slice(self.current().span);
                        let inner = Self::strip_delimiters(raw, 1);
                        let s = Self::unescape_string(inner);
                        self.advance();
                        s
                    }
                    _ => {
                        let span = self.current().span;
                        self.push_error(span, "Ожидался ключ атрибута импорта");
                        return Err(());
                    }
                };
                self.expect_punct(PunctuationKind::Colon, "Ожидалось ':' в атрибутах импорта")?;
                if !matches!(self.current().kind, TokenKind::StringLiteral) {
                    let span = self.current().span;
                    self.push_error(span, "Значение атрибута импорта должно быть строкой");
                    return Err(());
                }
                let raw = self.source.slice(self.current().span);
                let inner = Self::strip_delimiters(raw, 1);
                let value = Self::unescape_string(inner);
                self.advance();
                attrs.push((key, value));

                if matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::Comma)) {
                    self.advance();
                    if matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::RBrace)) {
                        break;
                    }
                } else {
                    break;
                }
            }
        }
        self.expect_punct(PunctuationKind::RBrace, "Ожидалась '}' в атрибутах импорта")?;
        Ok(attrs)
    }

    pub(super) fn parse_export_stmt(&mut self) -> Result<Stmt, ()> {
        let start = self.current().span.start;
        self.advance();

        if matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::LBrace)) {
            self.advance();
            let mut names: Vec<crate::ast::Identifier> = Vec::new();
            if !matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::RBrace)) {
                loop {
                    names.push(self.parse_identifier()?);
                    if matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::Comma)) {
                        self.advance();
                    } else {
                        break;
                    }
                }
            }
            self.expect_punct(PunctuationKind::RBrace, "Ожидалась '}' в списке экспортов")?;
            let end = self.expect_punct(PunctuationKind::Semicolon, "Ожидалась ';' после экспорта")?.end;
            return Ok(Stmt::Export { kind: crate::ast::ExportKind::Named(names), span: Span { start, end } });
        }

        let inner = self.parse_statement()?;
        let end = match &inner {
            Stmt::VarDecl { span, .. }
            | Stmt::FunctionDecl { span, .. }
            | Stmt::ClassDecl { span, .. }
            | Stmt::Expr { span, .. } => span.end,
            _ => {
                let span = self.current().span;
                self.push_error(span, "После 'предъява' ожидается переменная, функция или класс");
                return Err(());
            }
        };
        Ok(Stmt::Export { kind: crate::ast::ExportKind::Declaration(Box::new(inner)), span: Span { start, end } })
    }
}
