use std::collections::HashSet;

use tower_lsp::lsp_types::{CompletionItem, CompletionItemKind, Documentation, MarkupContent, MarkupKind};
use yps_interpreter::builtins::builtin_names;
use yps_lexer::KEYWORDS;

use crate::builtins::builtin_doc;
use crate::symbols::document_symbols;

fn markdown(value: &str) -> Documentation {
    Documentation::MarkupContent(MarkupContent { kind: MarkupKind::Markdown, value: value.to_string() })
}

#[must_use]
pub fn completion_items(text: &str) -> Vec<CompletionItem> {
    let mut items: Vec<CompletionItem> = KEYWORDS
        .iter()
        .map(|&kw| CompletionItem {
            label: kw.to_string(),
            kind: Some(CompletionItemKind::KEYWORD),
            ..Default::default()
        })
        .collect();

    for name in builtin_names() {
        items.push(CompletionItem {
            label: name.to_string(),
            kind: Some(CompletionItemKind::FUNCTION),
            documentation: builtin_doc(name).map(markdown),
            ..Default::default()
        });
    }

    let mut seen: HashSet<String> = HashSet::new();
    for symbol in document_symbols(text) {
        if seen.insert(symbol.name.clone()) {
            items.push(CompletionItem {
                label: symbol.name,
                kind: Some(symbol_completion_kind(symbol.kind)),
                detail: Some("из текущего файла".to_string()),
                ..Default::default()
            });
        }
    }

    items
}

fn symbol_completion_kind(kind: tower_lsp::lsp_types::SymbolKind) -> CompletionItemKind {
    use tower_lsp::lsp_types::SymbolKind;
    match kind {
        SymbolKind::FUNCTION => CompletionItemKind::FUNCTION,
        SymbolKind::CLASS => CompletionItemKind::CLASS,
        SymbolKind::CONSTANT => CompletionItemKind::CONSTANT,
        _ => CompletionItemKind::VARIABLE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels(items: &[CompletionItem]) -> Vec<&str> {
        items.iter().map(|i| i.label.as_str()).collect()
    }

    #[test]
    fn includes_keywords_and_builtins() {
        let items = completion_items("");
        let labels = labels(&items);
        assert!(labels.contains(&"йопта"));
        assert!(labels.contains(&"сказать"));
    }

    #[test]
    fn includes_document_declarations() {
        let src = "йопта мояФункция() {}\nясенХуй мояКонстанта = 1;";
        let items = completion_items(src);
        let labels = labels(&items);
        assert!(labels.contains(&"мояФункция"), "got {labels:?}");
        assert!(labels.contains(&"мояКонстанта"), "got {labels:?}");
    }

    #[test]
    fn builtins_carry_js_documentation() {
        let items = completion_items("");
        let say = items.iter().find(|i| i.label == "сказать").unwrap();
        match &say.documentation {
            Some(Documentation::MarkupContent(mc)) => assert!(mc.value.contains("console.log")),
            other => panic!("expected markdown docs for 'сказать', got {other:?}"),
        }
    }

    #[test]
    fn document_declaration_marked_with_detail() {
        let src = "йопта фу() {}";
        let items = completion_items(src);
        let fu = items.iter().find(|i| i.label == "фу").unwrap();
        assert_eq!(fu.detail.as_deref(), Some("из текущего файла"));
        assert_eq!(fu.kind, Some(CompletionItemKind::FUNCTION));
    }
}
