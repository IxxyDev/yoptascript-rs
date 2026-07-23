use std::collections::HashSet;

use yps_parser::ast::{
    Block, ClassMember, ExportKind, Expr, ImportSpec, Literal, ObjectEntry, Param, Pattern, Program, PropKey, Stmt,
    TemplatePart,
};

#[derive(Default)]
pub(crate) struct RootResolution {
    reads: HashSet<usize>,
}

impl RootResolution {
    pub(crate) fn is_empty(&self) -> bool {
        self.reads.is_empty()
    }

    pub(crate) fn is_root_read(&self, start: usize) -> bool {
        self.reads.contains(&start)
    }
}

pub(crate) fn resolve(program: &Program) -> RootResolution {
    let mut resolver = Resolver { reads: HashSet::new(), scopes: Vec::new(), disabled: false };
    for stmt in &program.items {
        resolver.walk_stmt(stmt);
    }
    if resolver.disabled { RootResolution::default() } else { RootResolution { reads: resolver.reads } }
}

const STACK_RED_ZONE: usize = 256 * 1024;
const STACK_GROW_SIZE: usize = 8 * 1024 * 1024;

struct Resolver {
    reads: HashSet<usize>,
    scopes: Vec<HashSet<String>>,
    disabled: bool,
}

impl Resolver {
    fn record_read(&mut self, name: &str, start: usize) {
        if self.scopes.iter().any(|scope| scope.contains(name)) {
            return;
        }
        self.reads.insert(start);
    }

    fn walk_function(&mut self, own_name: Option<&str>, params: &[Param], body: &Block) {
        let mut locals = HashSet::new();
        if let Some(name) = own_name {
            locals.insert(name.to_string());
        }
        for param in params {
            collect_param_names(param, &mut locals);
        }
        collect_block_locals(body, &mut locals);
        self.scopes.push(locals);
        for param in params {
            if let Some(pattern) = &param.pattern {
                self.walk_pattern_defaults(pattern);
            }
            if let Some(default) = &param.default {
                self.walk_expr(default);
            }
        }
        for stmt in &body.stmts {
            self.walk_stmt(stmt);
        }
        self.scopes.pop();
    }

    fn walk_pattern_defaults(&mut self, pattern: &Pattern) {
        match pattern {
            Pattern::Identifier(_) => {}
            Pattern::Default { pattern, default, .. } => {
                self.walk_pattern_defaults(pattern);
                self.walk_expr(default);
            }
            Pattern::Array { elements, rest, .. } => {
                for element in elements.iter().flatten() {
                    self.walk_pattern_defaults(element);
                }
                if let Some(rest) = rest {
                    self.walk_pattern_defaults(rest);
                }
            }
            Pattern::Object { properties, rest, .. } => {
                for prop in properties {
                    if let Some(value) = &prop.value {
                        self.walk_pattern_defaults(value);
                    }
                }
                if let Some(rest) = rest {
                    self.walk_pattern_defaults(rest);
                }
            }
        }
    }

    fn walk_block(&mut self, block: &Block) {
        let mut locals = HashSet::new();
        collect_block_locals(block, &mut locals);
        self.scopes.push(locals);
        for stmt in &block.stmts {
            self.walk_stmt(stmt);
        }
        self.scopes.pop();
    }

