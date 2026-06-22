use yps_parser::{
    Block, ClassMember, ExportKind, Expr, ImportSpec, Literal, ObjectEntry, ObjectPatternProp, Param, Pattern, Program,
    PropKey, Stmt, SwitchCase, TemplatePart, TemplateQuasi,
};

pub fn programs_equivalent(a: &Program, b: &Program) -> bool {
    stmts_eq(&filter_empty(&a.items), &filter_empty(&b.items))
}

fn filter_empty(stmts: &[Stmt]) -> Vec<&Stmt> {
    stmts.iter().filter(|s| !matches!(s, Stmt::Empty { .. })).collect()
}

fn stmts_eq(a: &[&Stmt], b: &[&Stmt]) -> bool {
    a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| stmt_eq(x, y))
}

fn block_eq(a: &Block, b: &Block) -> bool {
    stmts_eq(&filter_empty(&a.stmts), &filter_empty(&b.stmts))
}

fn opt_block_eq(a: &Option<Block>, b: &Option<Block>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(x), Some(y)) => block_eq(x, y),
        _ => false,
    }
}

fn stmt_eq(a: &Stmt, b: &Stmt) -> bool {
    match (a, b) {
        (
            Stmt::VarDecl { pattern: p1, init: i1, is_const: c1, .. },
            Stmt::VarDecl { pattern: p2, init: i2, is_const: c2, .. },
        ) => c1 == c2 && pattern_eq(p1, p2) && expr_eq(i1, i2),
        (Stmt::Using { name: n1, init: i1, .. }, Stmt::Using { name: n2, init: i2, .. }) => {
            n1.name == n2.name && expr_eq(i1, i2)
        }
        (Stmt::Expr { expr: e1, .. }, Stmt::Expr { expr: e2, .. }) => expr_eq(e1, e2),
        (Stmt::Block(b1), Stmt::Block(b2)) => block_eq(b1, b2),
        (Stmt::Empty { .. }, Stmt::Empty { .. }) => true,
        (
            Stmt::If { condition: c1, then_branch: t1, else_branch: e1, .. },
            Stmt::If { condition: c2, then_branch: t2, else_branch: e2, .. },
        ) => expr_eq(c1, c2) && stmt_eq(t1, t2) && opt_stmt_eq(e1, e2),
        (Stmt::While { condition: c1, body: b1, .. }, Stmt::While { condition: c2, body: b2, .. }) => {
            expr_eq(c1, c2) && stmt_eq(b1, b2)
        }
        (Stmt::DoWhile { body: b1, condition: c1, .. }, Stmt::DoWhile { body: b2, condition: c2, .. }) => {
            stmt_eq(b1, b2) && expr_eq(c1, c2)
        }
        (
            Stmt::For { init: i1, condition: c1, update: u1, body: b1, .. },
            Stmt::For { init: i2, condition: c2, update: u2, body: b2, .. },
        ) => opt_stmt_eq(i1, i2) && opt_expr_eq(c1, c2) && opt_expr_eq(u1, u2) && stmt_eq(b1, b2),
        (
            Stmt::ForIn { variable: v1, iterable: it1, body: b1, .. },
            Stmt::ForIn { variable: v2, iterable: it2, body: b2, .. },
        ) => v1.name == v2.name && expr_eq(it1, it2) && stmt_eq(b1, b2),
        (
            Stmt::ForOf { variable: v1, iterable: it1, body: b1, .. },
            Stmt::ForOf { variable: v2, iterable: it2, body: b2, .. },
        ) => v1.name == v2.name && expr_eq(it1, it2) && stmt_eq(b1, b2),
        (
            Stmt::ForAwaitOf { variable: v1, iterable: it1, body: b1, .. },
            Stmt::ForAwaitOf { variable: v2, iterable: it2, body: b2, .. },
        ) => v1.name == v2.name && expr_eq(it1, it2) && stmt_eq(b1, b2),
        (Stmt::Break { label: l1, .. }, Stmt::Break { label: l2, .. }) => opt_ident_eq(l1, l2),
        (Stmt::Continue { label: l1, .. }, Stmt::Continue { label: l2, .. }) => opt_ident_eq(l1, l2),
        (Stmt::Labeled { label: l1, body: b1, .. }, Stmt::Labeled { label: l2, body: b2, .. }) => {
            l1.name == l2.name && stmt_eq(b1, b2)
        }
        (
            Stmt::FunctionDecl { name: n1, params: p1, body: bd1, is_generator: g1, is_async: a1, .. },
            Stmt::FunctionDecl { name: n2, params: p2, body: bd2, is_generator: g2, is_async: a2, .. },
        ) => n1.name == n2.name && g1 == g2 && a1 == a2 && params_eq(p1, p2) && block_eq(bd1, bd2),
        (Stmt::Return { value: v1, .. }, Stmt::Return { value: v2, .. }) => opt_expr_eq(v1, v2),
        (Stmt::Throw { value: v1, .. }, Stmt::Throw { value: v2, .. }) => expr_eq(v1, v2),
        (
            Stmt::TryCatch { try_block: t1, catch_param: cp1, catch_block: cb1, finally_block: f1, .. },
            Stmt::TryCatch { try_block: t2, catch_param: cp2, catch_block: cb2, finally_block: f2, .. },
        ) => block_eq(t1, t2) && opt_ident_eq(cp1, cp2) && opt_block_eq(cb1, cb2) && opt_block_eq(f1, f2),
        (
            Stmt::Switch { expr: e1, cases: c1, default: d1, .. },
            Stmt::Switch { expr: e2, cases: c2, default: d2, .. },
        ) => expr_eq(e1, e2) && cases_eq(c1, c2) && opt_block_eq(d1, d2),
        (
            Stmt::ClassDecl { name: n1, super_class: s1, members: m1, decorators: d1, .. },
            Stmt::ClassDecl { name: n2, super_class: s2, members: m2, decorators: d2, .. },
        ) => n1.name == n2.name && opt_expr_eq(s1, s2) && exprs_eq(d1, d2) && members_eq(m1, m2),
        (Stmt::Debugger { .. }, Stmt::Debugger { .. }) => true,
        (
            Stmt::Import { specifiers: s1, source: src1, attributes: a1, .. },
            Stmt::Import { specifiers: s2, source: src2, attributes: a2, .. },
        ) => src1 == src2 && a1 == a2 && import_specs_eq(s1, s2),
        (Stmt::Export { kind: k1, .. }, Stmt::Export { kind: k2, .. }) => export_kind_eq(k1, k2),
        _ => false,
    }
}

