use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};
use yps_lsp::builtins::builtin_doc;
use yps_lsp::completion::completion_items;
use yps_lsp::definition::goto_definition;
use yps_lsp::format::format_document;
use yps_lsp::hover::keyword_hover;
use yps_lsp::position::{pos_to_byte, span_to_range, word_at};
use yps_lsp::types::{member_doc, type_doc};
use yps_lsp::{Analyzed, analyze};

struct Backend {
    client: Client,
    documents: Arc<RwLock<HashMap<Url, Arc<Analyzed>>>>,
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
        self.update_document(params.text_document.uri, &params.text_document.text).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        if let Some(change) = params.content_changes.into_iter().last() {
            self.update_document(uri, &change.text).await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        self.documents.write().await.remove(&params.text_document.uri);
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = &params.text_document_position.text_document.uri;
        let pos = params.text_document_position.position;
        let analyzed = self.get_document(uri).await.unwrap_or_else(|| Arc::new(analyze("")));
        let cursor = pos_to_byte(&analyzed.text, pos);
        Ok(Some(CompletionResponse::Array(completion_items(&analyzed.symbols, &analyzed.text, Some(cursor)))))
    }

    async fn document_symbol(&self, params: DocumentSymbolParams) -> Result<Option<DocumentSymbolResponse>> {
        let Some(analyzed) = self.get_document(&params.text_document.uri).await else {
            return Ok(None);
        };
        Ok(Some(DocumentSymbolResponse::Nested(analyzed.symbols.clone())))
    }

    async fn goto_definition(&self, params: GotoDefinitionParams) -> Result<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri.clone();
        let pos = params.text_document_position_params.position;

        let Some(analyzed) = self.get_document(&uri).await else {
            return Ok(None);
        };

        Ok(goto_definition(&analyzed.declarations, &analyzed.text, pos)
            .map(|span| GotoDefinitionResponse::Scalar(Location { uri, range: span_to_range(&analyzed.text, span) })))
    }

    async fn formatting(&self, params: DocumentFormattingParams) -> Result<Option<Vec<TextEdit>>> {
        let Some(analyzed) = self.get_document(&params.text_document.uri).await else {
            return Ok(None);
        };
        Ok(format_document(&analyzed.text))
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;

        let Some(analyzed) = self.get_document(uri).await else {
            return Ok(None);
        };

        let text = &analyzed.text;
        let byte_pos = pos_to_byte(text, pos);
        let word = word_at(text, byte_pos);

        if word.is_empty() {
            return Ok(None);
        }

        let doc = keyword_hover(word)
            .map(str::to_string)
            .or_else(|| builtin_doc(word).map(str::to_string))
            .or_else(|| type_doc(word))
            .or_else(|| member_doc(word));

        Ok(doc.map(|doc| Hover {
            contents: HoverContents::Markup(MarkupContent { kind: MarkupKind::Markdown, value: doc }),
            range: None,
        }))
    }
}

impl Backend {
    async fn get_document(&self, uri: &Url) -> Option<Arc<Analyzed>> {
        self.documents.read().await.get(uri).cloned()
    }

    async fn update_document(&self, uri: Url, text: &str) {
        let analyzed = Arc::new(analyze(text));
        let diagnostics = analyzed.diagnostics.clone();
        self.documents.write().await.insert(uri.clone(), analyzed);
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
