use crate::{
    Diagnostic,
    Span,
    Token,
    TokenKind,
    Severity,
    KeywordKind,
    OperatorKind,
    PunctuationKind
};

pub struct Lexer<'a> {
    source: &'a str,
    position: usize,
    diagnostics: Vec<Diagnostic>,
    char_indicies: std::str::CharIndices<'a>,
    peeked: Option<(usize, char)>,
}

impl<'a> Lexer<'a> {
    pub fn new(source: &'a str) -> Self {
        Self {
            source,
            position: 0,
            diagnostics: Vec::new(),
            char_indicies: source.char_indices(),
            peeked: None,
        }
    }

    pub fn next_token(&mut self) -> Token {
        self.skip_whitespace();

        let start = self.start_span();
        let Some((_, ch)) = self.peek_char() else {
            let span = self.span_from(start);
            return Token {
                kind: TokenKind::Eof,
                span,
            };
        };

        if ch.is_ascii_alphabetic() || ch == '_' {
            self.next_char();
            let (span, lexeme) = self.collect_identifier(start);
            let kind = Self::classify_identifier(lexeme);
            return Token { kind, span };
        }

        if ch.is_ascii_digit() {
            self.next_char();
            let (span, _) = self.collect_number(start);
            return Token {
                kind: TokenKind::Number,
                span,
            }
        }

        if ch == '"' {
            self.next_char();
            let (span, _value, terminated) = self.collect_string_literal(start);

            if !terminated {
                self.push_error(span, "незакрытая строка");
            }

            return Token {
                kind: TokenKind::StringLiteral,
                span
            }
        }

        if ch == '/' {
            self.next_char();
            if let Some((_, '/')) = self.peek_char() {
                self.peek_char();
                self.skip_line_comment();
                return self.next_token();
            } else {
                let span = self.span_from(start);
                self.push_error(span, "неподдержанный символ '/'");
                return Token { kind: TokenKind:: Unknown, span }
            }
        }

        let kind = match ch {
            '+' => {
                self.next_char();
                TokenKind::Operator(crate::OperatorKind::Plus)
            }
            '-' => {
                self.next_char();
                TokenKind::Operator(OperatorKind::Minus)
            }
            '*' => {
                self.next_char();
                TokenKind::Operator(OperatorKind::Multiply)
            }
            '%' => {
                self.next_char();
                TokenKind::Operator(OperatorKind::Modulo)
            }
            '(' => {
                self.next_char();
                TokenKind::Punctuation(PunctuationKind::LParen)
            }
            ')' => {
                self.next_char();
                TokenKind::Punctuation(PunctuationKind::RParen)
            }
            '{' => {
                self.next_char();
                TokenKind::Punctuation(PunctuationKind::LBrace)
            }
            '}' => {
                self.next_char();
                TokenKind::Punctuation(PunctuationKind::RBrace)
            }
            ',' => {
                self.next_char();
                TokenKind::Punctuation(PunctuationKind::Comma)
            }
            ';' => {
                self.next_char();
                TokenKind::Punctuation(PunctuationKind::Semicolon)
            }
            '=' => {
                self.next_char();
                if let Some((_, '=')) = self.peek_char() {
                    self.next_char();
                    if let Some((_, '=')) = self.peek_char() {
                        self.next_char();
                        return Token { kind: TokenKind::Operator(OperatorKind::StrictEquals), span: self.span_from(start) }
                    }
                    return Token { kind: TokenKind::Operator(OperatorKind::Equals), span: self.span_from(start) }
                } else {
                    return Token { kind: TokenKind::Operator(OperatorKind::Assign), span: self.span_from(start) }
                }
            }
            '!' => {
                self.next_char();
                if let Some((_, '=')) = self.peek_char() {
                    self.next_char();
                    if let Some((_, '=')) = self.peek_char() {
                        self.next_char();
                        TokenKind::Operator(OperatorKind::StrictNotEquals)
                    } else {
                        TokenKind::Operator(OperatorKind::NotEquals)
                    }
                } else {
                    TokenKind::Operator(OperatorKind::Not)
                }
            }
            '<' => {
                self.next_char();
                if let Some((_, '=')) = self.peek_char() {
                    self.next_char();
                    TokenKind::Operator(Operator::LessOrEqual)
                } else {
                    TokenKind::Operator(OperatorKind::Less)
                }
            }
            '>' => {
                self.next_char();
                if let Some((_, '=')) = self.peek_char() {
                    self.next_char();
                    TokenKind::Operator(Operator::GreaterOrEqual)
                } else {
                    TokenKind::Operator(OperatorKind::Greater)
                }
            }
            '&' => {
                self.next_char();
                if let Some((_, '&')) = self.peek_char() {
                    self.next_char();
                    TokenKind::Operator(OperatorKind::And)
                } else {
                    let span = self.span_from(start);
                    self.push_error(span, "одиночный '&' не поддерживается (используйте '&&')");
                    TokenKind::Unknown
                }
            }
            '|' => {
                self.next_char();
                if let Some((_, '|')) = self.peek_char() {
                    self.next_char();
                    TokenKind::Operator(OperatorKind::Or)
                } else {
                    let span = self.span_from(start);
                    self.push_error(span, "одиночный '|' не поддерживается (используйте '||')");
                    TokenKind::Unknown
                }
            }
            ';' => {
                self.next_char();
                TokenKind::Punctuation(PunctuationKind::Semicolon)
            }
            ',' => {
                self.next_char();
                TokenKind::Punctuation(PunctuationKind::Comma)
            }
            _ => {
                self.next_char();
                let span = self.span_from(start);
                self.push_error(span, format!("неизвестный символ: {}", ch));
                TokenKind::Unknown
            }
        };

        let span = self.span_from(start);
        Token { kind, span }
    }

    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    fn peek_char(&mut self) -> Option<(usize, char)> {
        if self.peeked.is_none() {
            self.peeked = self.char_indicies.next();
        }
        self.peeked
    }

