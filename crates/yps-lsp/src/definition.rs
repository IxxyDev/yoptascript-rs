use tower_lsp::lsp_types::Position;
use yps_lexer::Span;
use yps_parser::Program;
use yps_parser::ast::{
    Block, ClassMember, ExportKind, Expr, Identifier, ImportSpec, Literal, ObjectEntry, Param, Pattern, PropKey, Stmt,
};

use crate::position::{pos_to_byte, word_at};

pub struct Declaration {
    pub name: String,
    pub span: Span,
}

#[must_use]
pub fn declarations(program: &Program) -> Vec<Declaration> {
    let mut decls = Vec::new();
    for stmt in &program.items {
        collect_stmt(stmt, &mut decls);
    }
    decls
}

#[must_use]
pub fn goto_definition(declarations: &[Declaration], text: &str, pos: Position) -> Option<Span> {
    let byte = pos_to_byte(text, pos);
    let word = word_at(text, byte);
    if word.is_empty() {
        return None;
    }

    declarations.iter().find(|d| d.name == word).map(|d| d.span)
}

fn push_ident(ident: &Identifier, out: &mut Vec<Declaration>) {
    out.push(Declaration { name: ident.name.clone(), span: ident.span });
}

fn collect_pattern(pattern: &Pattern, out: &mut Vec<Declaration>) {
    match pattern {
        Pattern::Identifier(ident) => push_ident(ident, out),
        Pattern::Array { elements, rest, .. } => {
            for el in elements.iter().flatten() {
                collect_pattern(el, out);
            }
            if let Some(rest) = rest {
                collect_pattern(rest, out);
            }
        }
        Pattern::Object { properties, rest, .. } => {
            for prop in properties {
                match &prop.value {
                    Some(value) => collect_pattern(value, out),
                    None => push_ident(&prop.key, out),
                }
            }
            if let Some(rest) = rest {
                collect_pattern(rest, out);
            }
        }
        Pattern::Default { pattern, default, .. } => {
            collect_pattern(pattern, out);
            collect_expr(default, out);
        }
    }
}

fn collect_params(params: &[Param], out: &mut Vec<Declaration>) {
    for param in params {
        match &param.pattern {
            Some(pattern) => collect_pattern(pattern, out),
            None => push_ident(&param.name, out),
        }
        if let Some(default) = &param.default {
            collect_expr(default, out);
        }
    }
}

fn collect_block(block: &Block, out: &mut Vec<Declaration>) {
    for stmt in &block.stmts {
        collect_stmt(stmt, out);
    }
}

fn collect_members(members: &[ClassMember], out: &mut Vec<Declaration>) {
    for member in members {
        match member {
            ClassMember::Constructor { params, body, .. } => {
                collect_params(params, out);
                collect_block(body, out);
            }
            ClassMember::Method { params, body, decorators, .. } => {
                collect_params(params, out);
                collect_block(body, out);
                for d in decorators {
                    collect_expr(d, out);
                }
            }
            ClassMember::Field { init, decorators, .. } => {
                if let Some(init) = init {
                    collect_expr(init, out);
                }
                for d in decorators {
                    collect_expr(d, out);
                }
            }
            ClassMember::Getter { body, decorators, .. } => {
                collect_block(body, out);
                for d in decorators {
                    collect_expr(d, out);
                }
            }
            ClassMember::Setter { param, body, decorators, .. } => {
                collect_params(std::slice::from_ref(param), out);
                collect_block(body, out);
                for d in decorators {
                    collect_expr(d, out);
                }
            }
        }
    }
}

