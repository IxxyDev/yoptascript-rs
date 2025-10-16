#[allow(dead_code)]
pub struct Lexer<'src> {
    source: &'src SourceFile,
    position: usize,
    diagnostics: Vec<Diagnostic>,
}

#[allow(dead_code)]
impl<'src> Lexer<'src> {
    #[must_use]
    pub const fn new(source: &'src SourceFile) -> Self {
        Self { source, position: 0, diagnostics: Vec::new() }
    }

    fn current_char(&self) -> char {
        self.source.source[self.position..].chars().next().unwrap_or('\0')
    }

    fn peek_char(&self, offset: usize) -> char {
        let mut chars = self.source.source[self.position..].chars();

        for _ in 0..offset {
            chars.next();
        }

        chars.next().unwrap_or('\0')
    }

    fn advance(&mut self) -> char {
        let ch = self.current_char();

        if ch != '\0' {
            self.position += ch.len_utf8();
        }
        ch
    }

    #[must_use]
    const fn is_at_end(&self) -> bool {
        self.position >= self.source.source.len()
    }

    fn skip_whitespace(&mut self) {
        while matches!(self.current_char(), ' ' | '\t' | '\n' | '\r') {
            self.advance();
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub severity: Severity,
    pub message: String,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeywordKind {
    Pachan,
    Sliva,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperatorKind {
    Plus,
    Minus,
    Assign,
    Equals,
    StrictEquals,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PunctuationKind {
    LParen,
    RParen,
    LBrace,
    RBrace,
    Semicolon,
    Comma,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    Identifier,
    Number,
    StringLiteral,
    Keyword(KeywordKind),
    Operator(OperatorKind),
    Punctuation(PunctuationKind),
    Eof,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

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
