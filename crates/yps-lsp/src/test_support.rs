use yps_lexer::Span;

pub(crate) fn apply_edits(src: &str, edits: impl IntoIterator<Item = (Span, String)>) -> String {
    let mut edits: Vec<(Span, String)> = edits.into_iter().collect();
    edits.sort_by_key(|(span, _)| std::cmp::Reverse(span.start));
    let mut out = src.to_string();
    for (span, text) in edits {
        out.replace_range(span.start..span.end, &text);
    }
    out
}
