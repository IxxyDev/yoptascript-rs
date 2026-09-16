use std::collections::{HashMap, HashSet};
use std::hash::{BuildHasherDefault, Hasher};
use std::rc::Rc;

use yps_parser::ast::{
    Block, ClassMember, ExportKind, Expr, ImportSpec, Literal, ObjectEntry, Param, Pattern, Program, PropKey, Stmt,
    TemplatePart,
};

use crate::environment::{MAX_SLOTS, ScopeLayout};

#[derive(Default)]
pub(crate) struct SpanHasher(u64);

impl Hasher for SpanHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 = (self.0 ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3);
        }
    }

    fn write_usize(&mut self, value: usize) {
        let mixed = (value as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        self.0 = mixed ^ (mixed >> 29);
    }
}

type SpanMap<V> = HashMap<usize, V, BuildHasherDefault<SpanHasher>>;

#[derive(Clone, Copy)]
pub(crate) struct VarUse {
    pub(crate) hops: u16,
    pub(crate) slot: u16,
    pub(crate) layout: u32,
}

#[derive(Default)]
pub(crate) struct RootResolution {
    reads: HashSet<usize>,
    uses: SpanMap<VarUse>,
    layouts: SpanMap<Rc<ScopeLayout>>,
    root_layout: Option<Rc<ScopeLayout>>,
}

impl RootResolution {
    pub(crate) fn is_empty(&self) -> bool {
        self.reads.is_empty()
    }

    pub(crate) fn is_root_read(&self, start: usize) -> bool {
        self.reads.contains(&start)
    }

    #[inline]
    pub(crate) fn use_at(&self, start: usize) -> Option<VarUse> {
        if self.uses.is_empty() { None } else { self.uses.get(&start).copied() }
    }

    #[inline]
    pub(crate) fn layout_at(&self, key: usize) -> Option<Rc<ScopeLayout>> {
        if self.layouts.is_empty() { None } else { self.layouts.get(&key).cloned() }
    }

    pub(crate) fn root_layout(&self) -> Option<Rc<ScopeLayout>> {
        self.root_layout.clone()
    }
}

pub(crate) fn resolve(program: &Program) -> RootResolution {
    let mut resolver = Resolver {
        reads: HashSet::new(),
        scopes: Vec::new(),
        frames: Vec::new(),
        uses: SpanMap::default(),
        layouts: SpanMap::default(),
        disabled: false,
        slots_disabled: false,
        next_layout_id: 0,
    };
    let root = resolver.open_frame(scope_names(&program.items));
    for stmt in &program.items {
        resolver.walk_stmt(stmt);
    }
    resolver.frames.pop();
    if resolver.disabled {
        return RootResolution::default();
    }
    if resolver.slots_disabled {
        return RootResolution { reads: resolver.reads, ..RootResolution::default() };
    }
    RootResolution { reads: resolver.reads, uses: resolver.uses, layouts: resolver.layouts, root_layout: root }
}

const STACK_RED_ZONE: usize = 256 * 1024;
const STACK_GROW_SIZE: usize = 8 * 1024 * 1024;

/// One runtime `EnvFrame` as predicted by the resolver. The stack must mirror the interpreter's
/// `push_scope` / `fork_current` calls exactly, otherwise hop counts point at the wrong frame.
struct ResolvedFrame {
    names: Vec<Rc<str>>,
    layout: Option<u32>,
}

struct Resolver {
    reads: HashSet<usize>,
    scopes: Vec<HashSet<String>>,
    frames: Vec<ResolvedFrame>,
    uses: SpanMap<VarUse>,
    layouts: SpanMap<Rc<ScopeLayout>>,
    disabled: bool,
    slots_disabled: bool,
    next_layout_id: u32,
}

impl Resolver {
    fn open_frame(&mut self, mut layout: ScopeLayout) -> Option<Rc<ScopeLayout>> {
        let names = layout.names.clone();
        self.next_layout_id += 1;
        layout.id = self.next_layout_id;
        let shared = (names.len() <= MAX_SLOTS).then(|| Rc::new(layout));
        let id = shared.as_ref().map(|l| l.id);
        self.frames.push(ResolvedFrame { names, layout: id });
        shared
    }

    fn push_layout(&mut self, key: usize, layout: ScopeLayout) {
        let Some(rc) = self.open_frame(layout) else { return };
        if self.layouts.insert(key, rc).is_some() {
            self.slots_disabled = true;
        }
    }

    fn pop_frame(&mut self) {
        self.frames.pop();
    }