fn collect_stmt(stmt: &Stmt, out: &mut Vec<Declaration>) {
    match stmt {
        Stmt::VarDecl { pattern, init, .. } => {
            collect_pattern(pattern, out);
            collect_expr(init, out);
        }
        Stmt::Expr { expr, .. } | Stmt::Throw { value: expr, .. } => collect_expr(expr, out),
        Stmt::Block(block) => collect_block(block, out),
        Stmt::Empty { .. } | Stmt::Break { .. } | Stmt::Continue { .. } | Stmt::Debugger { .. } => {}
        Stmt::If { condition, then_branch, else_branch, .. } => {
            collect_expr(condition, out);
            collect_stmt(then_branch, out);
            if let Some(else_branch) = else_branch {
                collect_stmt(else_branch, out);
            }
        }
        Stmt::While { condition, body, .. } | Stmt::DoWhile { condition, body, .. } => {
            collect_expr(condition, out);
            collect_stmt(body, out);
        }
        Stmt::For { init, condition, update, body, .. } => {
            if let Some(init) = init {
                collect_stmt(init, out);
            }
            if let Some(condition) = condition {
                collect_expr(condition, out);
            }
            if let Some(update) = update {
                collect_expr(update, out);
            }
            collect_stmt(body, out);
        }
        Stmt::Labeled { body, .. } => collect_stmt(body, out),
        Stmt::FunctionDecl { name, params, body, .. } => {
            push_ident(name, out);
            collect_params(params, out);
            collect_block(body, out);
        }
        Stmt::Return { value, .. } => {
            if let Some(value) = value {
                collect_expr(value, out);
            }
        }
        Stmt::TryCatch { try_block, catch_param, catch_block, finally_block, .. } => {
            collect_block(try_block, out);
            if let Some(catch_param) = catch_param {
                push_ident(catch_param, out);
            }
            if let Some(catch_block) = catch_block {
                collect_block(catch_block, out);
            }
            if let Some(finally_block) = finally_block {
                collect_block(finally_block, out);
            }
        }
        Stmt::Switch { expr, cases, default, .. } => {
            collect_expr(expr, out);
            for case in cases {
                collect_expr(&case.value, out);
                collect_block(&case.body, out);
            }
            if let Some(default) = default {
                collect_block(default, out);
            }
        }
        Stmt::ForIn { variable, iterable, body, .. }
        | Stmt::ForOf { variable, iterable, body, .. }
        | Stmt::ForAwaitOf { variable, iterable, body, .. } => {
            collect_pattern(variable, out);
            collect_expr(iterable, out);
            collect_stmt(body, out);
        }
        Stmt::ClassDecl { name, super_class, members, decorators, .. } => {
            push_ident(name, out);
            if let Some(super_class) = super_class {
                collect_expr(super_class, out);
            }
            collect_members(members, out);
            for d in decorators {
                collect_expr(d, out);
            }
        }
        Stmt::Using { name, init, .. } => {
            push_ident(name, out);
            collect_expr(init, out);
        }
        Stmt::Import { specifiers, .. } => {
            for spec in specifiers {
                match spec {
                    ImportSpec::Default { local }
                    | ImportSpec::Named { local, .. }
                    | ImportSpec::Namespace { local } => push_ident(local, out),
                }
            }
        }
        Stmt::Export { kind, .. } => match kind {
            ExportKind::Declaration(stmt) => collect_stmt(stmt, out),
            ExportKind::Named(_) => {}
        },
    }
}

