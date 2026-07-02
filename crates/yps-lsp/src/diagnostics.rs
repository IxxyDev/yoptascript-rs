use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity};
use yps_lexer::Severity;

use crate::position::span_to_range;

#[must_use]
pub fn to_lsp_diagnostics(
    text: &str,
    lex_diags: &[yps_lexer::Diagnostic],
    parse_diags: &[yps_lexer::Diagnostic],
) -> Vec<Diagnostic> {
    lex_diags
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
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_source_has_no_diagnostics() {
        let diags = crate::analyze("ясенХуй x = 1;\n").diagnostics;
        assert!(diags.is_empty(), "unexpected diagnostics: {diags:?}");
    }

    #[test]
    fn broken_source_reports_error() {
        let diags = crate::analyze("йопта (").diagnostics;
        assert!(diags.iter().any(|d| d.severity == Some(DiagnosticSeverity::ERROR)));
    }
}
