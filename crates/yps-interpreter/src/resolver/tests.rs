use super::{RootResolution, resolve};
use yps_lexer::{Lexer, SourceFile};
use yps_parser::Parser;

fn resolved(src: &str) -> RootResolution {
    let source = SourceFile::new("test".to_string(), src.to_string());
    let (tokens, lex_diags) = Lexer::new(&source).tokenize();
    assert!(lex_diags.is_empty(), "Ошибки лексера: {lex_diags:?}");
    let (program, parse_diags) = Parser::new(&tokens, &source).parse_program();
    assert!(parse_diags.is_empty(), "Ошибки парсера: {parse_diags:?}");
    resolve(&program)
}

fn nth_offset(src: &str, needle: &str, n: usize) -> usize {
    src.match_indices(needle).nth(n).unwrap_or_else(|| panic!("нет {n}-го вхождения `{needle}`")).0
}

#[test]
fn global_read_inside_function_is_root() {
    let src = "гыы гло = 1;\nйопта фн() { отвечаю гло; }";
    let res = resolved(src);
    assert!(res.is_root_read(nth_offset(src, "гло", 1)));
}

#[test]
fn builtin_read_inside_function_is_root() {
    let src = "йопта фн() { сказать(1); }";
    let res = resolved(src);
    assert!(res.is_root_read(nth_offset(src, "сказать", 0)));
}

#[test]
fn parameter_read_is_not_root() {
    let src = "йопта фн(лок) { отвечаю лок; }";
    let res = resolved(src);
    assert!(!res.is_root_read(nth_offset(src, "лок", 1)));
}

#[test]
fn nested_block_local_is_not_root() {
    let src = "йопта фн() { { гыы лок = 1; } отвечаю лок; }";
    let res = resolved(src);
    assert!(!res.is_root_read(nth_offset(src, "лок", 1)));
}

#[test]
fn named_function_expression_self_reference_is_not_root() {
    let src = "гыы г = йопта фн() { отвечаю фн; };";
    let res = resolved(src);
    assert!(!res.is_root_read(nth_offset(src, "фн", 1)));
}

#[test]
fn shadowing_local_beats_global() {
    let src = "гыы гло = 1;\nйопта фн() { гыы гло = 2; отвечаю гло; }";
    let res = resolved(src);
    assert!(!res.is_root_read(nth_offset(src, "гло", 2)));
}

#[test]
fn top_level_block_shadowing_builtin_is_not_root() {
    let src = "{ гыы длина = 9; сказать(длина); }";
    let res = resolved(src);
    assert!(!res.is_root_read(nth_offset(src, "длина", 1)));
}

#[test]
fn top_level_block_shadowing_global_is_not_root() {
    let src = "гыы значение = 5;\n{ гыы значение = 9; сказать(значение); }";
    let res = resolved(src);
    assert!(!res.is_root_read(nth_offset(src, "значение", 2)));
}

#[test]
fn top_level_loop_variable_shadowing_global_is_not_root() {
    let src = "гыы и = 99;\nго (гыы и = 0; и < 3; и += 1) { сказать(и); }";
    let res = resolved(src);
    assert!(!res.is_root_read(nth_offset(src, "и", 4)));
}

#[test]
fn static_import_disables_resolution() {
    let src = "спиздить { икс } из \"модуль\";\nйопта фн() { отвечаю гло; }";
    let res = resolved(src);
    assert!(res.is_empty());
}

#[test]
fn dynamic_import_disables_resolution() {
    let src = "йопта фн() { отвечаю гло; }\nгыы п = спиздить(\"модуль\");";
    let res = resolved(src);
    assert!(res.is_empty());
}

#[test]
fn free_variable_without_import_is_root() {
    let src = "йопта фн() { отвечаю свободная; }";
    let res = resolved(src);
    assert!(res.is_root_read(nth_offset(src, "свободная", 0)));
}
