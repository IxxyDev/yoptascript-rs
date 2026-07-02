use super::*;

impl<'a> Parser<'a> {
    pub(super) fn parse_primary(&mut self) -> Result<Expr, ()> {
        match &self.current().kind {
            TokenKind::Number => Ok(self.parse_number()),
            TokenKind::StringLiteral => Ok(self.parse_string()),
            TokenKind::RegexLiteral => Ok(self.parse_regex()),
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
            TokenKind::Keyword(KeywordKind::This) => {
                let span = self.current().span;
                self.advance();
                Ok(Expr::This { span })
            }
            TokenKind::Keyword(KeywordKind::Super) => {
                let span = self.current().span;
                self.advance();
                Ok(Expr::Super { span })
            }
            TokenKind::Punctuation(PunctuationKind::LBracket) => self.parse_array(),
            TokenKind::Punctuation(PunctuationKind::LBrace) => self.parse_object(),
            _ => {
                let span = self.current().span;
                self.push_error(span, format!("Неожиданный токен: {:?}", self.current().kind));
                Err(())
            }
        }
    }

    pub(super) fn parse_number(&mut self) -> Expr {
        let span = self.current().span;
        let raw = self.source.slice(span).to_string();
        self.advance();
        if let Some(stripped) = raw.strip_suffix('n') {
            let cleaned: String = stripped.chars().filter(|c| *c != '_').collect();
            let parsed = if let Some(hex) = cleaned.strip_prefix("0x").or_else(|| cleaned.strip_prefix("0X")) {
                i128::from_str_radix(hex, 16)
            } else if let Some(oct) = cleaned.strip_prefix("0o").or_else(|| cleaned.strip_prefix("0O")) {
                i128::from_str_radix(oct, 8)
            } else if let Some(bin) = cleaned.strip_prefix("0b").or_else(|| cleaned.strip_prefix("0B")) {
                i128::from_str_radix(bin, 2)
            } else {
                cleaned.parse::<i128>()
            };
            match parsed {
                Ok(value) => Expr::Literal(Literal::BigInt { value, span }),
                Err(_) => {
                    self.push_error(span, format!("Невалидный BigInt: '{raw}'"));
                    Expr::Literal(Literal::BigInt { value: 0, span })
                }
            }
        } else {
            Expr::Literal(Literal::Number { raw, span })
        }
    }

    pub(super) fn parse_string(&mut self) -> Expr {
        let span = self.current().span;
        let raw = self.source.slice(span);
        let inner = Self::strip_delimiters(raw, 1);
        let value = Self::unescape_string(inner);
        self.advance();
        Expr::Literal(Literal::String { value, span })
    }

    pub(super) fn parse_regex(&mut self) -> Expr {
        let span = self.current().span;
        let raw = self.source.slice(span);
        let bytes = raw.as_bytes();
        let mut i = 1;
        let mut in_class = false;
        let mut pat_end = bytes.len();
        while i < bytes.len() {
            let c = bytes[i];
            if c == b'\\' && i + 1 < bytes.len() {
                i += 2;
                continue;
            }
            if c == b'[' {
                in_class = true;
            } else if c == b']' && in_class {
                in_class = false;
            } else if c == b'/' && !in_class {
                pat_end = i;
                break;
            }
            i += 1;
        }
        let pattern = raw.get(1..pat_end).unwrap_or("").to_string();
        let flags = if pat_end < raw.len() { raw[pat_end + 1..].to_string() } else { String::new() };
        self.advance();
        Expr::Literal(Literal::RegExp { pattern, flags, span })
    }

