pub mod builtins;
pub mod code_actions;
pub mod completion;
pub mod definition;
pub mod diagnostics;
pub mod format;
pub mod hover;
pub mod lint;
pub mod position;
pub mod references;
pub mod rename;
pub mod semantic_tokens;
pub mod signature_help;
pub mod symbols;
pub mod types;

use tower_lsp::lsp_types::{
    CodeActionKind, CodeActionOptions, CodeActionProviderCapability, CompletionOptions, Diagnostic, DocumentSymbol,
    HoverProviderCapability, OneOf, RenameOptions, SemanticTokensFullOptions, SemanticTokensOptions,
    SemanticTokensServerCapabilities, ServerCapabilities, SignatureHelpOptions, TextDocumentSyncCapability,
    TextDocumentSyncKind, WorkDoneProgressOptions,
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
        references_provider: Some(OneOf::Left(true)),
        semantic_tokens_provider: Some(SemanticTokensServerCapabilities::SemanticTokensOptions(
            SemanticTokensOptions {
                legend: semantic_tokens::legend(),
                full: Some(SemanticTokensFullOptions::Bool(true)),
                range: None,
                work_done_progress_options: WorkDoneProgressOptions::default(),
            },
        )),
        signature_help_provider: Some(SignatureHelpOptions {
            trigger_characters: Some(vec!["(".to_string(), ",".to_string()]),
            retrigger_characters: None,
            work_done_progress_options: WorkDoneProgressOptions::default(),
        }),
        code_action_provider: Some(CodeActionProviderCapability::Options(CodeActionOptions {
            code_action_kinds: Some(vec![CodeActionKind::QUICKFIX]),
            resolve_provider: None,
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
    pub lint: yps_lint::LintResult,
}

#[must_use]
pub fn analyze(text: &str) -> Analyzed {
    let sf = SourceFile::new("inline".to_string(), text.to_string());
    let (tokens, lex_diags) = Lexer::new(&sf).tokenize();
    let (program, parse_diags) = Parser::new(&tokens, &sf).parse_program();
    let lint = yps_lint::lint_source(text);

    let mut diagnostics = diagnostics::to_lsp_diagnostics(text, &lex_diags, &parse_diags);
    diagnostics.extend(lint::to_lsp_diagnostics(text, &lint.diagnostics));

    Analyzed {
        diagnostics,
        symbols: symbols::document_symbols(&program, text),
        declarations: definition::declarations(&program),
        lint,
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
        assert!(caps.references_provider.is_some());
        assert!(caps.semantic_tokens_provider.is_some());
        assert!(caps.signature_help_provider.is_some());
        assert!(caps.code_action_provider.is_some());
    }

    #[test]
    fn lint_diagnostics_are_included_in_published_diagnostics() {
        let analyzed = analyze("гыы х = 1;\n");
        assert!(analyzed.diagnostics.iter().any(|d| d.source.as_deref() == Some(lint::SOURCE)));
        assert_eq!(analyzed.lint.diagnostics.len(), 1);
    }

    #[test]
    fn broken_source_publishes_no_lint_diagnostics() {
        let analyzed = analyze("йопта (");
        assert!(analyzed.lint.diagnostics.is_empty());
        assert!(!analyzed.diagnostics.iter().any(|d| d.source.as_deref() == Some(lint::SOURCE)));
    }
}
