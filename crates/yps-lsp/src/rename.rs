use std::collections::HashMap;

use yps_lexer::{Lexer, SourceFile, Span, TokenKind};
use yps_parser::ast::{
    Block, ClassMember, ExportKind, Expr, Identifier, ImportSpec, Literal, ObjectEntry, Param, Pattern, PropKey, Stmt,
    TemplatePart,
};
use yps_parser::{Parser, Program};

fn identifier_token_at(text: &str, byte_pos: usize) -> Option<Span> {
    let sf = SourceFile::new("inline".to_string(), text.to_string());
    let (tokens, _) = Lexer::new(&sf).tokenize();
    tokens
        .into_iter()
        .find(|t| t.kind == TokenKind::Identifier && t.span.start <= byte_pos && byte_pos <= t.span.end)
        .map(|t| t.span)
}

fn is_valid_new_name(new_name: &str) -> bool {
    let sf = SourceFile::new("inline".to_string(), new_name.to_string());
    let (tokens, diags) = Lexer::new(&sf).tokenize();
    let non_eof: Vec<_> = tokens.into_iter().filter(|t| t.kind != TokenKind::Eof).collect();
    if !diags.is_empty() || non_eof.len() != 1 {
        return false;
    }
    non_eof[0].kind == TokenKind::Identifier && non_eof[0].span.start == 0 && non_eof[0].span.end == new_name.len()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OccurrenceKind {
    Plain,
    ShorthandKey,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Occurrence {
    span: Span,
    kind: OccurrenceKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RenameRefusal {
    NoIdentifier,
    Unresolved,
    NotRenamable,
}

struct Binding {
    occurrences: Vec<Occurrence>,
    renamable: bool,
}

struct Scope {
    parent: Option<usize>,
    names: HashMap<String, usize>,
}

struct Resolver {
    scopes: Vec<Scope>,
    bindings: Vec<Binding>,
    exporting: bool,
}

impl Resolver {
    fn build(text: &str) -> Self {
        let sf = SourceFile::new("inline".to_string(), text.to_string());
        let (tokens, _) = Lexer::new(&sf).tokenize();
        let (program, _) = Parser::new(&tokens, &sf).parse_program();
        let mut resolver = Self { scopes: Vec::new(), bindings: Vec::new(), exporting: false };
        let root = resolver.new_scope(None);
        resolver.resolve_program(root, &program);
        resolver
    }

    fn new_scope(&mut self, parent: Option<usize>) -> usize {
        let id = self.scopes.len();
        self.scopes.push(Scope { parent, names: HashMap::new() });
        id
    }

    fn declare(&mut self, scope: usize, ident: &Identifier) {
        self.declare_as(scope, ident, OccurrenceKind::Plain);
    }

    fn declare_as(&mut self, scope: usize, ident: &Identifier, kind: OccurrenceKind) -> usize {
        let bid = if let Some(&existing) = self.scopes[scope].names.get(&ident.name) {
            existing
        } else {
            let bid = self.bindings.len();
            self.bindings.push(Binding { occurrences: Vec::new(), renamable: true });
            self.scopes[scope].names.insert(ident.name.clone(), bid);
            bid
        };
        self.bindings[bid].occurrences.push(Occurrence { span: ident.span, kind });
        if self.exporting {
            self.bindings[bid].renamable = false;
        }
        bid
    }

    fn use_ident(&mut self, scope: usize, name: &str, span: Span) {
        self.use_ident_as(scope, name, span, OccurrenceKind::Plain);
    }

    fn use_ident_as(&mut self, scope: usize, name: &str, span: Span, kind: OccurrenceKind) -> Option<usize> {
        let mut current = Some(scope);
        while let Some(s) = current {
            if let Some(&bid) = self.scopes[s].names.get(name) {
                self.bindings[bid].occurrences.push(Occurrence { span, kind });
                return Some(bid);
            }
            current = self.scopes[s].parent;
        }
        None
    }

    fn binding_at(&self, byte_pos: usize) -> Option<&Binding> {
        self.bindings.iter().find(|b| b.occurrences.iter().any(|o| o.span.start <= byte_pos && byte_pos <= o.span.end))
    }

    fn resolve_program(&mut self, scope: usize, program: &Program) {
        self.resolve_stmts(scope, &program.items);
    }

    fn resolve_stmts(&mut self, scope: usize, stmts: &[Stmt]) {
        for stmt in stmts {
            self.hoist_stmt(scope, stmt);
        }
        for stmt in stmts {
            self.resolve_stmt(scope, stmt);
        }
    }

    fn hoist_stmt(&mut self, scope: usize, stmt: &Stmt) {
        match stmt {
            Stmt::VarDecl { pattern, .. } => self.declare_pattern(scope, pattern),
            Stmt::FunctionDecl { name, .. } | Stmt::ClassDecl { name, .. } | Stmt::Using { name, .. } => {
                self.declare(scope, name);
            }
            Stmt::Import { specifiers, .. } => {
                for spec in specifiers {
                    match spec {
                        ImportSpec::Default { local } | ImportSpec::Namespace { local } => self.declare(scope, local),
                        ImportSpec::Named { local, .. } => {
                            let bid = self.declare_as(scope, local, OccurrenceKind::Plain);
                            self.bindings[bid].renamable = false;
                        }
                    }
                }
            }
            Stmt::Export { kind: ExportKind::Declaration(inner), .. } => {
                self.exporting = true;
                self.hoist_stmt(scope, inner);
                self.exporting = false;
            }
            Stmt::If { then_branch, else_branch, .. } => {
                self.hoist_nonblock(scope, then_branch);
                if let Some(else_branch) = else_branch {
                    self.hoist_nonblock(scope, else_branch);
                }
            }
            Stmt::While { body, .. } | Stmt::DoWhile { body, .. } | Stmt::Labeled { body, .. } => {
                self.hoist_nonblock(scope, body);
            }
            _ => {}
        }
    }

    fn hoist_nonblock(&mut self, scope: usize, stmt: &Stmt) {
        if !matches!(stmt, Stmt::Block(_)) {
            self.hoist_stmt(scope, stmt);
        }
    }

    fn resolve_stmt(&mut self, scope: usize, stmt: &Stmt) {
        match stmt {
            Stmt::VarDecl { pattern, init, .. } => {
                self.resolve_pattern_defaults(scope, pattern);
                self.resolve_expr(scope, init);
            }
            Stmt::Using { init, .. } => self.resolve_expr(scope, init),
            Stmt::Expr { expr, .. } | Stmt::Throw { value: expr, .. } => self.resolve_expr(scope, expr),
            Stmt::Block(block) => self.resolve_block(scope, block),
            Stmt::Empty { .. } | Stmt::Break { .. } | Stmt::Continue { .. } | Stmt::Debugger { .. } => {}
            Stmt::If { condition, then_branch, else_branch, .. } => {
                self.resolve_expr(scope, condition);
                self.resolve_stmt(scope, then_branch);
                if let Some(else_branch) = else_branch {
                    self.resolve_stmt(scope, else_branch);
                }
            }
            Stmt::While { condition, body, .. } | Stmt::DoWhile { condition, body, .. } => {
                self.resolve_expr(scope, condition);
                self.resolve_stmt(scope, body);
            }
            Stmt::For { init, condition, update, body, .. } => {
                let loop_scope = self.new_scope(Some(scope));
                if let Some(init) = init {
                    self.hoist_stmt(loop_scope, init);
                    self.resolve_stmt(loop_scope, init);
                }
                if let Some(condition) = condition {
                    self.resolve_expr(loop_scope, condition);
                }
                if let Some(update) = update {
                    self.resolve_expr(loop_scope, update);
                }
                self.resolve_stmt(loop_scope, body);
            }
            Stmt::ForIn { variable, iterable, body, .. }
            | Stmt::ForOf { variable, iterable, body, .. }
            | Stmt::ForAwaitOf { variable, iterable, body, .. } => {
                self.resolve_expr(scope, iterable);
                let loop_scope = self.new_scope(Some(scope));
                self.declare_pattern(loop_scope, variable);
                self.resolve_pattern_defaults(loop_scope, variable);
                self.resolve_stmt(loop_scope, body);
            }
            Stmt::Labeled { body, .. } => self.resolve_stmt(scope, body),
            Stmt::FunctionDecl { params, body, .. } => self.resolve_function(scope, None, params, body),
            Stmt::Return { value, .. } => {
                if let Some(value) = value {
                    self.resolve_expr(scope, value);
                }
            }
            Stmt::TryCatch { try_block, catch_param, catch_block, finally_block, .. } => {
                self.resolve_block(scope, try_block);
                if catch_param.is_some() || catch_block.is_some() {
                    let catch_scope = self.new_scope(Some(scope));
                    if let Some(catch_param) = catch_param {
                        self.declare(catch_scope, catch_param);
                    }
                    if let Some(catch_block) = catch_block {
                        self.resolve_stmts(catch_scope, &catch_block.stmts);
                    }
                }
                if let Some(finally_block) = finally_block {
                    self.resolve_block(scope, finally_block);
                }
            }
            Stmt::Switch { expr, cases, default, .. } => {
                self.resolve_expr(scope, expr);
                for case in cases {
                    self.resolve_expr(scope, &case.value);
                    self.resolve_block(scope, &case.body);
                }
                if let Some(default) = default {
                    self.resolve_block(scope, default);
                }
            }
            Stmt::ClassDecl { super_class, members, decorators, .. } => {
                if let Some(super_class) = super_class {
                    self.resolve_expr(scope, super_class);
                }
                for decorator in decorators {
                    self.resolve_expr(scope, decorator);
                }
                self.resolve_members(scope, members);
            }
            Stmt::Import { .. } => {}
            Stmt::Export { kind, .. } => match kind {
                ExportKind::Declaration(inner) => self.resolve_stmt(scope, inner),
                ExportKind::Named(idents) => {
                    for ident in idents {
                        if let Some(bid) = self.use_ident_as(scope, &ident.name, ident.span, OccurrenceKind::Plain) {
                            self.bindings[bid].renamable = false;
                        }
                    }
                }
            },
        }
    }

    fn resolve_block(&mut self, parent: usize, block: &Block) {
        let scope = self.new_scope(Some(parent));
        self.resolve_stmts(scope, &block.stmts);
    }

    fn resolve_function(&mut self, parent: usize, name: Option<&Identifier>, params: &[Param], body: &Block) {
        let scope = self.new_scope(Some(parent));
        if let Some(name) = name {
            self.declare(scope, name);
        }
        self.declare_params(scope, params);
        self.resolve_param_defaults(scope, params);
        self.resolve_stmts(scope, &body.stmts);
    }

    fn resolve_members(&mut self, scope: usize, members: &[ClassMember]) {
        for member in members {
            match member {
                ClassMember::Constructor { params, body, .. } => self.resolve_function(scope, None, params, body),
                ClassMember::Method { params, body, decorators, .. } => {
                    for decorator in decorators {
                        self.resolve_expr(scope, decorator);
                    }
                    self.resolve_function(scope, None, params, body);
                }
                ClassMember::Field { init, decorators, .. } => {
                    for decorator in decorators {
                        self.resolve_expr(scope, decorator);
                    }
                    if let Some(init) = init {
                        self.resolve_expr(scope, init);
                    }
                }
                ClassMember::Getter { body, decorators, .. } => {
                    for decorator in decorators {
                        self.resolve_expr(scope, decorator);
                    }
                    self.resolve_function(scope, None, &[], body);
                }
                ClassMember::Setter { param, body, decorators, .. } => {
                    for decorator in decorators {
                        self.resolve_expr(scope, decorator);
                    }
                    self.resolve_function(scope, None, std::slice::from_ref(param), body);
                }
                ClassMember::StaticBlock { body, .. } => {
                    self.resolve_function(scope, None, &[], body);
                }
            }
        }
    }

    fn declare_params(&mut self, scope: usize, params: &[Param]) {
        for param in params {
            match &param.pattern {
                Some(pattern) => self.declare_pattern(scope, pattern),
                None => self.declare(scope, &param.name),
            }
        }
    }

    fn resolve_param_defaults(&mut self, scope: usize, params: &[Param]) {
        for param in params {
            if let Some(default) = &param.default {
                self.resolve_expr(scope, default);
            }
            if let Some(pattern) = &param.pattern {
                self.resolve_pattern_defaults(scope, pattern);
            }
        }
    }

    fn declare_pattern(&mut self, scope: usize, pattern: &Pattern) {
        match pattern {
            Pattern::Identifier(ident) => self.declare(scope, ident),
            Pattern::Array { elements, rest, .. } => {
                for element in elements.iter().flatten() {
                    self.declare_pattern(scope, element);
                }
                if let Some(rest) = rest {
                    self.declare_pattern(scope, rest);
                }
            }
            Pattern::Object { properties, rest, .. } => {
                for prop in properties {
                    match &prop.value {
                        None => {
                            self.declare_as(scope, &prop.key, OccurrenceKind::ShorthandKey);
                        }
                        Some(Pattern::Default { pattern, .. }) if matches!(pattern.as_ref(), Pattern::Identifier(id) if id.span == prop.key.span) =>
                        {
                            self.declare_as(scope, &prop.key, OccurrenceKind::ShorthandKey);
                        }
                        Some(value) => self.declare_pattern(scope, value),
                    }
                }
                if let Some(rest) = rest {
                    self.declare_pattern(scope, rest);
                }
            }
            Pattern::Default { pattern, .. } => self.declare_pattern(scope, pattern),
        }
    }

    fn resolve_pattern_defaults(&mut self, scope: usize, pattern: &Pattern) {
        match pattern {
            Pattern::Identifier(_) => {}
            Pattern::Array { elements, rest, .. } => {
                for element in elements.iter().flatten() {
                    self.resolve_pattern_defaults(scope, element);
                }
                if let Some(rest) = rest {
                    self.resolve_pattern_defaults(scope, rest);
                }
            }
            Pattern::Object { properties, rest, .. } => {
                for prop in properties {
                    if let Some(value) = &prop.value {
                        self.resolve_pattern_defaults(scope, value);
                    }
                }
                if let Some(rest) = rest {
                    self.resolve_pattern_defaults(scope, rest);
                }
            }
            Pattern::Default { pattern, default, .. } => {
                self.resolve_pattern_defaults(scope, pattern);
                self.resolve_expr(scope, default);
            }
        }
    }

    fn resolve_expr(&mut self, scope: usize, expr: &Expr) {
        match expr {
            Expr::Identifier(ident) => self.use_ident(scope, &ident.name, ident.span),
            Expr::This { .. } | Expr::Super { .. } => {}
            Expr::Literal(lit) => self.resolve_literal(scope, lit),
            Expr::Unary { expr, .. }
            | Expr::Postfix { expr, .. }
            | Expr::Grouping { expr, .. }
            | Expr::Spread { expr, .. }
            | Expr::Await { argument: expr, .. } => self.resolve_expr(scope, expr),
            Expr::Binary { lhs, rhs, .. } => {
                self.resolve_expr(scope, lhs);
                self.resolve_expr(scope, rhs);
            }
            Expr::Assignment { target, value, .. } => {
                self.use_ident(scope, &target.name, target.span);
                self.resolve_expr(scope, value);
            }
            Expr::Call { callee, args, .. }
            | Expr::OptionalCall { callee, args, .. }
            | Expr::New { callee, args, .. } => {
                self.resolve_expr(scope, callee);
                for arg in args {
                    self.resolve_expr(scope, arg);
                }
            }
            Expr::Index { object, index, .. } | Expr::OptionalIndex { object, index, .. } => {
                self.resolve_expr(scope, object);
                self.resolve_expr(scope, index);
            }
            Expr::Member { object, .. } | Expr::OptionalMember { object, .. } => self.resolve_expr(scope, object),
            Expr::Conditional { condition, then_expr, else_expr, .. } => {
                self.resolve_expr(scope, condition);
                self.resolve_expr(scope, then_expr);
                self.resolve_expr(scope, else_expr);
            }
            Expr::ArrowFunction { params, body, .. } => self.resolve_function(scope, None, params, body),
            Expr::FunctionExpr { name, params, body, .. } => self.resolve_function(scope, name.as_ref(), params, body),
            Expr::TemplateLiteral { parts, .. } => {
                for part in parts {
                    if let TemplatePart::Expr(e) = part {
                        self.resolve_expr(scope, e);
                    }
                }
            }
            Expr::TaggedTemplate { tag, expressions, .. } => {
                self.resolve_expr(scope, tag);
                for e in expressions {
                    self.resolve_expr(scope, e);
                }
            }
            Expr::Yield { argument, .. } => {
                if let Some(argument) = argument {
                    self.resolve_expr(scope, argument);
                }
            }
            Expr::DynamicImport { source, .. } => self.resolve_expr(scope, source),
        }
    }

    fn resolve_literal(&mut self, scope: usize, lit: &Literal) {
        match lit {
            Literal::Array { elements, .. } => {
                for element in elements {
                    self.resolve_expr(scope, element);
                }
            }
            Literal::Object { entries, .. } => {
                for entry in entries {
                    match entry {
                        ObjectEntry::Property { key: PropKey::Identifier(key), value: Expr::Identifier(value) }
                            if key.span == value.span =>
                        {
                            self.use_ident_as(scope, &value.name, value.span, OccurrenceKind::ShorthandKey);
                        }
                        ObjectEntry::Property { key, value } => {
                            self.resolve_prop_key(scope, key);
                            self.resolve_expr(scope, value);
                        }
                        ObjectEntry::Spread(expr) => self.resolve_expr(scope, expr),
                        ObjectEntry::Getter { key, body, .. } => {
                            self.resolve_prop_key(scope, key);
                            self.resolve_function(scope, None, &[], body);
                        }
                        ObjectEntry::Setter { key, param, body, .. } => {
                            self.resolve_prop_key(scope, key);
                            self.resolve_function(scope, None, std::slice::from_ref(param), body);
                        }
                    }
                }
            }
            Literal::Number { .. }
            | Literal::BigInt { .. }
            | Literal::String { .. }
            | Literal::Boolean { .. }
            | Literal::Null { .. }
            | Literal::Undefined { .. }
            | Literal::RegExp { .. } => {}
        }
    }

    fn resolve_prop_key(&mut self, scope: usize, key: &PropKey) {
        if let PropKey::Computed(expr) = key {
            self.resolve_expr(scope, expr);
        }
    }
}

#[must_use]
pub fn prepare(text: &str, byte_pos: usize) -> Option<Span> {
    rename_occurrences_at(text, byte_pos).ok()?;
    identifier_token_at(text, byte_pos)
}

#[must_use]
pub fn occurrences_at(text: &str, byte_pos: usize) -> Option<Vec<Span>> {
    identifier_token_at(text, byte_pos)?;
    let resolver = Resolver::build(text);
    resolver.binding_at(byte_pos).map(|b| b.occurrences.iter().map(|o| o.span).collect())
}

fn rename_occurrences_at(text: &str, byte_pos: usize) -> Result<Vec<Occurrence>, RenameRefusal> {
    identifier_token_at(text, byte_pos).ok_or(RenameRefusal::NoIdentifier)?;
    let resolver = Resolver::build(text);
    let binding = resolver.binding_at(byte_pos).ok_or(RenameRefusal::Unresolved)?;
    if !binding.renamable {
        return Err(RenameRefusal::NotRenamable);
    }
    Ok(binding.occurrences.clone())
}

#[must_use]
pub fn rename_edits(text: &str, byte_pos: usize, new_name: &str) -> Option<Vec<(Span, String)>> {
    if !is_valid_new_name(new_name) {
        return None;
    }
    let occurrences = rename_occurrences_at(text, byte_pos).ok()?;
    let edits = occurrences
        .into_iter()
        .map(|o| {
            let new_text = match o.kind {
                OccurrenceKind::Plain => new_name.to_string(),
                OccurrenceKind::ShorthandKey => format!("{}: {new_name}", &text[o.span.start..o.span.end]),
            };
            (o.span, new_text)
        })
        .collect();
    Some(edits)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::apply_edits;

    fn spans_texts<'a>(src: &'a str, spans: &[Span]) -> Vec<&'a str> {
        spans.iter().map(|s| &src[s.start..s.end]).collect()
    }

    fn rename_spans(src: &str, byte_pos: usize, new_name: &str) -> Option<Vec<Span>> {
        rename_edits(src, byte_pos, new_name).map(|edits| edits.into_iter().map(|(span, _)| span).collect())
    }

    fn renamed(src: &str, needle: &str, new_name: &str) -> Option<String> {
        let edits = rename_edits(src, src.find(needle)?, new_name)?;
        Some(apply_edits(src, edits))
    }

    #[test]
    fn renames_declaration_and_all_usages() {
        let src = "ясенХуй x = 1;\nсказать(x);\nсказать(x + 1);";
        let usage = src.find('x').unwrap();
        let spans = rename_spans(src, usage, "y").expect("should collect edits");
        assert_eq!(spans.len(), 3);
        for text in spans_texts(src, &spans) {
            assert_eq!(text, "x");
        }
    }

    #[test]
    fn rename_on_keyword_returns_none() {
        let src = "гыы x = 1;";
        let byte = src.find("гыы").unwrap();
        assert!(prepare(src, byte).is_none());
        assert!(rename_spans(src, byte, "y").is_none());
    }

    #[test]
    fn rename_on_string_literal_returns_none() {
        let src = "гыы x = \"привет\";";
        let byte = src.find("привет").unwrap();
        assert!(prepare(src, byte).is_none());
        assert!(rename_spans(src, byte, "y").is_none());
    }

    #[test]
    fn rename_on_number_returns_none() {
        let src = "гыы x = 42;";
        let byte = src.find("42").unwrap();
        assert!(prepare(src, byte).is_none());
        assert!(rename_spans(src, byte, "y").is_none());
    }

    #[test]
    fn new_name_that_is_keyword_is_rejected() {
        let src = "гыы x = 1;";
        let byte = src.find('x').unwrap();
        assert!(rename_spans(src, byte, "потрещим").is_none());
    }

    #[test]
    fn new_name_with_spaces_is_rejected() {
        let src = "гыы x = 1;";
        let byte = src.find('x').unwrap();
        assert!(rename_spans(src, byte, "два слова").is_none());
    }

    #[test]
    fn new_name_with_leading_whitespace_is_rejected() {
        let src = "гыы x = 1;";
        let byte = src.find('x').unwrap();
        assert!(rename_spans(src, byte, " y").is_none());
    }

    #[test]
    fn new_name_empty_is_rejected() {
        let src = "гыы x = 1;";
        let byte = src.find('x').unwrap();
        assert!(rename_spans(src, byte, "").is_none());
    }

    #[test]
    fn new_name_cyrillic_identifier_is_accepted() {
        let src = "гыы x = 1;\nсказать(x);";
        let byte = src.find('x').unwrap();
        let spans = rename_spans(src, byte, "переменная").expect("should collect edits");
        assert_eq!(spans.len(), 2);
    }

    #[test]
    fn prepare_on_identifier_returns_its_span() {
        let src = "гыы перемен = 1;";
        let byte = src.find("перемен").unwrap();
        let span = prepare(src, byte).expect("should resolve");
        assert_eq!(&src[span.start..span.end], "перемен");
    }

    #[test]
    fn prepare_in_whitespace_returns_none() {
        let src = "гыы x = 1;   ";
        assert!(prepare(src, src.len() - 1).is_none());
    }

    #[test]
    fn sibling_functions_do_not_share_scope() {
        let src = "йопта фу() { гыы x = 1; отвечаю x; }\nйопта бар() { гыы x = 2; отвечаю x; }";
        let byte = src.find('x').unwrap();
        let spans = rename_spans(src, byte, "y").expect("should collect edits");
        assert_eq!(spans.len(), 2);
        let last_fu = src[..src.find("бар").unwrap()].rfind('x').unwrap();
        assert!(spans.iter().all(|s| s.start <= last_fu));
    }

    #[test]
    fn shadowed_variable_is_isolated_per_scope() {
        let src = "гыы x = 1;\nйопта фу() { гыы x = 2; отвечаю x; }\nсказать(x);";
        let outer = src.find('x').unwrap();
        let outer_spans = rename_spans(src, outer, "y").expect("outer edits");
        assert_eq!(outer_spans.len(), 2);
        let inner = src.find("гыы x = 2").unwrap() + "гыы ".len();
        let inner_spans = rename_spans(src, inner, "z").expect("inner edits");
        assert_eq!(inner_spans.len(), 2);
        assert!(outer_spans.iter().all(|o| inner_spans.iter().all(|i| i.start != o.start)));
    }

    #[test]
    fn closure_capture_renames_together() {
        let src = "гыы счёт = 0;\nйопта увеличить() { отвечаю () => счёт + 1; }";
        let decl = src.find("счёт").unwrap();
        let spans = rename_spans(src, decl, "итог").expect("should collect edits");
        assert_eq!(spans.len(), 2);
        for text in spans_texts(src, &spans) {
            assert_eq!(text, "счёт");
        }
    }

    #[test]
    fn function_name_renames_declaration_and_calls() {
        let src = "йопта посчитать() { отвечаю 1; }\nпосчитать();\nсказать(посчитать());";
        let byte = src.find("посчитать").unwrap();
        let spans = rename_spans(src, byte, "вычислить").expect("should collect edits");
        assert_eq!(spans.len(), 3);
    }

    #[test]
    fn parameter_renames_only_inside_body() {
        let src = "гыы арг = 99;\nйопта фу(арг) { отвечаю арг + 1; }\nсказать(арг);";
        let param = src.find("фу(арг)").unwrap() + "фу(".len();
        let spans = rename_spans(src, param, "п").expect("should collect edits");
        assert_eq!(spans.len(), 2);
        let outer_first = src.find("арг").unwrap();
        assert!(spans.iter().all(|s| s.start > outer_first));
    }

    #[test]
    fn member_property_with_same_name_not_renamed() {
        let src = "гыы длина = 1;\nсказать(массив.длина);\nсказать(длина);";
        let byte = src.find("длина").unwrap();
        let spans = rename_spans(src, byte, "размер").expect("should collect edits");
        assert_eq!(spans.len(), 2);
        let member = src.find("массив.длина").unwrap() + "массив.".len();
        assert!(spans.iter().all(|s| !(s.start <= member && member <= s.end)));
    }

    #[test]
    fn rename_on_member_property_returns_none() {
        let src = "гыы длина = 1;\nсказать(массив.длина);";
        let member = src.find("массив.длина").unwrap() + "массив.".len();
        assert!(prepare(src, member).is_none());
        assert!(rename_spans(src, member, "размер").is_none());
    }

    #[test]
    fn object_literal_key_with_same_name_not_renamed() {
        let src = "гыы ключ = 1;\nгыы объект = { ключ: 2 };\nсказать(ключ);";
        let byte = src.find("ключ").unwrap();
        let spans = rename_spans(src, byte, "поле").expect("should collect edits");
        assert_eq!(spans.len(), 2);
        let key = src.find("{ ключ").unwrap() + "{ ".len();
        assert!(spans.iter().all(|s| !(s.start <= key && key <= s.end)));
    }

    #[test]
    fn object_pattern_destructuring_binding_renames() {
        let src = "гыы { х } = объект;\nсказать(х);";
        let byte = src.rfind('х').unwrap();
        let spans = rename_spans(src, byte, "значение").expect("should collect edits");
        assert_eq!(spans.len(), 2);
    }

    #[test]
    fn array_pattern_rest_binding_renames() {
        let src = "гыы [ а, ...хвост ] = список;\nсказать(хвост);";
        let byte = src.find("хвост").unwrap();
        let spans = rename_spans(src, byte, "остаток").expect("should collect edits");
        assert_eq!(spans.len(), 2);
        for text in spans_texts(src, &spans) {
            assert_eq!(text, "хвост");
        }
    }

    #[test]
    fn import_binding_renames_declaration_and_uses() {
        let src = "спиздить кент из \"./модуль\";\nсказать(кент);";
        let byte = src.find("кент").unwrap();
        let spans = rename_spans(src, byte, "друг").expect("should collect edits");
        assert_eq!(spans.len(), 2);
        for text in spans_texts(src, &spans) {
            assert_eq!(text, "кент");
        }
    }

    #[test]
    fn builtin_reference_returns_none() {
        let src = "сказать(1);";
        let byte = src.find("сказать").unwrap();
        assert!(prepare(src, byte).is_none());
        assert!(rename_spans(src, byte, "печать").is_none());
    }

    #[test]
    fn catch_param_renames_within_catch() {
        let src = "хапнуть { кидай 1; } гоп (ошибка) { сказать(ошибка); }";
        let byte = src.find("ошибка").unwrap();
        let spans = rename_spans(src, byte, "исключение").expect("should collect edits");
        assert_eq!(spans.len(), 2);
    }

    #[test]
    fn shorthand_pattern_in_var_decl_expands_to_key_value() {
        let src = "гыы {а} = о;\nсказать(а);";
        assert_eq!(renamed(src, "а", "_а").unwrap(), "гыы {а: _а} = о;\nсказать(_а);");
    }

    #[test]
    fn shorthand_pattern_in_param_expands_to_key_value() {
        let src = "йопта ф({а}) { отвечаю а; }";
        assert_eq!(renamed(src, "а})", "б").unwrap(), "йопта ф({а: б}) { отвечаю б; }");
    }

    #[test]
    fn shorthand_pattern_with_default_keeps_default() {
        let src = "гыы {а = 1} = о;\nсказать(а);";
        assert_eq!(renamed(src, "а", "_а").unwrap(), "гыы {а: _а = 1} = о;\nсказать(_а);");
    }

    #[test]
    fn nested_shorthand_pattern_inside_array_expands() {
        let src = "гыы [{а}] = о;\nсказать(а);";
        assert_eq!(renamed(src, "а", "_а").unwrap(), "гыы [{а: _а}] = о;\nсказать(_а);");
    }

    #[test]
    fn explicit_key_value_pattern_renames_only_value() {
        let src = "гыы {а: б} = о;\nсказать(б);";
        assert_eq!(renamed(src, "б", "в").unwrap(), "гыы {а: в} = о;\nсказать(в);");
    }

    #[test]
    fn array_pattern_renames_plainly() {
        let src = "гыы [а] = о;\nсказать(а);";
        assert_eq!(renamed(src, "а", "б").unwrap(), "гыы [б] = о;\nсказать(б);");
    }

    #[test]
    fn object_literal_shorthand_usage_expands_to_key_value() {
        let src = "гыы а = 1;\nгыы о = {а};";
        assert_eq!(renamed(src, "а", "б").unwrap(), "гыы б = 1;\nгыы о = {а: б};");
    }

    #[test]
    fn named_import_rename_is_refused() {
        let src = "спиздить { фу } из \"./м\";\nсказать(фу);";
        let byte = src.find("фу").unwrap();
        assert!(prepare(src, byte).is_none());
        assert!(rename_edits(src, byte, "бар").is_none());
        assert_eq!(rename_occurrences_at(src, src.rfind("фу").unwrap()), Err(RenameRefusal::NotRenamable));
        assert_eq!(occurrences_at(src, byte).map(|o| o.len()), Some(2));
    }

    #[test]
    fn default_and_namespace_imports_rename() {
        let src = "спиздить кент из \"./м\";\nсказать(кент);";
        assert_eq!(renamed(src, "кент", "друг").unwrap(), "спиздить друг из \"./м\";\nсказать(друг);");
        let src = "спиздить * как м из \"./м\";\nсказать(м.фу);";
        assert_eq!(renamed(src, "м из", "мод").unwrap(), "спиздить * как мод из \"./м\";\nсказать(мод.фу);");
    }

    #[test]
    fn refusal_reasons_are_distinguished() {
        let src = "гыы х = 1;\nсказать(х);";
        assert_eq!(rename_occurrences_at(src, 0), Err(RenameRefusal::NoIdentifier));
        assert_eq!(rename_occurrences_at(src, src.find("сказать").unwrap()), Err(RenameRefusal::Unresolved));
        assert_eq!(rename_occurrences_at(src, src.find('х').unwrap()).map(|o| o.len()), Ok(2));
    }

    #[test]
    fn named_export_rename_is_refused() {
        let src = "гыы а = 1;\nгыы б = 2;\nсказать(б);\nпредъява { а };";
        let byte = src.find('а').unwrap();
        assert!(prepare(src, byte).is_none());
        assert!(rename_edits(src, byte, "в").is_none());
        assert_eq!(occurrences_at(src, byte).map(|o| o.len()), Some(2));
        assert_eq!(renamed(src, "б", "в").unwrap(), "гыы а = 1;\nгыы в = 2;\nсказать(в);\nпредъява { а };");
    }

    #[test]
    fn exported_declarations_rename_is_refused() {
        for src in
            ["предъява гыы а = 1;\nсказать(а);", "предъява йопта а() {}\nа();", "предъява клёво а {}\nгыйбать а();"]
        {
            let byte = src.rfind('а').unwrap();
            assert!(prepare(src, byte).is_none(), "{src}");
            assert!(rename_edits(src, byte, "в").is_none(), "{src}");
            assert_eq!(occurrences_at(src, byte).map(|o| o.len()), Some(2), "{src}");
        }
    }

    #[test]
    fn non_exported_sibling_of_exported_declaration_renames() {
        let src = "предъява гыы а = б;\nгыы б = 1;\nсказать(б);";
        assert_eq!(renamed(src, "б;", "в").unwrap(), "предъява гыы а = в;\nгыы в = 1;\nсказать(в);");
    }
}
