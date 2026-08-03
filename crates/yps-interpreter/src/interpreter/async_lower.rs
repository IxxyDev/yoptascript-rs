use yps_lexer::Span;
use yps_parser::ast::{BinaryOp, Block, Expr, Identifier, Literal, ObjectEntry, Pattern, Stmt, TemplatePart, UnaryOp};

pub(crate) fn lower_async_body(body: &Block) -> Block {
    if !has_await_stmts(&body.stmts) {
        return body.clone();
    }
    let mut lowerer = Lowerer { counter: 0 };
    Block { stmts: lowerer.stmts(&body.stmts), span: body.span }
}

fn compound_base(op: BinaryOp) -> Option<BinaryOp> {
    match op {
        BinaryOp::PlusAssign => Some(BinaryOp::Add),
        BinaryOp::MinusAssign => Some(BinaryOp::Sub),
        BinaryOp::MulAssign => Some(BinaryOp::Mul),
        BinaryOp::DivAssign => Some(BinaryOp::Div),
        BinaryOp::ModAssign => Some(BinaryOp::Mod),
        BinaryOp::ExpAssign => Some(BinaryOp::Exp),
        BinaryOp::BitAndAssign => Some(BinaryOp::BitAnd),
        BinaryOp::BitOrAssign => Some(BinaryOp::BitOr),
        BinaryOp::BitXorAssign => Some(BinaryOp::BitXor),
        BinaryOp::ShlAssign => Some(BinaryOp::LeftShift),
        BinaryOp::ShrAssign => Some(BinaryOp::RightShift),
        BinaryOp::UshrAssign => Some(BinaryOp::UnsignedRightShift),
        _ => None,
    }
}

fn assign_bool(target: &Identifier, value: bool, span: Span) -> Stmt {
    Stmt::Expr {
        expr: Expr::Binary {
            op: BinaryOp::Assign,
            lhs: Box::new(Expr::Identifier(target.clone())),
            rhs: Box::new(Expr::Literal(Literal::Boolean { value, span })),
            span,
        },
        span,
    }
}

const fn is_short_circuit(op: BinaryOp) -> bool {
    matches!(op, BinaryOp::And | BinaryOp::Or | BinaryOp::NullishCoalescing)
}

fn has_await(e: &Expr) -> bool {
    match e {
        Expr::Await { .. } => true,
        Expr::Identifier(_) | Expr::This { .. } | Expr::Super { .. } => false,
        Expr::ArrowFunction { .. } | Expr::FunctionExpr { .. } => false,
        Expr::Literal(lit) => match lit {
            Literal::Array { elements, .. } => elements.iter().any(has_await),
            Literal::Object { entries, .. } => entries.iter().any(|entry| match entry {
                ObjectEntry::Property { value, .. } => has_await(value),
                ObjectEntry::Spread(e) => has_await(e),
                ObjectEntry::Getter { .. } | ObjectEntry::Setter { .. } => false,
            }),
            _ => false,
        },
        Expr::Unary { expr, .. }
        | Expr::Postfix { expr, .. }
        | Expr::Grouping { expr, .. }
        | Expr::Spread { expr, .. }
        | Expr::DynamicImport { source: expr, .. } => has_await(expr),
        Expr::Binary { lhs, rhs, .. } => has_await(lhs) || has_await(rhs),
        Expr::Assignment { value, .. } => has_await(value),
        Expr::Call { callee, args, .. } | Expr::OptionalCall { callee, args, .. } | Expr::New { callee, args, .. } => {
            has_await(callee) || args.iter().any(has_await)
        }
        Expr::Index { object, index, .. } | Expr::OptionalIndex { object, index, .. } => {
            has_await(object) || has_await(index)
        }
        Expr::Member { object, .. } | Expr::OptionalMember { object, .. } => has_await(object),
        Expr::Conditional { condition, then_expr, else_expr, .. } => {
            has_await(condition) || has_await(then_expr) || has_await(else_expr)
        }
        Expr::TemplateLiteral { parts, .. } => parts.iter().any(|p| match p {
            TemplatePart::Expr(e) => has_await(e),
            TemplatePart::Str(_) => false,
        }),
        Expr::TaggedTemplate { tag, expressions, .. } => has_await(tag) || expressions.iter().any(has_await),
        Expr::Yield { argument, .. } => argument.as_deref().is_some_and(has_await),
    }
}

