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

        let kind = match ch {
            '+' => {
                self.next_char();
                TokenKind::Operator(crate::OperatorKind::Plus)
            }
            '-' => {
                self.next_char();
                TokenKind::Operator(OperatorKind::Minus)
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
        if let Some(peeked) = self.peeked {
            return Some(peeked);
        }
        if let Some(next) = self.char_indicies.clone().next() {
            self.peeked = Some(next);
            return Some(next);
        }
        None
    }

    fn next_char(&mut self) -> Option<(usize, char)> {
        if let Some(peeked) = self.peeked.take() {
            self.position = peeked.0 + peeked.1.len_utf8();
            return Some(peeked);
        }
        if let Some((idx, ch)) = self.char_indicies.next() {
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
}
