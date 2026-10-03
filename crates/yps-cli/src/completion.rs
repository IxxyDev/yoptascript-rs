use std::collections::BTreeSet;

use rustyline::completion::Completer;
use rustyline::error::ReadlineError;
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::validate::Validator;
use rustyline::{Context, Helper};

use yps_interpreter::builtins::builtin_names;
use yps_lexer::KEYWORDS;
use yps_parser::{ExportKind, ImportSpec, Pattern, Program, Stmt};

fn is_word_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

fn word_before_cursor(line: &str, pos: usize) -> (usize, &str) {
    let start =
        line[..pos].char_indices().rev().take_while(|(_, ch)| is_word_char(*ch)).last().map_or(pos, |(idx, _)| idx);
    (start, &line[start..pos])
}

fn owner_before(line: &str, word_start: usize) -> Option<&str> {
    let before_dot = line[..word_start].strip_suffix('.')?;
    let (_, owner) = word_before_cursor(before_dot, before_dot.len());
    (!owner.is_empty()).then_some(owner)
}

fn complete_candidates(owner: Option<&str>, prefix: &str, locals: &BTreeSet<String>) -> Vec<String> {
    let candidates: BTreeSet<&str> = match owner {
        Some(owner) => builtin_names()
            .iter()
            .filter_map(|name| name.strip_prefix(owner)?.strip_prefix('.'))
            .filter(|member| member.starts_with(prefix))
            .collect(),
        None => KEYWORDS
            .iter()
            .chain(builtin_names())
            .copied()
            .chain(locals.iter().map(String::as_str))
            .filter(|name| name.starts_with(prefix))
            .collect(),
    };
    candidates.into_iter().map(str::to_string).collect()
}

pub(crate) fn declared_names(program: &Program) -> Vec<String> {
    let mut names = Vec::new();
    for stmt in &program.items {
        collect_declared(stmt, &mut names);
    }
    names
}

fn collect_declared(stmt: &Stmt, names: &mut Vec<String>) {
    match stmt {
        Stmt::VarDecl { pattern, .. } => collect_bound(pattern, names),
        Stmt::FunctionDecl { name, .. } | Stmt::ClassDecl { name, .. } | Stmt::Using { name, .. } => {
            names.push(name.name.clone());
        }
        Stmt::Import { specifiers, .. } => {
            for specifier in specifiers {
                let (ImportSpec::Default { local } | ImportSpec::Named { local, .. } | ImportSpec::Namespace { local }) =
                    specifier;
                names.push(local.name.clone());
            }
        }
        Stmt::Export { kind: ExportKind::Declaration(inner), .. } => collect_declared(inner, names),
        _ => {}
    }
}

fn collect_bound(pattern: &Pattern, names: &mut Vec<String>) {
    match pattern {
        Pattern::Identifier(ident) => names.push(ident.name.clone()),
        Pattern::Array { elements, rest, .. } => {
            for element in elements.iter().flatten().chain(rest.as_deref()) {
                collect_bound(element, names);
            }
        }
        Pattern::Object { properties, rest, .. } => {
            for property in properties {
                match &property.value {
                    Some(value) => collect_bound(value, names),
                    None => names.push(property.key.name.clone()),
                }
            }
            if let Some(rest) = rest {
                collect_bound(rest, names);
            }
        }
        Pattern::Default { pattern, .. } => collect_bound(pattern, names),
    }
}

#[derive(Default)]
pub(crate) struct YpsHelper {
    locals: BTreeSet<String>,
}

impl YpsHelper {
    pub(crate) fn record_declarations(&mut self, names: Vec<String>) {
        self.locals.extend(names);
    }

    pub(crate) fn reset_locals(&mut self) {
        self.locals.clear();
    }
}

impl Completer for YpsHelper {
    type Candidate = String;