fn has_await_stmts(stmts: &[Stmt]) -> bool {
    stmts.iter().any(has_await_stmt)
}

fn has_await_stmt(s: &Stmt) -> bool {
    match s {
        Stmt::VarDecl { init, .. } => has_await(init),
        Stmt::Expr { expr, .. } | Stmt::Throw { value: expr, .. } => has_await(expr),
        Stmt::Return { value, .. } => value.as_ref().is_some_and(has_await),
        Stmt::Block(b) => has_await_stmts(&b.stmts),
        Stmt::If { condition, then_branch, else_branch, .. } => {
            has_await(condition) || has_await_stmt(then_branch) || else_branch.as_deref().is_some_and(has_await_stmt)
        }
        Stmt::While { condition, body, .. } | Stmt::DoWhile { condition, body, .. } => {
            has_await(condition) || has_await_stmt(body)
        }
        Stmt::For { init, condition, update, body, .. } => {
            init.as_deref().is_some_and(has_await_stmt)
                || condition.as_ref().is_some_and(has_await)
                || update.as_ref().is_some_and(has_await)
                || has_await_stmt(body)
        }
        Stmt::ForIn { iterable, body, .. } | Stmt::ForOf { iterable, body, .. } => {
            has_await(iterable) || has_await_stmt(body)
        }
        Stmt::ForAwaitOf { iterable, body, .. } => has_await(iterable) || has_await_stmt(body),
        Stmt::TryCatch { try_block, catch_block, finally_block, .. } => {
            has_await_stmts(&try_block.stmts)
                || catch_block.as_ref().is_some_and(|b| has_await_stmts(&b.stmts))
                || finally_block.as_ref().is_some_and(|b| has_await_stmts(&b.stmts))
        }
        Stmt::Labeled { body, .. } => has_await_stmt(body),
        Stmt::Switch { expr, cases, default, .. } => {
            has_await(expr)
                || cases.iter().any(|c| has_await(&c.value) || has_await_stmts(&c.body.stmts))
                || default.as_ref().is_some_and(|b| has_await_stmts(&b.stmts))
        }
        Stmt::Using { init, .. } => has_await(init),
        Stmt::ClassDecl { super_class, decorators, .. } => {
            super_class.as_ref().is_some_and(has_await) || decorators.iter().any(has_await)
        }
        _ => false,
    }
}

struct Lowerer {
    counter: usize,
}

impl Lowerer {
    fn fresh(&mut self, span: Span) -> Identifier {
        self.counter += 1;
        Identifier { name: format!("#ожидание{}", self.counter), span }
    }

    fn flag(&mut self, span: Span, out: &mut Vec<Stmt>) -> Identifier {
        let id = self.fresh(span);
        out.push(Stmt::VarDecl {
            pattern: Pattern::Identifier(id.clone()),
            init: Expr::Literal(Literal::Boolean { value: true, span }),
            is_const: false,
            span,
        });
        id
    }

    fn bind(&mut self, init: Expr, span: Span, out: &mut Vec<Stmt>) -> Expr {
        let id = self.fresh(span);
        out.push(Stmt::VarDecl { pattern: Pattern::Identifier(id.clone()), init, is_const: false, span });
        Expr::Identifier(id)
    }

    fn stmts(&mut self, input: &[Stmt]) -> Vec<Stmt> {
        let mut out = Vec::new();
        for s in input {
            self.stmt(s, &mut out);
        }
        out
    }

    fn block(&mut self, b: &Block) -> Block {
        Block { stmts: self.stmts(&b.stmts), span: b.span }
    }

    fn body(&mut self, s: &Stmt) -> Box<Stmt> {
        let mut out = Vec::new();
        self.stmt(s, &mut out);
        if out.len() == 1 {
            Box::new(out.pop().unwrap_or_else(|| Stmt::Empty { span: s.span() }))
        } else {
            Box::new(Stmt::Block(Block { stmts: out, span: s.span() }))
        }
    }

