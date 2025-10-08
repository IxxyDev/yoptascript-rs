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
    }

    impl<'a> Lexer<'a> {
        pub fn new(source: &'a str) -> Self {
            Self {
                source,
                position: 0,
                diagnostics: Vec::new(),
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
    }
}
