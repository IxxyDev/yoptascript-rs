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

pub mod lexer {
    use super::{Diagnostic, Span};

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

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum KeywordKind {
        Pachan,
        Sliva,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum OperatorKind {
        Plus,
        Minus,
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

    #[derive(Debug, Clone)]
    pub struct Token {
        pub kind: TokenKind,
        pub span: Span,
    }

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
            Token {
                kind: TokenKind::Eof,
                span: Span {
                    start: self.position,
                    end: self.position,
                },
            }
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
    }
}