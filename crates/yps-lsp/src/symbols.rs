use tower_lsp::lsp_types::{DocumentSymbol, Location, SymbolInformation, SymbolKind, Url};
use yps_lexer::Span;
use yps_parser::Program;
use yps_parser::ast::{ClassMember, Identifier, Pattern, Stmt};

use crate::position::span_to_range;

#[must_use]
pub fn document_symbols(program: &Program, text: &str) -> Vec<DocumentSymbol> {
    let mut out = Vec::new();
    for stmt in &program.items {
        collect(stmt, text, &mut out);
    }
    out
}

#[allow(deprecated)]
#[must_use]
pub fn workspace_symbols<'a>(
    documents: impl IntoIterator<Item = (&'a Url, &'a [DocumentSymbol])>,
    query: &str,
) -> Vec<SymbolInformation> {
    let query_lower = query.to_lowercase();
    let mut out = Vec::new();
    for (uri, symbols) in documents {
        for symbol in symbols {
            flatten_symbol(symbol, uri, None, &query_lower, &mut out);
        }
    }
    out
}

#[allow(deprecated)]
fn flatten_symbol(
    symbol: &DocumentSymbol,
    uri: &Url,
    container_name: Option<&str>,
    query_lower: &str,
    out: &mut Vec<SymbolInformation>,
) {
    if symbol.name.to_lowercase().contains(query_lower) {
        out.push(SymbolInformation {
            name: symbol.name.clone(),
            kind: symbol.kind,
            tags: None,
            deprecated: None,
            location: Location { uri: uri.clone(), range: symbol.selection_range },
            container_name: container_name.map(str::to_string),
        });
    }
    if let Some(children) = &symbol.children {
        for child in children {
            flatten_symbol(child, uri, Some(&symbol.name), query_lower, out);
        }
    }
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
        .filter_map(|member| match member {
            ClassMember::Constructor { span, .. } => {
                Some(symbol(text, "constructor", None, SymbolKind::CONSTRUCTOR, *span, *span, None))
            }
            ClassMember::Method { name, is_static, span, .. } => {
                Some(symbol(text, &name.name, static_detail(*is_static), SymbolKind::METHOD, *span, name.span, None))
            }
            ClassMember::Field { name, is_static, span, .. } => {
                Some(symbol(text, &name.name, static_detail(*is_static), SymbolKind::FIELD, *span, name.span, None))
            }
            ClassMember::Getter { name, span, .. } => {
                Some(symbol(text, &name.name, Some("get".to_string()), SymbolKind::PROPERTY, *span, name.span, None))
            }
            ClassMember::Setter { name, span, .. } => {
                Some(symbol(text, &name.name, Some("set".to_string()), SymbolKind::PROPERTY, *span, name.span, None))
            }
            ClassMember::StaticBlock { .. } => None,
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

    fn symbols_of(src: &str) -> Vec<DocumentSymbol> {
        crate::analyze(src).symbols
    }

    #[test]
    fn collects_functions_classes_and_vars() {
        let src = "йопта приветствие(имя) { отвечаю имя; }\nясенХуй x = 1;\nгыы y = 2;";
        let syms = symbols_of(src);
        assert_eq!(names(&syms), vec!["приветствие", "x", "y"]);
        assert_eq!(syms[0].kind, SymbolKind::FUNCTION);
        assert_eq!(syms[1].kind, SymbolKind::CONSTANT);
        assert_eq!(syms[2].kind, SymbolKind::VARIABLE);
    }

    #[test]
    fn class_members_are_nested() {
        let src = "клёво Кот { constructor() {} мяу() {} }";
        let syms = symbols_of(src);
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].kind, SymbolKind::CLASS);
        let children = syms[0].children.as_ref().unwrap();
        let child_names = names(children);
        assert!(child_names.contains(&"мяу"), "got {child_names:?}");
    }

    #[test]
    fn selection_range_points_at_name() {
        let src = "йопта фу() {}";
        let syms = symbols_of(src);
        let sel = syms[0].selection_range;
        assert_eq!(sel.start.character, 6);
    }

    #[test]
    fn array_destructuring_yields_one_symbol_per_binding() {
        let src = "ясенХуй [первый, второй] = [1, 2];";
        let syms = symbols_of(src);
        assert_eq!(names(&syms), vec!["первый", "второй"]);
        assert!(syms.iter().all(|s| s.kind == SymbolKind::CONSTANT));
    }

    #[test]
    fn object_destructuring_yields_one_symbol_per_binding() {
        let src = "гыы { ключ, значение } = объект;";
        let syms = symbols_of(src);
        assert_eq!(names(&syms), vec!["ключ", "значение"]);
        assert!(syms.iter().all(|s| s.kind == SymbolKind::VARIABLE));
    }

    fn url(s: &str) -> Url {
        Url::parse(s).unwrap()
    }

    #[test]
    fn workspace_query_matches_top_level_function() {
        let uri = url("file:///a.yopta");
        let syms = symbols_of("йопта приветствие(имя) { отвечаю имя; }");
        let results = workspace_symbols([(&uri, syms.as_slice())], "привет");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "приветствие");
        assert_eq!(results[0].location.uri, uri);
        assert_eq!(results[0].container_name, None);
    }

    #[test]
    fn workspace_query_matches_nested_class_method_with_container() {
        let uri = url("file:///a.yopta");
        let syms = symbols_of("клёво Кот { constructor() {} мяу() {} }");
        let results = workspace_symbols([(&uri, syms.as_slice())], "мяу");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "мяу");
        assert_eq!(results[0].container_name.as_deref(), Some("Кот"));
    }

    #[test]
    fn empty_query_returns_everything() {
        let uri = url("file:///a.yopta");
        let syms = symbols_of("клёво Кот { constructor() {} мяу() {} }");
        let results = workspace_symbols([(&uri, syms.as_slice())], "");
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn no_match_returns_empty() {
        let uri = url("file:///a.yopta");
        let syms = symbols_of("йопта фу() {}");
        let results = workspace_symbols([(&uri, syms.as_slice())], "нетакогонет");
        assert!(results.is_empty());
    }

    #[test]
    fn searches_symbols_from_multiple_documents() {
        let uri_a = url("file:///a.yopta");
        let uri_b = url("file:///b.yopta");
        let syms_a = symbols_of("йопта первый() {}");
        let syms_b = symbols_of("йопта второй() {}");
        let results = workspace_symbols([(&uri_a, syms_a.as_slice()), (&uri_b, syms_b.as_slice())], "");
        assert_eq!(results.len(), 2);
        assert!(results.iter().any(|s| s.name == "первый" && s.location.uri == uri_a));
        assert!(results.iter().any(|s| s.name == "второй" && s.location.uri == uri_b));
    }

    #[test]
    fn matching_is_case_insensitive() {
        let uri = url("file:///a.yopta");
        let syms = symbols_of("йопта Foo() {}");
        let results = workspace_symbols([(&uri, syms.as_slice())], "foo");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "Foo");
    }
}
