use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity};
use yps_lexer::{Lexer, Severity, SourceFile};
use yps_parser::Parser;

use crate::position::span_to_range;

#[must_use]
pub fn analyze(uri: &str, text: &str) -> Vec<Diagnostic> {
    let sf = SourceFile::new(uri.to_string(), text.to_string());
    let lexer = Lexer::new(&sf);
    let (tokens, lex_diags) = lexer.tokenize();

    let parser = Parser::new(&tokens, &sf);
    let (_, parse_diags) = parser.parse_program();

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
        let diags = analyze("file:///a.yop", "участковый x = 1;\n");
        assert!(diags.is_empty(), "unexpected diagnostics: {diags:?}");
    }

    #[test]
    fn broken_source_reports_error() {
        let diags = analyze("file:///a.yop", "йопта (");
        assert!(diags.iter().any(|d| d.severity == Some(DiagnosticSeverity::ERROR)));
    }
}