fn opt_stmt_eq(a: &Option<Box<Stmt>>, b: &Option<Box<Stmt>>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(x), Some(y)) => stmt_eq(x, y),
        _ => false,
    }
}

fn cases_eq(a: &[SwitchCase], b: &[SwitchCase]) -> bool {
    a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| expr_eq(&x.value, &y.value) && block_eq(&x.body, &y.body))
}

fn members_eq(a: &[ClassMember], b: &[ClassMember]) -> bool {
    a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| member_eq(x, y))
}

fn member_eq(a: &ClassMember, b: &ClassMember) -> bool {
    match (a, b) {
        (
            ClassMember::Constructor { params: p1, body: b1, .. },
            ClassMember::Constructor { params: p2, body: b2, .. },
        ) => params_eq(p1, p2) && block_eq(b1, b2),
        (
            ClassMember::Method {
                name: n1, params: p1, body: b1, is_static: s1, is_private: pr1, decorators: d1, ..
            },
            ClassMember::Method {
                name: n2, params: p2, body: b2, is_static: s2, is_private: pr2, decorators: d2, ..
            },
        ) => n1.name == n2.name && s1 == s2 && pr1 == pr2 && exprs_eq(d1, d2) && params_eq(p1, p2) && block_eq(b1, b2),
        (
            ClassMember::Field { name: n1, init: i1, is_static: s1, is_private: pr1, decorators: d1, .. },
            ClassMember::Field { name: n2, init: i2, is_static: s2, is_private: pr2, decorators: d2, .. },
        ) => n1.name == n2.name && s1 == s2 && pr1 == pr2 && exprs_eq(d1, d2) && opt_expr_eq(i1, i2),
        (
            ClassMember::Getter { name: n1, body: b1, is_static: s1, is_private: pr1, decorators: d1, .. },
            ClassMember::Getter { name: n2, body: b2, is_static: s2, is_private: pr2, decorators: d2, .. },
        ) => n1.name == n2.name && s1 == s2 && pr1 == pr2 && exprs_eq(d1, d2) && block_eq(b1, b2),
        (
            ClassMember::Setter {
                name: n1, param: pa1, body: b1, is_static: s1, is_private: pr1, decorators: d1, ..
            },
            ClassMember::Setter {
                name: n2, param: pa2, body: b2, is_static: s2, is_private: pr2, decorators: d2, ..
            },
        ) => n1.name == n2.name && s1 == s2 && pr1 == pr2 && exprs_eq(d1, d2) && param_eq(pa1, pa2) && block_eq(b1, b2),
        _ => false,
    }
}

