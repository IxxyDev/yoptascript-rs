use super::*;

#[test]
fn test_parser_recovers_from_unknown_keyword_after_brace() {
    let (_, diags) = parse_program_from_source("{ } йопта 5;");
    assert!(!diags.is_empty());
}

#[test]
#[should_panic(expected = "TokenKind::Eof")]
fn parser_new_rejects_empty_tokens() {
    let source = SourceFile::new("test.yopta".to_string(), String::new());
    Parser::new(&[], &source);
}

#[test]
#[should_panic(expected = "TokenKind::Eof")]
fn parser_new_rejects_tokens_without_eof() {
    let source = SourceFile::new("test.yopta".to_string(), "1".to_string());
    let tokens = vec![Token { kind: TokenKind::Number, span: Span { start: 0, end: 1 } }];
    Parser::new(&tokens, &source);
}

#[test]
fn parser_new_accepts_eof_only_tokens() {
    let source = SourceFile::new("test.yopta".to_string(), String::new());
    let tokens = vec![Token { kind: TokenKind::Eof, span: Span { start: 0, end: 0 } }];
    let parser = Parser::new(&tokens, &source);
    let (program, diags) = parser.parse_program();
    assert!(program.items.is_empty());
    assert!(diags.is_empty());
}

#[test]
fn diag_unclosed_paren_in_grouping() {
    let (_, diags) = parse_program_from_source("гыы х = (1 + 2;");
    assert!(
        diags.iter().any(|d| d.message.contains("Ожидался ')'")),
        "expected unclosed-paren diagnostic, got: {:?}",
        diag_messages(&diags)
    );
}

#[test]
fn diag_unclosed_bracket_in_array() {
    let (_, diags) = parse_program_from_source("гыы а = [1, 2;");
    assert!(
        diags.iter().any(|d| d.message.contains("Ожидался ']'")),
        "expected unclosed-bracket diagnostic, got: {:?}",
        diag_messages(&diags)
    );
}

