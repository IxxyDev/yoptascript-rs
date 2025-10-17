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

    #[must_use]
    pub fn tokenize(mut self) -> (Vec<Token>, Vec<Diagnostic>) {
        let mut tokens = Vec::new();

        loop {
            let token = self.next_token();
            let is_eof = matches!(token.kind, TokenKind::Eof);
            tokens.push(token);

            if is_eof {
                break;
            }
        }

        (tokens, self.diagnostics)
    }

    fn next_token(&mut self) -> Token {
        self.skip_whitespace();

        let start = self.position;

        if self.is_at_end() {
            return Token { kind: TokenKind::Eof, span: Span { start, end: start } };
        }

        let ch = self.current_char();

        if ch.is_alphabetic() || ch == '_' {
            return self.read_identifier();
        }

        if ch.is_ascii_digit() {
            return self.read_number();
        }

        if ch == '"' || ch == '\'' {
            return self.read_string();
        }

        self.read_operator_or_punctuation()
    }

    fn read_identifier(&mut self) -> Token {
        let start = self.position;

        while self.current_char().is_alphanumeric() || self.current_char() == '_' {
            self.advance();
        }

        let end = self.position;
        let span = Span { start, end };
        let text = self.source.slice(span);

        let kind = match text {
            "pachan" => TokenKind::Keyword(KeywordKind::Pachan),
            "sliva" => TokenKind::Keyword(KeywordKind::Sliva),
            _ => TokenKind::Identifier,
        };

        Token { kind, span }
    }

    fn read_string(&mut self) -> Token {
        let start = self.position;
        let quote = self.advance();

        while !self.is_at_end() && self.current_char() != quote {
            if self.current_char() == '\\' {
                self.advance();
                if !self.is_at_end() {
                    self.advance();
                }
            } else {
                self.advance();
            }
        }

        if self.is_at_end() {
            self.diagnostics.push(Diagnostic {
                severity: Severity::Error,
                message: "Незакрытая строка".into(),
                span: Span { start, end: self.position },
            });
        } else {
            self.advance();
        }

        let end = self.position;
        Token { kind: TokenKind::StringLiteral, span: Span { start, end } }
    }

    fn read_number(&mut self) -> Token {
        let start = self.position;

        while self.current_char().is_ascii_digit() {
            self.advance();
        }

        if self.current_char() == '.' && self.peek_char(1).is_ascii_digit() {
            self.advance();

            while self.current_char().is_ascii_digit() {
                self.advance();
            }
        }

        let end = self.position;
        Token { kind: TokenKind::Number, span: Span { start, end } }
    }

    fn read_operator_or_punctuation(&mut self) -> Token {
        let start = self.position;
        let ch = self.advance();

        let kind = match ch {
            '+' => TokenKind::Operator(OperatorKind::Plus),
            '-' => TokenKind::Operator(OperatorKind::Minus),
            '=' => {
                if self.current_char() == '=' {
                    self.advance();

                    if self.current_char() == '=' {
                        self.advance();
                        TokenKind::Operator(OperatorKind::StrictEquals)
                    } else {
                        TokenKind::Operator(OperatorKind::Equals)
                    }
                } else {
                    TokenKind::Operator(OperatorKind::Assign)
                }
            }
            '(' => TokenKind::Punctuation(PunctuationKind::LParen),
            ')' => TokenKind::Punctuation(PunctuationKind::RParen),
            '{' => TokenKind::Punctuation(PunctuationKind::LBrace),
            '}' => TokenKind::Punctuation(PunctuationKind::RBrace),
            ';' => TokenKind::Punctuation(PunctuationKind::Semicolon),
            ',' => TokenKind::Punctuation(PunctuationKind::Comma),
            _ => {
                self.diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    message: format!("Неизвестный символ: '{ch}'"),
                    span: Span { start, end: self.position },
                });
                TokenKind::Unknown
            }
        };

        Token { kind, span: Span { start, end: self.position } }
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
