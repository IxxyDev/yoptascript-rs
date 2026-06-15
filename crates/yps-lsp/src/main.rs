use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};
use yps_interpreter::builtins::builtin_names;
use yps_lexer::{Lexer, Severity, SourceFile};
use yps_parser::Parser;

struct Backend {
    client: Client,
    documents: Arc<RwLock<HashMap<Url, String>>>,
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
                completion_provider: Some(CompletionOptions::default()),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                ..Default::default()
            },
            ..Default::default()
        })
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
        self.publish_diagnostics(uri, &text).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        if let Some(change) = params.content_changes.into_iter().last() {
            let text = change.text;
            self.documents.write().await.insert(uri.clone(), text.clone());
            self.publish_diagnostics(uri, &text).await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        self.documents.write().await.remove(&params.text_document.uri);
    }

    async fn completion(&self, _: CompletionParams) -> Result<Option<CompletionResponse>> {
        let keywords = [
            "гыы",
            "ясенХуй",
            "участковый",
            "вилкойвглаз",
            "иливжопураз",
            "потрещим",
            "го",
            "харэ",
            "двигай",
            "йопта",
            "отвечаю",
            "правда",
            "лож",
            "ноль",
            "хапнуть",
            "побратски",
            "гоп",
            "аченетак",
            "тюряжка",
            "кидай",
            "клёво",
            "клево",
            "батя",
            "яга",
            "захуярить",
            "тырыпыры",
            "попонятия",
            "чезажижан",
            "шкура",
            "пиздюли",
            "поебалу",
            "ассо",
            "сидетьНахуй",
            "спиздить",
            "предъява",
            "сашаГрей",
            "ёбнуть",
            "куку",
            "юзай",
            "базарпо",
            "тема",
            "нуичо",
            "крутани",
        ];

        let mut items: Vec<CompletionItem> = keywords
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
                ..Default::default()
            });
        }

        Ok(Some(CompletionResponse::Array(items)))
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;

        let docs = self.documents.read().await;
        let text = match docs.get(uri) {
            Some(t) => t,
            None => return Ok(None),
        };

        let byte_pos = pos_to_byte(text, pos);
        let word = word_at(text, byte_pos);

        if word.is_empty() {
            return Ok(None);
        }

        match keyword_hover(word) {
            Some(doc) => Ok(Some(Hover {
                contents: HoverContents::Markup(MarkupContent { kind: MarkupKind::Markdown, value: doc.to_string() }),
                range: None,
            })),
            None => Ok(None),
        }
    }
}

impl Backend {
    async fn publish_diagnostics(&self, uri: Url, text: &str) {
        let sf = SourceFile::new(uri.to_string(), text.to_string());
        let lexer = Lexer::new(&sf);
        let (tokens, lex_diags) = lexer.tokenize();

        let parser = Parser::new(&tokens, &sf);
        let (_, parse_diags) = parser.parse_program();

        let diagnostics: Vec<Diagnostic> = lex_diags
            .iter()
            .chain(parse_diags.iter())
            .map(|d| {
                let severity = match d.severity {
                    Severity::Error => DiagnosticSeverity::ERROR,
                    Severity::Warning => DiagnosticSeverity::WARNING,
                };
                Diagnostic {
                    range: span_to_range(text, d.span),
                    severity: Some(severity),
                    message: d.message.clone(),
                    ..Default::default()
                }
            })
            .collect();

        self.client.publish_diagnostics(uri, diagnostics, None).await;
    }
}

fn span_to_range(src: &str, span: yps_lexer::Span) -> Range {
    Range { start: byte_to_pos(src, span.start), end: byte_to_pos(src, span.end) }
}

fn byte_to_pos(src: &str, offset: usize) -> Position {
    let raw = offset.min(src.len());
    let clamped = (0..=raw).rev().find(|&i| src.is_char_boundary(i)).unwrap_or(0);
    let prefix = &src[..clamped];
    let line = prefix.bytes().filter(|&b| b == b'\n').count() as u32;
    let line_start = prefix.rfind('\n').map(|i| i + 1).unwrap_or(0);
    let character = prefix[line_start..].encode_utf16().count() as u32;
    Position { line, character }
}

