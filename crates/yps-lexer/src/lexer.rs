use crate::{
    Diagnostic, KeywordKind, OperatorKind, PunctuationKind, Severity, SourceFile, Span, Token, TokenKind, Trivia,
    TriviaKind,
};

pub const KEYWORDS: &[&str] = &[
    "гыы",
    "участковый",
    "ясенХуй",
    "ЯсенХуй",
    "вилкойвглаз",
    "иливжопураз",
    "потрещим",
    "го",
    "харэ",
    "двигай",
    "йопта",
    "отвечаю",
    "правда",
    "трулио",
    "чётко",
    "четко",
    "чотко",
    "лож",
    "нетрулио",
    "пиздишь",
    "нечётко",
    "нечетко",
    "нечотко",
    "ноль",
    "нуллио",
    "порожняк",
    "неибу",
    "хапнуть",
    "побратски",
    "пабрацки",
    "пабратски",
    "гоп",
    "аченетак",
    "аченитак",
    "ачёнетак",
    "тюряжка",
    "кидай",
    "пнх",
    "базарпо",
    "естьчо",
    "тема",
    "лещ",
    "аеслинайду",
    "нуичо",
    "пахану",
    "апохуй",
    "наотыбись",
    "крутани",
    "крч",
    "из",
    "чоунастут",
    "клёво",
    "клево",
    "батя",
    "яга",
    "захуярить",
    "гыйбать",
    "тырыпыры",
    "попонятия",
    "чезажижан",
    "шкура",
    "пиздюли",
    "поебалу",
    "поебалуна",
    "ассо",
    "сидетьНахуй",
    "спиздить",
    "предъява",
    "откуда",
    "сашаГрей",
    "ёбнуть",
    "ебнуть",
    "куку",
    "юзай",
    "логопед",
    "мой",
    "подкрыша",
    "ебанное",
];

pub struct Lexer<'src> {
    source: &'src SourceFile,
    position: usize,
    diagnostics: Vec<Diagnostic>,
    template_brace_depth: Vec<usize>,
    last_kind: Option<TokenKind>,
    trivia: Vec<Trivia>,
}

impl<'src> Lexer<'src> {
    #[must_use]
    pub const fn new(source: &'src SourceFile) -> Self {
        Self {
            source,
            position: 0,
            diagnostics: Vec::new(),
            template_brace_depth: Vec::new(),
            last_kind: None,
            trivia: Vec::new(),
        }
    }

    #[must_use]
    pub fn tokenize(self) -> (Vec<Token>, Vec<Diagnostic>) {
        let (tokens, _trivia, diagnostics) = self.tokenize_with_trivia();
        (tokens, diagnostics)
    }

    #[must_use]
    pub fn tokenize_with_trivia(mut self) -> (Vec<Token>, Vec<Trivia>, Vec<Diagnostic>) {
        let mut tokens = Vec::new();

        loop {
            let token = self.next_token();
            let is_eof = matches!(token.kind, TokenKind::Eof);
            if !is_eof {
                self.last_kind = Some(token.kind.clone());
            }
            tokens.push(token);

            if is_eof {
                break;
            }
        }

        (tokens, self.trivia, self.diagnostics)
    }

    fn regex_context(&self) -> bool {
        match &self.last_kind {
            None => true,
            Some(k) => match k {
                TokenKind::Identifier
                | TokenKind::PrivateIdentifier
                | TokenKind::Number
                | TokenKind::StringLiteral
                | TokenKind::TemplateNoSub
                | TokenKind::TemplateTail
                | TokenKind::RegexLiteral => false,
                TokenKind::Keyword(kw) => !matches!(
                    kw,
                    KeywordKind::Pravda
                        | KeywordKind::Lozh
                        | KeywordKind::Nol
                        | KeywordKind::Undefined
                        | KeywordKind::This
                        | KeywordKind::Super
                ),
                TokenKind::Operator(op) => !matches!(op, OperatorKind::Increment | OperatorKind::Decrement),
                TokenKind::Punctuation(p) => matches!(
                    p,
                    PunctuationKind::LParen
                        | PunctuationKind::LBracket
                        | PunctuationKind::LBrace
                        | PunctuationKind::Comma
                        | PunctuationKind::Semicolon
                        | PunctuationKind::Colon
                        | PunctuationKind::Question
                        | PunctuationKind::Arrow
                        | PunctuationKind::Spread
                        | PunctuationKind::At
                ),
                TokenKind::TemplateHead | TokenKind::TemplateMiddle => true,
                TokenKind::Eof | TokenKind::Unknown => true,
            },
        }
    }

