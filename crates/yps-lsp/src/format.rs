use tower_lsp::lsp_types::{Position, Range, TextEdit};

use crate::position::byte_to_pos;

#[must_use]
pub fn format_document(text: &str) -> Option<Vec<TextEdit>> {
    let outcome = yps_fmt::format_source(text).ok()?;
    if outcome.already_formatted {
        return Some(Vec::new());
    }
    let range = Range { start: Position { line: 0, character: 0 }, end: byte_to_pos(text, text.len()) };
    Some(vec![TextEdit { range, new_text: outcome.text }])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unformatted_source_yields_edit() {
        let src = "ясенХуй    x=1;";
        let edits = format_document(src).expect("should format");
        assert_eq!(edits.len(), 1);
        assert!(edits[0].new_text.contains("ясенХуй x = 1;"));
    }

    #[test]
    fn already_canonical_yields_no_edit() {
        let src = format_document("ясенХуй    x=1;").unwrap()[0].new_text.clone();
        let edits = format_document(&src).expect("should format");
        assert!(edits.is_empty(), "expected no edits, got {edits:?}");
    }

    #[test]
    fn unparsable_source_returns_none() {
        assert!(format_document("йопта (").is_none());
    }

    #[test]
    fn edit_replaces_whole_document() {
        let src = "гыы   y =2;\n";
        let edits = format_document(src).expect("should format");
        let range = edits[0].range;
        assert_eq!(range.start, Position { line: 0, character: 0 });
        assert_eq!(range.end, byte_to_pos(src, src.len()));
    }
}