    fn stmt(&mut self, s: &Stmt, out: &mut Vec<Stmt>) {
        if !has_await_stmt(s) {
            out.push(s.clone());
            return;
        }
        match s {
            Stmt::VarDecl { pattern, init, is_const, span } => {
                let init = self.expr(init, out);
                out.push(Stmt::VarDecl { pattern: pattern.clone(), init, is_const: *is_const, span: *span });
            }
            Stmt::Expr { expr: Expr::Await { argument, span: aw }, span } => {
                let argument = self.expr(argument, out);
                out.push(Stmt::Expr { expr: Expr::Await { argument: Box::new(argument), span: *aw }, span: *span });
            }
            Stmt::Expr { expr, span } => {
                let expr = self.expr(expr, out);
                out.push(Stmt::Expr { expr, span: *span });
            }
            Stmt::Return { value: Some(value), span } => {
                let value = self.expr(value, out);
                out.push(Stmt::Return { value: Some(value), span: *span });
            }
            Stmt::Throw { value, span } => {
                let value = self.expr(value, out);
                out.push(Stmt::Throw { value, span: *span });
            }
            Stmt::Block(b) => out.push(Stmt::Block(self.block(b))),
            Stmt::If { condition, then_branch, else_branch, span } => {
                let condition = self.expr(condition, out);
                out.push(Stmt::If {
                    condition,
                    then_branch: self.body(then_branch),
                    else_branch: else_branch.as_deref().map(|b| self.body(b)),
                    span: *span,
                });
            }
            Stmt::While { condition, body, span } => {
                if has_await(condition) {
                    let mut inner = Vec::new();
                    let cond = self.expr(condition, &mut inner);
                    inner.push(Stmt::If {
                        condition: Expr::Unary { op: UnaryOp::Not, expr: Box::new(cond), span: *span },
                        then_branch: Box::new(Stmt::Break { label: None, span: *span }),
                        else_branch: None,
                        span: *span,
                    });
                    self.stmt(body, &mut inner);
                    out.push(Stmt::While {
                        condition: Expr::Literal(Literal::Boolean { value: true, span: *span }),
                        body: Box::new(Stmt::Block(Block { stmts: inner, span: *span })),
                        span: *span,
                    });
                } else {
                    out.push(Stmt::While { condition: condition.clone(), body: self.body(body), span: *span });
                }
            }
            Stmt::DoWhile { body, condition, span } if !has_await(condition) => {
                out.push(Stmt::DoWhile { body: self.body(body), condition: condition.clone(), span: *span });
            }
            Stmt::DoWhile { body, condition, span } => {
                let first = self.flag(*span, out);
                let mut check = Vec::new();
                let cond = self.expr(condition, &mut check);
                check.push(Stmt::If {
                    condition: Expr::Unary { op: UnaryOp::Not, expr: Box::new(cond), span: *span },
                    then_branch: Box::new(Stmt::Break { label: None, span: *span }),
                    else_branch: None,
                    span: *span,
                });
                let mut inner = vec![
                    Stmt::If {
                        condition: Expr::Unary {
                            op: UnaryOp::Not,
                            expr: Box::new(Expr::Identifier(first.clone())),
                            span: *span,
                        },
                        then_branch: Box::new(Stmt::Block(Block { stmts: check, span: *span })),
                        else_branch: None,
                        span: *span,
                    },
                    assign_bool(&first, false, *span),
                ];
                self.stmt(body, &mut inner);
                out.push(Stmt::While {
                    condition: Expr::Literal(Literal::Boolean { value: true, span: *span }),
                    body: Box::new(Stmt::Block(Block { stmts: inner, span: *span })),
                    span: *span,
                });
            }
            Stmt::For { init, condition, update, body, span }
                if !condition.as_ref().is_some_and(has_await) && !update.as_ref().is_some_and(has_await) =>
            {
                out.push(Stmt::For {
                    init: init.clone(),
                    condition: condition.clone(),
                    update: update.clone(),
                    body: self.body(body),
                    span: *span,
                });
            }
            Stmt::For { init, condition, update, body, span } => {
                let first = self.flag(*span, out);
                let mut inner = Vec::new();
                let mut step = Vec::new();
                if let Some(u) = update {
                    let lowered = self.expr(u, &mut step);
                    step.push(Stmt::Expr { expr: lowered, span: *span });
                }
                inner.push(Stmt::If {
                    condition: Expr::Unary {
                        op: UnaryOp::Not,
                        expr: Box::new(Expr::Identifier(first.clone())),
                        span: *span,
                    },
                    then_branch: Box::new(Stmt::Block(Block { stmts: step, span: *span })),
                    else_branch: None,
                    span: *span,
                });
                inner.push(assign_bool(&first, false, *span));
                if let Some(c) = condition {
                    let cond = self.expr(c, &mut inner);
                    inner.push(Stmt::If {
                        condition: Expr::Unary { op: UnaryOp::Not, expr: Box::new(cond), span: *span },
                        then_branch: Box::new(Stmt::Break { label: None, span: *span }),
                        else_branch: None,
                        span: *span,
                    });
                }
                self.stmt(body, &mut inner);
                out.push(Stmt::For {
                    init: init.clone(),
                    condition: None,
                    update: None,
                    body: Box::new(Stmt::Block(Block { stmts: inner, span: *span })),
                    span: *span,
                });
            }
            Stmt::ForOf { variable, iterable, body, span } => {
                let iterable = self.expr(iterable, out);
                out.push(Stmt::ForOf { variable: variable.clone(), iterable, body: self.body(body), span: *span });
            }
            Stmt::ForIn { variable, iterable, body, span } => {
                let iterable = self.expr(iterable, out);
                out.push(Stmt::ForIn { variable: variable.clone(), iterable, body: self.body(body), span: *span });
            }
            Stmt::ForAwaitOf { variable, iterable, body, span } => {
                let iterable = self.expr(iterable, out);
                out.push(Stmt::ForAwaitOf { variable: variable.clone(), iterable, body: self.body(body), span: *span });
            }
            Stmt::TryCatch { try_block, catch_param, catch_block, finally_block, span } => {
                out.push(Stmt::TryCatch {
                    try_block: self.block(try_block),
                    catch_param: catch_param.clone(),
                    catch_block: catch_block.as_ref().map(|b| self.block(b)),
                    finally_block: finally_block.as_ref().map(|b| self.block(b)),
                    span: *span,
                });
            }
            Stmt::Labeled { label, body, span } => {
                out.push(Stmt::Labeled { label: label.clone(), body: self.body(body), span: *span });
            }
            Stmt::Switch { expr, cases, default, span }
                if !cases.iter().any(|c| has_await_stmts(&c.body.stmts))
                    && !default.as_ref().is_some_and(|b| has_await_stmts(&b.stmts)) =>
            {
                let expr = self.expr(expr, out);
                out.push(Stmt::Switch { expr, cases: cases.clone(), default: default.clone(), span: *span });
            }
            Stmt::Switch { expr, cases, default, span } => {
                let discriminant = self.expr(expr, out);
                let slot = self.bind(discriminant, *span, out);
                let mut chain = default.as_ref().map(|b| Stmt::Block(self.block(b)));
                for case in cases.iter().rev() {
                    let mut guard = Vec::new();
                    let value = self.expr(&case.value, &mut guard);
                    let branch = Stmt::If {
                        condition: Expr::Binary {
                            op: BinaryOp::StrictEquals,
                            lhs: Box::new(slot.clone()),
                            rhs: Box::new(value),
                            span: case.span,
                        },
                        then_branch: Box::new(Stmt::Block(self.block(&case.body))),
                        else_branch: chain.map(Box::new),
                        span: case.span,
                    };
                    guard.push(branch);
                    chain = Some(if guard.len() == 1 {
                        guard.pop().unwrap_or(Stmt::Empty { span: case.span })
                    } else {
                        Stmt::Block(Block { stmts: guard, span: case.span })
                    });
                }
                if let Some(stmt) = chain {
                    out.push(stmt);
                }
            }
            Stmt::ClassDecl { name, super_class, members, decorators, span } => {
                let super_class = super_class.as_ref().map(|s| self.expr(s, out));
                let decorators = decorators.iter().map(|d| self.expr(d, out)).collect();
                out.push(Stmt::ClassDecl {
                    name: name.clone(),
                    super_class,
                    members: members.clone(),
                    decorators,
                    span: *span,
                });
            }
            Stmt::Using { name, init, is_await, span } => {
                let init = self.expr(init, out);
                out.push(Stmt::Using { name: name.clone(), init, is_await: *is_await, span: *span });
            }
            other => out.push(other.clone()),
        }
    }

