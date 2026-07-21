use super::*;

impl<'a> Parser<'a> {
    pub(super) fn parse_expr(&mut self) -> Result<Expr, ()> {
        match self.current().kind {
            TokenKind::Keyword(KeywordKind::Yield) => self.parse_yield_expr(false),
            TokenKind::Keyword(KeywordKind::YieldDelegate) => self.parse_yield_expr(true),
            _ => self.parse_expression_with_precedence(0),
        }
    }

    pub(super) fn parse_yield_expr(&mut self, delegate: bool) -> Result<Expr, ()> {
        let start = self.current().span.start;
        let mut end = self.current().span.end;
        self.advance();

        let argument = if delegate || self.is_yield_argument_start() {
            let expr = self.parse_expr()?;
            end = expr.span().end;
            Some(Box::new(expr))
        } else {
            None
        };

        if delegate && argument.is_none() {
            self.push_error(Span { start, end }, "'поебалуна' требует аргумент");
            return Err(());
        }

        Ok(Expr::Yield { argument, delegate, span: Span { start, end } })
    }

    pub(super) fn is_yield_argument_start(&self) -> bool {
        !matches!(
            self.current().kind,
            TokenKind::Punctuation(
                PunctuationKind::Semicolon
                    | PunctuationKind::RParen
                    | PunctuationKind::RBracket
                    | PunctuationKind::RBrace
                    | PunctuationKind::Comma
                    | PunctuationKind::Colon
            ) | TokenKind::Eof
        )
    }

    pub(super) fn parse_expression_with_precedence(&mut self, min_precedence: u8) -> Result<Expr, ()> {
        self.enter_depth()?;
        let result = stacker::maybe_grow(STACK_RED_ZONE, STACK_GROW_SIZE, || {
            self.parse_expression_with_precedence_inner(min_precedence)
        });
        self.depth -= 1;
        result
    }

    pub(super) fn parse_expression_with_precedence_inner(&mut self, min_precedence: u8) -> Result<Expr, ()> {
        let mut lhs = self.parse_prefix()?;
        let mut chain_len = 0usize;

        loop {
            chain_len += 1;
            if chain_len > MAX_CHAIN_LEN {
                let span = self.current().span;
                self.push_error(span, "Слишком длинная цепочка операций");
                return Err(());
            }
            if matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::Question))
                && min_precedence <= TERNARY_PRECEDENCE
            {
                let start = lhs.span().start;
                self.advance();
                let then_expr = self.parse_expression_with_precedence(0)?;
                self.expect_punct(PunctuationKind::Colon, "Ожидалось ':' в тернарном операторе")?;
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

            let Some(op) = self.try_parse_binary_op() else {
                break;
            };

            let precedence = crate::precedence::binary_precedence(op);
            if precedence < min_precedence {
                break;
            }

            self.advance();

            let right_assoc = crate::precedence::binary_is_right_assoc(op);
            let next_prec = if right_assoc { precedence } else { precedence + 1 };
            let rhs = self.parse_expression_with_precedence(next_prec)?;

            let start = lhs.span().start;
            let end = rhs.span().end;
            lhs = Expr::Binary { op, lhs: Box::new(lhs), rhs: Box::new(rhs), span: Span { start, end } };
        }

