use std::collections::BTreeSet;

use yps_lexer::SourceFile;
use yps_parser::ast::{
    Block, ClassMember, ExportKind, Expr, Literal, ObjectEntry, Param, Pattern, Program, PropKey, Stmt, TemplatePart,
};

/// Every line on which a statement starts. Only statements are stepping/breakpoint targets,
/// because the debug hook fires once per `Stmt`.
#[must_use]
pub fn statement_lines(program: &Program, source: &SourceFile) -> BTreeSet<usize> {
    let mut out = BTreeSet::new();
    let mut collector = Collector { source, out: &mut out };
    for stmt in &program.items {
        collector.stmt(stmt);
    }
    out
}

/// Resolution rule: a requested line binds to the first statement line at or after it.
/// `None` means the request is past the last statement and the breakpoint stays unverified.
#[must_use]
pub fn resolve_line(requested: usize, lines: &BTreeSet<usize>) -> Option<usize> {
    lines.range(requested..).next().copied()
}

struct Collector<'a> {
    source: &'a SourceFile,
    out: &'a mut BTreeSet<usize>,
}

impl Collector<'_> {
    fn mark(&mut self, stmt: &Stmt) {
        let (line, _) = self.source.position(stmt.span().start);
        self.out.insert(line);
    }

    fn block(&mut self, block: &Block) {
        for stmt in &block.stmts {
            self.stmt(stmt);
        }
    }

    fn params(&mut self, params: &[Param]) {
        for param in params {
            if let Some(default) = &param.default {
                self.expr(default);
            }
            if let Some(pattern) = &param.pattern {
                self.pattern(pattern);
            }
        }
    }

    fn pattern(&mut self, pattern: &Pattern) {
        match pattern {
            Pattern::Identifier(_) => {}
            Pattern::Array { elements, rest, .. } => {
                for element in elements.iter().flatten() {
                    self.pattern(element);
                }
                if let Some(rest) = rest {
                    self.pattern(rest);
                }
            }
            Pattern::Object { properties, rest, .. } => {
                for prop in properties {
                    if let Some(value) = &prop.value {
                        self.pattern(value);
                    }
                }
                if let Some(rest) = rest {
                    self.pattern(rest);
                }
            }
            Pattern::Default { pattern, default, .. } => {
                self.pattern(pattern);
                self.expr(default);
            }
        }
    }

    fn stmt(&mut self, stmt: &Stmt) {
        self.mark(stmt);
        match stmt {
            Stmt::VarDecl { pattern, init, .. } => {
                self.pattern(pattern);
                self.expr(init);
            }
            Stmt::Expr { expr, .. } | Stmt::Throw { value: expr, .. } | Stmt::Using { init: expr, .. } => {
                self.expr(expr);
            }
            Stmt::Block(block) => self.block(block),
            Stmt::Empty { .. }
            | Stmt::Break { .. }
            | Stmt::Continue { .. }
            | Stmt::Debugger { .. }
            | Stmt::Import { .. } => {}
            Stmt::If { condition, then_branch, else_branch, .. } => {
                self.expr(condition);
                self.stmt(then_branch);
                if let Some(else_branch) = else_branch {
                    self.stmt(else_branch);
                }
            }
            Stmt::While { condition, body, .. } | Stmt::DoWhile { condition, body, .. } => {
                self.expr(condition);
                self.stmt(body);
            }
            Stmt::For { init, condition, update, body, .. } => {
                if let Some(init) = init {
                    self.stmt(init);
                }
                if let Some(condition) = condition {
                    self.expr(condition);
                }
                if let Some(update) = update {
                    self.expr(update);
                }
                self.stmt(body);
            }
            Stmt::ForIn { variable, iterable, body, .. }
            | Stmt::ForOf { variable, iterable, body, .. }
            | Stmt::ForAwaitOf { variable, iterable, body, .. } => {
                self.pattern(variable);
                self.expr(iterable);
                self.stmt(body);
            }
            Stmt::Labeled { body, .. } => self.stmt(body),
            Stmt::FunctionDecl { params, body, .. } => {
                self.params(params);
                self.block(body);
            }
            Stmt::Return { value, .. } => {
                if let Some(value) = value {
                    self.expr(value);
                }
            }
            Stmt::TryCatch { try_block, catch_block, finally_block, .. } => {
                self.block(try_block);
                if let Some(catch_block) = catch_block {
                    self.block(catch_block);
                }
                if let Some(finally_block) = finally_block {
                    self.block(finally_block);
                }
            }
            Stmt::Switch { expr, cases, default, .. } => {
                self.expr(expr);
                for case in cases {
                    self.expr(&case.value);
                    self.block(&case.body);
                }
                if let Some(default) = default {
                    self.block(default);
                }
            }
            Stmt::ClassDecl { super_class, members, decorators, .. } => {
                if let Some(super_class) = super_class {
                    self.expr(super_class);
                }
                self.exprs(decorators);
                for member in members {
                    self.class_member(member);
                }
            }
            Stmt::Export { kind, .. } => match kind {
                ExportKind::Declaration(decl) => self.stmt(decl),
                ExportKind::Named(_) => {}
            },
        }
    }

    fn class_member(&mut self, member: &ClassMember) {
        match member {
            ClassMember::Constructor { params, body, .. } => {
                self.params(params);
                self.block(body);
            }
            ClassMember::Method { params, body, decorators, .. } => {
                self.exprs(decorators);
                self.params(params);
                self.block(body);
            }
            ClassMember::Field { init, decorators, .. } => {
                self.exprs(decorators);
                if let Some(init) = init {
                    self.expr(init);
                }
            }
            ClassMember::Getter { body, decorators, .. } => {
                self.exprs(decorators);
                self.block(body);
            }
            ClassMember::Setter { param, body, decorators, .. } => {
                self.exprs(decorators);
                self.params(std::slice::from_ref(param));
                self.block(body);
            }
            ClassMember::StaticBlock { body, .. } => self.block(body),
        }
    }

    fn exprs(&mut self, exprs: &[Expr]) {
        for expr in exprs {
            self.expr(expr);
        }
    }

    fn prop_key(&mut self, key: &PropKey) {
        if let PropKey::Computed(expr) = key {
            self.expr(expr);
        }
    }

    fn literal(&mut self, literal: &Literal) {
        match literal {
            Literal::Array { elements, .. } => self.exprs(elements),
            Literal::Object { entries, .. } => {
                for entry in entries {
                    match entry {
                        ObjectEntry::Property { key, value } => {
                            self.prop_key(key);
                            self.expr(value);
                        }
                        ObjectEntry::Spread(expr) => self.expr(expr),
                        ObjectEntry::Getter { key, body, .. } => {
                            self.prop_key(key);
                            self.block(body);
                        }
                        ObjectEntry::Setter { key, param, body, .. } => {
                            self.prop_key(key);
                            self.params(std::slice::from_ref(param));
                            self.block(body);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Identifier(_) | Expr::This { .. } | Expr::Super { .. } => {}
            Expr::Literal(literal) => self.literal(literal),
            Expr::Unary { expr, .. }
            | Expr::Postfix { expr, .. }
            | Expr::Grouping { expr, .. }
            | Expr::Spread { expr, .. } => self.expr(expr),
            Expr::Assignment { value, .. } => self.expr(value),
            Expr::Binary { lhs, rhs, .. } => {
                self.expr(lhs);
                self.expr(rhs);
            }
            Expr::Call { callee, args, .. }
            | Expr::OptionalCall { callee, args, .. }
            | Expr::New { callee, args, .. } => {
                self.expr(callee);
                self.exprs(args);
            }
            Expr::Index { object, index, .. } | Expr::OptionalIndex { object, index, .. } => {
                self.expr(object);
                self.expr(index);
            }
            Expr::Member { object, .. } | Expr::OptionalMember { object, .. } => self.expr(object),
            Expr::Conditional { condition, then_expr, else_expr, .. } => {
                self.expr(condition);
                self.expr(then_expr);
                self.expr(else_expr);
            }
            Expr::ArrowFunction { params, body, .. } | Expr::FunctionExpr { params, body, .. } => {
                self.params(params);
                self.block(body);
            }
            Expr::TemplateLiteral { parts, .. } => {
                for part in parts {
                    if let TemplatePart::Expr(expr) = part {
                        self.expr(expr);
                    }
                }
            }
            Expr::TaggedTemplate { tag, expressions, .. } => {
                self.expr(tag);
                self.exprs(expressions);
            }
            Expr::Yield { argument, .. } => {
                if let Some(argument) = argument {
                    self.expr(argument);
                }
            }
            Expr::Await { argument: inner, .. } | Expr::DynamicImport { source: inner, .. } => self.expr(inner),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yps_lexer::Lexer;
    use yps_parser::Parser;

    fn lines_of(src: &str) -> BTreeSet<usize> {
        let source = SourceFile::new("t.yopta".to_string(), src.to_string());
        let (tokens, _) = Lexer::new(&source).tokenize();
        let (program, diags) = Parser::new(&tokens, &source).parse_program();
        assert!(diags.is_empty(), "{diags:?}");
        statement_lines(&program, &source)
    }

    #[test]
    fn collects_top_level_and_nested_statement_lines() {
        let lines = lines_of("гыы a = 1;\nйопта f() {\n    отвечаю a;\n}\nf();\n");
        assert_eq!(lines, BTreeSet::from([1, 2, 3, 5]));
    }

    #[test]
    fn collects_arrow_function_bodies() {
        let lines = lines_of("гыы f = () => {\n    гыы b = 1;\n    отвечаю b;\n};\n");
        assert!(lines.contains(&2) && lines.contains(&3));
    }

    #[test]
    fn resolution_snaps_forward_to_the_next_statement() {
        let lines = BTreeSet::from([1, 3, 7]);
        assert_eq!(resolve_line(1, &lines), Some(1));
        assert_eq!(resolve_line(2, &lines), Some(3));
        assert_eq!(resolve_line(7, &lines), Some(7));
        assert_eq!(resolve_line(8, &lines), None);
    }
}
