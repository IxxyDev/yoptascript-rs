use yps_lexer::Span;
use yps_parser::{
    BinaryOp, Block, ClassMember, ExportKind, Expr, ImportSpec, Literal, ObjectEntry, Param, Pattern, Program, PropKey,
    Stmt, TemplatePart,
};

use crate::{LintDiagnostic, LintSeverity, Rule};

fn is_compound_assign(op: BinaryOp) -> bool {
    matches!(
        op,
        BinaryOp::PlusAssign
            | BinaryOp::MinusAssign
            | BinaryOp::MulAssign
            | BinaryOp::DivAssign
            | BinaryOp::ExpAssign
            | BinaryOp::ModAssign
            | BinaryOp::NullishAssign
            | BinaryOp::AndAssign
            | BinaryOp::OrAssign
            | BinaryOp::BitAndAssign
            | BinaryOp::BitOrAssign
            | BinaryOp::BitXorAssign
            | BinaryOp::ShlAssign
            | BinaryOp::ShrAssign
            | BinaryOp::UshrAssign
    )
}

pub fn lint_program(program: &Program) -> Vec<LintDiagnostic> {
    let mut linter = Linter { scopes: Vec::new(), diags: Vec::new() };
    linter.push_scope();
    linter.visit_stmt_list(&program.items);
    linter.pop_scope();
    linter.diags.sort_by_key(|d| (d.span.start, d.span.end));
    linter.diags
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeclKind {
    Var,
    Const,
    Param,
    NonFlag,
}

#[derive(Debug, Clone, Copy)]
struct ParamMeta {
    slot: usize,
    simple: bool,
    rest: bool,
}

struct DeclInfo {
    name: String,
    span: Span,
    kind: DeclKind,
    used: bool,
    exported: bool,
    param: Option<ParamMeta>,
}

struct Scope {
    decls: Vec<DeclInfo>,
}

struct Linter {
    scopes: Vec<Scope>,
    diags: Vec<LintDiagnostic>,
}

impl Linter {
    fn push_scope(&mut self) {
        self.scopes.push(Scope { decls: Vec::new() });
    }

    fn pop_scope(&mut self) {
        let scope = self.scopes.pop().expect("сбалансированный стек областей");

        let mut last_used_slot: isize = -1;
        for decl in &scope.decls {
            if let Some(meta) = decl.param
                && decl.used
            {
                last_used_slot = last_used_slot.max(meta.slot as isize);
            }
        }

        for decl in &scope.decls {
            if decl.used || decl.exported || decl.name.starts_with('_') {
                continue;
            }
            match decl.kind {
                DeclKind::Var | DeclKind::Const => self.report_unused(decl),
                DeclKind::Param => {
                    let meta = decl.param.expect("параметр несёт метаданные");
                    if meta.rest {
                        continue;
                    }
                    if meta.simple && (meta.slot as isize) <= last_used_slot {
                        continue;
                    }
                    self.report_unused(decl);
                }
                DeclKind::NonFlag => {}
            }
        }
    }

    fn report_unused(&mut self, decl: &DeclInfo) {
        let message = match decl.kind {
            DeclKind::Var => format!("переменная «{}» объявлена, но не используется", decl.name),
            DeclKind::Const => format!("константа «{}» объявлена, но не используется", decl.name),
            DeclKind::Param => format!("параметр «{}» не используется", decl.name),
            DeclKind::NonFlag => return,
        };
        self.diags.push(LintDiagnostic {
            span: decl.span,
            rule: Rule::UnusedVariable,
            severity: LintSeverity::Warning,
            message,
        });
    }

    fn declare(&mut self, name: &str, span: Span, kind: DeclKind, exported: bool, param: Option<ParamMeta>) {
        if self.shadows_outer(name) {
            self.diags.push(LintDiagnostic {
                span,
                rule: Rule::ShadowedDeclaration,
                severity: LintSeverity::Hint,
                message: format!("объявление «{name}» затеняет биндинг из внешней области видимости"),
            });
        }

        let scope = self.scopes.last_mut().expect("активная область");
        if let Some(existing) = scope.decls.iter_mut().find(|d| d.name == name) {
            if exported {
                existing.exported = true;
            }
            return;
        }
        scope.decls.push(DeclInfo { name: name.to_string(), span, kind, used: false, exported, param });
    }

    fn shadows_outer(&self, name: &str) -> bool {
        let top = self.scopes.len().saturating_sub(1);
        self.scopes[..top].iter().any(|scope| scope.decls.iter().any(|d| d.name == name))
    }

    fn read(&mut self, name: &str) {
        for scope in self.scopes.iter_mut().rev() {
            let mut found = false;
            for decl in scope.decls.iter_mut().filter(|d| d.name == name) {
                decl.used = true;
                found = true;
            }
            if found {
                return;
            }
        }
    }

    fn visit_stmt_list(&mut self, stmts: &[Stmt]) {
        self.check_unreachable(stmts);
        for stmt in stmts {
            self.hoist_stmt(stmt, false);
        }
        for stmt in stmts {
            self.visit_stmt(stmt);
        }
    }

    fn check_unreachable(&mut self, stmts: &[Stmt]) {
        let mut terminator: Option<&'static str> = None;
        for stmt in stmts {
            if let Some(keyword) = terminator {
                if matches!(stmt, Stmt::FunctionDecl { .. } | Stmt::Empty { .. }) {
                    continue;
                }
                self.diags.push(LintDiagnostic {
                    span: stmt.span(),
                    rule: Rule::UnreachableCode,
                    severity: LintSeverity::Warning,
                    message: format!("недостижимый код после «{keyword}»"),
                });
                return;
            }
            terminator = match stmt {
                Stmt::Return { .. } => Some("отвечаю"),
                Stmt::Throw { .. } => Some("кидай"),
                Stmt::Break { .. } => Some("харэ"),
                Stmt::Continue { .. } => Some("двигай"),
                _ => None,
            };
        }
    }

    fn hoist_stmt(&mut self, stmt: &Stmt, exported: bool) {
        match stmt {
            Stmt::VarDecl { pattern, is_const, .. } => {
                let kind = if *is_const { DeclKind::Const } else { DeclKind::Var };
                self.declare_pattern(pattern, kind, exported);
            }
            Stmt::FunctionDecl { name, .. } | Stmt::ClassDecl { name, .. } | Stmt::Using { name, .. } => {
                self.declare(&name.name, name.span, DeclKind::NonFlag, exported, None);
            }
            Stmt::Import { specifiers, .. } => {
                for spec in specifiers {
                    let local = match spec {
                        ImportSpec::Default { local }
                        | ImportSpec::Named { local, .. }
                        | ImportSpec::Namespace { local } => local,
                    };
                    self.declare(&local.name, local.span, DeclKind::NonFlag, exported, None);
                }
            }
            Stmt::Export { kind: ExportKind::Declaration(inner), .. } => {
                self.hoist_stmt(inner, true);
            }
            _ => {}
        }
    }

    fn declare_pattern(&mut self, pattern: &Pattern, kind: DeclKind, exported: bool) {
        match pattern {
            Pattern::Identifier(id) => self.declare(&id.name, id.span, kind, exported, None),
            Pattern::Array { elements, rest, .. } => {
                for element in elements.iter().flatten() {
                    self.declare_pattern(element, kind, exported);
                }
                if let Some(rest) = rest {
                    self.declare_pattern(rest, kind, exported);
                }
            }
            Pattern::Object { properties, rest, .. } => {
                for prop in properties {
                    match &prop.value {
                        Some(value) => self.declare_pattern(value, kind, exported),
                        None => self.declare(&prop.key.name, prop.key.span, kind, exported, None),
                    }
                }
                if let Some(rest) = rest {
                    self.declare_pattern(rest, kind, exported);
                }
            }
            Pattern::Default { pattern, .. } => self.declare_pattern(pattern, kind, exported),
        }
    }

    fn visit_pattern_exprs(&mut self, pattern: &Pattern) {
        match pattern {
            Pattern::Identifier(_) => {}
            Pattern::Array { elements, rest, .. } => {
                for element in elements.iter().flatten() {
                    self.visit_pattern_exprs(element);
                }
                if let Some(rest) = rest {
                    self.visit_pattern_exprs(rest);
                }
            }
            Pattern::Object { properties, rest, .. } => {
                for prop in properties {
                    if let Some(value) = &prop.value {
                        self.visit_pattern_exprs(value);
                    }
                }
                if let Some(rest) = rest {
                    self.visit_pattern_exprs(rest);
                }
            }
            Pattern::Default { pattern, default, .. } => {
                self.visit_pattern_exprs(pattern);
                self.visit_expr(default);
            }
        }
    }

    fn visit_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::VarDecl { pattern, init, .. } => {
                self.visit_pattern_exprs(pattern);
                self.visit_expr(init);
            }
            Stmt::Using { init, .. } => self.visit_expr(init),
            Stmt::Expr { expr, .. } => self.visit_discarded_expr(expr),
            Stmt::Block(block) => self.visit_block(block),
            Stmt::Empty { .. } | Stmt::Break { .. } | Stmt::Continue { .. } | Stmt::Debugger { .. } => {}
            Stmt::If { condition, then_branch, else_branch, .. } => {
                self.visit_expr(condition);
                self.visit_branch(then_branch);
                if let Some(else_branch) = else_branch {
                    self.visit_branch(else_branch);
                }
            }
            Stmt::While { condition, body, .. } => {
                self.visit_expr(condition);
                self.visit_branch(body);
            }
            Stmt::DoWhile { body, condition, .. } => {
                self.visit_branch(body);
                self.visit_expr(condition);
            }
            Stmt::For { init, condition, update, body, .. } => {
                self.push_scope();
                if let Some(init) = init {
                    self.hoist_stmt(init, false);
                    self.visit_stmt(init);
                }
                if let Some(condition) = condition {
                    self.visit_expr(condition);
                }
                if let Some(update) = update {
                    self.visit_discarded_expr(update);
                }
                self.visit_branch(body);
                self.pop_scope();
            }
            Stmt::ForIn { variable, iterable, body, .. }
            | Stmt::ForOf { variable, iterable, body, .. }
            | Stmt::ForAwaitOf { variable, iterable, body, .. } => {
                self.visit_expr(iterable);
                self.push_scope();
                self.declare_pattern(variable, DeclKind::Var, false);
                self.visit_pattern_exprs(variable);
                self.visit_branch(body);
                self.pop_scope();
            }
            Stmt::Labeled { body, .. } => self.visit_stmt(body),
            Stmt::FunctionDecl { params, body, .. } => self.visit_function(params, body),
            Stmt::Return { value, .. } => {
                if let Some(value) = value {
                    self.visit_expr(value);
                }
            }
            Stmt::Throw { value, .. } => self.visit_expr(value),
            Stmt::TryCatch { try_block, catch_param, catch_block, finally_block, .. } => {
                self.visit_block(try_block);
                if let Some(catch_block) = catch_block {
                    self.push_scope();
                    if let Some(param) = catch_param {
                        self.declare(&param.name, param.span, DeclKind::NonFlag, false, None);
                    }
                    self.visit_stmt_list(&catch_block.stmts);
                    self.pop_scope();
                }
                if let Some(finally_block) = finally_block {
                    self.visit_block(finally_block);
                }
            }
            Stmt::Switch { expr, cases, default, .. } => {
                self.visit_expr(expr);
                self.push_scope();
                for case in cases {
                    for inner in &case.body.stmts {
                        self.hoist_stmt(inner, false);
                    }
                }
                if let Some(default) = default {
                    for inner in &default.stmts {
                        self.hoist_stmt(inner, false);
                    }
                }
                for case in cases {
                    self.visit_expr(&case.value);
                    self.check_unreachable(&case.body.stmts);
                    for inner in &case.body.stmts {
                        self.visit_stmt(inner);
                    }
                }
                if let Some(default) = default {
                    self.check_unreachable(&default.stmts);
                    for inner in &default.stmts {
                        self.visit_stmt(inner);
                    }
                }
                self.pop_scope();
            }
            Stmt::ClassDecl { super_class, members, decorators, .. } => {
                for decorator in decorators {
                    self.visit_expr(decorator);
                }
                if let Some(super_class) = super_class {
                    self.visit_expr(super_class);
                }
                for member in members {
                    self.visit_class_member(member);
                }
            }
            Stmt::Import { .. } => {}
            Stmt::Export { kind, .. } => match kind {
                ExportKind::Declaration(inner) => self.visit_stmt(inner),
                ExportKind::Named(names) => {
                    for name in names {
                        self.read(&name.name);
                    }
                }
            },
        }
    }

    fn visit_branch(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Block(block) => self.visit_block(block),
            other => {
                self.push_scope();
                self.visit_stmt_list(std::slice::from_ref(other));
                self.pop_scope();
            }
        }
    }

    fn visit_block(&mut self, block: &Block) {
        self.push_scope();
        self.visit_stmt_list(&block.stmts);
        self.pop_scope();
    }

    fn visit_function(&mut self, params: &[Param], body: &Block) {
        self.push_scope();
        self.declare_params(params);
        self.visit_param_defaults(params);
        self.visit_stmt_list(&body.stmts);
        self.pop_scope();
    }

    fn declare_params(&mut self, params: &[Param]) {
        for (slot, param) in params.iter().enumerate() {
            match &param.pattern {
                Some(pattern) => self.declare_param_pattern(pattern, slot, param.is_rest),
                None => {
                    let meta = ParamMeta { slot, simple: !param.is_rest, rest: param.is_rest };
                    self.declare(&param.name.name, param.name.span, DeclKind::Param, false, Some(meta));
                }
            }
        }
    }

    fn declare_param_pattern(&mut self, pattern: &Pattern, slot: usize, rest: bool) {
        let meta = ParamMeta { slot, simple: false, rest };
        match pattern {
            Pattern::Identifier(id) => self.declare(&id.name, id.span, DeclKind::Param, false, Some(meta)),
            Pattern::Array { elements, rest: inner_rest, .. } => {
                for element in elements.iter().flatten() {
                    self.declare_param_pattern(element, slot, rest);
                }
                if let Some(inner_rest) = inner_rest {
                    self.declare_param_pattern(inner_rest, slot, rest);
                }
            }
            Pattern::Object { properties, rest: inner_rest, .. } => {
                for prop in properties {
                    match &prop.value {
                        Some(value) => self.declare_param_pattern(value, slot, rest),
                        None => self.declare(&prop.key.name, prop.key.span, DeclKind::Param, false, Some(meta)),
                    }
                }
                if let Some(inner_rest) = inner_rest {
                    self.declare_param_pattern(inner_rest, slot, rest);
                }
            }
            Pattern::Default { pattern, .. } => self.declare_param_pattern(pattern, slot, rest),
        }
    }

    fn visit_param_defaults(&mut self, params: &[Param]) {
        for param in params {
            if let Some(pattern) = &param.pattern {
                self.visit_pattern_exprs(pattern);
            }
            if let Some(default) = &param.default {
                self.visit_expr(default);
            }
        }
    }

    fn visit_class_member(&mut self, member: &ClassMember) {
        match member {
            ClassMember::Constructor { params, body, .. } => self.visit_function(params, body),
            ClassMember::Method { params, body, decorators, .. } => {
                for decorator in decorators {
                    self.visit_expr(decorator);
                }
                self.visit_function(params, body);
            }
            ClassMember::Field { init, decorators, .. } => {
                for decorator in decorators {
                    self.visit_expr(decorator);
                }
                if let Some(init) = init {
                    self.visit_expr(init);
                }
            }
            ClassMember::Getter { body, decorators, .. } => {
                for decorator in decorators {
                    self.visit_expr(decorator);
                }
                self.visit_function(&[], body);
            }
            ClassMember::Setter { param, body, decorators, .. } => {
                for decorator in decorators {
                    self.visit_expr(decorator);
                }
                self.visit_function(std::slice::from_ref(param), body);
            }
            ClassMember::StaticBlock { body, .. } => self.visit_block(body),
        }
    }

    fn visit_discarded_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Grouping { expr, .. } => self.visit_discarded_expr(expr),
            Expr::Binary { op, lhs, rhs, .. }
                if is_compound_assign(*op) && matches!(lhs.as_ref(), Expr::Identifier(_)) =>
            {
                self.visit_expr(rhs);
            }
            Expr::Postfix { expr: operand, .. } if matches!(operand.as_ref(), Expr::Identifier(_)) => {}
            other => self.visit_expr(other),
        }
    }

    fn visit_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Identifier(id) => self.read(&id.name),
            Expr::Literal(literal) => self.visit_literal(literal),
            Expr::Grouping { expr, .. }
            | Expr::Unary { expr, .. }
            | Expr::Postfix { expr, .. }
            | Expr::Spread { expr, .. } => self.visit_expr(expr),
            Expr::Binary { op, lhs, rhs, .. } => {
                if matches!(op, BinaryOp::Assign) && matches!(lhs.as_ref(), Expr::Identifier(_)) {
                    self.visit_expr(rhs);
                } else {
                    self.visit_expr(lhs);
                    self.visit_expr(rhs);
                }
            }
            Expr::Assignment { value, .. } => self.visit_expr(value),
            Expr::Conditional { condition, then_expr, else_expr, .. } => {
                self.visit_expr(condition);
                self.visit_expr(then_expr);
                self.visit_expr(else_expr);
            }
            Expr::Call { callee, args, .. }
            | Expr::OptionalCall { callee, args, .. }
            | Expr::New { callee, args, .. } => {
                self.visit_expr(callee);
                for arg in args {
                    self.visit_expr(arg);
                }
            }
            Expr::Index { object, index, .. } | Expr::OptionalIndex { object, index, .. } => {
                self.visit_expr(object);
                self.visit_expr(index);
            }
            Expr::Member { object, .. } | Expr::OptionalMember { object, .. } => self.visit_expr(object),
            Expr::ArrowFunction { params, body, .. } => self.visit_function(params, body),
            Expr::FunctionExpr { name, params, body, .. } => {
                self.push_scope();
                if let Some(name) = name {
                    self.declare(&name.name, name.span, DeclKind::NonFlag, false, None);
                }
                self.declare_params(params);
                self.visit_param_defaults(params);
                self.visit_stmt_list(&body.stmts);
                self.pop_scope();
            }
            Expr::TemplateLiteral { parts, .. } => {
                for part in parts {
                    if let TemplatePart::Expr(expr) = part {
                        self.visit_expr(expr);
                    }
                }
            }
            Expr::TaggedTemplate { tag, expressions, .. } => {
                self.visit_expr(tag);
                for expr in expressions {
                    self.visit_expr(expr);
                }
            }
            Expr::Yield { argument, .. } => {
                if let Some(argument) = argument {
                    self.visit_expr(argument);
                }
            }
            Expr::Await { argument, .. } => self.visit_expr(argument),
            Expr::DynamicImport { source, .. } => self.visit_expr(source),
            Expr::This { .. } | Expr::Super { .. } => {}
        }
    }

    fn visit_literal(&mut self, literal: &Literal) {
        match literal {
            Literal::Array { elements, .. } => {
                for element in elements {
                    self.visit_expr(element);
                }
            }
            Literal::Object { entries, .. } => {
                for entry in entries {
                    self.visit_object_entry(entry);
                }
            }
            _ => {}
        }
    }

    fn visit_object_entry(&mut self, entry: &ObjectEntry) {
        match entry {
            ObjectEntry::Property { key, value } => {
                self.visit_prop_key(key);
                self.visit_expr(value);
            }
            ObjectEntry::Spread(expr) => self.visit_expr(expr),
            ObjectEntry::Getter { key, body, .. } => {
                self.visit_prop_key(key);
                self.visit_function(&[], body);
            }
            ObjectEntry::Setter { key, param, body, .. } => {
                self.visit_prop_key(key);
                self.visit_function(std::slice::from_ref(param), body);
            }
        }
    }

    fn visit_prop_key(&mut self, key: &PropKey) {
        if let PropKey::Computed(expr) = key {
            self.visit_expr(expr);
        }
    }
}