    fn seq(&mut self, parts: &[&Expr], out: &mut Vec<Stmt>) -> Vec<Expr> {
        let mut result = Vec::with_capacity(parts.len());
        for (i, part) in parts.iter().enumerate() {
            let later = parts[i + 1..].iter().any(|q| has_await(q));
            let lowered = self.expr(part, out);
            if later && !matches!(lowered, Expr::Literal(_)) {
                let span = lowered.span();
                result.push(self.bind(lowered, span, out));
            } else {
                result.push(lowered);
            }
        }
        result
    }

    fn pair(&mut self, a: &Expr, b: &Expr, span: Span, out: &mut Vec<Stmt>) -> (Expr, Expr) {
        let mut it = self.seq(&[a, b], out).into_iter();
        let first = it.next().unwrap_or(Expr::Literal(Literal::Undefined { span }));
        let second = it.next().unwrap_or(Expr::Literal(Literal::Undefined { span }));
        (first, second)
    }

    fn args(&mut self, args: &[Expr], out: &mut Vec<Stmt>) -> Vec<Expr> {
        let refs: Vec<&Expr> = args.iter().collect();
        self.seq(&refs, out)
    }

    fn expr(&mut self, e: &Expr, out: &mut Vec<Stmt>) -> Expr {
        if !has_await(e) {
            return e.clone();
        }
        match e {
            Expr::Await { argument, span } => {
                let argument = self.expr(argument, out);
                self.bind(Expr::Await { argument: Box::new(argument), span: *span }, *span, out)
            }
            Expr::Grouping { expr, span } => {
                let expr = self.expr(expr, out);
                Expr::Grouping { expr: Box::new(expr), span: *span }
            }
            Expr::Unary { op, expr, span } => {
                let expr = self.expr(expr, out);
                Expr::Unary { op: *op, expr: Box::new(expr), span: *span }
            }
            Expr::Postfix { op, expr, span } => {
                let expr = self.expr(expr, out);
                Expr::Postfix { op: *op, expr: Box::new(expr), span: *span }
            }
            Expr::Yield { argument, delegate, span } => {
                let argument = argument.as_deref().map(|a| Box::new(self.expr(a, out)));
                Expr::Yield { argument, delegate: *delegate, span: *span }
            }
            Expr::Spread { expr, span } => {
                let expr = self.expr(expr, out);
                Expr::Spread { expr: Box::new(expr), span: *span }
            }
            Expr::DynamicImport { source, span } => {
                let source = self.expr(source, out);
                Expr::DynamicImport { source: Box::new(source), span: *span }
            }
            Expr::Binary { op, lhs, rhs, span } => self.binary(*op, lhs, rhs, *span, out),
            Expr::Assignment { target, value, span } => {
                let value = self.expr(value, out);
                Expr::Assignment { target: target.clone(), value: Box::new(value), span: *span }
            }
            Expr::Conditional { condition, then_expr, else_expr, span } => {
                if has_await(then_expr) || has_await(else_expr) {
                    let condition = self.expr(condition, out);
                    let slot = self.fresh(*span);
                    out.push(Stmt::VarDecl {
                        pattern: Pattern::Identifier(slot.clone()),
                        init: Expr::Literal(Literal::Undefined { span: *span }),
                        is_const: false,
                        span: *span,
                    });
                    let then_branch = self.branch_assign(&slot, then_expr, *span);
                    let else_branch = self.branch_assign(&slot, else_expr, *span);
                    out.push(Stmt::If {
                        condition,
                        then_branch: Box::new(then_branch),
                        else_branch: Some(Box::new(else_branch)),
                        span: *span,
                    });
                    Expr::Identifier(slot)
                } else {
                    let condition = self.expr(condition, out);
                    Expr::Conditional {
                        condition: Box::new(condition),
                        then_expr: then_expr.clone(),
                        else_expr: else_expr.clone(),
                        span: *span,
                    }
                }
            }
            Expr::Call { callee, args, span } => {
                let (callee, args) = self.callee_and_args(callee, args, out);
                Expr::Call { callee: Box::new(callee), args, span: *span }
            }
            Expr::OptionalCall { callee, args, span } => {
                let (callee, args) = self.callee_and_args(callee, args, out);
                Expr::OptionalCall { callee: Box::new(callee), args, span: *span }
            }
            Expr::New { callee, args, span } => {
                let mut refs: Vec<&Expr> = vec![callee];
                refs.extend(args.iter());
                let mut lowered = self.seq(&refs, out);
                let new_callee = lowered.remove(0);
                Expr::New { callee: Box::new(new_callee), args: lowered, span: *span }
            }
            Expr::Index { object, index, span } => {
                let (object, index) = self.pair(object, index, *span, out);
                Expr::Index { object: Box::new(object), index: Box::new(index), span: *span }
            }
            Expr::OptionalIndex { object, index, span } => {
                let (object, index) = self.pair(object, index, *span, out);
                Expr::OptionalIndex { object: Box::new(object), index: Box::new(index), span: *span }
            }
            Expr::Member { object, property, span } => {
                let object = self.expr(object, out);
                Expr::Member { object: Box::new(object), property: property.clone(), span: *span }
            }
            Expr::OptionalMember { object, property, span } => {
                let object = self.expr(object, out);
                Expr::OptionalMember { object: Box::new(object), property: property.clone(), span: *span }
            }
            Expr::Literal(Literal::Array { elements, span }) => {
                let lowered = self.args(elements, out);
                Expr::Literal(Literal::Array { elements: lowered, span: *span })
            }
            Expr::Literal(Literal::Object { entries, span }) => {
                let values: Vec<&Expr> = entries
                    .iter()
                    .filter_map(|entry| match entry {
                        ObjectEntry::Property { value, .. } => Some(value),
                        ObjectEntry::Spread(e) => Some(e),
                        ObjectEntry::Getter { .. } | ObjectEntry::Setter { .. } => None,
                    })
                    .collect();
                let mut lowered = self.seq(&values, out).into_iter();
                let entries = entries
                    .iter()
                    .map(|entry| match entry {
                        ObjectEntry::Property { key, value } => ObjectEntry::Property {
                            key: key.clone(),
                            value: lowered.next().unwrap_or_else(|| value.clone()),
                        },
                        ObjectEntry::Spread(e) => ObjectEntry::Spread(lowered.next().unwrap_or_else(|| e.clone())),
                        other => other.clone(),
                    })
                    .collect();
                Expr::Literal(Literal::Object { entries, span: *span })
            }
            Expr::TemplateLiteral { parts, span } => {
                let values: Vec<&Expr> = parts
                    .iter()
                    .filter_map(|p| match p {
                        TemplatePart::Expr(e) => Some(e.as_ref()),
                        TemplatePart::Str(_) => None,
                    })
                    .collect();
                let mut lowered = self.seq(&values, out).into_iter();
                let parts = parts
                    .iter()
                    .map(|p| match p {
                        TemplatePart::Expr(e) => {
                            TemplatePart::Expr(Box::new(lowered.next().unwrap_or_else(|| (**e).clone())))
                        }
                        TemplatePart::Str(s) => TemplatePart::Str(s.clone()),
                    })
                    .collect();
                Expr::TemplateLiteral { parts, span: *span }
            }
            Expr::TaggedTemplate { tag, quasis, expressions, span } => {
                let mut refs: Vec<&Expr> = vec![tag];
                refs.extend(expressions.iter());
                let mut lowered = self.seq(&refs, out);
                let tag = lowered.remove(0);
                Expr::TaggedTemplate { tag: Box::new(tag), quasis: quasis.clone(), expressions: lowered, span: *span }
            }
            other => other.clone(),
        }
    }

