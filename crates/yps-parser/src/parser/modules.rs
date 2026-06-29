use super::*;

impl<'a> Parser<'a> {
    pub(super) fn parse_dynamic_import(&mut self) -> Result<Expr, ()> {
        let start = self.current().span.start;
        self.advance();
        if !matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::LParen)) {
            let span = self.current().span;
            self.push_error(span, "Ожидалась '(' после 'спиздить' в динамическом импорте");
            return Err(());
        }
        self.advance();
        let source = self.parse_expr()?;
        if !matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::RParen)) {
            let span = self.current().span;
            self.push_error(span, "Ожидалась ')' в динамическом импорте");
            return Err(());
        }
        let end = self.current().span.end;
        self.advance();
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
            if !matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::RBrace)) {
                let span = self.current().span;
                self.push_error(span, "Ожидалась '}' в списке импортов");
                return Err(());
            }
            self.advance();
        } else if matches!(self.current().kind, TokenKind::Identifier) {
            let local = self.parse_identifier()?;
            specifiers.push(crate::ast::ImportSpec::Default { local });
        } else {
            let span = self.current().span;
            self.push_error(span, "Ожидался идентификатор или '{' после 'спиздить'");
            return Err(());
        }

        if !matches!(self.current().kind, TokenKind::Keyword(KeywordKind::In)) {
            let span = self.current().span;
            self.push_error(span, "Ожидалось 'из' в импорте");
            return Err(());
        }
        self.advance();

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

        if !matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::Semicolon)) {
            let span = self.current().span;
            self.push_error(span, "Ожидалась ';' после импорта");
            return Err(());
        }
        let end = self.current().span.end;
        self.advance();

        Ok(Stmt::Import { specifiers, source, attributes, span: Span { start, end } })
    }

    pub(super) fn parse_import_attributes(&mut self) -> Result<Vec<(String, String)>, ()> {
        if !matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::LBrace)) {
            let span = self.current().span;
            self.push_error(span, "Ожидалась '{' после 'with' в импорте");
            return Err(());
        }
        self.advance();

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
                if !matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::Colon)) {
                    let span = self.current().span;
                    self.push_error(span, "Ожидалось ':' в атрибутах импорта");
                    return Err(());
                }
                self.advance();
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
        if !matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::RBrace)) {
            let span = self.current().span;
            self.push_error(span, "Ожидалась '}' в атрибутах импорта");
            return Err(());
        }
        self.advance();
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
            if !matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::RBrace)) {
                let span = self.current().span;
                self.push_error(span, "Ожидалась '}' в списке экспортов");
                return Err(());
            }
            self.advance();
            if !matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::Semicolon)) {
                let span = self.current().span;
                self.push_error(span, "Ожидалась ';' после экспорта");
                return Err(());
            }
            let end = self.current().span.end;
            self.advance();
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