fn import_specs_eq(a: &[ImportSpec], b: &[ImportSpec]) -> bool {
    a.len() == b.len()
        && a.iter().zip(b.iter()).all(|(x, y)| match (x, y) {
            (ImportSpec::Default { local: l1 }, ImportSpec::Default { local: l2 }) => l1.name == l2.name,
            (ImportSpec::Namespace { local: l1 }, ImportSpec::Namespace { local: l2 }) => l1.name == l2.name,
            (ImportSpec::Named { imported: i1, local: lo1 }, ImportSpec::Named { imported: i2, local: lo2 }) => {
                i1.name == i2.name && lo1.name == lo2.name
            }
            _ => false,
        })
}

fn export_kind_eq(a: &ExportKind, b: &ExportKind) -> bool {
    match (a, b) {
        (ExportKind::Named(n1), ExportKind::Named(n2)) => {
            n1.len() == n2.len() && n1.iter().zip(n2.iter()).all(|(x, y)| x.name == y.name)
        }
        (ExportKind::Declaration(d1), ExportKind::Declaration(d2)) => stmt_eq(d1, d2),
        _ => false,
    }
}

fn pattern_eq(a: &Pattern, b: &Pattern) -> bool {
    match (a, b) {
        (Pattern::Identifier(i1), Pattern::Identifier(i2)) => i1.name == i2.name,
        (Pattern::Array { elements: e1, rest: r1, .. }, Pattern::Array { elements: e2, rest: r2, .. }) => {
            e1.len() == e2.len()
                && e1.iter().zip(e2.iter()).all(|(x, y)| opt_pattern_eq(x, y))
                && opt_boxed_pattern_eq(r1, r2)
        }
        (Pattern::Object { properties: p1, rest: r1, .. }, Pattern::Object { properties: p2, rest: r2, .. }) => {
            p1.len() == p2.len()
                && p1.iter().zip(p2.iter()).all(|(x, y)| obj_pattern_prop_eq(x, y))
                && opt_boxed_pattern_eq(r1, r2)
        }
        (Pattern::Default { pattern: p1, default: d1, .. }, Pattern::Default { pattern: p2, default: d2, .. }) => {
            pattern_eq(p1, p2) && expr_eq(d1, d2)
        }
        _ => false,
    }
}

fn opt_pattern_eq(a: &Option<Pattern>, b: &Option<Pattern>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(x), Some(y)) => pattern_eq(x, y),
        _ => false,
    }
}

fn opt_boxed_pattern_eq(a: &Option<Box<Pattern>>, b: &Option<Box<Pattern>>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(x), Some(y)) => pattern_eq(x, y),
        _ => false,
    }
}

fn obj_pattern_prop_eq(a: &ObjectPatternProp, b: &ObjectPatternProp) -> bool {
    a.key.name == b.key.name && opt_pattern_eq(&a.value, &b.value)
}

fn params_eq(a: &[Param], b: &[Param]) -> bool {
    a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| param_eq(x, y))
}

fn param_eq(a: &Param, b: &Param) -> bool {
    a.name.name == b.name.name && a.is_rest == b.is_rest && opt_expr_eq(&a.default, &b.default)
}

fn opt_ident_eq(a: &Option<yps_parser::Identifier>, b: &Option<yps_parser::Identifier>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(x), Some(y)) => x.name == y.name,
        _ => false,
    }
}

fn exprs_eq(a: &[Expr], b: &[Expr]) -> bool {
    a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| expr_eq(x, y))
}

