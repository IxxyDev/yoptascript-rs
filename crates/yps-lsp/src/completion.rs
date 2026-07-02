use std::collections::HashSet;

use tower_lsp::lsp_types::{
    CompletionItem, CompletionItemKind, DocumentSymbol, Documentation, MarkupContent, MarkupKind,
};
use yps_interpreter::builtins::builtin_names;
use yps_lexer::KEYWORDS;

use crate::builtins::builtin_doc;
use crate::position::member_receiver;
use crate::types::{global_type_items, is_known_global, member_items_for};

fn markdown(value: &str) -> Documentation {
    Documentation::MarkupContent(MarkupContent { kind: MarkupKind::Markdown, value: value.to_string() })
}

#[must_use]
pub fn completion_items(symbols: &[DocumentSymbol], text: &str, cursor: Option<usize>) -> Vec<CompletionItem> {
    if let Some(byte) = cursor
        && let Some(receiver) = member_receiver(text, byte)
    {
        return member_completion(receiver);
    }

    let mut items: Vec<CompletionItem> = KEYWORDS
        .iter()
        .map(|&kw| CompletionItem {
            label: kw.to_string(),
            kind: Some(CompletionItemKind::KEYWORD),
            ..Default::default()
        })
        .collect();

    let mut seen: HashSet<String> = HashSet::new();
    for name in builtin_names() {
        seen.insert(name.to_string());
        items.push(CompletionItem {
            label: name.to_string(),
            kind: Some(CompletionItemKind::FUNCTION),
            documentation: builtin_doc(name).map(markdown),
            ..Default::default()
        });
    }

    for item in global_type_items() {
        if seen.insert(item.label.clone()) {
            items.push(item);
        }
    }

    for symbol in symbols {
        if seen.insert(symbol.name.clone()) {
            items.push(CompletionItem {
                label: symbol.name.clone(),
                kind: Some(symbol_completion_kind(symbol.kind)),
                detail: Some("из текущего файла".to_string()),
                ..Default::default()
            });
        }
    }

    items
}

fn member_completion(receiver: &str) -> Vec<CompletionItem> {
    if !receiver.is_empty() {
        let prefix = format!("{receiver}.");
        let builtin_members: Vec<CompletionItem> = builtin_names()
            .iter()
            .filter_map(|name| name.strip_prefix(&prefix).map(|sub| (sub, *name)))
            .map(|(sub, name)| CompletionItem {
                label: sub.to_string(),
                kind: Some(CompletionItemKind::METHOD),
                detail: Some(name.to_string()),
                documentation: builtin_doc(name).map(markdown),
                ..Default::default()
            })
            .collect();
        if !builtin_members.is_empty() {
            return builtin_members;
        }

        if is_known_global(receiver) {
            return member_items_for(Some(receiver));
        }
    }

    member_items_for(None)
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

    fn items_for(src: &str, cursor: Option<usize>) -> Vec<CompletionItem> {
        completion_items(&crate::analyze(src).symbols, src, cursor)
    }

    #[test]
    fn includes_keywords_and_builtins() {
        let items = items_for("", None);
        let labels = labels(&items);
        assert!(labels.contains(&"йопта"));
        assert!(labels.contains(&"сказать"));
    }

    #[test]
    fn includes_document_declarations() {
        let src = "йопта мояФункция() {}\nясенХуй мояКонстанта = 1;";
        let items = items_for(src, None);
        let labels = labels(&items);
        assert!(labels.contains(&"мояФункция"), "got {labels:?}");
        assert!(labels.contains(&"мояКонстанта"), "got {labels:?}");
    }

    #[test]
    fn builtins_carry_js_documentation() {
        let items = items_for("", None);
        let say = items.iter().find(|i| i.label == "сказать").unwrap();
        match &say.documentation {
            Some(Documentation::MarkupContent(mc)) => assert!(mc.value.contains("console.log")),
            other => panic!("expected markdown docs for 'сказать', got {other:?}"),
        }
    }

    #[test]
    fn document_declaration_marked_with_detail() {
        let src = "йопта фу() {}";
        let items = items_for(src, None);
        let fu = items.iter().find(|i| i.label == "фу").unwrap();
        assert_eq!(fu.detail.as_deref(), Some("из текущего файла"));
        assert_eq!(fu.kind, Some(CompletionItemKind::FUNCTION));
    }

    #[test]
    fn includes_builtin_classes_and_namespaces() {
        let items = items_for("", None);
        let labels = labels(&items);
        assert!(labels.contains(&"Матан"), "ожидался namespace Матан");
        assert!(labels.contains(&"Карта"), "ожидался класс Карта");
        assert!(labels.contains(&"Жсон"));
    }

    #[test]
    fn member_position_offers_namespace_members() {
        let src = "Матан.";
        let items = items_for(src, Some(src.len()));
        let labels = labels(&items);
        assert!(labels.contains(&"корень"), "got {labels:?}");
        assert!(!labels.contains(&"йопта"), "ключевые слова не должны быть среди членов");
        assert!(!labels.contains(&"добавить"), "методы массива не относятся к Матан");
    }

    #[test]
    fn member_position_unknown_receiver_unions_members() {
        let src = "x.";
        let items = items_for(src, Some(src.len()));
        let labels = labels(&items);
        assert!(labels.contains(&"вВерхнийРегистр"));
        assert!(labels.contains(&"добавить"));
    }

    #[test]
    fn member_position_offers_console_family() {
        let src = "сказать.";
        let items = items_for(src, Some(src.len()));
        let labels = labels(&items);
        assert!(labels.contains(&"ошибка"), "ожидалось сказать.ошибка, got {labels:?}");
        assert!(labels.contains(&"время"));
        assert!(!labels.contains(&"добавить"), "методы массива не относятся к сказать");
    }
}