    pub(super) fn unescape_string(s: &str) -> String {
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

    pub(super) fn strip_delimiters(raw: &str, suffix: usize) -> &str {
        raw.get(1..raw.len().saturating_sub(suffix)).unwrap_or("")
    }

    pub(super) fn parse_template_nosub(&mut self) -> Expr {
        let span = self.current().span;
        let raw = self.source.slice(span);
        let inner = Self::strip_delimiters(raw, 1);
        let value = Self::unescape_string(inner);
        self.advance();
        Expr::Literal(Literal::String { value, span })
    }

    pub(super) fn parse_template_literal(&mut self) -> Result<Expr, ()> {
        let start = self.current().span.start;
        let mut parts = Vec::new();

        let head_span = self.current().span;
        let head_raw = self.source.slice(head_span);
        let head_text = Self::strip_delimiters(head_raw, 2);
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
                    let mid_text = Self::strip_delimiters(mid_raw, 2);
                    parts.push(TemplatePart::Str(Self::unescape_string(mid_text)));
                    self.advance();
                }
                TokenKind::TemplateTail => {
                    let tail_span = self.current().span;
                    let tail_raw = self.source.slice(tail_span);
                    let tail_text = Self::strip_delimiters(tail_raw, 1);
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

    pub(super) fn parse_tagged_template(&mut self, tag: Expr) -> Result<Expr, ()> {
        let start = tag.span().start;
        let mut quasis: Vec<TemplateQuasi> = Vec::new();
        let mut expressions: Vec<Expr> = Vec::new();
        let end;

        if matches!(self.current().kind, TokenKind::TemplateNoSub) {
            let span = self.current().span;
            let raw_slice = self.source.slice(span);
            let raw = Self::strip_delimiters(raw_slice, 1).to_string();
            quasis.push(TemplateQuasi { cooked: Self::unescape_string(&raw), raw });
            end = span.end;
            self.advance();
        } else {
            let head_span = self.current().span;
            let head_raw = self.source.slice(head_span);
            let head_text = Self::strip_delimiters(head_raw, 2).to_string();
            quasis.push(TemplateQuasi { cooked: Self::unescape_string(&head_text), raw: head_text });
            self.advance();

            loop {
                let expr = self.parse_expr()?;
                expressions.push(expr);

                match &self.current().kind {
                    TokenKind::TemplateMiddle => {
                        let span = self.current().span;
                        let raw_slice = self.source.slice(span);
                        let text = Self::strip_delimiters(raw_slice, 2).to_string();
                        quasis.push(TemplateQuasi { cooked: Self::unescape_string(&text), raw: text });
                        self.advance();
                    }
                    TokenKind::TemplateTail => {
                        let span = self.current().span;
                        let raw_slice = self.source.slice(span);
                        let text = Self::strip_delimiters(raw_slice, 1).to_string();
                        quasis.push(TemplateQuasi { cooked: Self::unescape_string(&text), raw: text });
                        end = span.end;
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
        }

        Ok(Expr::TaggedTemplate { tag: Box::new(tag), quasis, expressions, span: Span { start, end } })
    }

    pub(super) fn parse_identifier(&mut self) -> Result<Identifier, ()> {
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

    pub(super) fn parse_grouping(&mut self) -> Result<Expr, ()> {
        let start = self.current().span.start;
        self.advance();

        let expr = self.parse_expr()?;

        let end = self.expect_punct(PunctuationKind::RParen, "Ожидался ')'")?.end;

        Ok(Expr::Grouping { expr: Box::new(expr), span: Span { start, end } })
    }

    pub(super) fn parse_array(&mut self) -> Result<Expr, ()> {
        let start = self.current().span.start;
        self.advance();

        let mut elements = Vec::new();
        if !matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::RBracket)) {
            loop {
                if matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::Spread)) {
                    let spread_start = self.current().span.start;
                    self.advance();
                    let expr = self.parse_expr()?;
                    let spread_end = expr.span().end;
                    elements.push(Expr::Spread {
                        expr: Box::new(expr),
                        span: Span { start: spread_start, end: spread_end },
                    });
                } else {
                    elements.push(self.parse_expr()?);
                }

                if matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::Comma)) {
                    self.advance();
                } else {
                    break;
                }
            }
        }

        let end = self.expect_punct(PunctuationKind::RBracket, "Ожидался ']'")?.end;

        Ok(Expr::Literal(Literal::Array { elements, span: Span { start, end } }))
    }

