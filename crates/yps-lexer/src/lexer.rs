use crate::{Diagnostic, KeywordKind, OperatorKind, PunctuationKind, Severity, SourceFile, Span, Token, TokenKind};

pub struct Lexer<'src> {
    source: &'src SourceFile,
    position: usize,
    diagnostics: Vec<Diagnostic>,
    template_brace_depth: Vec<usize>,
}

impl<'src> Lexer<'src> {
    #[must_use]
    pub const fn new(source: &'src SourceFile) -> Self {
        Self { source, position: 0, diagnostics: Vec::new(), template_brace_depth: Vec::new() }
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

        if ch == '#' && (self.peek_char(1).is_alphabetic() || self.peek_char(1) == '_') {
            return self.read_private_identifier();
        }

        if ch.is_alphabetic() || ch == '_' {
            return self.read_identifier();
        }

        if ch.is_ascii_digit() {
            return self.read_number();
        }

        if ch == '"' || ch == '\'' {
            return self.read_string();
        }

        if ch == '`' {
            return self.read_template_literal();
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
            "гыы" => TokenKind::Keyword(KeywordKind::Gyy),
            "участковый" => TokenKind::Keyword(KeywordKind::Uchastkoviy),
            "ясенХуй" | "ЯсенХуй" => TokenKind::Keyword(KeywordKind::YasenHuy),
            "вилкойвглаз" => TokenKind::Keyword(KeywordKind::Vilkoyvglaz),
            "иливжопураз" => TokenKind::Keyword(KeywordKind::Ilivzhopuraz),
            "потрещим" => TokenKind::Keyword(KeywordKind::Potreshchim),
            "го" => TokenKind::Keyword(KeywordKind::Go),
            "харэ" => TokenKind::Keyword(KeywordKind::Hare),
            "двигай" => TokenKind::Keyword(KeywordKind::Dvigay),
            "йопта" => TokenKind::Keyword(KeywordKind::Yopta),
            "отвечаю" => TokenKind::Keyword(KeywordKind::Otvechayu),
            "правда" | "трулио" | "чётко" | "четко" | "чотко" => {
                TokenKind::Keyword(KeywordKind::Pravda)
            }
            "лож" | "нетрулио" | "пиздишь" | "нечётко" | "нечетко" | "нечотко" => {
                TokenKind::Keyword(KeywordKind::Lozh)
            }
            "ноль" | "нуллио" | "порожняк" => TokenKind::Keyword(KeywordKind::Nol),
            "неибу" => TokenKind::Keyword(KeywordKind::Undefined),
            "хапнуть" | "побратски" | "пабрацки" | "пабратски" => {
                TokenKind::Keyword(KeywordKind::Try)
            }
            "гоп" | "аченетак" | "аченитак" | "ачёнетак" => {
                TokenKind::Keyword(KeywordKind::Catch)
            }
            "тюряжка" => TokenKind::Keyword(KeywordKind::Finally),
            "кидай" | "пнх" => TokenKind::Keyword(KeywordKind::Throw),
            "базарпо" | "естьчо" => TokenKind::Keyword(KeywordKind::Switch),
            "тема" | "лещ" | "аеслинайду" => TokenKind::Keyword(KeywordKind::Case),
            "нуичо" | "пахану" | "апохуй" | "наотыбись" => {
                TokenKind::Keyword(KeywordKind::Default)
            }
            "крутани" | "крч" => TokenKind::Keyword(KeywordKind::DoWhile),
            "из" => TokenKind::Keyword(KeywordKind::In),
            "клёво" | "клево" => TokenKind::Keyword(KeywordKind::Class),
            "батя" => TokenKind::Keyword(KeywordKind::Extends),
            "яга" => TokenKind::Keyword(KeywordKind::Super),
            "захуярить" | "гыйбать" => TokenKind::Keyword(KeywordKind::New),
            "тырыпыры" => TokenKind::Keyword(KeywordKind::This),
            "попонятия" => TokenKind::Keyword(KeywordKind::Static),
            "чезажижан" => TokenKind::Keyword(KeywordKind::Typeof),
            "шкура" => TokenKind::Keyword(KeywordKind::Instanceof),
            "пиздюли" => TokenKind::Keyword(KeywordKind::GeneratorFn),
            "поебалу" | "поебалуна" => TokenKind::Keyword(KeywordKind::Yield),
            "ассо" => TokenKind::Keyword(KeywordKind::Async),
            "сидетьНахуй" => TokenKind::Keyword(KeywordKind::Await),
            "спиздить" => TokenKind::Keyword(KeywordKind::Import),
            "предъява" => TokenKind::Keyword(KeywordKind::Export),
            "откуда" => TokenKind::Keyword(KeywordKind::From),
            "сашаГрей" => TokenKind::Keyword(KeywordKind::Of),
            "ёбнуть" | "ебнуть" => TokenKind::Keyword(KeywordKind::Delete),
            "куку" => TokenKind::Keyword(KeywordKind::Void),
            "юзай" => TokenKind::Keyword(KeywordKind::Using),
            _ => TokenKind::Identifier,
        };

        Token { kind, span }
    }

