use crate::{Diagnostic, Span};

#[derive(Debug, Clone)]
pub struct SourceFile {
    pub name: String,
    pub source: String,
    base: usize,
}

impl SourceFile {
    #[must_use]
    pub const fn new(name: String, source: String) -> Self {
        Self::with_base(name, source, 0)
    }

    #[must_use]
    pub const fn with_base(name: String, source: String, base: usize) -> Self {
        Self { name, source, base }
    }

    #[must_use]
    pub const fn base(&self) -> usize {
        self.base
    }

    #[must_use]
    pub const fn contains(&self, offset: usize) -> bool {
        offset >= self.base && offset <= self.base + self.source.len()
    }

    #[must_use]
    pub fn slice(&self, span: Span) -> &str {
        &self.source[span.start - self.base..span.end - self.base]
    }

    #[must_use]
    pub fn describe(&self, diagnostic: &Diagnostic) -> String {
        let (line, col) = self.position(diagnostic.span.start);
        format!("{}:{line}:{col}: {}: {}", self.name, diagnostic.severity, diagnostic.message)
    }

    #[must_use]
    pub fn position(&self, offset: usize) -> (usize, usize) {
        let offset = offset.saturating_sub(self.base);
        let mut line = 1;
        let mut col = 1;

        for (byte_pos, ch) in self.source.char_indices() {
            if byte_pos >= offset {
                break;
            }

            if ch == '\n' {
                line += 1;
                col = 1;
            } else {
                col += 1;
            }
        }
        (line, col)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Severity;

    #[test]
    fn test_source_file_slice_keyword() {
        let source = SourceFile::new("test.yopta".into(), "pachan x = 228;".into());
        let span = Span { start: 0, end: 6 };

        let result = source.slice(span);

        assert_eq!(result, "pachan");
    }

    #[test]
    fn test_source_file_slice_identifier() {
        let source = SourceFile::new("test.yopta".into(), "pachan x = 228;".into());
        let span = Span { start: 7, end: 8 };

        let result = source.slice(span);

        assert_eq!(result, "x");
    }

    #[test]
    fn test_source_file_slice_number() {
        let source = SourceFile::new("test.yopta".into(), "pachan x = 228;".into());
        let span = Span { start: 11, end: 14 };

        let result = source.slice(span);

        assert_eq!(result, "228");
    }

    #[test]
    fn test_source_file_slice_unicode() {
        let source = SourceFile::new("test.yopta".into(), "пацан x = 5;".into());
        let span = Span { start: 0, end: 10 };

        let result = source.slice(span);

        assert_eq!(result, "пацан");
    }

    #[test]
    fn test_source_file_position_start_of_file() {
        let source = SourceFile::new("test.yopta".into(), "line1\nline2\nline3".into());

        let (line, col) = source.position(0);

        assert_eq!(line, 1);
        assert_eq!(col, 1);
    }

    #[test]
    fn test_source_file_position_middle_of_first_line() {
        let source = SourceFile::new("test.yopta".into(), "line1\nline2\nline3".into());

        let (line, col) = source.position(3);

        assert_eq!(line, 1);
        assert_eq!(col, 4);
    }

    #[test]
    fn test_source_file_position_start_of_second_line() {
        let source = SourceFile::new("test.yopta".into(), "line1\nline2\nline3".into());

        let (line, col) = source.position(6);

        assert_eq!(line, 2);
        assert_eq!(col, 1);
    }

    #[test]
    fn test_source_file_position_middle_of_second_line() {
        let source = SourceFile::new("test.yopta".into(), "line1\nline2\nline3".into());

        let (line, col) = source.position(9);

        assert_eq!(line, 2);
        assert_eq!(col, 4);
    }

    #[test]
    fn test_source_file_position_start_of_third_line() {
        let source = SourceFile::new("test.yopta".into(), "line1\nline2\nline3".into());

        let (line, col) = source.position(12);

        assert_eq!(line, 3);
        assert_eq!(col, 1);
    }

    #[test]
    fn test_source_file_empty_file_position() {
        let source = SourceFile::new("empty.yopta".into(), String::new());

        let (line, col) = source.position(0);

        assert_eq!(line, 1);
        assert_eq!(col, 1);
    }

    #[test]
    fn based_source_slices_and_positions_with_global_offsets() {
        let source = SourceFile::with_base("mod.yopta".into(), "line1\nline2".into(), 100);

        assert_eq!(source.base(), 100);
        assert_eq!(source.slice(Span { start: 106, end: 111 }), "line2");
        assert_eq!(source.position(106), (2, 1));
        assert_eq!(source.position(100), (1, 1));
    }

    #[test]
    fn based_source_contains_only_its_own_offsets() {
        let source = SourceFile::with_base("mod.yopta".into(), "abc".into(), 10);

        assert!(!source.contains(9));
        assert!(source.contains(10));
        assert!(source.contains(13));
        assert!(!source.contains(14));
    }

    #[test]
    fn plain_source_has_zero_base() {
        let source = SourceFile::new("a.yopta".into(), "abc".into());

        assert_eq!(source.base(), 0);
        assert!(source.contains(0));
    }

    #[test]
    fn describe_renders_name_position_severity_and_message() {
        let source = SourceFile::with_base("mod.yopta".into(), "раз\nдва".into(), 50);
        let diagnostic =
            Diagnostic { severity: Severity::Error, message: "Плохо".into(), span: Span { start: 57, end: 58 } };

        assert_eq!(source.describe(&diagnostic), "mod.yopta:2:1: Ошибка: Плохо");
    }
}