    fn branch_assign(&mut self, slot: &Identifier, value: &Expr, span: Span) -> Stmt {
        let mut inner = Vec::new();
        let lowered = self.expr(value, &mut inner);
        inner.push(Stmt::Expr {
            expr: Expr::Binary {
                op: BinaryOp::Assign,
                lhs: Box::new(Expr::Identifier(slot.clone())),
                rhs: Box::new(lowered),
                span,
            },
            span,
        });
        Stmt::Block(Block { stmts: inner, span })
    }

    fn callee_and_args(&mut self, callee: &Expr, args: &[Expr], out: &mut Vec<Stmt>) -> (Expr, Vec<Expr>) {
        let args_await = args.iter().any(has_await);
        match callee {
            Expr::Member { object, property, span } if args_await => {
                let object = self.expr(object, out);
                let object = if matches!(object, Expr::Literal(_)) {
                    object
                } else {
                    let s = object.span();
                    self.bind(object, s, out)
                };
                let lowered = self.args(args, out);
                (Expr::Member { object: Box::new(object), property: property.clone(), span: *span }, lowered)
            }
            Expr::Index { object, index, span } if args_await => {
                let (object, index) = self.pair(object, index, *span, out);
                let object = if matches!(object, Expr::Literal(_)) {
                    object
                } else {
                    let s = object.span();
                    self.bind(object, s, out)
                };
                let lowered = self.args(args, out);
                (Expr::Index { object: Box::new(object), index: Box::new(index), span: *span }, lowered)
            }
            _ => {
                let mut refs: Vec<&Expr> = vec![callee];
                refs.extend(args.iter());
                let mut lowered = self.seq(&refs, out);
                let callee = lowered.remove(0);
                (callee, lowered)
            }
        }
    }

