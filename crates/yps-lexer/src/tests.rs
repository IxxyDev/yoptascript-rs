use super::*;

#[test]
fn lexes_basic_tokens() {
  let kinds = collect_kinds("pachan x + 42");

  assert_eq!(
    kinds,
    vec![
      TokenKind::Keyword(KeywordKind::Pachan),
      TokenKind::Identifier,
      TokenKind::Operator(OperatorKind::Plus),
      TokenKind::Number,
      TokenKind::Punctuation(PunctuationKind::Semicolon),
      TokenKind::Eof,
    ]
  )
}

#[test]
fn reports_unknown_charachter() {
  let mut lexer = Lexer::new("pachan $foo");
  let _ = lexer.next_token();
  let unknown = lexer.next_token();
  let diagnostics = lexer.diagnostics();

  assert!(matches!(unknown.kind, TokenKind::Unknown));
  assert_eq!(diagnostics[0].message, "неизвестный символ: $");
}

#[test]
fn skips_whitespace_and_newlines() {
  let kinds = collect_kinds("\n sliva\t\n42");

  assert_eq!(
    kinds,
    vec![
      TokenKind::Keyword(KeywordKind::Sliva),
      TokenKind::Number,
      TokenKind::Eof,
    ]
  )
}

#[test]
fn returns_eof_on_empty_input() {
  let kinds = collect_kinds("");

  assert_eq!(kinds, vec![TokenKind::Eof]);
}

#[test]
fn spans_track_original_positions() {
  let mut lexer = Lexer::new("pachan 42");
  let kw = lexer.next_token();

  assert_eq!(kw.kind, TokenKind::Keyword(KeywordKind::Pachan));
  assert_eq!(kw.span.start, 0);
  assert_eq!(kw.span.end, 6);
}

#[test]
fn lexes_assignment() {
  let kinds = collect_kinds("x = 1");

  assert_eq!(
    kinds,
    vec![
      TokenKind::Identifier,
      TokenKind::Operator(OperatorKind::Assign),
      TokenKind::Number,
      TokenKind::Eof,
    ]
  )
}

#[test]
fn lexes_equality() {
  let kinds = collect_kinds("x == 1");

  assert_eq!(
    kinds,
    vec![
      TokenKind::Identifier,
      TokenKind::Operator(OperatorKind::Equals),
      TokenKind::Number,
      TokenKind::Eof,
    ]
  )
}

#[test]
fn lexes_strict_equality() {
  let kinds = collect_kinds("x === 1");

  assert_eq!(
    kinds,
    vec![
      TokenKind::Identifier,
      TokenKind::Operator(OperatorKind::StrictEquals),
      TokenKind::Number,
      TokenKind::Eof,
    ]
  )
}

fn collect_kinds(src: &str) -> Vec<TokenKind> {
  let mut lexer = Lexer::new(src);
  let mut kinds = Vec::new();

  loop {
    let token = lexer.next_token();
    kinds.push(token.kind.clone());
    if matches!(token.kind, TokenKind::Eof) {
      break;
    }
  }

  kinds
}