use yps_lexer::Span;

use crate::rename::occurrences_at;

#[must_use]
pub fn references(text: &str, byte_pos: usize, include_declaration: bool) -> Option<Vec<Span>> {
    let mut spans = occurrences_at(text, byte_pos)?;
    if !include_declaration && !spans.is_empty() {
        spans.remove(0);
    }
    Some(spans)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spans_texts<'a>(src: &'a str, spans: &[Span]) -> Vec<&'a str> {
        spans.iter().map(|s| &src[s.start..s.end]).collect()
    }

    #[test]
    fn finds_all_references_for_variable() {
        let src = "ясенХуй x = 1;\nсказать(x);\nсказать(x + 1);";
        let byte = src.rfind('x').unwrap();
        let spans = references(src, byte, true).expect("should collect references");
        assert_eq!(spans.len(), 3);
        for text in spans_texts(src, &spans) {
            assert_eq!(text, "x");
        }
    }

    #[test]
    fn excludes_declaration_when_requested() {
        let src = "ясенХуй x = 1;\nсказать(x);\nсказать(x + 1);";
        let decl = src.find('x').unwrap();
        let with_decl = references(src, decl, true).expect("with declaration");
        let without_decl = references(src, decl, false).expect("without declaration");
        assert_eq!(with_decl.len(), 3);
        assert_eq!(without_decl.len(), 2);
        assert!(without_decl.iter().all(|s| s.start != decl));
    }

    #[test]
    fn finds_all_references_for_parameter() {
        let src = "гыы арг = 99;\nйопта фу(арг) { отвечаю арг + 1; }\nсказать(арг);";
        let param = src.find("фу(арг)").unwrap() + "фу(".len();
        let spans = references(src, param, true).expect("should collect references");
        assert_eq!(spans.len(), 2);
        let without_decl = references(src, param, false).expect("without declaration");
        assert_eq!(without_decl.len(), 1);
        assert_eq!(&src[without_decl[0].start..without_decl[0].end], "арг");
    }

    #[test]
    fn shadowed_scopes_stay_isolated() {
        let src = "гыы x = 1;\nйопта фу() { гыы x = 2; отвечаю x; }\nсказать(x);";
        let outer = src.find('x').unwrap();
        let outer_spans = references(src, outer, true).expect("outer references");
        assert_eq!(outer_spans.len(), 2);
        let inner = src.find("гыы x = 2").unwrap() + "гыы ".len();
        let inner_spans = references(src, inner, true).expect("inner references");
        assert_eq!(inner_spans.len(), 2);
        assert!(outer_spans.iter().all(|o| inner_spans.iter().all(|i| i.start != o.start)));
    }

    #[test]
    fn references_on_keyword_returns_none() {
        let src = "гыы x = 1;";
        let byte = src.find("гыы").unwrap();
        assert!(references(src, byte, true).is_none());
    }

    #[test]
    fn references_on_builtin_returns_none() {
        let src = "сказать(1);";
        let byte = src.find("сказать").unwrap();
        assert!(references(src, byte, true).is_none());
    }

    #[test]
    fn member_property_is_not_confused_with_binding() {
        let src = "гыы длина = 1;\nсказать(массив.длина);\nсказать(длина);";
        let byte = src.rfind("длина").unwrap();
        let spans = references(src, byte, true).expect("should collect references");
        assert_eq!(spans.len(), 2);
        let member = src.find("массив.длина").unwrap() + "массив.".len();
        assert!(spans.iter().all(|s| s.start != member));
    }
}
