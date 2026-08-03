use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, NumberOrString};
use yps_lint::{LintDiagnostic, LintSeverity};

use crate::position::span_to_range;

pub const SOURCE: &str = "yps-lint";

#[must_use]
pub fn to_lsp_diagnostics(text: &str, lint_diags: &[LintDiagnostic]) -> Vec<Diagnostic> {
    lint_diags
        .iter()
        .map(|d| {
            let severity = match d.severity {
                LintSeverity::Warning => DiagnosticSeverity::WARNING,
                LintSeverity::Hint => DiagnosticSeverity::HINT,
            };
            Diagnostic {
                range: span_to_range(text, d.span),
                severity: Some(severity),
                code: Some(NumberOrString::String(d.rule.code().to_string())),
                source: Some(SOURCE.to_string()),
                message: d.message.clone(),
                ..Default::default()
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use yps_lint::lint_source;

    #[test]
    fn lint_diagnostic_carries_rule_code_and_source() {
        let src = "гыы х = 1;\n";
        let result = lint_source(src);
        let diags = to_lsp_diagnostics(src, &result.diagnostics);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].source.as_deref(), Some(SOURCE));
        assert_eq!(diags[0].code, Some(NumberOrString::String("unused-variable".to_string())));
        assert_eq!(diags[0].severity, Some(DiagnosticSeverity::WARNING));
    }

    #[test]
    fn shadowed_declaration_is_a_hint() {
        let src = "гыы х = 1;\nсказать(х);\nйопта ф() { гыы х = 2; сказать(х); }\n";
        let result = lint_source(src);
        let diags = to_lsp_diagnostics(src, &result.diagnostics);
        assert!(diags.iter().any(|d| d.severity == Some(DiagnosticSeverity::HINT)));
    }

    #[test]
    fn cyrillic_positions_use_utf16_columns() {
        let src = "йопта фу() { отвечаю 1; сказать(2); }\n";
        let result = lint_source(src);
        let diags = to_lsp_diagnostics(src, &result.diagnostics);
        let unreachable = diags.iter().find(|d| d.code == Some(NumberOrString::String("unreachable-code".to_string())));
        let diag = unreachable.expect("unreachable diagnostic");
        assert_eq!(diag.range.start.character, 24);
    }
}