    fn record_use(&mut self, name: &str, start: usize) {
        if self.slots_disabled {
            return;
        }
        let mut hops: u16 = 0;
        for frame in self.frames.iter().rev() {
            if let Some(index) = frame.names.iter().position(|n| &**n == name) {
                if let Some(layout) = frame.layout {
                    self.uses.insert(start, VarUse { hops, slot: index as u16, layout });
                }
                return;
            }
            hops = match hops.checked_add(1) {
                Some(next) => next,
                None => return,
            };
        }
    }

    fn record_read(&mut self, name: &str, start: usize) {
        self.record_use(name, start);
        if self.scopes.iter().any(|scope| scope.contains(name)) {
            return;
        }
        self.reads.insert(start);
    }

    fn walk_function(&mut self, own_name: Option<&yps_parser::ast::Identifier>, params: &[Param], body: &Block) {
        let mut locals = HashSet::new();
        if let Some(name) = own_name {
            locals.insert(name.name.clone());
        }
        for param in params {
            collect_param_names(param, &mut locals);
        }
        collect_block_locals(body, &mut locals);
        self.scopes.push(locals);

        if let Some(name) = own_name {
            let layout = ScopeLayout { names: vec![Rc::from(name.name.as_str())], tdz_mask: 0, id: 0 };
            self.push_layout(name.span.start, layout);
        }
        self.push_layout(body.span.start, function_scope_names(params, &body.stmts));

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

        self.pop_frame();
        if own_name.is_some() {
            self.pop_frame();
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
        self.push_layout(block.span.start, scope_names(&block.stmts));
        for stmt in &block.stmts {
            self.walk_stmt(stmt);
        }
        self.pop_frame();
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
            Stmt::For { init, condition, update, body, span } => {
                let mut locals = HashSet::new();
                if let Some(init) = init {
                    collect_stmt_locals(init, &mut locals);
                }
                collect_stmt_locals(body, &mut locals);
                self.scopes.push(locals);
                let head = match init {
                    Some(init) => head_names(std::slice::from_ref(init.as_ref())),
                    None => ScopeLayout::default(),
                };
                self.push_layout(span.start, head);
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
                self.pop_frame();
                self.scopes.pop();
            }
            Stmt::Labeled { body, .. } => self.walk_stmt(body),
            Stmt::FunctionDecl { params, body, is_generator, is_async, .. } => {
                if *is_generator || *is_async {
                    self.slots_disabled = true;
                }
                self.walk_function(None, params, body);
            }
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
                    let mut layout = scope_names(&catch_block.stmts);
                    if let Some(param) = catch_param {
                        prepend_name(&mut layout, &param.name);
                    }
                    self.push_layout(catch_block.span.start, layout);
                    for stmt in &catch_block.stmts {
                        self.walk_stmt(stmt);
                    }
                    self.pop_frame();
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
            Stmt::ForIn { variable, iterable, body, span, .. }
            | Stmt::ForOf { variable, iterable, body, span, .. }
            | Stmt::ForAwaitOf { variable, iterable, body, span, .. } => {
                if matches!(stmt, Stmt::ForAwaitOf { .. }) {
                    self.slots_disabled = true;
                }
                self.walk_expr(iterable);
                let mut locals = HashSet::new();
                collect_pattern_names(variable, &mut locals);
                collect_stmt_locals(body, &mut locals);
                self.scopes.push(locals);
                let mut names = Vec::new();
                collect_pattern_names_ordered(variable, &mut names);
                self.push_layout(span.start, ScopeLayout { names, tdz_mask: 0, id: 0 });
                self.walk_pattern_defaults(variable);
                self.walk_stmt(body);
                self.pop_frame();
                self.scopes.pop();
            }
            Stmt::ClassDecl { super_class, members, decorators, .. } => {
                self.slots_disabled = true;
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
            Expr::Assignment { target, value, .. } => {
                self.record_use(&target.name, target.span.start);
                self.walk_expr(value);
            }
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
            Expr::ArrowFunction { params, body, is_async, .. } => {
                if *is_async {
                    self.slots_disabled = true;
                }
                self.walk_function(None, params, body);
            }
            Expr::FunctionExpr { name, params, body, is_generator, is_async, .. } => {
                if *is_generator || *is_async {
                    self.slots_disabled = true;
                }
                self.walk_function(name.as_ref(), params, body);
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
                self.slots_disabled = true;
                if let Some(argument) = argument {
                    self.walk_expr(argument);
                }
            }
            Expr::Await { argument, .. } => {
                self.slots_disabled = true;
                self.walk_expr(argument);
            }
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

fn push_name(names: &mut Vec<Rc<str>>, name: &str) {
    if !names.iter().any(|n| &**n == name) {
        names.push(Rc::from(name));
    }
}

fn prepend_name(layout: &mut ScopeLayout, name: &str) {
    if layout.names.iter().any(|n| &**n == name) {
        return;
    }
    layout.names.insert(0, Rc::from(name));
    layout.tdz_mask <<= 1;
}

/// Names a block-like frame owns: the lexical declarations that need TDZ marking, followed by the
/// hoisted function declarations of the same statement list.
fn scope_names(stmts: &[Stmt]) -> ScopeLayout {
    let mut names = Vec::new();
    collect_lexical_ordered(stmts, &mut names);
    let tdz_mask = mask_for(names.len());
    collect_function_decls(stmts, &mut names);
    ScopeLayout { names, tdz_mask, id: 0 }
}

fn head_names(stmts: &[Stmt]) -> ScopeLayout {
    let mut names = Vec::new();
    collect_lexical_ordered(stmts, &mut names);
    ScopeLayout { names, tdz_mask: 0, id: 0 }
}

fn function_scope_names(params: &[Param], stmts: &[Stmt]) -> ScopeLayout {
    let mut names = Vec::new();
    for param in params {
        collect_param_names_ordered(param, &mut names);
    }
    let params_len = names.len();
    collect_lexical_ordered(stmts, &mut names);
    let tdz_mask = mask_for(names.len()) & !mask_for(params_len);
    collect_function_decls(stmts, &mut names);
    ScopeLayout { names, tdz_mask, id: 0 }
}

fn mask_for(count: usize) -> u64 {
    if count >= MAX_SLOTS { u64::MAX } else { (1u64 << count) - 1 }
}

fn collect_lexical_ordered(stmts: &[Stmt], out: &mut Vec<Rc<str>>) {
    for stmt in stmts {
        match stmt {
            Stmt::VarDecl { pattern, .. } => collect_pattern_names_ordered(pattern, out),
            Stmt::ClassDecl { name, .. } | Stmt::Using { name, .. } => push_name(out, &name.name),
            Stmt::Export { kind: ExportKind::Declaration(decl), .. } => match decl.as_ref() {
                Stmt::VarDecl { pattern, .. } => collect_pattern_names_ordered(pattern, out),
                Stmt::ClassDecl { name, .. } => push_name(out, &name.name),
                _ => {}
            },
            _ => {}
        }
    }
}

fn collect_function_decls(stmts: &[Stmt], out: &mut Vec<Rc<str>>) {
    for stmt in stmts {
        match stmt {
            Stmt::FunctionDecl { name, .. } => push_name(out, &name.name),
            Stmt::Export { kind: ExportKind::Declaration(decl), .. } => {
                if let Stmt::FunctionDecl { name, .. } = decl.as_ref() {
                    push_name(out, &name.name);
                }
            }
            _ => {}
        }
    }
}

fn collect_param_names_ordered(param: &Param, out: &mut Vec<Rc<str>>) {
    match &param.pattern {
        Some(pattern) => collect_pattern_names_ordered(pattern, out),
        None => push_name(out, &param.name.name),
    }
}

fn collect_pattern_names_ordered(pattern: &Pattern, out: &mut Vec<Rc<str>>) {
    match pattern {
        Pattern::Identifier(ident) => push_name(out, &ident.name),
        Pattern::Default { pattern, .. } => collect_pattern_names_ordered(pattern, out),
        Pattern::Array { elements, rest, .. } => {
            for element in elements.iter().flatten() {
                collect_pattern_names_ordered(element, out);
            }
            if let Some(rest) = rest {
                collect_pattern_names_ordered(rest, out);
            }
        }
        Pattern::Object { properties, rest, .. } => {
            for prop in properties {
                match &prop.value {
                    Some(value) => collect_pattern_names_ordered(value, out),
                    None => push_name(out, &prop.key.name),
                }
            }
            if let Some(rest) = rest {
                collect_pattern_names_ordered(rest, out);
            }
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

pub(crate) fn lexical_declarations(stmts: &[Stmt]) -> Vec<String> {
    let mut out = HashSet::new();
    for stmt in stmts {
        match stmt {
            Stmt::VarDecl { pattern, .. } => collect_pattern_names(pattern, &mut out),
            Stmt::ClassDecl { name, .. } | Stmt::Using { name, .. } => {
                out.insert(name.name.clone());
            }
            Stmt::Export { kind: ExportKind::Declaration(decl), .. } => match decl.as_ref() {
                Stmt::VarDecl { pattern, .. } => collect_pattern_names(pattern, &mut out),
                Stmt::ClassDecl { name, .. } => {
                    out.insert(name.name.clone());
                }
                _ => {}
            },
            _ => {}
        }
    }
    out.into_iter().collect()
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
