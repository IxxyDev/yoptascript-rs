pub mod builtins;
pub mod completion;
pub mod definition;
pub mod diagnostics;
pub mod format;
pub mod hover;
pub mod position;
pub mod symbols;
pub mod types;

use tower_lsp::lsp_types::{
    CompletionOptions, HoverProviderCapability, OneOf, ServerCapabilities, TextDocumentSyncCapability,
    TextDocumentSyncKind,
};
use yps_lexer::{Lexer, SourceFile};
use yps_parser::{Parser, Program};

#[must_use]
pub fn server_capabilities() -> ServerCapabilities {
    ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
        completion_provider: Some(CompletionOptions {
            trigger_characters: Some(vec![".".to_string()]),
            ..Default::default()
        }),
        hover_provider: Some(HoverProviderCapability::Simple(true)),
        document_symbol_provider: Some(OneOf::Left(true)),
        document_formatting_provider: Some(OneOf::Left(true)),
        definition_provider: Some(OneOf::Left(true)),
        ..Default::default()
    }
}

#[must_use]
pub fn parse_program(text: &str) -> Program {
    let sf = SourceFile::new("inline".to_string(), text.to_string());
    let (tokens, _) = Lexer::new(&sf).tokenize();
    let (program, _) = Parser::new(&tokens, &sf).parse_program();
    program
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_providers_are_advertised() {
        let caps = server_capabilities();
        assert!(caps.completion_provider.is_some());
        assert!(caps.hover_provider.is_some());
        assert!(caps.document_symbol_provider.is_some());
        assert!(caps.document_formatting_provider.is_some());
        assert!(caps.definition_provider.is_some());
        assert!(caps.text_document_sync.is_some());
    }
}