    fn complete(&self, line: &str, pos: usize, _ctx: &Context<'_>) -> Result<(usize, Vec<String>), ReadlineError> {
        let (start, prefix) = word_before_cursor(line, pos);
        let candidates = complete_candidates(owner_before(line, start), prefix, &self.locals);
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
    use rustyline::history::DefaultHistory;
    use yps_lexer::{Lexer, SourceFile};
    use yps_parser::Parser;

    use super::*;

    fn declared(code: &str) -> Vec<String> {
        let source = SourceFile::new("<тест>".to_string(), code.to_string());
        let (tokens, lex_diagnostics) = Lexer::new(&source).tokenize();
        assert!(lex_diagnostics.is_empty(), "{lex_diagnostics:?}");
        let (program, parse_diagnostics) = Parser::new(&tokens, &source).parse_program();
        assert!(parse_diagnostics.is_empty(), "{parse_diagnostics:?}");
        declared_names(&program)
    }

    #[test]
    fn word_before_cursor_handles_cyrillic_prefix() {
        let line = "сказ";

        assert_eq!(word_before_cursor(line, line.len()), (0, "сказ"));
    }

    #[test]
    fn word_before_cursor_handles_empty_prefix() {
        let line = "сказать(";

        assert_eq!(word_before_cursor(line, line.len()), (line.len(), ""));
    }

    #[test]
    fn word_before_cursor_stops_at_a_dot() {
        let line = "сказать.ош";

        assert_eq!(word_before_cursor(line, line.len()), ("сказать.".len(), "ош"));
    }

    #[test]
    fn owner_is_the_word_in_front_of_the_dot() {
        let line = "гыы х = сказать.ош";
        let (start, _) = word_before_cursor(line, line.len());

        assert_eq!(owner_before(line, start), Some("сказать"));
    }

    #[test]
    fn a_plain_word_has_no_owner() {
        assert_eq!(owner_before("сказ", 0), None);
        assert_eq!(owner_before("а + .х", "а + .".len()), None);
    }

    #[test]
    fn complete_candidates_finds_builtin() {
        let candidates = complete_candidates(None, "сказ", &BTreeSet::new());

        assert!(candidates.contains(&"сказать".to_string()));
    }

    #[test]
    fn complete_candidates_finds_keyword() {
        let candidates = complete_candidates(None, "гы", &BTreeSet::new());

        assert!(candidates.contains(&"гыы".to_string()));
    }

    #[test]
    fn complete_candidates_finds_local_declaration() {
        let locals = BTreeSet::from(["мояПеременная".to_string()]);

        assert_eq!(complete_candidates(None, "моя", &locals), ["мояПеременная"]);
    }

    #[test]
    fn complete_candidates_empty_prefix_returns_everything_matching() {
        let candidates = complete_candidates(None, "", &BTreeSet::new());

        assert!(candidates.len() > 10);
    }

    #[test]
    fn members_of_a_dotted_builtin_complete_after_the_dot() {
        assert_eq!(complete_candidates(Some("сказать"), "ош", &BTreeSet::new()), ["ошибка"]);
    }

    #[test]
    fn an_empty_member_prefix_lists_every_member_and_nothing_else() {
        let candidates = complete_candidates(Some("сказать"), "", &BTreeSet::from(["локальная".to_string()]));

        assert!(candidates.contains(&"ошибка".to_string()), "{candidates:?}");
        assert!(candidates.contains(&"таблица".to_string()), "{candidates:?}");
        assert!(!candidates.contains(&"локальная".to_string()), "{candidates:?}");
        assert!(!candidates.contains(&"гыы".to_string()), "{candidates:?}");
    }

    #[test]
    fn an_unknown_owner_offers_no_members() {
        assert!(complete_candidates(Some("неизвестное"), "", &BTreeSet::new()).is_empty());
    }

    #[test]
    fn helper_completes_a_member_through_the_rustyline_interface() {
        let helper = YpsHelper::default();
        let history = DefaultHistory::new();
        let line = "сказать.ош";

        let (start, candidates) = helper.complete(line, line.len(), &Context::new(&history)).unwrap();

        assert_eq!(start, "сказать.".len());
        assert_eq!(candidates, ["ошибка"]);
    }

    #[test]
    fn helper_offers_recorded_declarations_until_reset() {
        let mut helper = YpsHelper::default();
        let history = DefaultHistory::new();
        helper.record_declarations(vec!["мояПеременная".to_string()]);

        let (_, candidates) = helper.complete("моя", "моя".len(), &Context::new(&history)).unwrap();
        assert_eq!(candidates, ["мояПеременная"]);

        helper.reset_locals();
        let (_, candidates) = helper.complete("моя", "моя".len(), &Context::new(&history)).unwrap();
        assert!(candidates.is_empty());
    }

    #[test]
    fn declared_names_cover_variables_constants_functions_and_classes() {
        assert_eq!(declared("гыы а = 1;\nясенХуй б = 2;\nйопта в() {}\nклёво Г {}\n"), ["а", "б", "в", "Г"]);
    }

    #[test]
    fn declared_names_ignore_non_declarations() {
        assert!(declared("сказать(1);\n").is_empty());
    }

    #[test]
    fn declared_names_skip_declarations_nested_in_bodies() {
        assert_eq!(declared("йопта ф() { гыы внутри = 1; }\nвилкойвглаз (правда) { гыы вБлоке = 2; }\n"), ["ф"]);
    }

    #[test]
    fn declared_names_unpack_destructuring_patterns() {
        let names = declared("гыы { а, б: в, ...г } = {};\nгыы [д, , е = 1, ...ж] = [];\n");

        assert_eq!(names, ["а", "в", "г", "д", "е", "ж"]);
    }

    #[test]
    fn declared_names_include_imports_and_exported_declarations() {
        assert_eq!(declared("спиздить { х } из \"./м\";\nпредъява гыы э = 1;\n"), ["х", "э"]);
    }
}