fn pos_to_byte(src: &str, pos: Position) -> usize {
    let mut line = 0u32;
    let mut line_start = 0usize;
    for (i, ch) in src.char_indices() {
        if line == pos.line {
            line_start = i;
            break;
        }
        if ch == '\n' {
            line += 1;
        }
        if line == pos.line {
            line_start = i + ch.len_utf8();
            break;
        }
    }
    if pos.line > 0 && line < pos.line {
        line_start = src.len();
    }
    let mut chars_left = pos.character as usize;
    let mut idx = line_start;
    for ch in src[line_start..].chars() {
        if chars_left == 0 {
            break;
        }
        chars_left = chars_left.saturating_sub(ch.len_utf16());
        idx += ch.len_utf8();
    }
    idx
}

fn word_at(src: &str, byte_pos: usize) -> &str {
    let is_ident = |c: char| c.is_alphanumeric() || c == '_';
    let raw = byte_pos.min(src.len());
    let clamped = (0..=raw).rev().find(|&i| src.is_char_boundary(i)).unwrap_or(0);

    let start =
        src[..clamped].char_indices().rev().take_while(|(_, c)| is_ident(*c)).last().map(|(i, _)| i).unwrap_or(clamped);

    let end = src[clamped..]
        .char_indices()
        .take_while(|(_, c)| is_ident(*c))
        .last()
        .map(|(i, c)| clamped + i + c.len_utf8())
        .unwrap_or(clamped);

    &src[start..end]
}

fn keyword_hover(word: &str) -> Option<&'static str> {
    match word {
        "йопта" => Some("**function** — объявление функции"),
        "гыы" => Some("**var** — объявление переменной"),
        "ясенХуй" | "ЯсенХуй" => Some("**const** — объявление константы"),
        "участковый" => Some("**const** — объявление константы"),
        "вилкойвглаз" => Some("**if** — условный оператор"),
        "иливжопураз" => Some("**else** — ветвь else"),
        "потрещим" => Some("**while** — цикл while"),
        "го" => Some("**for** — цикл for"),
        "харэ" => Some("**break** — прервать цикл"),
        "двигай" => Some("**continue** — следующая итерация"),
        "отвечаю" => Some("**return** — вернуть значение"),
        "правда" => Some("**true** — булево истина"),
        "лож" => Some("**false** — булево ложь"),
        "ноль" => Some("**null** — нулевое значение"),
        "хапнуть" | "побратски" => Some("**try** — блок try"),
        "гоп" | "аченетак" => Some("**catch** — поймать ошибку"),
        "тюряжка" => Some("**finally** — блок finally"),
        "кидай" => Some("**throw** — бросить ошибку"),
        "клёво" | "клево" => Some("**class** — объявление класса"),
        "батя" => Some("**extends** — наследование"),
        "яга" => Some("**super** — обращение к родителю"),
        "захуярить" | "гыйбать" => Some("**new** — создать экземпляр"),
        "тырыпыры" => Some("**this** — текущий объект"),
        "попонятия" => Some("**static** — статический член"),
        "чезажижан" => Some("**typeof** — тип значения"),
        "шкура" => Some("**instanceof** — проверка типа"),
        "пиздюли" => Some("**function\\*** — функция-генератор"),
        "поебалу" => Some("**yield** — отдать значение из генератора"),
        "ассо" => Some("**async** — асинхронная функция"),
        "сидетьНахуй" => Some("**await** — ожидать промис"),
        "спиздить" => Some("**import** — импорт модуля"),
        "предъява" => Some("**export** — экспорт"),
        "сашаГрей" => Some("**of** — итерация (for-of)"),
        "ёбнуть" | "ебнуть" => Some("**delete** — удалить свойство"),
        "куку" => Some("**void** — вычислить и вернуть undefined"),
        "юзай" => Some("**using** — управление ресурсом"),
        "базарпо" => Some("**switch** — множественный выбор"),
        "тема" => Some("**case** — ветвь switch"),
        "нуичо" => Some("**default** — ветвь по умолчанию"),
        "крутани" => Some("**do** — цикл do-while"),
        _ => None,
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