    fn walk_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::VarDecl { pattern, init, .. } => {
                self.walk_pattern_defaults(pattern);
                self.walk_expr(init);
            }
            Stmt::Expr { expr, .. } => self.walk_expr(expr),
            Stmt::Block(block) => self.walk_block(block),
            Stmt::Empty { .. } | Stmt::Break { .. } | Stmt::Continue { .. } | Stmt::Debugger { .. } => {}
            Stmt::If { condition, then_branch, else_branch, .. } => {
                self.walk_expr(condition);
                self.walk_stmt(then_branch);
                if let Some(else_branch) = else_branch {
                    self.walk_stmt(else_branch);
                }
            }
            Stmt::While { condition, body, .. } => {
                self.walk_expr(condition);
                self.walk_stmt(body);
            }
            Stmt::DoWhile { body, condition, .. } => {
                self.walk_stmt(body);
                self.walk_expr(condition);
            }
            Stmt::For { init, condition, update, body, .. } => {
                let mut locals = HashSet::new();
                if let Some(init) = init {
                    collect_stmt_locals(init, &mut locals);
                }
                collect_stmt_locals(body, &mut locals);
                self.scopes.push(locals);
                if let Some(init) = init {
                    self.walk_stmt(init);
                }
                if let Some(condition) = condition {
                    self.walk_expr(condition);
                }
                if let Some(update) = update {
                    self.walk_expr(update);
                }
                self.walk_stmt(body);
                self.scopes.pop();
            }
            Stmt::Labeled { body, .. } => self.walk_stmt(body),
            Stmt::FunctionDecl { params, body, .. } => self.walk_function(None, params, body),
            Stmt::Return { value, .. } => {
                if let Some(value) = value {
                    self.walk_expr(value);
                }
            }
            Stmt::TryCatch { try_block, catch_param, catch_block, finally_block, .. } => {
                self.walk_block(try_block);
                if let Some(catch_block) = catch_block {
                    let mut locals = HashSet::new();
                    if let Some(param) = catch_param {
                        locals.insert(param.name.clone());
                    }
                    collect_block_locals(catch_block, &mut locals);
                    self.scopes.push(locals);
                    for stmt in &catch_block.stmts {
                        self.walk_stmt(stmt);
                    }
                    self.scopes.pop();
                }
                if let Some(finally_block) = finally_block {
                    self.walk_block(finally_block);
                }
            }
            Stmt::Throw { value, .. } => self.walk_expr(value),
            Stmt::Switch { expr, cases, default, .. } => {
                self.walk_expr(expr);
                for case in cases {
                    self.walk_expr(&case.value);
                    self.walk_block(&case.body);
                }
                if let Some(default) = default {
                    self.walk_block(default);
                }
            }
            Stmt::ForIn { variable, iterable, body, .. }
            | Stmt::ForOf { variable, iterable, body, .. }
            | Stmt::ForAwaitOf { variable, iterable, body, .. } => {
                self.walk_expr(iterable);
                let mut locals = HashSet::new();
                collect_pattern_names(variable, &mut locals);
                collect_stmt_locals(body, &mut locals);
                self.scopes.push(locals);
                self.walk_pattern_defaults(variable);
                self.walk_stmt(body);
                self.scopes.pop();
            }
            Stmt::ClassDecl { super_class, members, decorators, .. } => {
                if let Some(super_class) = super_class {
                    self.walk_expr(super_class);
                }
                for decorator in decorators {
                    self.walk_expr(decorator);
                }
                for member in members {
                    self.walk_class_member(member);
                }
            }
            Stmt::Using { init, .. } => self.walk_expr(init),
            Stmt::Import { .. } => self.disabled = true,
            Stmt::Export { kind, .. } => match kind {
                ExportKind::Declaration(decl) => self.walk_stmt(decl),
                ExportKind::Named(_) => {}
            },
        }
    }

    fn walk_class_member(&mut self, member: &ClassMember) {
        match member {
            ClassMember::Constructor { params, body, .. } => self.walk_function(None, params, body),
            ClassMember::Method { params, body, decorators, .. } => {
                for decorator in decorators {
                    self.walk_expr(decorator);
                }
                self.walk_function(None, params, body);
            }
            ClassMember::Field { init, decorators, .. } => {
                for decorator in decorators {
                    self.walk_expr(decorator);
                }
                if let Some(init) = init {
                    self.walk_expr(init);
                }
            }
            ClassMember::Getter { body, decorators, .. } => {
                for decorator in decorators {
                    self.walk_expr(decorator);
                }
                self.walk_function(None, &[], body);
            }
            ClassMember::Setter { param, body, decorators, .. } => {
                for decorator in decorators {
                    self.walk_expr(decorator);
                }
                self.walk_function(None, std::slice::from_ref(param), body);
            }
            ClassMember::StaticBlock { body, .. } => self.walk_function(None, &[], body),
        }
    }

    fn walk_expr(&mut self, expr: &Expr) {
        stacker::maybe_grow(STACK_RED_ZONE, STACK_GROW_SIZE, || self.walk_expr_inner(expr));
    }

    fn walk_expr_inner(&mut self, expr: &Expr) {
        match expr {
            Expr::Identifier(ident) => self.record_read(&ident.name, ident.span.start),
            Expr::Literal(literal) => self.walk_literal(literal),
            Expr::Unary { expr, .. }
            | Expr::Postfix { expr, .. }
            | Expr::Grouping { expr, .. }
            | Expr::Spread { expr, .. } => self.walk_expr(expr),
            Expr::Binary { lhs, rhs, .. } => {
                self.walk_expr(lhs);
                self.walk_expr(rhs);
            }
            Expr::Assignment { value, .. } => self.walk_expr(value),
            Expr::Call { callee, args, .. }
            | Expr::OptionalCall { callee, args, .. }
            | Expr::New { callee, args, .. } => {
                self.walk_expr(callee);
                for arg in args {
                    self.walk_expr(arg);
                }
            }
            Expr::Index { object, index, .. } | Expr::OptionalIndex { object, index, .. } => {
                self.walk_expr(object);
                self.walk_expr(index);
            }
            Expr::Member { object, .. } | Expr::OptionalMember { object, .. } => self.walk_expr(object),
            Expr::Conditional { condition, then_expr, else_expr, .. } => {
                self.walk_expr(condition);
                self.walk_expr(then_expr);
                self.walk_expr(else_expr);
            }
            Expr::ArrowFunction { params, body, .. } => self.walk_function(None, params, body),
            Expr::FunctionExpr { name, params, body, .. } => {
                let own_name = name.as_ref().map(|name| name.name.as_str());
                self.walk_function(own_name, params, body);
            }
            Expr::TemplateLiteral { parts, .. } => {
                for part in parts {
                    if let TemplatePart::Expr(expr) = part {
                        self.walk_expr(expr);
                    }
                }
            }
            Expr::TaggedTemplate { tag, expressions, .. } => {
                self.walk_expr(tag);
                for expr in expressions {
                    self.walk_expr(expr);
                }
            }
            Expr::This { .. } | Expr::Super { .. } => {}
            Expr::Yield { argument, .. } => {
                if let Some(argument) = argument {
                    self.walk_expr(argument);
                }
            }
            Expr::Await { argument, .. } => self.walk_expr(argument),
            Expr::DynamicImport { source, .. } => {
                self.disabled = true;
                self.walk_expr(source);
            }
        }
    }

    fn walk_literal(&mut self, literal: &Literal) {
        match literal {
            Literal::Array { elements, .. } => {
                for element in elements {
                    self.walk_expr(element);
                }
            }
            Literal::Object { entries, .. } => {
                for entry in entries {
                    self.walk_object_entry(entry);
                }
            }
            _ => {}
        }
    }

    fn walk_object_entry(&mut self, entry: &ObjectEntry) {
        match entry {
            ObjectEntry::Property { key, value } => {
                self.walk_prop_key(key);
                self.walk_expr(value);
            }
            ObjectEntry::Spread(expr) => self.walk_expr(expr),
            ObjectEntry::Getter { key, body, .. } => {
                self.walk_prop_key(key);
                self.walk_function(None, &[], body);
            }
            ObjectEntry::Setter { key, param, body, .. } => {
                self.walk_prop_key(key);
                self.walk_function(None, std::slice::from_ref(param), body);
            }
        }
    }

    fn walk_prop_key(&mut self, key: &PropKey) {
        if let PropKey::Computed(expr) = key {
            self.walk_expr(expr);
        }
    }
}

