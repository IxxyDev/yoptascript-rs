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