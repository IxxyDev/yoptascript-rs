use std::cell::RefCell;
use std::collections::BTreeSet;

use rustyline::completion::Completer;
use rustyline::error::ReadlineError;
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::validate::Validator;
use rustyline::{Context, Helper};

use yps_interpreter::builtins::builtin_names;
use yps_lexer::{KEYWORDS, KeywordKind, Lexer, SourceFile, TokenKind};

fn is_word_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

pub fn word_before_cursor(line: &str, pos: usize) -> (usize, String) {
    let mut start = pos;
    for (idx, ch) in line[..pos].char_indices().rev() {
        if is_word_char(ch) {
            start = idx;
        } else {
            break;
        }
    }
    (start, line[start..pos].to_string())
}

pub fn complete_candidates(prefix: &str, locals: &BTreeSet<String>) -> Vec<String> {
    let mut set: BTreeSet<String> = BTreeSet::new();
    for kw in KEYWORDS {
        if kw.starts_with(prefix) {
            set.insert((*kw).to_string());
        }
    }
    for b in builtin_names() {
        if b.starts_with(prefix) {
            set.insert((*b).to_string());
        }
    }
    for l in locals {
        if l.starts_with(prefix) {
            set.insert(l.clone());
        }
    }
    set.into_iter().collect()
}

fn is_decl_keyword(kind: &TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::Keyword(KeywordKind::Gyy)
            | TokenKind::Keyword(KeywordKind::Uchastkoviy)
            | TokenKind::Keyword(KeywordKind::YasenHuy)
            | TokenKind::Keyword(KeywordKind::Yopta)
    )
}

pub fn extract_declared_idents(source_text: &str) -> Vec<String> {
    let source = SourceFile::new("<repl>".to_string(), source_text.to_string());
    let (tokens, diagnostics) = Lexer::new(&source).tokenize();
    if !diagnostics.is_empty() {
        return Vec::new();
    }
    let mut idents = Vec::new();
    for i in 0..tokens.len() {
        if !is_decl_keyword(&tokens[i].kind) {
            continue;
        }
        if let Some(next) = tokens.get(i + 1)
            && next.kind == TokenKind::Identifier
        {
            idents.push(source.slice(next.span).to_string());
        }
    }
    idents
}

pub struct YpsHelper {
    locals: RefCell<BTreeSet<String>>,
}

impl YpsHelper {
    pub fn new() -> Self {
        Self { locals: RefCell::new(BTreeSet::new()) }
    }

    pub fn record_declarations(&self, source_text: &str) {
        let mut locals = self.locals.borrow_mut();
        for ident in extract_declared_idents(source_text) {
            locals.insert(ident);
        }
    }

    pub fn reset_locals(&self) {
        self.locals.borrow_mut().clear();
    }
}

impl Completer for YpsHelper {
    type Candidate = String;

    fn complete(&self, line: &str, pos: usize, _ctx: &Context<'_>) -> Result<(usize, Vec<String>), ReadlineError> {
        let (start, prefix) = word_before_cursor(line, pos);
        let candidates = complete_candidates(&prefix, &self.locals.borrow());
        Ok((start, candidates))
    }
}

impl Hinter for YpsHelper {
    type Hint = String;
}

impl Highlighter for YpsHelper {}

impl Validator for YpsHelper {}

impl Helper for YpsHelper {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn word_before_cursor_handles_cyrillic_prefix() {
        let line = "сказ";
        let (start, word) = word_before_cursor(line, line.len());
        assert_eq!(start, 0);
        assert_eq!(word, "сказ");
    }

    #[test]
    fn word_before_cursor_handles_empty_prefix() {
        let line = "сказать(";
        let (start, word) = word_before_cursor(line, line.len());
        assert_eq!(start, line.len());
        assert_eq!(word, "");
    }

    #[test]
    fn complete_candidates_finds_builtin() {
        let locals = BTreeSet::new();
        let candidates = complete_candidates("сказ", &locals);
        assert!(candidates.contains(&"сказать".to_string()));
    }

    #[test]
    fn complete_candidates_finds_keyword() {
        let locals = BTreeSet::new();
        let candidates = complete_candidates("гы", &locals);
        assert!(candidates.contains(&"гыы".to_string()));
    }

    #[test]
    fn complete_candidates_finds_local_declaration() {
        let mut locals = BTreeSet::new();
        locals.insert("мояПеременная".to_string());
        let candidates = complete_candidates("моя", &locals);
        assert_eq!(candidates, vec!["мояПеременная".to_string()]);
    }

    #[test]
    fn complete_candidates_empty_prefix_returns_everything_matching() {
        let locals = BTreeSet::new();
        let candidates = complete_candidates("", &locals);
        assert!(candidates.len() > 10);
    }

    #[test]
    fn extract_declared_idents_finds_var_and_const_and_fn() {
        let idents = extract_declared_idents("гыы а = 1;\nясенХуй б = 2;\nйопта в() {}\n");
        assert_eq!(idents, vec!["а".to_string(), "б".to_string(), "в".to_string()]);
    }

    #[test]
    fn extract_declared_idents_ignores_non_declarations() {
        let idents = extract_declared_idents("сказать(1);\n");
        assert!(idents.is_empty());
    }
}
