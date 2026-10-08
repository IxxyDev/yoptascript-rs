use std::collections::HashSet;

use yps_lexer::Span;
use yps_parser::{
    BinaryOp, Block, ClassMember, ExportKind, Expr, Identifier, ImportSpec, Literal, ObjectEntry, Param, Pattern,
    Program, PropKey, Stmt, SwitchCase, TemplatePart,
};

use crate::{LintDiagnostic, Rule};

const STACK_RED_ZONE: usize = 128 * 1024;
const STACK_GROW_SIZE: usize = 4 * 1024 * 1024;

pub(crate) fn lint_program(program: &Program) -> Vec<LintDiagnostic> {
    let mut linter = Linter { scopes: Vec::new(), diags: Vec::new() };
    linter.push_scope();
    linter.visit_stmt_list(&program.items);
    linter.pop_scope();
    linter.diags.sort_by_key(|d| (d.span.start, d.span.end));
    linter.diags
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Accessor {
    Get,
    Set,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParamShape {
    Simple,
    Rest,
    Destructured,
}

#[derive(Debug, Clone, Copy)]
struct ParamMeta {
    slot: usize,
    shape: ParamShape,
}

#[derive(Debug, Clone, Copy)]
enum DeclKind {
    Var,
    Const,
    Import,
    Param(ParamMeta),
    Silent,
}

impl DeclKind {
    const fn binding(is_const: bool) -> Self {
        if is_const { Self::Const } else { Self::Var }
    }

    fn unused_report(self, name: &str) -> Option<(Rule, String)> {
        match self {
            Self::Var => Some((Rule::UnusedVariable, format!("переменная «{name}» объявлена, но не используется"))),
            Self::Const => Some((Rule::UnusedVariable, format!("константа «{name}» объявлена, но не используется"))),
            Self::Param(_) => Some((Rule::UnusedVariable, format!("параметр «{name}» не используется"))),
            Self::Import => Some((Rule::UnusedImport, format!("импорт «{name}» не используется"))),
            Self::Silent => None,
        }
    }
}

struct DeclInfo {
    name: String,
    span: Span,
    kind: DeclKind,
    used: bool,
    exported: bool,
}

struct Scope {
    decls: Vec<DeclInfo>,
}

struct Linter {
    scopes: Vec<Scope>,
    diags: Vec<LintDiagnostic>,
}

fn for_each_binding(pattern: &Pattern, f: &mut impl FnMut(&Identifier)) {
    match pattern {
        Pattern::Identifier(id) => f(id),
        Pattern::Array { elements, rest, .. } => {
            for element in elements.iter().flatten() {
                for_each_binding(element, f);
            }
            if let Some(rest) = rest {
                for_each_binding(rest, f);
            }
        }
        Pattern::Object { properties, rest, .. } => {
            for prop in properties {
                match &prop.value {
                    Some(value) => for_each_binding(value, f),
                    None => f(&prop.key),
                }
            }
            if let Some(rest) = rest {
                for_each_binding(rest, f);
            }
        }
        Pattern::Default { pattern, .. } => for_each_binding(pattern, f),
    }
}

impl Linter {
    fn report(&mut self, rule: Rule, span: Span, message: String) {
        self.diags.push(LintDiagnostic { span, rule, severity: rule.severity(), message });
    }

    fn push_scope(&mut self) {
        self.scopes.push(Scope { decls: Vec::new() });
    }

    fn pop_scope(&mut self) {
        let scope = self.scopes.pop().expect("сбалансированный стек областей");

        let last_used_slot = scope
            .decls
            .iter()
            .filter(|decl| decl.used)
            .filter_map(|decl| match decl.kind {
                DeclKind::Param(meta) => Some(meta.slot),
                _ => None,
            })
            .max();

        for decl in &scope.decls {
            if decl.used || decl.exported || decl.name.starts_with('_') {
                continue;
            }
            if let DeclKind::Param(ParamMeta { slot, shape }) = decl.kind {
                let before_used = shape == ParamShape::Simple && last_used_slot.is_some_and(|last| slot <= last);
                if shape == ParamShape::Rest || before_used {
                    continue;
                }
            }
            if let Some((rule, message)) = decl.kind.unused_report(&decl.name) {
                self.report(rule, decl.span, message);
            }
        }
    }

    fn declare(&mut self, id: &Identifier, kind: DeclKind, exported: bool) {
        if self.shadows_outer(&id.name) {
            self.report(
                Rule::ShadowedDeclaration,
                id.span,
                format!("объявление «{}» затеняет биндинг из внешней области видимости", id.name),
            );
        }
        self.bind(id, kind, exported);
    }

    fn bind(&mut self, id: &Identifier, kind: DeclKind, exported: bool) {
        let scope = self.scopes.last_mut().expect("активная область");
        if let Some(existing) = scope.decls.iter_mut().find(|d| d.name == id.name) {
            if matches!(existing.kind, DeclKind::Silent) {
                existing.kind = kind;
                existing.span = id.span;
            }
            existing.exported |= exported;
            return;
        }
        scope.decls.push(DeclInfo { name: id.name.clone(), span: id.span, kind, used: false, exported });
    }

    fn shadows_outer(&self, name: &str) -> bool {
        let top = self.scopes.len().saturating_sub(1);
        self.scopes[..top].iter().any(|scope| scope.decls.iter().any(|d| d.name == name))
    }

    fn read(&mut self, id: &Identifier) {
        let target = self.scopes.iter_mut().rev().find_map(|scope| scope.decls.iter_mut().find(|d| d.name == id.name));
        if let Some(decl) = target {
            decl.used = true;
        }
    }

    fn declare_pattern(&mut self, pattern: &Pattern, kind: DeclKind, exported: bool) {
        for_each_binding(pattern, &mut |id| self.declare(id, kind, exported));
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
                self.report(Rule::UnreachableCode, stmt.span(), format!("недостижимый код после «{keyword}»"));
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
                self.declare_pattern(pattern, DeclKind::binding(*is_const), exported);
            }
            Stmt::FunctionDecl { name, .. } | Stmt::ClassDecl { name, .. } | Stmt::Using { name, .. } => {
                self.declare(name, DeclKind::Silent, exported);
            }
            Stmt::Import { specifiers, .. } => {
                for spec in specifiers {
                    let (ImportSpec::Default { local }
                    | ImportSpec::Named { local, .. }
                    | ImportSpec::Namespace { local }) = spec;
                    self.declare(local, DeclKind::Import, exported);
                }
            }
            Stmt::Export { kind: ExportKind::Declaration(inner), .. } => self.hoist_stmt(inner, true),
            _ => {}
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
                for value in properties.iter().filter_map(|prop| prop.value.as_ref()) {
                    self.visit_pattern_exprs(value);
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
        stacker::maybe_grow(STACK_RED_ZONE, STACK_GROW_SIZE, || self.visit_stmt_inner(stmt));
    }

    fn visit_stmt_inner(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::VarDecl { pattern, init, .. } => {
                self.visit_pattern_exprs(pattern);
                self.visit_expr(init);
            }
            Stmt::Using { init, .. } | Stmt::Throw { value: init, .. } => self.visit_expr(init),
            Stmt::Expr { expr, .. } => self.visit_discarded_expr(expr),
            Stmt::Block(block) => self.visit_block(block),
            Stmt::Empty { .. }
            | Stmt::Break { .. }
            | Stmt::Continue { .. }
            | Stmt::Debugger { .. }
            | Stmt::Import { .. }
            | Stmt::Return { value: None, .. } => {}
            Stmt::Return { value: Some(value), .. } => self.visit_expr(value),
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
                self.visit_for(init.as_deref(), condition.as_ref(), update.as_ref(), body);
            }
            Stmt::ForIn { variable, iterable, body, is_const, .. }
            | Stmt::ForOf { variable, iterable, body, is_const, .. }
            | Stmt::ForAwaitOf { variable, iterable, body, is_const, .. } => {
                self.visit_for_each(variable, iterable, body, *is_const);
            }
            Stmt::Labeled { body, .. } => self.visit_stmt(body),
            Stmt::FunctionDecl { params, body, .. } => self.visit_function(params, body),
            Stmt::TryCatch { try_block, catch_param, catch_block, finally_block, .. } => {
                self.visit_try(try_block, catch_param.as_ref(), catch_block.as_ref(), finally_block.as_ref());
            }
            Stmt::Switch { expr, cases, default, .. } => self.visit_switch(expr, cases, default.as_ref()),
            Stmt::ClassDecl { super_class, members, decorators, .. } => {
                self.visit_class(decorators, super_class.as_ref(), members);
            }
            Stmt::Export { kind: ExportKind::Declaration(inner), .. } => self.visit_stmt(inner),
            Stmt::Export { kind: ExportKind::Named(names), .. } => {
                for name in names {
                    self.read(name);
                }
            }
        }
    }

    fn visit_for(&mut self, init: Option<&Stmt>, condition: Option<&Expr>, update: Option<&Expr>, body: &Stmt) {
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

    fn visit_for_each(&mut self, variable: &Pattern, iterable: &Expr, body: &Stmt, is_const: bool) {
        self.visit_expr(iterable);
        self.push_scope();
        self.declare_pattern(variable, DeclKind::binding(is_const), false);
        self.visit_pattern_exprs(variable);
        self.visit_branch(body);
        self.pop_scope();
    }

    fn visit_try(
        &mut self,
        try_block: &Block,
        catch_param: Option<&Identifier>,
        catch_block: Option<&Block>,
        finally_block: Option<&Block>,
    ) {
        self.visit_block(try_block);
        if let Some(catch_block) = catch_block {
            self.push_scope();
            if let Some(param) = catch_param {
                self.declare(param, DeclKind::Silent, false);
            }
            self.visit_stmt_list(&catch_block.stmts);
            self.pop_scope();
        }
        if let Some(finally_block) = finally_block {
            self.visit_block(finally_block);
        }
    }

    fn visit_switch(&mut self, expr: &Expr, cases: &[SwitchCase], default: Option<&Block>) {
        self.visit_expr(expr);
        for case in cases {
            self.visit_expr(&case.value);
            self.visit_block(&case.body);
        }
        if let Some(default) = default {
            self.visit_block(default);
        }
    }

    fn visit_class(&mut self, decorators: &[Expr], super_class: Option<&Expr>, members: &[ClassMember]) {
        self.visit_exprs(decorators);
        if let Some(super_class) = super_class {
            self.visit_expr(super_class);
        }
        for member in members {
            self.visit_class_member(member);
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
        self.visit_function_body(None, params, body);
    }

    fn visit_function_body(&mut self, name: Option<&Identifier>, params: &[Param], body: &Block) {
        self.push_scope();
        if let Some(name) = name {
            self.bind(name, DeclKind::Silent, false);
        }
        self.declare_params(params);
        self.visit_param_defaults(params);
        self.visit_stmt_list(&body.stmts);
        self.pop_scope();
    }

    fn check_duplicate_object_keys(&mut self, entries: &[ObjectEntry]) {
        let mut seen: HashSet<(&str, Accessor)> = HashSet::new();
        for entry in entries {
            let (id, accessors): (_, &[Accessor]) = match entry {
                ObjectEntry::Property { key: PropKey::Identifier(id), .. } => (id, &[Accessor::Get, Accessor::Set]),
                ObjectEntry::Getter { key: PropKey::Identifier(id), .. } => (id, &[Accessor::Get]),
                ObjectEntry::Setter { key: PropKey::Identifier(id), .. } => (id, &[Accessor::Set]),
                _ => continue,
            };
            let mut duplicate = false;
            for &accessor in accessors {
                duplicate |= !seen.insert((id.name.as_str(), accessor));
            }
            if duplicate {
                self.report(
                    Rule::DuplicateObjectKey,
                    id.span,
                    format!("ключ «{}» повторяется в объектном литерале", id.name),
                );
            }
        }
    }

    fn check_duplicate_params(&mut self, params: &[Param]) {
        let mut seen: HashSet<&str> = HashSet::new();
        for param in params.iter().filter(|param| !param.is_rest && param.pattern.is_none()) {
            if !seen.insert(&param.name.name) {
                self.report(
                    Rule::DuplicateParam,
                    param.name.span,
                    format!("параметр «{}» повторяется в списке параметров", param.name.name),
                );
            }
        }
    }

    fn declare_params(&mut self, params: &[Param]) {
        self.check_duplicate_params(params);
        for (slot, param) in params.iter().enumerate() {
            let shape = if param.is_rest {
                ParamShape::Rest
            } else if param.pattern.is_some() {
                ParamShape::Destructured
            } else {
                ParamShape::Simple
            };
            let kind = DeclKind::Param(ParamMeta { slot, shape });
            match &param.pattern {
                Some(pattern) => self.declare_pattern(pattern, kind, false),
                None => self.declare(&param.name, kind, false),
            }
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
                self.visit_exprs(decorators);
                self.visit_function(params, body);
            }
            ClassMember::Field { init, decorators, .. } => {
                self.visit_exprs(decorators);
                if let Some(init) = init {
                    self.visit_expr(init);
                }
            }
            ClassMember::Getter { body, decorators, .. } => {
                self.visit_exprs(decorators);
                self.visit_function(&[], body);
            }
            ClassMember::Setter { param, body, decorators, .. } => {
                self.visit_exprs(decorators);
                self.visit_function(std::slice::from_ref(param), body);
            }
            ClassMember::StaticBlock { body, .. } => self.visit_block(body),
        }
    }

    fn visit_discarded_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Grouping { expr, .. } => self.visit_discarded_expr(expr),
            Expr::Binary { op, lhs, rhs, span }
                if op.is_compound_assign() && matches!(lhs.as_ref(), Expr::Identifier(_)) =>
            {
                self.check_self_assignment(*op, lhs, rhs, *span);
                self.visit_expr(rhs);
            }
            Expr::Postfix { expr: operand, .. } if matches!(operand.as_ref(), Expr::Identifier(_)) => {}
            other => self.visit_expr(other),
        }
    }

    fn visit_exprs(&mut self, exprs: &[Expr]) {
        for expr in exprs {
            self.visit_expr(expr);
        }
    }

    fn visit_expr(&mut self, expr: &Expr) {
        stacker::maybe_grow(STACK_RED_ZONE, STACK_GROW_SIZE, || self.visit_expr_inner(expr));
    }

    fn visit_expr_inner(&mut self, expr: &Expr) {
        match expr {
            Expr::Identifier(id) => self.read(id),
            Expr::Literal(literal) => self.visit_literal(literal),
            Expr::Grouping { expr, .. }
            | Expr::Unary { expr, .. }
            | Expr::Postfix { expr, .. }
            | Expr::Spread { expr, .. }
            | Expr::Await { argument: expr, .. }
            | Expr::Yield { argument: Some(expr), .. }
            | Expr::DynamicImport { source: expr, .. }
            | Expr::Assignment { value: expr, .. }
            | Expr::Member { object: expr, .. }
            | Expr::OptionalMember { object: expr, .. } => self.visit_expr(expr),
            Expr::Binary { op, lhs, rhs, span } => {
                self.check_self_assignment(*op, lhs, rhs, *span);
                if *op != BinaryOp::Assign || !matches!(lhs.as_ref(), Expr::Identifier(_)) {
                    self.visit_expr(lhs);
                }
                self.visit_expr(rhs);
            }
            Expr::Conditional { condition, then_expr, else_expr, .. } => {
                self.visit_expr(condition);
                self.visit_expr(then_expr);
                self.visit_expr(else_expr);
            }
            Expr::Call { callee, args, .. }
            | Expr::OptionalCall { callee, args, .. }
            | Expr::New { callee, args, .. }
            | Expr::TaggedTemplate { tag: callee, expressions: args, .. } => {
                self.visit_expr(callee);
                self.visit_exprs(args);
            }
            Expr::Index { object, index, .. } | Expr::OptionalIndex { object, index, .. } => {
                self.visit_expr(object);
                self.visit_expr(index);
            }
            Expr::ArrowFunction { params, body, .. } => self.visit_function(params, body),
            Expr::FunctionExpr { name, params, body, .. } => self.visit_function_body(name.as_ref(), params, body),
            Expr::TemplateLiteral { parts, .. } => {
                for part in parts {
                    if let TemplatePart::Expr(expr) = part {
                        self.visit_expr(expr);
                    }
                }
            }
            Expr::Yield { argument: None, .. } | Expr::This { .. } | Expr::Super { .. } => {}
        }
    }

    fn check_self_assignment(&mut self, op: BinaryOp, lhs: &Expr, rhs: &Expr, span: Span) {
        if matches!(op, BinaryOp::Assign | BinaryOp::NullishAssign | BinaryOp::OrAssign | BinaryOp::AndAssign)
            && let (Expr::Identifier(target), Expr::Identifier(source)) = (lhs, rhs)
            && target.name == source.name
        {
            self.report(
                Rule::SelfAssignment,
                span,
                format!("присваивание «{}» самому себе не имеет эффекта", target.name),
            );
        }
    }

    fn visit_literal(&mut self, literal: &Literal) {
        match literal {
            Literal::Array { elements, .. } => self.visit_exprs(elements),
            Literal::Object { entries, .. } => {
                self.check_duplicate_object_keys(entries);
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
