use tower_lsp::lsp_types::{Position, Range};
use yps_lexer::Span;

#[must_use]
pub fn span_to_range(src: &str, span: Span) -> Range {
    Range { start: byte_to_pos(src, span.start), end: byte_to_pos(src, span.end) }
}

fn is_ident_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn clamp_to_boundary(src: &str, byte_pos: usize) -> usize {
    let raw = byte_pos.min(src.len());
    (0..=raw).rev().find(|&i| src.is_char_boundary(i)).unwrap_or(0)
}

fn ident_start(src: &str, boundary: usize) -> usize {
    src[..boundary].char_indices().rev().take_while(|(_, c)| is_ident_char(*c)).last().map_or(boundary, |(i, _)| i)
}

#[must_use]
pub fn byte_to_pos(src: &str, offset: usize) -> Position {
    let clamped = clamp_to_boundary(src, offset);
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
    let clamped = clamp_to_boundary(src, byte_pos);
    let start = ident_start(src, clamped);
    let end = src[clamped..]
        .char_indices()
        .take_while(|(_, c)| is_ident_char(*c))
        .last()
        .map_or(clamped, |(i, c)| clamped + i + c.len_utf8());

    &src[start..end]
}

#[must_use]
pub fn member_receiver(src: &str, byte_pos: usize) -> Option<&str> {
    let clamped = clamp_to_boundary(src, byte_pos);
    let member_start = ident_start(src, clamped);

    let (dot_byte, dot_char) = src[..member_start].char_indices().next_back()?;
    if dot_char != '.' {
        return None;
    }

    let recv_end = dot_byte;
    let recv_start = ident_start(src, recv_end);

    let receiver = &src[recv_start..recv_end];
    if !receiver.is_empty() && receiver.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }

    Some(receiver)
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
        let src = "ясенХуй x = 1;\nсказать(x);";
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

    #[test]
    fn pos_to_byte_clamps_character_past_line_end() {
        let src = "abc";
        assert_eq!(pos_to_byte(src, Position { line: 0, character: 99 }), src.len());
    }

    #[test]
    fn pos_to_byte_clamps_line_past_end_of_file() {
        let src = "abc";
        assert_eq!(pos_to_byte(src, Position { line: 9, character: 0 }), src.len());
    }

    #[test]
    fn member_receiver_detects_namespace() {
        let src = "Матан.";
        assert_eq!(member_receiver(src, src.len()), Some("Матан"));
    }

    #[test]
    fn member_receiver_detects_partial_member() {
        let src = "Матан.по";
        assert_eq!(member_receiver(src, src.len()), Some("Матан"));
    }

    #[test]
    fn member_receiver_empty_for_non_ident_receiver() {
        let src = "вызов().";
        assert_eq!(member_receiver(src, src.len()), Some(""));
    }

    #[test]
    fn member_receiver_none_without_dot() {
        let src = "Матан";
        assert_eq!(member_receiver(src, src.len()), None);
    }

    #[test]
    fn member_receiver_none_for_numeric_literal() {
        let src = "3.14";
        let byte = src.find('.').unwrap() + 1;
        assert_eq!(member_receiver(src, byte), None);
    }

    #[test]
    fn member_receiver_keeps_identifier_ending_in_digit() {
        let src = "массив2.";
        assert_eq!(member_receiver(src, src.len()), Some("массив2"));
    }
}
