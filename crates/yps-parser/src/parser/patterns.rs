use super::*;

impl<'a> Parser<'a> {
    pub(super) fn parse_pattern_with_default(&mut self) -> Result<Pattern, ()> {
        let pattern = self.parse_pattern()?;
        if matches!(self.current().kind, TokenKind::Operator(OperatorKind::Assign)) {
            self.advance();
            let default = self.parse_expr()?;
            let span = Span { start: pattern.span().start, end: default.span().end };
            return Ok(Pattern::Default { pattern: Box::new(pattern), default: Box::new(default), span });
        }
        Ok(pattern)
    }

    pub(super) fn parse_pattern(&mut self) -> Result<Pattern, ()> {
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

    pub(super) fn parse_array_pattern(&mut self) -> Result<Pattern, ()> {
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
                    elements.push(Some(self.parse_pattern_with_default()?));
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

    pub(super) fn parse_object_pattern(&mut self) -> Result<Pattern, ()> {
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
                    Some(self.parse_pattern_with_default()?)
                } else if matches!(self.current().kind, TokenKind::Operator(OperatorKind::Assign)) {
                    self.advance();
                    let default = self.parse_expr()?;
                    let span = Span { start: key.span.start, end: default.span().end };
                    Some(Pattern::Default {
                        pattern: Box::new(Pattern::Identifier(key.clone())),
                        default: Box::new(default),
                        span,
                    })
                } else {
                    None
                };

                let prop_end = value.as_ref().map_or(key.span.end, |p| p.span().end);

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
}
