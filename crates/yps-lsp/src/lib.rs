pub mod builtins;
pub mod completion;
pub mod definition;
pub mod diagnostics;
pub mod format;
pub mod hover;
pub mod position;
pub mod rename;
pub mod symbols;
pub mod types;

use tower_lsp::lsp_types::{
    CompletionOptions, Diagnostic, DocumentSymbol, HoverProviderCapability, OneOf, RenameOptions, ServerCapabilities,
    TextDocumentSyncCapability, TextDocumentSyncKind, WorkDoneProgressOptions,
};
use yps_lexer::{Lexer, SourceFile};
use yps_parser::Parser;

use crate::definition::Declaration;

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
        rename_provider: Some(OneOf::Right(RenameOptions {
            prepare_provider: Some(true),
            work_done_progress_options: WorkDoneProgressOptions::default(),
        })),
        ..Default::default()
    }
}

pub struct Analyzed {
    pub text: String,
    pub diagnostics: Vec<Diagnostic>,
    pub symbols: Vec<DocumentSymbol>,
    pub declarations: Vec<Declaration>,
}

#[must_use]
pub fn analyze(text: &str) -> Analyzed {
    let sf = SourceFile::new("inline".to_string(), text.to_string());
    let (tokens, lex_diags) = Lexer::new(&sf).tokenize();
    let (program, parse_diags) = Parser::new(&tokens, &sf).parse_program();
    Analyzed {
        diagnostics: diagnostics::to_lsp_diagnostics(text, &lex_diags, &parse_diags),
        symbols: symbols::document_symbols(&program, text),
        declarations: definition::declarations(&program),
        text: text.to_string(),
    }
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
        assert!(caps.rename_provider.is_some());
    }
}
