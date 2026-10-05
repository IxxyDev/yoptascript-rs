use yps_lexer::SourceFile;

pub struct LineIndex {
    base: usize,
    starts: Vec<usize>,
}

impl LineIndex {
    #[must_use]
    pub fn new(source: &SourceFile) -> Self {
        let mut starts = vec![0];
        starts.extend(source.source.match_indices('\n').map(|(index, _)| index + 1));
        Self { base: source.base(), starts }
    }

    #[must_use]
    pub fn line(&self, offset: usize) -> usize {
        let offset = offset.saturating_sub(self.base);
        self.starts.partition_point(|&start| start <= offset)
    }

    #[must_use]
    pub fn position(&self, source: &SourceFile, offset: usize) -> (usize, usize) {
        let line = self.line(offset);
        let start = self.starts[line - 1];
        let offset = offset.saturating_sub(self.base);
        let column = source.source[start..]
            .char_indices()
            .take_while(|(index, _)| start + index < offset)
            .map(|(_, ch)| ch.len_utf16())
            .sum::<usize>()
            + 1;
        (line, column)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_matches_oracle(source: &SourceFile) {
        let index = LineIndex::new(source);
        let base = source.base();
        for offset in 0..=source.source.len() + 3 {
            let global = base + offset;
            if offset <= source.source.len() && !source.source.is_char_boundary(offset) {
                continue;
            }
            let expected = source.position(global);
            assert_eq!(index.position(source, global), expected, "offset {offset}");
            assert_eq!(index.line(global), expected.0, "line at offset {offset}");
        }
    }

    #[test]
    fn matches_source_file_position_for_multibyte_text() {
        let source = SourceFile::new("t.yopta".into(), "гыы а = 1;\nсказать(\"привет\");\n\nх".into());

        assert_matches_oracle(&source);
    }

    #[test]
    fn matches_source_file_position_with_base() {
        let source = SourceFile::with_base("t.yopta".into(), "раз\nдва\nтри".into(), 100);

        assert_matches_oracle(&source);
    }

    #[test]
    fn covers_first_line_last_line_newline_and_past_end() {
        let source = SourceFile::new("t.yopta".into(), "ab\ncd\n".into());
        let index = LineIndex::new(&source);

        assert_eq!(index.position(&source, 0), (1, 1));
        assert_eq!(index.position(&source, 2), (1, 3));
        assert_eq!(index.position(&source, 3), (2, 1));
        assert_eq!(index.position(&source, 6), (3, 1));
        assert_eq!(index.position(&source, 50), source.position(50));
        assert_eq!(index.line(2), 1);
        assert_eq!(index.line(3), 2);
    }

    #[test]
    fn columns_are_counted_in_utf16_code_units() {
        let source = SourceFile::new("t.yopta".into(), "\"😀\"; х".into());
        let index = LineIndex::new(&source);
        let offset = source.source.find('х').unwrap();

        assert_eq!(index.position(&source, offset), (1, 7));
    }

    #[test]
    fn empty_source_is_a_single_line() {
        let source = SourceFile::new("t.yopta".into(), String::new());
        let index = LineIndex::new(&source);

        assert_eq!(index.position(&source, 0), (1, 1));
        assert_eq!(index.line(10), 1);
    }
}