    fn read_regex(&mut self, start: usize) -> Token {
        let mut in_class = false;
        let mut closed = false;
        while !self.is_at_end() {
            let ch = self.current_char();
            if ch == '\n' {
                break;
            }
            if ch == '\\' {
                self.advance();
                if !self.is_at_end() && self.current_char() != '\n' {
                    self.advance();
                }
                continue;
            }
            if ch == '[' {
                in_class = true;
                self.advance();
                continue;
            }
            if ch == ']' && in_class {
                in_class = false;
                self.advance();
                continue;
            }
            if ch == '/' && !in_class {
                self.advance();
                closed = true;
                break;
            }
            self.advance();
        }
        if !closed {
            self.error(Span { start, end: self.position }, "Незавершённый regex-литерал");
            return Token { kind: TokenKind::Unknown, span: Span { start, end: self.position } };
        }
        while !self.is_at_end() && self.current_char().is_ascii_alphabetic() {
            self.advance();
        }
        let end = self.position;
        Token { kind: TokenKind::RegexLiteral, span: Span { start, end } }
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
            "из" | "чоунастут" => TokenKind::Keyword(KeywordKind::In),
            "клёво" | "клево" => TokenKind::Keyword(KeywordKind::Class),
            "батя" => TokenKind::Keyword(KeywordKind::Extends),
            "яга" => TokenKind::Keyword(KeywordKind::Super),
            "захуярить" | "гыйбать" => TokenKind::Keyword(KeywordKind::New),
            "тырыпыры" => TokenKind::Keyword(KeywordKind::This),
            "попонятия" => TokenKind::Keyword(KeywordKind::Static),
            "чезажижан" => TokenKind::Keyword(KeywordKind::Typeof),
            "шкура" => TokenKind::Keyword(KeywordKind::Instanceof),
            "пиздюли" => TokenKind::Keyword(KeywordKind::GeneratorFn),
            "поебалу" => TokenKind::Keyword(KeywordKind::Yield),
            "поебалуна" => TokenKind::Keyword(KeywordKind::YieldDelegate),
            "ассо" => TokenKind::Keyword(KeywordKind::Async),
            "сидетьНахуй" => TokenKind::Keyword(KeywordKind::Await),
            "спиздить" => TokenKind::Keyword(KeywordKind::Import),
            "предъява" => TokenKind::Keyword(KeywordKind::Export),
            "откуда" => TokenKind::Keyword(KeywordKind::From),
            "сашаГрей" => TokenKind::Keyword(KeywordKind::Of),
            "ёбнуть" | "ебнуть" => TokenKind::Keyword(KeywordKind::Delete),
            "куку" => TokenKind::Keyword(KeywordKind::Void),
            "юзай" => TokenKind::Keyword(KeywordKind::Using),
            "логопед" => TokenKind::Keyword(KeywordKind::Debugger),
            "мой" => TokenKind::Keyword(KeywordKind::Private),
            "подкрыша" => TokenKind::Keyword(KeywordKind::Protected),
            "ебанное" => TokenKind::Keyword(KeywordKind::Public),
            "эквалио" | "однахуйня" | "ровно" | "типа" | "блясука" => {
                TokenKind::Operator(OperatorKind::Equals)
            }
            "блябуду" | "чёткоровно" | "четкоровно" | "чоткоровно" | "конкретно" => {
                TokenKind::Operator(OperatorKind::StrictEquals)
            }
            "ичо" => TokenKind::Operator(OperatorKind::And),
            "иличо" => TokenKind::Operator(OperatorKind::Or),
            "пизже" => TokenKind::Operator(OperatorKind::Greater),
            "хуёвей" | "хуевей" => TokenKind::Operator(OperatorKind::Less),
            "поцик" => TokenKind::Operator(OperatorKind::GreaterOrEqual),
            "поц" => TokenKind::Operator(OperatorKind::LessOrEqual),
            "сука" | "внатуре" => TokenKind::Operator(OperatorKind::Assign),
            "чобля" => TokenKind::Operator(OperatorKind::Not),
            "плюсуюНа" => TokenKind::Operator(OperatorKind::Increment),
            "слилсяНа" => TokenKind::Operator(OperatorKind::Decrement),
            "жЫ" => TokenKind::Punctuation(PunctuationKind::LBrace),
            "нах" | "нахуй" | "бля" => TokenKind::Punctuation(PunctuationKind::Semicolon),
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
            self.error(Span { start, end: self.position }, "Незакрытая строка");
        } else {
            self.advance();
        }