    fn binary(&mut self, op: BinaryOp, lhs: &Expr, rhs: &Expr, span: Span, out: &mut Vec<Stmt>) -> Expr {
        if is_short_circuit(op) && has_await(rhs) {
            let head = self.expr(lhs, out);
            let slot = self.fresh(span);
            out.push(Stmt::VarDecl { pattern: Pattern::Identifier(slot.clone()), init: head, is_const: false, span });
            let condition = match op {
                BinaryOp::And => Expr::Identifier(slot.clone()),
                BinaryOp::Or => Expr::Unary { op: UnaryOp::Not, expr: Box::new(Expr::Identifier(slot.clone())), span },
                _ => Expr::Binary {
                    op: BinaryOp::Equals,
                    lhs: Box::new(Expr::Identifier(slot.clone())),
                    rhs: Box::new(Expr::Literal(Literal::Null { span })),
                    span,
                },
            };
            let then_branch = self.branch_assign(&slot, rhs, span);
            out.push(Stmt::If { condition, then_branch: Box::new(then_branch), else_branch: None, span });
            return Expr::Identifier(slot);
        }
        if matches!(op, BinaryOp::Assign) {
            let rhs = self.expr(rhs, out);
            return Expr::Binary { op, lhs: Box::new(lhs.clone()), rhs: Box::new(rhs), span };
        }
        if let Some(base) = compound_base(op)
            && has_await(rhs)
        {
            let current = self.expr(lhs, out);
            let saved = self.bind(current, span, out);
            let rhs = self.expr(rhs, out);
            let combined = Expr::Binary { op: base, lhs: Box::new(saved), rhs: Box::new(rhs), span };
            return Expr::Binary { op: BinaryOp::Assign, lhs: Box::new(lhs.clone()), rhs: Box::new(combined), span };
        }
        let (lhs, rhs) = self.pair(lhs, rhs, span, out);
        Expr::Binary { op, lhs: Box::new(lhs), rhs: Box::new(rhs), span }
    }
}