fn opt_expr_eq(a: &Option<Expr>, b: &Option<Expr>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(x), Some(y)) => expr_eq(x, y),
        _ => false,
    }
}

fn unwrap_grouping(expr: &Expr) -> &Expr {
    match expr {
        Expr::Grouping { expr, .. } => unwrap_grouping(expr),
        other => other,
    }
}

fn expr_eq(a: &Expr, b: &Expr) -> bool {
    let a = unwrap_grouping(a);
    let b = unwrap_grouping(b);
    match (a, b) {
        (Expr::Identifier(i1), Expr::Identifier(i2)) => i1.name == i2.name,
        (Expr::Literal(l1), Expr::Literal(l2)) => literal_eq(l1, l2),
        (Expr::This { .. }, Expr::This { .. }) => true,
        (Expr::Super { .. }, Expr::Super { .. }) => true,
        (Expr::Unary { op: o1, expr: e1, .. }, Expr::Unary { op: o2, expr: e2, .. }) => o1 == o2 && expr_eq(e1, e2),
        (Expr::Postfix { op: o1, expr: e1, .. }, Expr::Postfix { op: o2, expr: e2, .. }) => o1 == o2 && expr_eq(e1, e2),
        (Expr::Binary { op: o1, lhs: l1, rhs: r1, .. }, Expr::Binary { op: o2, lhs: l2, rhs: r2, .. }) => {
            o1 == o2 && expr_eq(l1, l2) && expr_eq(r1, r2)
        }
        (Expr::Assignment { target: t1, value: v1, .. }, Expr::Assignment { target: t2, value: v2, .. }) => {
            t1.name == t2.name && expr_eq(v1, v2)
        }
        (
            Expr::Conditional { condition: c1, then_expr: t1, else_expr: e1, .. },
            Expr::Conditional { condition: c2, then_expr: t2, else_expr: e2, .. },
        ) => expr_eq(c1, c2) && expr_eq(t1, t2) && expr_eq(e1, e2),
        (Expr::Call { callee: c1, args: a1, .. }, Expr::Call { callee: c2, args: a2, .. }) => {
            expr_eq(c1, c2) && exprs_eq(a1, a2)
        }
        (Expr::OptionalCall { callee: c1, args: a1, .. }, Expr::OptionalCall { callee: c2, args: a2, .. }) => {
            expr_eq(c1, c2) && exprs_eq(a1, a2)
        }
        (Expr::New { callee: c1, args: a1, .. }, Expr::New { callee: c2, args: a2, .. }) => {
            expr_eq(c1, c2) && exprs_eq(a1, a2)
        }
        (Expr::Index { object: o1, index: i1, .. }, Expr::Index { object: o2, index: i2, .. }) => {
            expr_eq(o1, o2) && expr_eq(i1, i2)
        }
        (Expr::OptionalIndex { object: o1, index: i1, .. }, Expr::OptionalIndex { object: o2, index: i2, .. }) => {
            expr_eq(o1, o2) && expr_eq(i1, i2)
        }
        (Expr::Member { object: o1, property: p1, .. }, Expr::Member { object: o2, property: p2, .. }) => {
            expr_eq(o1, o2) && p1.name == p2.name
        }
        (
            Expr::OptionalMember { object: o1, property: p1, .. },
            Expr::OptionalMember { object: o2, property: p2, .. },
        ) => expr_eq(o1, o2) && p1.name == p2.name,
        (
            Expr::ArrowFunction { params: p1, body: b1, is_async: a1, .. },
            Expr::ArrowFunction { params: p2, body: b2, is_async: a2, .. },
        ) => a1 == a2 && params_eq(p1, p2) && block_eq(b1, b2),
        (
            Expr::FunctionExpr { name: n1, params: p1, body: b1, is_async: a1, .. },
            Expr::FunctionExpr { name: n2, params: p2, body: b2, is_async: a2, .. },
        ) => a1 == a2 && opt_ident_eq(n1, n2) && params_eq(p1, p2) && block_eq(b1, b2),
        (Expr::Spread { expr: e1, .. }, Expr::Spread { expr: e2, .. }) => expr_eq(e1, e2),
        (Expr::Yield { argument: a1, delegate: d1, .. }, Expr::Yield { argument: a2, delegate: d2, .. }) => {
            d1 == d2 && opt_boxed_expr_eq(a1, a2)
        }
        (Expr::Await { argument: a1, .. }, Expr::Await { argument: a2, .. }) => expr_eq(a1, a2),
        (Expr::DynamicImport { source: s1, .. }, Expr::DynamicImport { source: s2, .. }) => expr_eq(s1, s2),
        (Expr::TemplateLiteral { parts: p1, .. }, Expr::TemplateLiteral { parts: p2, .. }) => template_parts_eq(p1, p2),
        (
            Expr::TaggedTemplate { tag: t1, quasis: q1, expressions: e1, .. },
            Expr::TaggedTemplate { tag: t2, quasis: q2, expressions: e2, .. },
        ) => expr_eq(t1, t2) && quasis_eq(q1, q2) && exprs_eq(e1, e2),
        _ => false,
    }
}

