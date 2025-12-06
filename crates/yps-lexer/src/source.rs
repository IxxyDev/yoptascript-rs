use crate::Span;

#[derive(Debug, Clone)]
pub struct SourceFile {
    pub name: String,
    pub source: String,
}

impl SourceFile {
    #[must_use]
    pub const fn new(name: String, source: String) -> Self {
        Self { name, source }
    }

    #[must_use]
    pub fn slice(&self, span: Span) -> &str {
        &self.source[span.start..span.end]
    }

    #[must_use]
    pub fn position(&self, offset: usize) -> (usize, usize) {
        let mut line = 1;
        let mut col = 1;

        for (i, ch) in self.source.chars().enumerate() {
            if i >= offset {
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

    #[must_use]
    pub fn get_line(&self, line_num: usize) -> Option<&str> {
        self.source.lines().nth(line_num.saturating_sub(1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_source_file_slice_keyword() {
        let source = SourceFile::new("test.yop".into(), "pachan x = 228;".into());
        let span = Span { start: 0, end: 6 };

        let result = source.slice(span);

        assert_eq!(result, "pachan");
    }

    #[test]
    fn test_source_file_slice_identifier() {
        let source = SourceFile::new("test.yop".into(), "pachan x = 228;".into());
        let span = Span { start: 7, end: 8 };

        let result = source.slice(span);

        assert_eq!(result, "x");
    }

    #[test]
    fn test_source_file_slice_number() {
        let source = SourceFile::new("test.yop".into(), "pachan x = 228;".into());
        let span = Span { start: 11, end: 14 };

        let result = source.slice(span);

        assert_eq!(result, "228");
    }

    #[test]
    fn test_source_file_slice_unicode() {
        let source = SourceFile::new("test.yop".into(), "пацан x = 5;".into());
        let span = Span { start: 0, end: 10 };

        let result = source.slice(span);

        assert_eq!(result, "пацан");
    }

    #[test]
    fn test_source_file_position_start_of_file() {
        let source = SourceFile::new("test.yop".into(), "line1\nline2\nline3".into());

        let (line, col) = source.position(0);

        assert_eq!(line, 1);
        assert_eq!(col, 1);
    }

    #[test]
    fn test_source_file_position_middle_of_first_line() {
        let source = SourceFile::new("test.yop".into(), "line1\nline2\nline3".into());

        let (line, col) = source.position(3);

        assert_eq!(line, 1);
        assert_eq!(col, 4);
    }

    #[test]
    fn test_source_file_position_start_of_second_line() {
        let source = SourceFile::new("test.yop".into(), "line1\nline2\nline3".into());

        let (line, col) = source.position(6);

        assert_eq!(line, 2);
        assert_eq!(col, 1);
    }

    #[test]
    fn test_source_file_position_middle_of_second_line() {
        let source = SourceFile::new("test.yop".into(), "line1\nline2\nline3".into());

        let (line, col) = source.position(9);

        assert_eq!(line, 2);
        assert_eq!(col, 4);
    }

    #[test]
    fn test_source_file_position_start_of_third_line() {
        let source = SourceFile::new("test.yop".into(), "line1\nline2\nline3".into());

        let (line, col) = source.position(12);

        assert_eq!(line, 3);
        assert_eq!(col, 1);
    }

    #[test]
    fn test_source_file_get_line_first() {
        let source = SourceFile::new("test.yop".into(), "line1\nline2\nline3".into());

        let result = source.get_line(1);

        assert_eq!(result, Some("line1"));
    }

    #[test]
    fn test_source_file_get_line_second() {
        let source = SourceFile::new("test.yop".into(), "line1\nline2\nline3".into());

        let result = source.get_line(2);

        assert_eq!(result, Some("line2"));
    }

    #[test]
    fn test_source_file_get_line_third() {
        let source = SourceFile::new("test.yop".into(), "line1\nline2\nline3".into());

        let result = source.get_line(3);

        assert_eq!(result, Some("line3"));
    }

    #[test]
    fn test_source_file_get_line_nonexistent() {
        let source = SourceFile::new("test.yop".into(), "line1\nline2\nline3".into());

        let result = source.get_line(4);

        assert_eq!(result, None);
    }

    #[test]
    fn test_source_file_get_line_out_of_bounds() {
        let source = SourceFile::new("test.yop".into(), "line1\nline2\nline3".into());

        let result = source.get_line(100);

        assert_eq!(result, None);
    }

    #[test]
    fn test_source_file_empty_file_slice() {
        let source = SourceFile::new("empty.yop".into(), String::new());
        let span = Span { start: 0, end: 0 };

        let result = source.slice(span);

        assert_eq!(result, "");
    }

    #[test]
    fn test_source_file_empty_file_position() {
        let source = SourceFile::new("empty.yop".into(), String::new());

        let (line, col) = source.position(0);

        assert_eq!(line, 1);
        assert_eq!(col, 1);
    }
}
