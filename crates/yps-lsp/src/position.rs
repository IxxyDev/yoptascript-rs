use tower_lsp::lsp_types::{Position, Range};
use yps_lexer::Span;

#[must_use]
pub fn span_to_range(src: &str, span: Span) -> Range {
    Range { start: byte_to_pos(src, span.start), end: byte_to_pos(src, span.end) }
}

#[must_use]
pub fn byte_to_pos(src: &str, offset: usize) -> Position {
    let raw = offset.min(src.len());
    let clamped = (0..=raw).rev().find(|&i| src.is_char_boundary(i)).unwrap_or(0);
    let prefix = &src[..clamped];
    let line = prefix.bytes().filter(|&b| b == b'\n').count() as u32;
    let line_start = prefix.rfind('\n').map_or(0, |i| i + 1);
    let character = prefix[line_start..].encode_utf16().count() as u32;
    Position { line, character }
}

#[must_use]
pub fn pos_to_byte(src: &str, pos: Position) -> usize {
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

#[must_use]
pub fn word_at(src: &str, byte_pos: usize) -> &str {
    let is_ident = |c: char| c.is_alphanumeric() || c == '_';
    let raw = byte_pos.min(src.len());
    let clamped = (0..=raw).rev().find(|&i| src.is_char_boundary(i)).unwrap_or(0);

    let start =
        src[..clamped].char_indices().rev().take_while(|(_, c)| is_ident(*c)).last().map_or(clamped, |(i, _)| i);

    let end = src[clamped..]
        .char_indices()
        .take_while(|(_, c)| is_ident(*c))
        .last()
        .map_or(clamped, |(i, c)| clamped + i + c.len_utf8());

    &src[start..end]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn byte_to_pos_handles_ascii_lines() {
        let src = "abc\ndef";
        assert_eq!(byte_to_pos(src, 0), Position { line: 0, character: 0 });
        assert_eq!(byte_to_pos(src, 4), Position { line: 1, character: 0 });
        assert_eq!(byte_to_pos(src, 6), Position { line: 1, character: 2 });
    }

    #[test]
    fn byte_to_pos_counts_utf16_units_for_cyrillic() {
        let src = "йопта foo";
        let byte = src.find("foo").unwrap();
        let pos = byte_to_pos(src, byte);
        assert_eq!(pos, Position { line: 0, character: 6 });
    }

    #[test]
    fn pos_to_byte_round_trips_cyrillic() {
        let src = "участковый x = 1;\nсказать(x);";
        for (byte, _) in src.char_indices() {
            let pos = byte_to_pos(src, byte);
            assert_eq!(pos_to_byte(src, pos), byte, "round-trip failed at byte {byte}");
        }
    }

    #[test]
    fn word_at_extracts_cyrillic_identifier() {
        let src = "сказать(привет)";
        let byte = src.find("привет").unwrap();
        assert_eq!(word_at(src, byte), "привет");
        assert_eq!(word_at(src, byte + "привет".len() - 2), "привет");
    }

    #[test]
    fn word_at_empty_on_punctuation() {
        let src = "a + b";
        let byte = src.find('+').unwrap();
        assert_eq!(word_at(src, byte), "");
    }
}
