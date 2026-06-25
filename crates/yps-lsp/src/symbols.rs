use tower_lsp::lsp_types::{DocumentSymbol, SymbolKind};
use yps_lexer::Span;
use yps_parser::ast::{ClassMember, Identifier, Pattern, Stmt};

use crate::parse_program;
use crate::position::span_to_range;

#[must_use]
pub fn document_symbols(text: &str) -> Vec<DocumentSymbol> {
    let program = parse_program(text);
    let mut out = Vec::new();
    for stmt in &program.items {
        collect(stmt, text, &mut out);
    }
    out
}

fn collect(stmt: &Stmt, text: &str, out: &mut Vec<DocumentSymbol>) {
    match stmt {
        Stmt::FunctionDecl { name, span, is_async, is_generator, .. } => {
            let detail = function_detail(*is_async, *is_generator);
            out.push(symbol(text, &name.name, detail, SymbolKind::FUNCTION, *span, name.span, None));
        }
        Stmt::ClassDecl { name, members, span, .. } => {
            let children = class_children(members, text);
            out.push(symbol(text, &name.name, None, SymbolKind::CLASS, *span, name.span, Some(children)));
        }
        Stmt::VarDecl { pattern, is_const, span, .. } => {
            let kind = if *is_const { SymbolKind::CONSTANT } else { SymbolKind::VARIABLE };
            for ident in pattern_idents(pattern) {
                out.push(symbol(text, &ident.name, None, kind, *span, ident.span, None));
            }
        }
        _ => {}
    }
}

fn class_children(members: &[ClassMember], text: &str) -> Vec<DocumentSymbol> {
    members
        .iter()
        .map(|member| match member {
            ClassMember::Constructor { span, .. } => {
                symbol(text, "constructor", None, SymbolKind::CONSTRUCTOR, *span, *span, None)
            }
            ClassMember::Method { name, is_static, span, .. } => {
                symbol(text, &name.name, static_detail(*is_static), SymbolKind::METHOD, *span, name.span, None)
            }
            ClassMember::Field { name, is_static, span, .. } => {
                symbol(text, &name.name, static_detail(*is_static), SymbolKind::FIELD, *span, name.span, None)
            }
            ClassMember::Getter { name, span, .. } => {
                symbol(text, &name.name, Some("get".to_string()), SymbolKind::PROPERTY, *span, name.span, None)
            }
            ClassMember::Setter { name, span, .. } => {
                symbol(text, &name.name, Some("set".to_string()), SymbolKind::PROPERTY, *span, name.span, None)
            }
        })
        .collect()
}

fn pattern_idents(pattern: &Pattern) -> Vec<&Identifier> {
    let mut out = Vec::new();
    push_pattern_idents(pattern, &mut out);
    out
}

fn push_pattern_idents<'a>(pattern: &'a Pattern, out: &mut Vec<&'a Identifier>) {
    match pattern {
        Pattern::Identifier(ident) => out.push(ident),
        Pattern::Array { elements, rest, .. } => {
            for el in elements.iter().flatten() {
                push_pattern_idents(el, out);
            }
            if let Some(rest) = rest {
                push_pattern_idents(rest, out);
            }
        }
        Pattern::Object { properties, rest, .. } => {
            for prop in properties {
                match &prop.value {
                    Some(value) => push_pattern_idents(value, out),
                    None => out.push(&prop.key),
                }
            }
            if let Some(rest) = rest {
                push_pattern_idents(rest, out);
            }
        }
        Pattern::Default { pattern, .. } => push_pattern_idents(pattern, out),
    }
}

fn function_detail(is_async: bool, is_generator: bool) -> Option<String> {
    match (is_async, is_generator) {
        (true, true) => Some("async function*".to_string()),
        (true, false) => Some("async function".to_string()),
        (false, true) => Some("function*".to_string()),
        (false, false) => None,
    }
}

fn static_detail(is_static: bool) -> Option<String> {
    is_static.then(|| "static".to_string())
}

#[allow(deprecated)]
fn symbol(
    text: &str,
    name: &str,
    detail: Option<String>,
    kind: SymbolKind,
    span: Span,
    selection: Span,
    children: Option<Vec<DocumentSymbol>>,
) -> DocumentSymbol {
    DocumentSymbol {
        name: name.to_string(),
        detail,
        kind,
        tags: None,
        deprecated: None,
        range: span_to_range(text, span),
        selection_range: span_to_range(text, selection),
        children,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(symbols: &[DocumentSymbol]) -> Vec<&str> {
        symbols.iter().map(|s| s.name.as_str()).collect()
    }

    #[test]
    fn collects_functions_classes_and_vars() {
        let src = "йопта приветствие(имя) { отвечаю имя; }\nясенХуй x = 1;\nгыы y = 2;";
        let syms = document_symbols(src);
        assert_eq!(names(&syms), vec!["приветствие", "x", "y"]);
        assert_eq!(syms[0].kind, SymbolKind::FUNCTION);
        assert_eq!(syms[1].kind, SymbolKind::CONSTANT);
        assert_eq!(syms[2].kind, SymbolKind::VARIABLE);
    }

    #[test]
    fn class_members_are_nested() {
        let src = "клёво Кот { constructor() {} мяу() {} }";
        let syms = document_symbols(src);
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].kind, SymbolKind::CLASS);
        let children = syms[0].children.as_ref().unwrap();
        let child_names = names(children);
        assert!(child_names.contains(&"мяу"), "got {child_names:?}");
    }

    #[test]
    fn selection_range_points_at_name() {
        let src = "йопта фу() {}";
        let syms = document_symbols(src);
        let sel = syms[0].selection_range;
        assert_eq!(sel.start.character, 6);
    }
}