fn collect_literal(lit: &Literal, out: &mut Vec<Declaration>) {
    match lit {
        Literal::Array { elements, .. } => {
            for el in elements {
                collect_expr(el, out);
            }
        }
        Literal::Object { entries, .. } => {
            for entry in entries {
                match entry {
                    ObjectEntry::Property { key, value } => {
                        collect_prop_key(key, out);
                        collect_expr(value, out);
                    }
                    ObjectEntry::Spread(expr) => collect_expr(expr, out),
                    ObjectEntry::Getter { key, body, .. } => {
                        collect_prop_key(key, out);
                        collect_block(body, out);
                    }
                    ObjectEntry::Setter { key, param, body, .. } => {
                        collect_prop_key(key, out);
                        collect_params(std::slice::from_ref(param), out);
                        collect_block(body, out);
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

fn collect_prop_key(key: &PropKey, out: &mut Vec<Declaration>) {
    if let PropKey::Computed(expr) = key {
        collect_expr(expr, out);
    }
}

fn collect_expr(expr: &Expr, out: &mut Vec<Declaration>) {
    match expr {
        Expr::Identifier(_) | Expr::This { .. } | Expr::Super { .. } => {}
        Expr::Literal(lit) => collect_literal(lit, out),
        Expr::Unary { expr, .. }
        | Expr::Postfix { expr, .. }
        | Expr::Grouping { expr, .. }
        | Expr::Spread { expr, .. }
        | Expr::Await { argument: expr, .. } => collect_expr(expr, out),
        Expr::Binary { lhs, rhs, .. } => {
            collect_expr(lhs, out);
            collect_expr(rhs, out);
        }
        Expr::Assignment { value, .. } => collect_expr(value, out),
        Expr::Call { callee, args, .. } | Expr::OptionalCall { callee, args, .. } | Expr::New { callee, args, .. } => {
            collect_expr(callee, out);
            for arg in args {
                collect_expr(arg, out);
            }
        }
        Expr::Index { object, index, .. } | Expr::OptionalIndex { object, index, .. } => {
            collect_expr(object, out);
            collect_expr(index, out);
        }
        Expr::Member { object, .. } | Expr::OptionalMember { object, .. } => collect_expr(object, out),
        Expr::Conditional { condition, then_expr, else_expr, .. } => {
            collect_expr(condition, out);
            collect_expr(then_expr, out);
            collect_expr(else_expr, out);
        }
        Expr::ArrowFunction { params, body, .. } => {
            collect_params(params, out);
            collect_block(body, out);
        }
        Expr::FunctionExpr { name, params, body, .. } => {
            if let Some(name) = name {
                push_ident(name, out);
            }
            collect_params(params, out);
            collect_block(body, out);
        }
        Expr::TemplateLiteral { parts, .. } => {
            for part in parts {
                if let yps_parser::ast::TemplatePart::Expr(e) = part {
                    collect_expr(e, out);
                }
            }
        }
        Expr::TaggedTemplate { tag, expressions, .. } => {
            collect_expr(tag, out);
            for e in expressions {
                collect_expr(e, out);
            }
        }
        Expr::Yield { argument, .. } => {
            if let Some(argument) = argument {
                collect_expr(argument, out);
            }
        }
        Expr::DynamicImport { source, .. } => collect_expr(source, out),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::position::byte_to_pos;

    fn resolve(src: &str, pos: Position) -> Option<Span> {
        goto_definition(&crate::analyze(src).declarations, src, pos)
    }

    fn def_at(src: &str, needle: &str) -> Option<Span> {
        let byte = src.find(needle).unwrap();
        resolve(src, byte_to_pos(src, byte))
    }

    #[test]
    fn jumps_to_function_declaration() {
        let src = "йопта фу() { отвечаю 1; }\nфу();";
        let usage = src.rfind("фу").unwrap();
        let span = resolve(src, byte_to_pos(src, usage)).expect("should resolve");
        assert_eq!(span.start, src.find("фу").unwrap());
    }

    #[test]
    fn jumps_to_variable_declaration() {
        let src = "ясенХуй x = 1;\nсказать(x);";
        let usage = src.rfind('x').unwrap();
        let span = resolve(src, byte_to_pos(src, usage)).expect("should resolve");
        assert_eq!(span.start, src.find('x').unwrap());
    }

    #[test]
    fn jumps_to_parameter() {
        let src = "йопта фу(парам) { отвечаю парам; }";
        let usage = src.rfind("парам").unwrap();
        let span = resolve(src, byte_to_pos(src, usage)).expect("should resolve");
        assert_eq!(span.start, src.find("парам").unwrap());
    }

    #[test]
    fn resolves_param_inside_array_literal_arrow() {
        let src = "ясенХуй список = [йопта(элемент) { отвечаю элемент; }];";
        let usage = src.rfind("элемент").unwrap();
        let span = resolve(src, byte_to_pos(src, usage)).expect("should resolve");
        assert_eq!(span.start, src.find("элемент").unwrap());
    }

    #[test]
    fn resolves_param_inside_object_method_value() {
        let src = "ясенХуй объект = { метод: йопта(арг) { отвечаю арг; } };";
        let usage = src.rfind("арг").unwrap();
        let span = resolve(src, byte_to_pos(src, usage)).expect("should resolve");
        assert_eq!(span.start, src.find("арг").unwrap());
    }

    #[test]
    fn keyword_has_no_definition() {
        let src = "йопта фу() {}";
        assert!(def_at(src, "йопта").is_none());
    }

    #[test]
    fn unknown_identifier_has_no_definition() {
        let src = "ясенХуй x = 1;";
        let pos = byte_to_pos(src, 0);
        assert!(resolve("неизвестно;", pos).is_none());
    }
}
