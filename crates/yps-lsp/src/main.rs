use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};
use yps_lsp::builtins::builtin_doc;
use yps_lsp::completion::completion_items;
use yps_lsp::definition::goto_definition;
use yps_lsp::diagnostics::analyze;
use yps_lsp::format::format_document;
use yps_lsp::hover::keyword_hover;
use yps_lsp::position::{pos_to_byte, span_to_range, word_at};
use yps_lsp::symbols::document_symbols;

struct Backend {
    client: Client,
    documents: Arc<RwLock<HashMap<Url, String>>>,
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult { capabilities: yps_lsp::server_capabilities(), ..Default::default() })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client.log_message(MessageType::INFO, "yps-lsp initialized").await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let text = params.text_document.text;
        self.documents.write().await.insert(uri.clone(), text.clone());
        self.publish(uri, &text).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        if let Some(change) = params.content_changes.into_iter().last() {
            let text = change.text;
            self.documents.write().await.insert(uri.clone(), text.clone());
            self.publish(uri, &text).await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        self.documents.write().await.remove(&params.text_document.uri);
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = &params.text_document_position.text_document.uri;
        let docs = self.documents.read().await;
        let text = docs.get(uri).map(String::as_str).unwrap_or_default();
        Ok(Some(CompletionResponse::Array(completion_items(text))))
    }

    async fn document_symbol(&self, params: DocumentSymbolParams) -> Result<Option<DocumentSymbolResponse>> {
        let docs = self.documents.read().await;
        let Some(text) = docs.get(&params.text_document.uri) else {
            return Ok(None);
        };
        Ok(Some(DocumentSymbolResponse::Nested(document_symbols(text))))
    }

    async fn goto_definition(&self, params: GotoDefinitionParams) -> Result<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri.clone();
        let pos = params.text_document_position_params.position;

        let docs = self.documents.read().await;
        let Some(text) = docs.get(&uri) else {
            return Ok(None);
        };

        Ok(goto_definition(text, pos)
            .map(|span| GotoDefinitionResponse::Scalar(Location { uri, range: span_to_range(text, span) })))
    }

    async fn formatting(&self, params: DocumentFormattingParams) -> Result<Option<Vec<TextEdit>>> {
        let docs = self.documents.read().await;
        let Some(text) = docs.get(&params.text_document.uri) else {
            return Ok(None);
        };
        Ok(format_document(text))
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;

        let docs = self.documents.read().await;
        let Some(text) = docs.get(uri) else {
            return Ok(None);
        };

        let byte_pos = pos_to_byte(text, pos);
        let word = word_at(text, byte_pos);

        if word.is_empty() {
            return Ok(None);
        }

        Ok(keyword_hover(word).or_else(|| builtin_doc(word)).map(|doc| Hover {
            contents: HoverContents::Markup(MarkupContent { kind: MarkupKind::Markdown, value: doc.to_string() }),
            range: None,
        }))
    }
}

impl Backend {
    async fn publish(&self, uri: Url, text: &str) {
        let diagnostics = analyze(uri.as_str(), text);
        self.client.publish_diagnostics(uri, diagnostics, None).await;
    }
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let (service, socket) =
        LspService::new(|client| Backend { client, documents: Arc::new(RwLock::new(HashMap::new())) });
    Server::new(stdin, stdout, socket).serve(service).await;
}