fn collect_param_names(param: &Param, out: &mut HashSet<String>) {
    if let Some(pattern) = &param.pattern {
        collect_pattern_names(pattern, out);
    } else {
        out.insert(param.name.name.clone());
    }
}

fn collect_pattern_names(pattern: &Pattern, out: &mut HashSet<String>) {
    match pattern {
        Pattern::Identifier(ident) => {
            out.insert(ident.name.clone());
        }
        Pattern::Default { pattern, .. } => collect_pattern_names(pattern, out),
        Pattern::Array { elements, rest, .. } => {
            for element in elements.iter().flatten() {
                collect_pattern_names(element, out);
            }
            if let Some(rest) = rest {
                collect_pattern_names(rest, out);
            }
        }
        Pattern::Object { properties, rest, .. } => {
            for prop in properties {
                match &prop.value {
                    Some(value) => collect_pattern_names(value, out),
                    None => {
                        out.insert(prop.key.name.clone());
                    }
                }
            }
            if let Some(rest) = rest {
                collect_pattern_names(rest, out);
            }
        }
    }
}

fn collect_block_locals(block: &Block, out: &mut HashSet<String>) {
    for stmt in &block.stmts {
        collect_stmt_locals(stmt, out);
    }
}

fn collect_stmt_locals(stmt: &Stmt, out: &mut HashSet<String>) {
    match stmt {
        Stmt::VarDecl { pattern, .. } => collect_pattern_names(pattern, out),
        Stmt::FunctionDecl { name, .. } | Stmt::ClassDecl { name, .. } | Stmt::Using { name, .. } => {
            out.insert(name.name.clone());
        }
        Stmt::Block(block) => collect_block_locals(block, out),
        Stmt::If { then_branch, else_branch, .. } => {
            collect_stmt_locals(then_branch, out);
            if let Some(else_branch) = else_branch {
                collect_stmt_locals(else_branch, out);
            }
        }
        Stmt::While { body, .. } | Stmt::DoWhile { body, .. } | Stmt::Labeled { body, .. } => {
            collect_stmt_locals(body, out);
        }
        Stmt::For { init, body, .. } => {
            if let Some(init) = init {
                collect_stmt_locals(init, out);
            }
            collect_stmt_locals(body, out);
        }
        Stmt::ForIn { variable, body, .. }
        | Stmt::ForOf { variable, body, .. }
        | Stmt::ForAwaitOf { variable, body, .. } => {
            collect_pattern_names(variable, out);
            collect_stmt_locals(body, out);
        }
        Stmt::TryCatch { try_block, catch_param, catch_block, finally_block, .. } => {
            collect_block_locals(try_block, out);
            if let Some(param) = catch_param {
                out.insert(param.name.clone());
            }
            if let Some(catch_block) = catch_block {
                collect_block_locals(catch_block, out);
            }
            if let Some(finally_block) = finally_block {
                collect_block_locals(finally_block, out);
            }
        }
        Stmt::Switch { cases, default, .. } => {
            for case in cases {
                collect_block_locals(&case.body, out);
            }
            if let Some(default) = default {
                collect_block_locals(default, out);
            }
        }
        Stmt::Export { kind: ExportKind::Declaration(decl), .. } => collect_stmt_locals(decl, out),
        Stmt::Import { specifiers, .. } => {
            for spec in specifiers {
                let local = match spec {
                    ImportSpec::Default { local } | ImportSpec::Namespace { local } => local,
                    ImportSpec::Named { local, .. } => local,
                };
                out.insert(local.name.clone());
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests;