fn opt_boxed_expr_eq(a: &Option<Box<Expr>>, b: &Option<Box<Expr>>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(x), Some(y)) => expr_eq(x, y),
        _ => false,
    }
}

fn template_parts_eq(a: &[TemplatePart], b: &[TemplatePart]) -> bool {
    a.len() == b.len()
        && a.iter().zip(b.iter()).all(|(x, y)| match (x, y) {
            (TemplatePart::Str(s1), TemplatePart::Str(s2)) => s1 == s2,
            (TemplatePart::Expr(e1), TemplatePart::Expr(e2)) => expr_eq(e1, e2),
            _ => false,
        })
}

fn quasis_eq(a: &[TemplateQuasi], b: &[TemplateQuasi]) -> bool {
    a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| x.raw == y.raw)
}

fn literal_eq(a: &Literal, b: &Literal) -> bool {
    match (a, b) {
        (Literal::Number { raw: r1, .. }, Literal::Number { raw: r2, .. }) => r1 == r2,
        (Literal::BigInt { value: v1, .. }, Literal::BigInt { value: v2, .. }) => v1 == v2,
        (Literal::String { value: v1, .. }, Literal::String { value: v2, .. }) => v1 == v2,
        (Literal::Boolean { value: v1, .. }, Literal::Boolean { value: v2, .. }) => v1 == v2,
        (Literal::Null { .. }, Literal::Null { .. }) => true,
        (Literal::Undefined { .. }, Literal::Undefined { .. }) => true,
        (Literal::RegExp { pattern: p1, flags: f1, .. }, Literal::RegExp { pattern: p2, flags: f2, .. }) => {
            p1 == p2 && f1 == f2
        }
        (Literal::Array { elements: e1, .. }, Literal::Array { elements: e2, .. }) => exprs_eq(e1, e2),
        (Literal::Object { entries: e1, .. }, Literal::Object { entries: e2, .. }) => entries_eq(e1, e2),
        _ => false,
    }
}

fn entries_eq(a: &[ObjectEntry], b: &[ObjectEntry]) -> bool {
    a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| entry_eq(x, y))
}

fn entry_eq(a: &ObjectEntry, b: &ObjectEntry) -> bool {
    match (a, b) {
        (ObjectEntry::Property { key: k1, value: v1 }, ObjectEntry::Property { key: k2, value: v2 }) => {
            prop_key_eq(k1, k2) && expr_eq(v1, v2)
        }
        (ObjectEntry::Spread(e1), ObjectEntry::Spread(e2)) => expr_eq(e1, e2),
        (ObjectEntry::Getter { key: k1, body: b1, .. }, ObjectEntry::Getter { key: k2, body: b2, .. }) => {
            prop_key_eq(k1, k2) && block_eq(b1, b2)
        }
        (
            ObjectEntry::Setter { key: k1, param: p1, body: b1, .. },
            ObjectEntry::Setter { key: k2, param: p2, body: b2, .. },
        ) => prop_key_eq(k1, k2) && param_eq(p1, p2) && block_eq(b1, b2),
        _ => false,
    }
}

fn prop_key_eq(a: &PropKey, b: &PropKey) -> bool {
    match (a, b) {
        (PropKey::Identifier(i1), PropKey::Identifier(i2)) => i1.name == i2.name,
        (PropKey::Computed(e1), PropKey::Computed(e2)) => expr_eq(e1, e2),
        _ => false,
    }
}