    pub(super) fn parse_object(&mut self) -> Result<Expr, ()> {
        let start = self.current().span.start;
        self.advance();

        let mut entries = Vec::new();
        if !matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::RBrace)) {
            loop {
                if matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::Spread)) {
                    self.advance();
                    let expr = self.parse_expr()?;
                    entries.push(ObjectEntry::Spread(expr));
                } else if matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::LBracket)) {
                    self.advance();
                    let key_expr = self.parse_expr()?;
                    self.expect_punct(PunctuationKind::RBracket, "Ожидался ']' после вычисляемого ключа")?;
                    if matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::LParen)) {
                        self.advance();
                        let params = self.parse_function_params()?;
                        self.expect_punct(PunctuationKind::RParen, "Ожидалась ')' после параметров метода")?;
                        let body = self.parse_block()?;
                        let func_span = body.span;
                        let value = Expr::ArrowFunction {
                            params: params.into(),
                            body: Rc::new(body),
                            is_async: false,
                            span: func_span,
                        };
                        entries.push(ObjectEntry::Property { key: PropKey::Computed(key_expr), value });
                    } else if matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::Colon)) {
                        self.advance();
                        let value = self.parse_expr()?;
                        entries.push(ObjectEntry::Property { key: PropKey::Computed(key_expr), value });
                    } else {
                        let span = self.current().span;
                        self.push_error(span, "Ожидалось ':' или '(' после вычисляемого ключа объекта");
                        return Err(());
                    }
                } else if matches!(self.current().kind, TokenKind::StringLiteral) {
                    let string_expr = self.parse_string();
                    let key_expr = match &string_expr {
                        Expr::Literal(Literal::String { value, span }) => {
                            PropKey::Identifier(Identifier { name: value.clone(), span: *span })
                        }
                        _ => unreachable!(),
                    };
                    self.expect_punct(PunctuationKind::Colon, "Ожидалось ':' после ключа объекта")?;
                    let value = self.parse_expr()?;
                    entries.push(ObjectEntry::Property { key: key_expr, value });
                } else if matches!(self.current().kind, TokenKind::Identifier)
                    && self.source.slice(self.current().span) == "get"
                    && matches!(self.peek(1).kind, TokenKind::Identifier)
                {
                    let gs_start = self.current().span.start;
                    self.advance();
                    let key = self.parse_identifier()?;
                    self.expect_punct(PunctuationKind::LParen, "Ожидалась '(' после имени геттера")?;
                    self.expect_punct(PunctuationKind::RParen, "Геттер не принимает параметров")?;
                    let body = self.parse_block()?;
                    let gs_end = body.span.end;
                    entries.push(ObjectEntry::Getter {
                        key: PropKey::Identifier(key),
                        body,
                        span: Span { start: gs_start, end: gs_end },
                    });
                } else if matches!(self.current().kind, TokenKind::Identifier)
                    && self.source.slice(self.current().span) == "set"
                    && matches!(self.peek(1).kind, TokenKind::Identifier)
                {
                    let gs_start = self.current().span.start;
                    self.advance();
                    let key = self.parse_identifier()?;
                    self.expect_punct(PunctuationKind::LParen, "Ожидалась '(' после имени сеттера")?;
                    let params = self.parse_function_params()?;
                    if params.len() != 1 {
                        let span = self.current().span;
                        self.push_error(span, "Сеттер принимает ровно один параметр");
                        return Err(());
                    }
                    self.expect_punct(PunctuationKind::RParen, "Ожидалась ')' после параметра сеттера")?;
                    let body = self.parse_block()?;
                    let gs_end = body.span.end;
                    let param = params.into_iter().next().unwrap();
                    entries.push(ObjectEntry::Setter {
                        key: PropKey::Identifier(key),
                        param,
                        body,
                        span: Span { start: gs_start, end: gs_end },
                    });
                } else {
                    let key = self.parse_identifier()?;
                    if matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::Colon)) {
                        self.advance();
                        let value = self.parse_expr()?;
                        entries.push(ObjectEntry::Property { key: PropKey::Identifier(key), value });
                    } else if matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::LParen)) {
                        self.advance();
                        let params = self.parse_function_params()?;
                        self.expect_punct(PunctuationKind::RParen, "Ожидалась ')' после параметров метода")?;
                        let body = self.parse_block()?;
                        let func_span = Span { start: key.span.start, end: body.span.end };
                        let value = Expr::ArrowFunction {
                            params: params.into(),
                            body: Rc::new(body),
                            is_async: false,
                            span: func_span,
                        };
                        entries.push(ObjectEntry::Property { key: PropKey::Identifier(key), value });
                    } else {
                        let value = Expr::Identifier(key.clone());
                        entries.push(ObjectEntry::Property { key: PropKey::Identifier(key), value });
                    }
                }

                if matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::Comma)) {
                    self.advance();
                } else {
                    break;
                }
            }
        }

        let end = self.expect_punct(PunctuationKind::RBrace, "Ожидался '}'")?.end;

        Ok(Expr::Literal(Literal::Object { entries, span: Span { start, end } }))
    }
}
