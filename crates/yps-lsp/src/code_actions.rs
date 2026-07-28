use tower_lsp::lsp_types::TextEdit;
use yps_lint::{LintDiagnostic, Rule};

use crate::position::span_to_range;
use crate::rename::occurrences_at;

#[must_use]
pub fn quick_fix(text: &str, diag: &LintDiagnostic) -> Option<(String, Vec<TextEdit>)> {
    match diag.rule {
        Rule::UnusedVariable => unused_variable_fix(text, diag),
        Rule::UnreachableCode => Some(unreachable_code_fix(text, diag)),
        Rule::ShadowedDeclaration => None,
    }
}

fn unused_variable_fix(text: &str, diag: &LintDiagnostic) -> Option<(String, Vec<TextEdit>)> {
    let original = &text[diag.span.start..diag.span.end];
    if original.starts_with('_') {
        return None;
    }
    let new_name = format!("_{original}");

    let spans = occurrences_at(text, diag.span.start).unwrap_or_else(|| vec![diag.span]);
    let edits: Vec<TextEdit> = spans
        .into_iter()
        .map(|span| TextEdit { range: span_to_range(text, span), new_text: new_name.clone() })
        .collect();

    Some((format!("Переименовать «{original}» в «{new_name}»"), edits))
}

fn unreachable_code_fix(text: &str, diag: &LintDiagnostic) -> (String, Vec<TextEdit>) {
    let edit = TextEdit { range: span_to_range(text, diag.span), new_text: String::new() };
    ("Удалить недостижимый код".to_string(), vec![edit])
}

#[cfg(test)]
mod tests {
    use super::*;
    use yps_lint::lint_source;

    fn only_diag(src: &str, rule: Rule) -> LintDiagnostic {
        let result = lint_source(src);
        result.diagnostics.into_iter().find(|d| d.rule == rule).expect("ожидалась диагностика")
    }

    #[test]
    fn unused_variable_fix_renames_declaration_with_underscore_prefix() {
        let src = "гыы х = 1;\n";
        let diag = only_diag(src, Rule::UnusedVariable);
        let (title, edits) = quick_fix(src, &diag).expect("должен быть quick fix");
        assert!(title.contains('_'));
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].new_text, "_х");
    }

    #[test]
    fn unused_variable_fix_renames_every_occurrence_including_writes() {
        let src = "гыы х = 1;\nх = 2;\n";
        let diag = only_diag(src, Rule::UnusedVariable);
        let (_, edits) = quick_fix(src, &diag).expect("должен быть quick fix");
        assert_eq!(edits.len(), 2);
        assert!(edits.iter().all(|e| e.new_text == "_х"));
    }

    #[test]
    fn unused_variable_fix_does_not_double_prefix() {
        let src = "гыы _х = 1;\n";
        let start = src.find("_х").unwrap();
        let diag = LintDiagnostic {
            span: yps_lexer::Span { start, end: start + "_х".len() },
            rule: Rule::UnusedVariable,
            severity: yps_lint::LintSeverity::Warning,
            message: String::new(),
        };
        assert!(quick_fix(src, &diag).is_none());
    }

    #[test]
    fn unreachable_code_fix_deletes_the_statement() {
        let src = "йопта ф() { отвечаю 1; сказать(2); }\n";
        let diag = only_diag(src, Rule::UnreachableCode);
        let (title, edits) = quick_fix(src, &diag).expect("должен быть quick fix");
        assert!(title.contains("Удалить"));
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].new_text, "");
        assert_eq!(&src[diag.span.start..diag.span.end], "сказать(2);");
    }

    #[test]
    fn shadowed_declaration_has_no_quick_fix() {
        let src = "гыы х = 1;\nсказать(х);\nйопта ф() { гыы х = 2; сказать(х); }\n";
        let diag = only_diag(src, Rule::ShadowedDeclaration);
        assert!(quick_fix(src, &diag).is_none());
    }
}