        Ok(lhs)
    }

    pub(super) fn parse_unary(&mut self, op: UnaryOp) -> Result<Expr, ()> {
        let start = self.current().span.start;
        self.advance();
        let expr = self.parse_expression_with_precedence(UNARY_PRECEDENCE)?;
        let end = expr.span().end;
        Ok(Expr::Unary { op, expr: Box::new(expr), span: Span { start, end } })
    }

    pub(super) fn parse_prefix(&mut self) -> Result<Expr, ()> {
        let mut expr = match &self.current().kind {
            TokenKind::Operator(OperatorKind::Plus) => self.parse_unary(UnaryOp::Plus)?,
            TokenKind::Operator(OperatorKind::Minus) => self.parse_unary(UnaryOp::Minus)?,
            TokenKind::Operator(OperatorKind::Not) => self.parse_unary(UnaryOp::Not)?,
            TokenKind::Operator(OperatorKind::BitwiseNot) => self.parse_unary(UnaryOp::BitwiseNot)?,
            TokenKind::Keyword(KeywordKind::Typeof) => self.parse_unary(UnaryOp::Typeof)?,
            TokenKind::Keyword(KeywordKind::Delete) => self.parse_unary(UnaryOp::Delete)?,
            TokenKind::Keyword(KeywordKind::Void) => self.parse_unary(UnaryOp::Void)?,
            TokenKind::Keyword(KeywordKind::Await) => {
                let start = self.current().span.start;
                self.advance();
                let expr = self.parse_expression_with_precedence(UNARY_PRECEDENCE)?;
                let end = expr.span().end;
                Expr::Await { argument: Box::new(expr), span: Span { start, end } }
            }
            TokenKind::Keyword(KeywordKind::Async) => self.parse_async_expr()?,
            TokenKind::Keyword(KeywordKind::Yopta) => self.parse_function_expr()?,
            TokenKind::Keyword(KeywordKind::GeneratorFn) => self.parse_generator_expr()?,
            TokenKind::Keyword(KeywordKind::New) => self.parse_new_expr()?,
            TokenKind::Keyword(KeywordKind::Import)
                if matches!(self.peek(1).kind, TokenKind::Punctuation(PunctuationKind::LParen)) =>
            {
                self.parse_dynamic_import()?
            }
            _ => self.parse_primary()?,
        };

        let mut chain_len = 0usize;
        loop {
            chain_len += 1;
            if chain_len > MAX_CHAIN_LEN {
                let span = self.current().span;
                self.push_error(span, "Слишком длинная цепочка обращений");
                return Err(());
            }
            if matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::LParen)) {
                expr = self.parse_call(expr)?;
            } else if matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::LBracket)) {
                expr = self.parse_index(expr)?;
            } else if matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::Dot)) {
                expr = self.parse_member(expr)?;
            } else if matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::OptionalChain)) {
                expr = self.parse_optional_chain(expr)?;
            } else if matches!(self.current().kind, TokenKind::TemplateHead | TokenKind::TemplateNoSub) {
                expr = self.parse_tagged_template(expr)?;
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

    pub(super) fn parse_arguments(&mut self, close: PunctuationKind, msg: &str) -> Result<(Vec<Expr>, Span), ()> {
        let mut args = Vec::new();
        if !matches!(&self.current().kind, TokenKind::Punctuation(k) if *k == close) {
            loop {
                if matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::Spread)) {
                    let spread_start = self.current().span.start;
                    self.advance();
                    let expr = self.parse_expr()?;
                    let spread_end = expr.span().end;
                    args.push(Expr::Spread {
                        expr: Box::new(expr),
                        span: Span { start: spread_start, end: spread_end },
                    });
                } else {
                    args.push(self.parse_expr()?);
                }

                if matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::Comma)) {
                    self.advance();
                } else {
                    break;
                }
            }
        }

        let close_span = self.expect_punct(close, msg)?;
        Ok((args, close_span))
    }

    pub(super) fn parse_call(&mut self, callee: Expr) -> Result<Expr, ()> {
        let start = callee.span().start;
        self.advance();

        let (args, close) = self.parse_arguments(PunctuationKind::RParen, "Ожидалась ')' после аргументов функции")?;

        Ok(Expr::Call { callee: Box::new(callee), args, span: Span { start, end: close.end } })
    }

    pub(super) fn parse_index(&mut self, object: Expr) -> Result<Expr, ()> {
        let start = object.span().start;
        self.advance();

        let index = self.parse_expr()?;

        let end = self.expect_punct(PunctuationKind::RBracket, "Ожидался ']'")?.end;

        Ok(Expr::Index { object: Box::new(object), index: Box::new(index), span: Span { start, end } })
    }

    pub(super) fn parse_member(&mut self, object: Expr) -> Result<Expr, ()> {
        let start = object.span().start;
        self.advance();

        let property = if matches!(self.current().kind, TokenKind::PrivateIdentifier) {
            let span = self.current().span;
            let name = self.source.slice(span).to_string();
            self.advance();
            Identifier { name, span }
        } else {
            self.parse_identifier()?
        };

        let end = property.span.end;

        Ok(Expr::Member { object: Box::new(object), property, span: Span { start, end } })
    }

    pub(super) fn parse_optional_chain(&mut self, object: Expr) -> Result<Expr, ()> {
        let start = object.span().start;
        self.advance();

        if matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::LParen)) {
            self.advance();
            let (args, close) =
                self.parse_arguments(PunctuationKind::RParen, "Ожидалась ')' после аргументов функции")?;
            Ok(Expr::OptionalCall { callee: Box::new(object), args, span: Span { start, end: close.end } })
        } else if matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::LBracket)) {
            self.advance();
            let index = self.parse_expr()?;
            let end = self.expect_punct(PunctuationKind::RBracket, "Ожидался ']'")?.end;
            Ok(Expr::OptionalIndex { object: Box::new(object), index: Box::new(index), span: Span { start, end } })
        } else {
            let property = self.parse_identifier()?;
            let end = property.span.end;
            Ok(Expr::OptionalMember { object: Box::new(object), property, span: Span { start, end } })
        }
    }

    pub(super) fn parse_new_expr(&mut self) -> Result<Expr, ()> {
        let start = self.current().span.start;
        self.advance();

        let mut callee = self.parse_primary()?;
        while matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::Dot)) {
            callee = self.parse_member(callee)?;
        }

        let (args, end) = if matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::LParen)) {
            self.advance();
            let (args, close) =
                self.parse_arguments(PunctuationKind::RParen, "Ожидалась ')' после аргументов конструктора")?;
            (args, close.end)
        } else {
            (Vec::new(), callee.span().end)
        };

        Ok(Expr::New { callee: Box::new(callee), args, span: Span { start, end } })
    }

    pub(super) fn try_parse_binary_op(&self) -> Option<BinaryOp> {
        match &self.current().kind {
            TokenKind::Operator(op_kind) => match op_kind {
                OperatorKind::Assign => Some(BinaryOp::Assign),
                OperatorKind::PlusAssign => Some(BinaryOp::PlusAssign),
                OperatorKind::MinusAssign => Some(BinaryOp::MinusAssign),
                OperatorKind::MulAssign => Some(BinaryOp::MulAssign),
                OperatorKind::DivAssign => Some(BinaryOp::DivAssign),
                OperatorKind::ExponentAssign => Some(BinaryOp::ExpAssign),
                OperatorKind::NullishAssign => Some(BinaryOp::NullishAssign),
                OperatorKind::AndAssign => Some(BinaryOp::AndAssign),
                OperatorKind::OrAssign => Some(BinaryOp::OrAssign),
                OperatorKind::ModAssign => Some(BinaryOp::ModAssign),
                OperatorKind::BitAndAssign => Some(BinaryOp::BitAndAssign),
                OperatorKind::BitOrAssign => Some(BinaryOp::BitOrAssign),
                OperatorKind::BitXorAssign => Some(BinaryOp::BitXorAssign),
                OperatorKind::ShlAssign => Some(BinaryOp::ShlAssign),
                OperatorKind::ShrAssign => Some(BinaryOp::ShrAssign),
                OperatorKind::UshrAssign => Some(BinaryOp::UshrAssign),
                OperatorKind::Or => Some(BinaryOp::Or),
                OperatorKind::NullishCoalescing => Some(BinaryOp::NullishCoalescing),
                OperatorKind::And => Some(BinaryOp::And),
                OperatorKind::BitOr => Some(BinaryOp::BitOr),
                OperatorKind::BitXor => Some(BinaryOp::BitXor),
                OperatorKind::BitAnd => Some(BinaryOp::BitAnd),
                OperatorKind::Equals => Some(BinaryOp::Equals),
                OperatorKind::StrictEquals => Some(BinaryOp::StrictEquals),
                OperatorKind::NotEquals => Some(BinaryOp::NotEquals),
                OperatorKind::StrictNotEquals => Some(BinaryOp::StrictNotEquals),
                OperatorKind::Less => Some(BinaryOp::Less),
                OperatorKind::Greater => Some(BinaryOp::Greater),
                OperatorKind::LessOrEqual => Some(BinaryOp::LessOrEqual),
                OperatorKind::GreaterOrEqual => Some(BinaryOp::GreaterOrEqual),
                OperatorKind::Pipeline => Some(BinaryOp::Pipeline),
                OperatorKind::LeftShift => Some(BinaryOp::LeftShift),
                OperatorKind::RightShift => Some(BinaryOp::RightShift),
                OperatorKind::UnsignedRightShift => Some(BinaryOp::UnsignedRightShift),
                OperatorKind::Plus => Some(BinaryOp::Add),
                OperatorKind::Minus => Some(BinaryOp::Sub),
                OperatorKind::Multiply => Some(BinaryOp::Mul),
                OperatorKind::Divide => Some(BinaryOp::Div),
                OperatorKind::Modulo => Some(BinaryOp::Mod),
                OperatorKind::Exponent => Some(BinaryOp::Exp),
                OperatorKind::Not | OperatorKind::BitwiseNot | OperatorKind::Increment | OperatorKind::Decrement => {
                    None
                }
            },
            TokenKind::Keyword(KeywordKind::Instanceof) => Some(BinaryOp::Instanceof),
            TokenKind::Keyword(KeywordKind::In) => Some(BinaryOp::In),
            _ => None,
        }
    }
}