    fn next_char(&mut self) -> Option<(usize, char)> {
        if let Some((idx, ch)) = self.peek_char() {
            self.peeked = None;
            self.position = idx + ch.len_utf8();
            Some((idx, ch))
        } else {
            None
        }
    }

    fn skip_whitespace(&mut self) {
        while let Some((_, ch)) = self.peek_char() {
            if ch.is_whitespace() {
                self.next_char();
            } else {
                break;
            }
        }
    }

    fn start_span(&self) -> usize {
        self.position
    }

    fn span_from(&self, start: usize) -> Span {
        Span { start, end: self.position }
    }

    fn collect_while<F>(&mut self, start: usize, mut predicate: F) -> (Span, &'a str)
    where
      F: FnMut(char) -> bool,
    {
      while let Some((_, ch)) = self.peek_char() {
          if predicate(ch) {
              self.next_char();
          } else {
              break;
          }
      }

      let end = self.position;
      let span = Span { start, end };
      let slice = &self.source[start..end];
      (span, slice)
    }

    fn push_error(&mut self, span: Span, message: impl Into<String>) {
      self.diagnostics.push(Diagnostic {
          severity: Severity::Error,
          message: message.into(),
          span,
      });
    }

    fn classify_identifier(lexeme: &str) -> TokenKind {
      match lexeme {
          "pachan" => TokenKind::Keyword(KeywordKind::Pachan),
          "sliva" => TokenKind::Keyword(KeywordKind::Sliva),
          _ => TokenKind::Identifier,
        }
    }

    fn collect_identifier(&mut self, start: usize) -> (Span, &'a str) {
        self.collect_while(start, |ch| ch.is_ascii_alphanumeric() || ch == '_')
    }

    fn collect_number(&mut self, start: usize) -> (Span, &'a str) {
        self.collect_while(start, |ch| ch.is_ascii_digit())
    }

    fn collect_string_literal(&mut self, start: usize) -> (Span, String, bool) {
        let mut value = String::new();
        let mut terminated = false;

        while let Some((_, ch)) = self.peek_char() {
            match ch {
                '"' => {
                    self.next_char();
                    terminated = true;
                    break;
                }
                '\\' => {
                    self.next_char();
                    match self.peek_char() {
                        Some((_, esc)) => {
                            self.next_char();
                            match esc {
                                '"' => value.push('"'),
                                '\\' => value.push('\\'),
                                'n' => value.push('\n'),
                                't' => value.push('\t'),
                                'r' => value.push('\r'),
                                other => {
                                    value.push(other);
                                    let span = self.span_from(start);
                                    self.push_error(span, format!("неизвестная escape-последовательность: \\{}", other));
                                }
                            }
                        }
                        None => break
                    }
                }
                _ => {
                    self.next_char();
                    value.push(ch);
                }
            }
        }
        let span = self.span_from(start);
        (span, value, terminated)
    }

    fn skip_line_comment(&mut self) {
        while let Some((_, ch)) = self.peek_char() {
            if ch == '\n' {
                break;
            }
            self.next_char();
        }
    }
}