#[test]
fn diag_object_missing_colon_after_key() {
    let (_, diags) = parse_program_from_source(r#"гыы о = {"к" 1};"#);
    assert!(
        diags.iter().any(|d| d.message.contains("':'")),
        "expected missing-colon diagnostic, got: {:?}",
        diag_messages(&diags)
    );
}

#[test]
fn diag_var_decl_missing_equals() {
    let (_, diags) = parse_program_from_source("гыы х;");
    assert!(
        diags.iter().any(|d| d.message.contains("'='")),
        "expected missing-equals diagnostic, got: {:?}",
        diag_messages(&diags)
    );
}

#[test]
fn diag_var_decl_missing_semicolon() {
    let (_, diags) = parse_program_from_source("гыы х = 1\nйопта ф() {}");
    assert!(
        diags.iter().any(|d| d.message.contains("';'")),
        "expected missing-semicolon diagnostic, got: {:?}",
        diag_messages(&diags)
    );
}

#[test]
fn diag_if_missing_open_paren() {
    let (_, diags) = parse_program_from_source("вилкойвглаз 1) {}");
    assert!(
        diags.iter().any(|d| d.message.contains("вилкойвглаз")),
        "expected if-open-paren diagnostic, got: {:?}",
        diag_messages(&diags)
    );
}

#[test]
fn diag_while_missing_open_paren() {
    let (_, diags) = parse_program_from_source("потрещим 1) {}");
    assert!(
        diags.iter().any(|d| d.message.contains("потрещим")),
        "expected while-open-paren diagnostic, got: {:?}",
        diag_messages(&diags)
    );
}

#[test]
fn diag_decorator_on_non_class() {
    let (_, diags) = parse_program_from_source("@дек\nгыы х = 1;");
    assert!(
        diags.iter().any(|d| d.message.contains("Декораторы")),
        "expected decorator-on-non-class diagnostic, got: {:?}",
        diag_messages(&diags)
    );
}

#[test]
fn synchronize_does_not_hang_on_missing_semi_in_arrow_block() {
    let (_, diags) = parse_program_from_source("ф(() => { z = 1 });\n");
    assert!(!diags.is_empty(), "ожидалась диагностика на пропущенную ';' в теле стрелки");
}

fn parse_extended(src: &str) -> (Program, Vec<Diagnostic>, bool) {
    let source = SourceFile::new("<test>".to_string(), src.to_string());
    let lexer = yps_lexer::Lexer::new(&source);
    let (tokens, _) = lexer.tokenize();
    Parser::new(&tokens, &source).parse_program_extended()
}

#[test]
fn unexpected_eof_unclosed_block() {
    let (_, diags, eof) = parse_extended("гыы x = {");
    assert!(!diags.is_empty());
    assert!(eof, "незакрытый блок должен давать unexpected_eof=true");
}

#[test]
fn unexpected_eof_unclosed_paren() {
    let (_, diags, eof) = parse_extended("вилкойвглаз (x");
    assert!(!diags.is_empty());
    assert!(eof, "незакрытая скобка должна давать unexpected_eof=true");
}

#[test]
fn unexpected_eof_false_for_mid_error() {
    let (_, diags, eof) = parse_extended("гыы x = ;");
    assert!(!diags.is_empty());
    assert!(!eof, "ошибка в середине не должна давать unexpected_eof=true");
}

#[test]
fn unexpected_eof_false_for_valid() {
    let (_, diags, eof) = parse_extended("гыы x = 1;");
    assert!(diags.is_empty());
    assert!(!eof, "валидная программа не должна давать unexpected_eof=true");
}

#[test]
fn unexpected_eof_false_for_bigint_overflow() {
    let (_, diags, eof) = parse_extended("99999999999999999999999999999999999999999999n");
    assert!(!diags.is_empty(), "ожидалась диагностика BigInt");
    assert!(!eof, "BigInt-переполнение не должно давать unexpected_eof=true");
}

#[test]
fn unexpected_eof_false_for_asso_without_func() {
    let (_, diags, eof) = parse_extended("ассо");
    assert!(!diags.is_empty());
    assert!(!eof, "'ассо' без продолжения не должно давать unexpected_eof=true");
}

#[test]
fn unexpected_eof_true_for_unclosed_if_block() {
    let (_, diags, eof) = parse_extended("вилкойвглаз (х > 5) {");
    assert!(!diags.is_empty());
    assert!(eof, "незакрытый блок if должен давать unexpected_eof=true");
}

#[test]
fn deeply_nested_parens_yield_diagnostic_not_crash() {
    let src = format!("гыы а = {}1{};", "(".repeat(10_000), ")".repeat(10_000));
    let (_, diags) = parse_program_from_source(&src);
    assert!(
        diags.iter().any(|d| d.message.contains("вложенность")),
        "ожидалась диагностика о вложенности: {:?}",
        diag_messages(&diags)
    );
}

#[test]
fn deeply_nested_arrays_yield_diagnostic_not_crash() {
    let src = format!("гыы а = {}1{};", "[".repeat(10_000), "]".repeat(10_000));
    let (_, diags) = parse_program_from_source(&src);
    assert!(
        diags.iter().any(|d| d.message.contains("вложенность")),
        "ожидалась диагностика о вложенности: {:?}",
        diag_messages(&diags)
    );
}

#[test]
fn deeply_nested_blocks_yield_diagnostic_not_crash() {
    let src = format!("{}{}", "вилкойвглаз (правда) { ".repeat(10_000), "}".repeat(10_000));
    let (_, diags) = parse_program_from_source(&src);
    assert!(
        diags.iter().any(|d| d.message.contains("вложенность")),
        "ожидалась диагностика о вложенности: {:?}",
        diag_messages(&diags)
    );
}

#[test]
fn nesting_within_limit_parses_without_diagnostics() {
    let src = format!("гыы а = {}1{};", "(".repeat(100), ")".repeat(100));
    let (_, diags) = parse_program_from_source(&src);
    assert!(diags.is_empty(), "ошибок не ожидается: {:?}", diag_messages(&diags));
}

#[test]
fn very_long_member_chain_yields_diagnostic_not_crash() {
    let src = format!("гыы о = {{}}; о{};", ".х".repeat(50_000));
    let (_, diags) = parse_program_from_source(&src);
    assert!(
        diags.iter().any(|d| d.message.contains("цепочка")),
        "ожидалась диагностика о длине цепочки: {:?}",
        diag_messages(&diags)
    );
}

#[test]
fn very_long_binary_chain_yields_diagnostic_not_crash() {
    let src = format!("гыы а = 1{};", " + 1".repeat(50_000));
    let (_, diags) = parse_program_from_source(&src);
    assert!(
        diags.iter().any(|d| d.message.contains("цепочка")),
        "ожидалась диагностика о длине цепочки: {:?}",
        diag_messages(&diags)
    );
}