    fn read_private_identifier(&mut self) -> Token {
        let start = self.position;
        self.advance();

        while self.current_char().is_alphanumeric() || self.current_char() == '_' {
            self.advance();
        }

        let end = self.position;
        Token { kind: TokenKind::PrivateIdentifier, span: Span { start, end } }
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

        while self.current_char().is_ascii_digit() || self.current_char() == '_' {
            self.advance();
        }

        if self.current_char() == '.' && self.peek_char(1).is_ascii_digit() {
            self.advance();

            while self.current_char().is_ascii_digit() || self.current_char() == '_' {
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
            '+' => {
                if self.current_char() == '=' {
                    self.advance();
                    TokenKind::Operator(OperatorKind::PlusAssign)
                } else if self.current_char() == '+' {
                    self.advance();
                    TokenKind::Operator(OperatorKind::Increment)
                } else {
                    TokenKind::Operator(OperatorKind::Plus)
                }
            }
            '-' => {
                if self.current_char() == '=' {
                    self.advance();
                    TokenKind::Operator(OperatorKind::MinusAssign)
                } else if self.current_char() == '-' {
                    self.advance();
                    TokenKind::Operator(OperatorKind::Decrement)
                } else {
                    TokenKind::Operator(OperatorKind::Minus)
                }
            }
            '*' => {
                if self.current_char() == '*' {
                    self.advance();
                    if self.current_char() == '=' {
                        self.advance();
                        TokenKind::Operator(OperatorKind::ExponentAssign)
                    } else {
                        TokenKind::Operator(OperatorKind::Exponent)
                    }
                } else if self.current_char() == '=' {
                    self.advance();
                    TokenKind::Operator(OperatorKind::MulAssign)
                } else {
                    TokenKind::Operator(OperatorKind::Multiply)
                }
            }
            '%' => TokenKind::Operator(OperatorKind::Modulo),
            '/' => {
                if self.current_char() == '/' {
                    self.advance();
                    while !self.is_at_end() && self.current_char() != '\n' {
                        self.advance();
                    }
                    return self.next_token();
                } else if self.current_char() == '=' {
                    self.advance();
                    TokenKind::Operator(OperatorKind::DivAssign)
                } else {
                    TokenKind::Operator(OperatorKind::Divide)
                }
            }
            '=' => {
                if self.current_char() == '=' {
                    self.advance();
                    if self.current_char() == '=' {
                        self.advance();
                        TokenKind::Operator(OperatorKind::StrictEquals)
                    } else {
                        TokenKind::Operator(OperatorKind::Equals)
                    }
                } else if self.current_char() == '>' {
                    self.advance();
                    TokenKind::Punctuation(PunctuationKind::Arrow)
                } else {
                    TokenKind::Operator(OperatorKind::Assign)
                }
            }
            '!' => {
                if self.current_char() == '=' {
                    self.advance();
                    if self.current_char() == '=' {
                        self.advance();
                        TokenKind::Operator(OperatorKind::StrictNotEquals)
                    } else {
                        TokenKind::Operator(OperatorKind::NotEquals)
                    }
                } else {
                    TokenKind::Operator(OperatorKind::Not)
                }
            }
            '<' => {
                if self.current_char() == '=' {
                    self.advance();
                    TokenKind::Operator(OperatorKind::LessOrEqual)
                } else {
                    TokenKind::Operator(OperatorKind::Less)
                }
            }
            '>' => {
                if self.current_char() == '=' {
                    self.advance();
                    TokenKind::Operator(OperatorKind::GreaterOrEqual)
                } else {
                    TokenKind::Operator(OperatorKind::Greater)
                }
            }
            '&' => {
                if self.current_char() == '&' {
                    self.advance();
                    if self.current_char() == '=' {
                        self.advance();
                        TokenKind::Operator(OperatorKind::AndAssign)
                    } else {
                        TokenKind::Operator(OperatorKind::And)
                    }
                } else {
                    self.diagnostics.push(Diagnostic {
                        severity: Severity::Error,
                        message: "одиночный '&' не поддерживается (используйте '&&')".to_string(),
                        span: Span { start, end: self.position },
                    });
                    TokenKind::Unknown
                }
            }
            '|' => {
                if self.current_char() == '|' {
                    self.advance();
                    if self.current_char() == '=' {
                        self.advance();
                        TokenKind::Operator(OperatorKind::OrAssign)
                    } else {
                        TokenKind::Operator(OperatorKind::Or)
                    }
                } else if self.current_char() == '>' {
                    self.advance();
                    TokenKind::Operator(OperatorKind::Pipeline)
                } else {
                    self.diagnostics.push(Diagnostic {
                        severity: Severity::Error,
                        message: "одиночный '|' не поддерживается (используйте '||' или '|>')".to_string(),
                        span: Span { start, end: self.position },
                    });
                    TokenKind::Unknown
                }
            }
            '(' => TokenKind::Punctuation(PunctuationKind::LParen),
            ')' => TokenKind::Punctuation(PunctuationKind::RParen),
            '{' => {
                if let Some(depth) = self.template_brace_depth.last_mut() {
                    *depth += 1;
                }
                TokenKind::Punctuation(PunctuationKind::LBrace)
            }
            '}' => {
                if let Some(depth) = self.template_brace_depth.last_mut() {
                    if *depth == 0 {
                        self.template_brace_depth.pop();
                        let hit_interp = self.read_template_chars();
                        if hit_interp {
                            self.template_brace_depth.push(0);
                            return Token { kind: TokenKind::TemplateMiddle, span: Span { start, end: self.position } };
                        }
                        return Token { kind: TokenKind::TemplateTail, span: Span { start, end: self.position } };
                    }
                    *depth -= 1;
                }
                TokenKind::Punctuation(PunctuationKind::RBrace)
            }
            '[' => TokenKind::Punctuation(PunctuationKind::LBracket),
            ']' => TokenKind::Punctuation(PunctuationKind::RBracket),
            ';' => TokenKind::Punctuation(PunctuationKind::Semicolon),
            ',' => TokenKind::Punctuation(PunctuationKind::Comma),
            ':' => TokenKind::Punctuation(PunctuationKind::Colon),
            '.' => {
                if self.current_char() == '.' && self.peek_char(1) == '.' {
                    self.advance();
                    self.advance();
                    TokenKind::Punctuation(PunctuationKind::Spread)
                } else {
                    TokenKind::Punctuation(PunctuationKind::Dot)
                }
            }
            '?' => {
                if self.current_char() == '.' && !self.peek_char(1).is_ascii_digit() {
                    self.advance();
                    TokenKind::Punctuation(PunctuationKind::OptionalChain)
                } else if self.current_char() == '?' {
                    self.advance();
                    if self.current_char() == '=' {
                        self.advance();
                        TokenKind::Operator(OperatorKind::NullishAssign)
                    } else {
                        TokenKind::Operator(OperatorKind::NullishCoalescing)
                    }
                } else {
                    TokenKind::Punctuation(PunctuationKind::Question)
                }
            }
            '@' => TokenKind::Punctuation(PunctuationKind::At),
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

    fn read_template_literal(&mut self) -> Token {
        let start = self.position;
        self.advance();
        let hit_interp = self.read_template_chars();
        if hit_interp {
            self.template_brace_depth.push(0);
            Token { kind: TokenKind::TemplateHead, span: Span { start, end: self.position } }
        } else {
            Token { kind: TokenKind::TemplateNoSub, span: Span { start, end: self.position } }
        }
    }

    fn read_template_chars(&mut self) -> bool {
        loop {
            if self.is_at_end() {
                self.diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    message: "Незакрытая шаблонная строка".into(),
                    span: Span { start: self.position, end: self.position },
                });
                return false;
            }
            let ch = self.current_char();
            if ch == '`' {
                self.advance();
                return false;
            }
            if ch == '$' && self.peek_char(1) == '{' {
                self.advance();
                self.advance();
                return true;
            }
            if ch == '\\' {
                self.advance();
                if !self.is_at_end() {
                    self.advance();
                }
            } else {
                self.advance();
            }
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_at_token() {
        let source = SourceFile::new("test.yop".to_string(), "@декоратор".to_string());
        let (tokens, diags) = Lexer::new(&source).tokenize();
        assert!(diags.is_empty(), "Unexpected diagnostics: {diags:?}");
        assert_eq!(tokens[0].kind, TokenKind::Punctuation(PunctuationKind::At));
        assert_eq!(tokens[1].kind, TokenKind::Identifier);
    }
}