        let end = self.position;
        Token { kind: TokenKind::StringLiteral, span: Span { start, end } }
    }

    fn read_number(&mut self) -> Token {
        let start = self.position;

        if self.current_char() == '0' {
            let radix_marker = self.peek_char(1);
            let radix = match radix_marker {
                'x' | 'X' => Some(16u32),
                'o' | 'O' => Some(8u32),
                'b' | 'B' => Some(2u32),
                _ => None,
            };
            if let Some(radix) = radix {
                self.advance();
                self.advance();
                self.consume_digit_run(radix);
                if self.current_char() == 'n' {
                    self.advance();
                }
                let end = self.position;
                return Token { kind: TokenKind::Number, span: Span { start, end } };
            }
        }

        self.consume_digit_run(10);

        let mut had_decimal = false;
        if self.current_char() == '.' && self.peek_char(1).is_ascii_digit() {
            self.advance();
            had_decimal = true;
            self.consume_digit_run(10);
        }

        let mut had_exponent = false;
        if self.current_char() == 'e' || self.current_char() == 'E' {
            let sign = self.peek_char(1);
            let digit_offset = if sign == '+' || sign == '-' { 2 } else { 1 };
            if self.peek_char(digit_offset).is_ascii_digit() {
                self.advance();
                if sign == '+' || sign == '-' {
                    self.advance();
                }
                self.consume_digit_run(10);
                had_exponent = true;
            }
        }

        if !had_decimal && !had_exponent && self.current_char() == 'n' {
            self.advance();
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
            '%' => {
                if self.current_char() == '=' {
                    self.advance();
                    TokenKind::Operator(OperatorKind::ModAssign)
                } else {
                    TokenKind::Operator(OperatorKind::Modulo)
                }
            }
            '/' => {
                if self.current_char() == '/' {
                    self.advance();
                    while !self.is_at_end() && self.current_char() != '\n' {
                        self.advance();
                    }
                    let span = Span { start, end: self.position };
                    self.trivia.push(Trivia {
                        kind: TriviaKind::LineComment,
                        text: self.source.slice(span).to_string(),
                        span,
                    });
                    return self.next_token();
                } else if self.current_char() == '*' {
                    self.advance();
                    loop {
                        if self.is_at_end() {
                            self.error(Span { start, end: self.position }, "Незакрытый блочный комментарий");
                            break;
                        }
                        if self.current_char() == '*' && self.peek_char(1) == '/' {
                            self.advance();
                            self.advance();
                            break;
                        }
                        self.advance();
                    }
                    let span = Span { start, end: self.position };
                    self.trivia.push(Trivia {
                        kind: TriviaKind::BlockComment,
                        text: self.source.slice(span).to_string(),
                        span,
                    });
                    return self.next_token();
                } else if self.regex_context() {
                    return self.read_regex(start);
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
                if self.current_char() == '<' {
                    self.advance();
                    if self.current_char() == '=' {
                        self.advance();
                        TokenKind::Operator(OperatorKind::ShlAssign)
                    } else {
                        TokenKind::Operator(OperatorKind::LeftShift)
                    }
                } else if self.current_char() == '=' {
                    self.advance();
                    TokenKind::Operator(OperatorKind::LessOrEqual)
                } else {
                    TokenKind::Operator(OperatorKind::Less)
                }
            }
            '>' => {
                if self.current_char() == '>' {
                    self.advance();
                    if self.current_char() == '>' {
                        self.advance();
                        if self.current_char() == '=' {
                            self.advance();
                            TokenKind::Operator(OperatorKind::UshrAssign)
                        } else {
                            TokenKind::Operator(OperatorKind::UnsignedRightShift)
                        }
                    } else if self.current_char() == '=' {
                        self.advance();
                        TokenKind::Operator(OperatorKind::ShrAssign)
                    } else {
                        TokenKind::Operator(OperatorKind::RightShift)
                    }
                } else if self.current_char() == '=' {
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
                } else if self.current_char() == '=' {
                    self.advance();
                    TokenKind::Operator(OperatorKind::BitAndAssign)
                } else {
                    TokenKind::Operator(OperatorKind::BitAnd)
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
                } else if self.current_char() == '=' {
                    self.advance();
                    TokenKind::Operator(OperatorKind::BitOrAssign)
                } else {
                    TokenKind::Operator(OperatorKind::BitOr)
                }
            }
            '^' => {
                if self.current_char() == '=' {
                    self.advance();
                    TokenKind::Operator(OperatorKind::BitXorAssign)
                } else {
                    TokenKind::Operator(OperatorKind::BitXor)
                }
            }
            '~' => TokenKind::Operator(OperatorKind::BitwiseNot),
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
                self.error(Span { start, end: self.position }, format!("Неизвестный символ: '{ch}'"));
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
                self.error(Span { start: self.position, end: self.position }, "Незакрытая шаблонная строка");
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

    #[inline]
    fn current_char(&self) -> char {
        let bytes = self.source.source.as_bytes();
        match bytes.get(self.position) {
            None => '\0',
            Some(&b) if b < 0x80 => b as char,
            Some(_) => self.source.source[self.position..].chars().next().unwrap_or('\0'),
        }
    }

    #[inline]
    fn peek_char(&self, offset: usize) -> char {
        let mut chars = self.source.source[self.position..].chars();

        for _ in 0..offset {
            chars.next();
        }

        chars.next().unwrap_or('\0')
    }

    #[inline]
    fn advance(&mut self) -> char {
        let ch = self.current_char();

        if !self.is_at_end() {
            self.position += ch.len_utf8();
        }
        ch
    }

    #[must_use]
    #[inline]
    const fn is_at_end(&self) -> bool {
        self.position >= self.source.source.len()
    }

    fn error(&mut self, span: Span, message: impl Into<String>) {
        self.diagnostics.push(Diagnostic { severity: Severity::Error, message: message.into(), span });
    }

    fn consume_digit_run(&mut self, radix: u32) {
        while self.current_char().is_digit(radix) || self.current_char() == '_' {
            self.advance();
        }
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
    fn every_keyword_spelling_lexes_as_keyword() {
        for kw in KEYWORDS {
            let source = SourceFile::new("test.yopta".to_string(), (*kw).to_string());
            let (tokens, diags) = Lexer::new(&source).tokenize();
            assert!(diags.is_empty(), "'{kw}' produced diagnostics: {diags:?}");
            assert!(
                matches!(tokens[0].kind, TokenKind::Keyword(_)),
                "'{kw}' lexed as {:?}, expected a keyword",
                tokens[0].kind
            );
        }
    }

    #[test]
    fn word_operator_aliases_lex_as_operators() {
        for (word, expected) in
            [("чобля", OperatorKind::Not), ("плюсуюНа", OperatorKind::Increment), ("слилсяНа", OperatorKind::Decrement)]
        {
            let source = SourceFile::new("test.yopta".to_string(), word.to_string());
            let (tokens, diags) = Lexer::new(&source).tokenize();
            assert!(diags.is_empty(), "'{word}' produced diagnostics: {diags:?}");
            assert_eq!(tokens[0].kind, TokenKind::Operator(expected), "'{word}' lexed as {:?}", tokens[0].kind);
        }
    }

    #[test]
    fn test_at_token() {
        let source = SourceFile::new("test.yopta".to_string(), "@декоратор".to_string());
        let (tokens, diags) = Lexer::new(&source).tokenize();
        assert!(diags.is_empty(), "Unexpected diagnostics: {diags:?}");
        assert_eq!(tokens[0].kind, TokenKind::Punctuation(PunctuationKind::At));
        assert_eq!(tokens[1].kind, TokenKind::Identifier);
    }

    #[test]
    fn diagnostic_unterminated_string() {
        let source = SourceFile::new("test.yopta".to_string(), r#""без закрывающей кавычки"#.to_string());
        let (_tokens, diags) = Lexer::new(&source).tokenize();
        assert_eq!(diags.len(), 1, "expected one diagnostic, got {diags:?}");
        assert!(matches!(diags[0].severity, Severity::Error));
        assert!(diags[0].message.contains("Незакрытая строка"), "got: {}", diags[0].message);
    }

    #[test]
    fn diagnostic_unterminated_template_literal() {
        let source = SourceFile::new("test.yopta".to_string(), "`привет".to_string());
        let (_tokens, diags) = Lexer::new(&source).tokenize();
        assert!(
            diags.iter().any(|d| d.message.contains("Незакрытая шаблонная строка")),
            "expected unterminated-template diagnostic, got: {diags:?}"
        );
    }

    #[test]
    fn diagnostic_single_ampersand_is_bitand() {
        let source = SourceFile::new("test.yopta".to_string(), "а & б".to_string());
        let (tokens, diags) = Lexer::new(&source).tokenize();
        assert!(diags.is_empty(), "unexpected diagnostics: {diags:?}");
        assert!(tokens.iter().any(|t| t.kind == TokenKind::Operator(OperatorKind::BitAnd)));
    }

    #[test]
    fn diagnostic_single_pipe_is_bitor() {
        let source = SourceFile::new("test.yopta".to_string(), "а | б".to_string());
        let (tokens, diags) = Lexer::new(&source).tokenize();
        assert!(diags.is_empty(), "unexpected diagnostics: {diags:?}");
        assert!(tokens.iter().any(|t| t.kind == TokenKind::Operator(OperatorKind::BitOr)));
    }

    #[test]
    fn diagnostic_pipe_greater_is_pipeline_no_diag() {
        let source = SourceFile::new("test.yopta".to_string(), "а |> б".to_string());
        let (tokens, diags) = Lexer::new(&source).tokenize();
        assert!(diags.is_empty(), "|> should not emit a diagnostic, got: {diags:?}");
        assert!(tokens.iter().any(|t| matches!(t.kind, TokenKind::Operator(OperatorKind::Pipeline))));
    }

    #[test]
    fn diagnostic_unknown_character() {
        let source = SourceFile::new("test.yopta".to_string(), "гыы х = §".to_string());
        let (_tokens, diags) = Lexer::new(&source).tokenize();
        assert!(
            diags.iter().any(|d| d.message.contains("Неизвестный символ") && d.message.contains('§')),
            "expected unknown-char diagnostic mentioning '§', got: {diags:?}"
        );
    }

    #[test]
    fn diagnostic_does_not_panic_on_eof_after_string_quote() {
        let source = SourceFile::new("test.yopta".to_string(), "\"".to_string());
        let (_tokens, diags) = Lexer::new(&source).tokenize();
        assert!(diags.iter().any(|d| d.message.contains("Незакрытая строка")));
    }

    #[test]
    fn block_comment_is_skipped() {
        let plain = SourceFile::new("test.yopta".to_string(), "гыы х = 1;".to_string());
        let (plain_tokens, plain_diags) = Lexer::new(&plain).tokenize();
        assert!(plain_diags.is_empty(), "unexpected diags: {plain_diags:?}");

        let commented = SourceFile::new("test.yopta".to_string(), "гыы /* это\nмногострочный */ х = 1;".to_string());
        let (commented_tokens, commented_diags) = Lexer::new(&commented).tokenize();
        assert!(commented_diags.is_empty(), "unexpected diags: {commented_diags:?}");
        let plain_kinds: Vec<_> = plain_tokens.iter().map(|t| &t.kind).collect();
        let commented_kinds: Vec<_> = commented_tokens.iter().map(|t| &t.kind).collect();
        assert_eq!(plain_kinds, commented_kinds, "блок-комментарий не должен порождать токены");
    }

    #[test]
    fn tokenize_with_trivia_collects_comments_without_changing_tokens() {
        let src = "гыы х = 1; // хвост\n/* блок */ сказать(х);";
        let source = SourceFile::new("test.yopta".to_string(), src.to_string());
        let (trivia_tokens, trivia, diags) = Lexer::new(&source).tokenize_with_trivia();
        assert!(diags.is_empty(), "unexpected diags: {diags:?}");

        let plain = SourceFile::new("test.yopta".to_string(), src.to_string());
        let (plain_tokens, _) = Lexer::new(&plain).tokenize();
        let trivia_kinds: Vec<_> = trivia_tokens.iter().map(|t| &t.kind).collect();
        let plain_kinds: Vec<_> = plain_tokens.iter().map(|t| &t.kind).collect();
        assert_eq!(trivia_kinds, plain_kinds, "trivia-метод не должен менять поток токенов");

        let texts: Vec<&str> = trivia.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, vec!["// хвост", "/* блок */"]);
    }

    #[test]
    fn block_comment_first_close_wins() {
        let source = SourceFile::new("test.yopta".to_string(), "/* а /* б */ х".to_string());
        let (tokens, diags) = Lexer::new(&source).tokenize();
        assert!(diags.is_empty(), "unexpected diags: {diags:?}");
        assert!(
            tokens.iter().any(|t| t.kind == TokenKind::Identifier),
            "ожидался идентификатор после закрытия комментария"
        );
    }

    #[test]
    fn diagnostic_unterminated_block_comment() {
        let source = SourceFile::new("test.yopta".to_string(), "гыы х = 1; /* хвост".to_string());
        let (_tokens, diags) = Lexer::new(&source).tokenize();
        assert!(
            diags.iter().any(|d| d.message.contains("Незакрытый блочный комментарий")),
            "expected unterminated-block-comment diagnostic, got: {diags:?}"
        );
    }

    #[test]
    fn nul_byte_terminates_with_diagnostic() {
        let source = SourceFile::new("test.yopta".to_string(), "\0".to_string());
        let (tokens, diags) = Lexer::new(&source).tokenize();
        assert!(matches!(tokens.last().map(|t| &t.kind), Some(TokenKind::Eof)));
        assert!(diags.iter().any(|d| d.message.contains("Неизвестный символ")), "ожидалась диагностика: {diags:?}");
    }

    #[test]
    fn nul_byte_between_tokens_terminates() {
        let source = SourceFile::new("test.yopta".to_string(), "гыы х\0= 1;".to_string());
        let (tokens, _diags) = Lexer::new(&source).tokenize();
        assert!(matches!(tokens.last().map(|t| &t.kind), Some(TokenKind::Eof)));
        assert!(tokens.len() < 20);
    }

    #[test]
    fn nul_byte_inside_string_literal_terminates() {
        let source = SourceFile::new("test.yopta".to_string(), "гыы с = \"а\0б\";".to_string());
        let (tokens, _diags) = Lexer::new(&source).tokenize();
        assert!(matches!(tokens.last().map(|t| &t.kind), Some(TokenKind::Eof)));
    }

    fn lex_single_number(src: &str) -> String {
        let source = SourceFile::new("test.yopta".to_string(), src.to_string());
        let (tokens, diags) = Lexer::new(&source).tokenize();
        assert!(diags.is_empty(), "unexpected diagnostics: {diags:?}");
        assert_eq!(tokens.len(), 2, "expected one Number token + Eof, got: {tokens:?}");
        assert_eq!(tokens[0].kind, TokenKind::Number, "got: {:?}", tokens[0].kind);
        assert_eq!(tokens[1].kind, TokenKind::Eof);
        source.slice(tokens[0].span).to_string()
    }

    #[test]
    fn number_radix_literals_single_token() {
        assert_eq!(lex_single_number("0x1F"), "0x1F");
        assert_eq!(lex_single_number("0X1f"), "0X1f");
        assert_eq!(lex_single_number("0o17"), "0o17");
        assert_eq!(lex_single_number("0O17"), "0O17");
        assert_eq!(lex_single_number("0b101"), "0b101");
        assert_eq!(lex_single_number("0B101"), "0B101");
    }

    #[test]
    fn number_exponent_literals_single_token() {
        assert_eq!(lex_single_number("1e21"), "1e21");
        assert_eq!(lex_single_number("1.5e-7"), "1.5e-7");
        assert_eq!(lex_single_number("2E3"), "2E3");
        assert_eq!(lex_single_number("1e+10"), "1e+10");
    }

    #[test]
    fn number_separators_preserved() {
        assert_eq!(lex_single_number("1_000"), "1_000");
        assert_eq!(lex_single_number("0xFF_FF"), "0xFF_FF");
    }

    fn lex_kinds(src: &str) -> Vec<TokenKind> {
        let source = SourceFile::new("test.yopta".to_string(), src.to_string());
        let (tokens, diags) = Lexer::new(&source).tokenize();
        assert!(diags.is_empty(), "неожиданные диагностики для {src:?}: {diags:?}");
        tokens.into_iter().map(|t| t.kind).collect()
    }

    fn infix_op(src: &str) -> TokenKind {
        lex_kinds(src).into_iter().nth(1).expect("ожидался инфиксный токен")
    }

    #[test]
    fn arithmetic_and_assignment_operators_lex_to_expected_kind() {
        use OperatorKind::{
            Assign, DivAssign, Divide, Exponent, ExponentAssign, Minus, MinusAssign, ModAssign, Modulo, MulAssign,
            Multiply, Plus, PlusAssign,
        };
        let cases = [
            ("1 + 2", Plus),
            ("1 - 2", Minus),
            ("1 * 2", Multiply),
            ("1 / 2", Divide),
            ("1 % 2", Modulo),
            ("1 ** 2", Exponent),
            ("a = 1", Assign),
            ("a += 1", PlusAssign),
            ("a -= 1", MinusAssign),
            ("a *= 1", MulAssign),
            ("a /= 1", DivAssign),
            ("a %= 1", ModAssign),
            ("a **= 1", ExponentAssign),
        ];
        for (src, op) in cases {
            assert_eq!(infix_op(src), TokenKind::Operator(op.clone()), "src {src:?}");
        }
    }

    #[test]
    fn comparison_and_shift_operators_lex_to_expected_kind() {
        use OperatorKind::{
            Equals, Greater, GreaterOrEqual, LeftShift, Less, LessOrEqual, NotEquals, RightShift, ShlAssign, ShrAssign,
            StrictEquals, StrictNotEquals, UnsignedRightShift, UshrAssign,
        };
        let cases = [
            ("1 < 2", Less),
            ("1 > 2", Greater),
            ("1 <= 2", LessOrEqual),
            ("1 >= 2", GreaterOrEqual),
            ("1 == 2", Equals),
            ("1 === 2", StrictEquals),
            ("1 != 2", NotEquals),
            ("1 !== 2", StrictNotEquals),
            ("1 << 2", LeftShift),
            ("1 >> 2", RightShift),
            ("1 >>> 2", UnsignedRightShift),
            ("1 <<= 2", ShlAssign),
            ("1 >>= 2", ShrAssign),
            ("1 >>>= 2", UshrAssign),
        ];
        for (src, op) in cases {
            assert_eq!(infix_op(src), TokenKind::Operator(op.clone()), "src {src:?}");
        }
    }

    #[test]
    fn logical_bitwise_and_nullish_operators_lex_to_expected_kind() {
        use OperatorKind::{
            And, AndAssign, BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, NullishAssign,
            NullishCoalescing, Or, OrAssign, Pipeline,
        };
        let cases = [
            ("a && b", And),
            ("a || b", Or),
            ("a & b", BitAnd),
            ("a | b", BitOr),
            ("a ^ b", BitXor),
            ("a &= b", BitAndAssign),
            ("a |= b", BitOrAssign),
            ("a ^= b", BitXorAssign),
            ("a &&= b", AndAssign),
            ("a ||= b", OrAssign),
            ("a ?? b", NullishCoalescing),
            ("a ??= b", NullishAssign),
            ("a |> b", Pipeline),
        ];
        for (src, op) in cases {
            assert_eq!(infix_op(src), TokenKind::Operator(op.clone()), "src {src:?}");
        }
    }

    #[test]
    fn prefix_operators_lex_to_expected_kind() {
        assert_eq!(lex_kinds("~a")[0], TokenKind::Operator(OperatorKind::BitwiseNot));
        assert_eq!(lex_kinds("!a")[0], TokenKind::Operator(OperatorKind::Not));
        assert_eq!(lex_kinds("++a")[0], TokenKind::Operator(OperatorKind::Increment));
        assert_eq!(lex_kinds("--a")[0], TokenKind::Operator(OperatorKind::Decrement));
    }

    #[test]
    fn punctuation_lexes_to_expected_kind() {
        use PunctuationKind::{Arrow, Colon, Dot, OptionalChain, Question, Spread};
        assert_eq!(infix_op("a => b"), TokenKind::Punctuation(Arrow));
        assert_eq!(infix_op("a.b"), TokenKind::Punctuation(Dot));
        assert_eq!(infix_op("a?.b"), TokenKind::Punctuation(OptionalChain));
        assert_eq!(infix_op("a ? b"), TokenKind::Punctuation(Question));
        assert_eq!(infix_op("a : b"), TokenKind::Punctuation(Colon));
        assert_eq!(lex_kinds("...a")[0], TokenKind::Punctuation(Spread));
    }

    #[test]
    fn optional_chain_is_not_taken_when_followed_by_digit() {
        let kinds = lex_kinds("a?.5");
        assert_eq!(kinds[1], TokenKind::Punctuation(PunctuationKind::Question));
    }

    #[test]
    fn regex_literal_at_expression_start() {
        let kinds = lex_kinds("/ab+/g");
        assert_eq!(kinds[0], TokenKind::RegexLiteral);
        assert_eq!(kinds[1], TokenKind::Eof);
    }

    #[test]
    fn slash_after_a_value_is_division_not_regex() {
        let kinds = lex_kinds("а / б");
        assert!(kinds.contains(&TokenKind::Operator(OperatorKind::Divide)), "{kinds:?}");
        assert!(!kinds.contains(&TokenKind::RegexLiteral), "{kinds:?}");
    }

    #[test]
    fn slash_equals_after_a_value_is_div_assign() {
        let kinds = lex_kinds("а /= б");
        assert!(kinds.contains(&TokenKind::Operator(OperatorKind::DivAssign)), "{kinds:?}");
    }

    #[test]
    fn unterminated_regex_emits_diagnostic() {
        let source = SourceFile::new("test.yopta".to_string(), "/abc".to_string());
        let (_tokens, diags) = Lexer::new(&source).tokenize();
        assert!(
            diags.iter().any(|d| d.message.contains("Незавершённый regex")),
            "ожидалась диагностика незавершённого regex: {diags:?}"
        );
    }

    #[test]
    fn template_without_substitution_is_a_single_token() {
        assert_eq!(lex_kinds("`привет`"), vec![TokenKind::TemplateNoSub, TokenKind::Eof]);
    }

    #[test]
    fn template_with_substitution_splits_into_head_and_tail() {
        let kinds = lex_kinds("`a${x}b`");
        assert_eq!(kinds[0], TokenKind::TemplateHead);
        assert!(kinds.contains(&TokenKind::TemplateTail), "{kinds:?}");
    }

    #[test]
    fn template_with_two_substitutions_has_a_middle() {
        let kinds = lex_kinds("`a${x}b${y}c`");
        assert!(kinds.contains(&TokenKind::TemplateMiddle), "{kinds:?}");
    }

    #[test]
    fn template_tracks_nested_braces_in_a_substitution() {
        let kinds = lex_kinds("`a${ {x:1} }b`");
        assert_eq!(kinds[0], TokenKind::TemplateHead);
        assert!(kinds.contains(&TokenKind::TemplateTail), "вложенный объект не должен закрывать шаблон: {kinds:?}");
    }

    #[test]
    fn private_identifier_is_recognized() {
        let kinds = lex_kinds("#поле");
        assert_eq!(kinds[0], TokenKind::PrivateIdentifier);
    }

    #[test]
    fn string_with_an_escaped_quote_is_a_single_token() {
        assert_eq!(lex_kinds(r#""а\"б""#), vec![TokenKind::StringLiteral, TokenKind::Eof]);
    }
}
