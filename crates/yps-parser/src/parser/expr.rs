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

            let right_assoc = crate::precedence::binary_is_right_assoc(op);
            let next_prec = if right_assoc { precedence } else { precedence + 1 };
            let rhs = self.parse_expression_with_precedence(next_prec)?;

            let start = lhs.span().start;
            let end = rhs.span().end;
            lhs = Expr::Binary { op, lhs: Box::new(lhs), rhs: Box::new(rhs), span: Span { start, end } };
        }

        Ok(lhs)
    }

    pub(super) fn parse_prefix(&mut self) -> Result<Expr, ()> {
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
            TokenKind::Operator(OperatorKind::BitwiseNot) => {
                let start = self.current().span.start;
                self.advance();
                let expr = self.parse_expression_with_precedence(UNARY_PRECEDENCE)?;
                let end = expr.span().end;
                Expr::Unary { op: UnaryOp::BitwiseNot, expr: Box::new(expr), span: Span { start, end } }
            }
            TokenKind::Keyword(KeywordKind::Typeof) => {
                let start = self.current().span.start;
                self.advance();
                let expr = self.parse_expression_with_precedence(UNARY_PRECEDENCE)?;
                let end = expr.span().end;
                Expr::Unary { op: UnaryOp::Typeof, expr: Box::new(expr), span: Span { start, end } }
            }
            TokenKind::Keyword(KeywordKind::Delete) => {
                let start = self.current().span.start;
                self.advance();
                let expr = self.parse_expression_with_precedence(UNARY_PRECEDENCE)?;
                let end = expr.span().end;
                Expr::Unary { op: UnaryOp::Delete, expr: Box::new(expr), span: Span { start, end } }
            }
            TokenKind::Keyword(KeywordKind::Void) => {
                let start = self.current().span.start;
                self.advance();
                let expr = self.parse_expression_with_precedence(UNARY_PRECEDENCE)?;
                let end = expr.span().end;
                Expr::Unary { op: UnaryOp::Void, expr: Box::new(expr), span: Span { start, end } }
            }
            TokenKind::Keyword(KeywordKind::Await) => {
                let start = self.current().span.start;
                self.advance();
                let expr = self.parse_expression_with_precedence(UNARY_PRECEDENCE)?;
                let end = expr.span().end;
                Expr::Await { argument: Box::new(expr), span: Span { start, end } }
            }
            TokenKind::Keyword(KeywordKind::Async) => self.parse_async_expr()?,
            TokenKind::Keyword(KeywordKind::Yopta) => self.parse_function_expr()?,
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

    pub(super) fn parse_call(&mut self, callee: Expr) -> Result<Expr, ()> {
        let start = callee.span().start;
        self.advance();

        let mut args = Vec::new();
        if !matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::RParen)) {
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

        if !matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::RParen)) {
            let span = self.current().span;
            self.push_error(span, "Ожидалась ')' после аргументов функции");
            return Err(());
        }
        let end = self.current().span.end;
        self.advance();

        Ok(Expr::Call { callee: Box::new(callee), args, span: Span { start, end } })
    }

    pub(super) fn parse_index(&mut self, object: Expr) -> Result<Expr, ()> {
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
            Ok(Expr::OptionalCall { callee: Box::new(object), args, span: Span { start, end } })
        } else if matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::LBracket)) {
            self.advance();
            let index = self.parse_expr()?;
            if !matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::RBracket)) {
                let span = self.current().span;
                self.push_error(span, "Ожидался ']'");
                return Err(());
            }
            let end = self.current().span.end;
            self.advance();
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

        let mut args = Vec::new();
        let end;
        if matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::LParen)) {
            self.advance();
            if !matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::RParen)) {
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
            if !matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::RParen)) {
                let span = self.current().span;
                self.push_error(span, "Ожидалась ')' после аргументов конструктора");
                return Err(());
            }
            end = self.current().span.end;
            self.advance();
        } else {
            end = callee.span().end;
        }

        Ok(Expr::New { callee: Box::new(callee), args, span: Span { start, end } })
    }

    pub(super) fn try_parse_binary_op(&self) -> Option<(BinaryOp, u8)> {
        match &self.current().kind {
            TokenKind::Operator(op_kind) => match op_kind {
                OperatorKind::Assign => Some((BinaryOp::Assign, 1)),
                OperatorKind::PlusAssign => Some((BinaryOp::PlusAssign, 1)),
                OperatorKind::MinusAssign => Some((BinaryOp::MinusAssign, 1)),
                OperatorKind::MulAssign => Some((BinaryOp::MulAssign, 1)),
                OperatorKind::DivAssign => Some((BinaryOp::DivAssign, 1)),
                OperatorKind::ExponentAssign => Some((BinaryOp::ExpAssign, 1)),
                OperatorKind::NullishAssign => Some((BinaryOp::NullishAssign, 1)),
                OperatorKind::AndAssign => Some((BinaryOp::AndAssign, 1)),
                OperatorKind::OrAssign => Some((BinaryOp::OrAssign, 1)),
                OperatorKind::ModAssign => Some((BinaryOp::ModAssign, 1)),
                OperatorKind::BitAndAssign => Some((BinaryOp::BitAndAssign, 1)),
                OperatorKind::BitOrAssign => Some((BinaryOp::BitOrAssign, 1)),
                OperatorKind::BitXorAssign => Some((BinaryOp::BitXorAssign, 1)),
                OperatorKind::ShlAssign => Some((BinaryOp::ShlAssign, 1)),
                OperatorKind::ShrAssign => Some((BinaryOp::ShrAssign, 1)),
                OperatorKind::UshrAssign => Some((BinaryOp::UshrAssign, 1)),
                OperatorKind::Or => Some((BinaryOp::Or, 3)),
                OperatorKind::NullishCoalescing => Some((BinaryOp::NullishCoalescing, 4)),
                OperatorKind::And => Some((BinaryOp::And, 5)),
                OperatorKind::BitOr => Some((BinaryOp::BitOr, 6)),
                OperatorKind::BitXor => Some((BinaryOp::BitXor, 7)),
                OperatorKind::BitAnd => Some((BinaryOp::BitAnd, 8)),
                OperatorKind::Equals => Some((BinaryOp::Equals, 9)),
                OperatorKind::StrictEquals => Some((BinaryOp::StrictEquals, 9)),
                OperatorKind::NotEquals => Some((BinaryOp::NotEquals, 9)),
                OperatorKind::StrictNotEquals => Some((BinaryOp::StrictNotEquals, 9)),
                OperatorKind::Less => Some((BinaryOp::Less, 10)),
                OperatorKind::Greater => Some((BinaryOp::Greater, 10)),
                OperatorKind::LessOrEqual => Some((BinaryOp::LessOrEqual, 10)),
                OperatorKind::GreaterOrEqual => Some((BinaryOp::GreaterOrEqual, 10)),
                OperatorKind::Pipeline => Some((BinaryOp::Pipeline, 11)),
                OperatorKind::LeftShift => Some((BinaryOp::LeftShift, 12)),
                OperatorKind::RightShift => Some((BinaryOp::RightShift, 12)),
                OperatorKind::UnsignedRightShift => Some((BinaryOp::UnsignedRightShift, 12)),
                OperatorKind::Plus => Some((BinaryOp::Add, 13)),
                OperatorKind::Minus => Some((BinaryOp::Sub, 13)),
                OperatorKind::Multiply => Some((BinaryOp::Mul, 14)),
                OperatorKind::Divide => Some((BinaryOp::Div, 14)),
                OperatorKind::Modulo => Some((BinaryOp::Mod, 14)),
                OperatorKind::Exponent => Some((BinaryOp::Exp, 15)),
                OperatorKind::Not | OperatorKind::BitwiseNot | OperatorKind::Increment | OperatorKind::Decrement => {
                    None
                }
            },
            TokenKind::Keyword(KeywordKind::Instanceof) => Some((BinaryOp::Instanceof, 10)),
            TokenKind::Keyword(KeywordKind::In) => Some((BinaryOp::In, 10)),
            _ => None,
        }
    }
}
