use crate::output::BufferSink;
use yps_lexer::{Lexer, SourceFile};
use yps_parser::Parser;

use super::Interpreter;

fn capture(src: &str) -> String {
    let source = SourceFile::new("test".to_string(), src.to_string());
    let (tokens, lex_diags) = Lexer::new(&source).tokenize();
    assert!(lex_diags.is_empty(), "Ошибки лексера: {lex_diags:?}");
    let (program, parse_diags) = Parser::new(&tokens, &source).parse_program();
    assert!(parse_diags.is_empty(), "Ошибки парсера: {parse_diags:?}");
    let buffer = BufferSink::new();
    let mut interp = Interpreter::new();
    interp.set_output_sink(Box::new(buffer.clone()));
    interp.run(&program).expect("Ошибка интерпретатора");
    buffer.take()
}

#[test]
fn default_interpreter_has_no_sink() {
    let source = SourceFile::new("test".to_string(), "сказать(\"привет\");".to_string());
    let (tokens, _) = Lexer::new(&source).tokenize();
    let (program, _) = Parser::new(&tokens, &source).parse_program();
    let mut interp = Interpreter::new();
    assert!(interp.output_sink.is_none());
    interp.run(&program).unwrap();
    assert!(interp.output_sink.is_none());
}

#[test]
fn sink_captures_multiple_calls() {
    assert_eq!(capture("сказать(\"а\"); сказать(\"б\", 1);"), "а\nб 1\n");
}

#[test]
fn sink_captures_console_family() {
    let out = capture(
        "сказать.инфо(\"и\"); сказать.отладка(\"о\"); сказать.ошибка(\"э\"); сказать.предупреждение(\"п\"); сказать.таблица([7, 8]);",
    );
    assert_eq!(out, "и\nо\nэ\nп\n0\t7\n1\t8\n");
}

#[test]
fn sink_captures_time_stop_label() {
    let out = capture("сказать.время(\"м\"); сказать.времяСтоп(\"м\");");
    assert!(out.starts_with("м: "), "получено {out:?}");
    assert!(out.trim_end().ends_with(" мс"), "получено {out:?}");
}

#[test]
fn sink_captures_output_from_nested_calls() {
    let out = capture(
        "йопта внутр(х) { сказать(\"внутр\", х); }
         йопта внеш() { внутр(1); внутр(2); }
         внеш();",
    );
    assert_eq!(out, "внутр 1\nвнутр 2\n");
}

#[test]
fn sink_captures_output_from_timers_and_promises() {
    let out = capture(
        "ассо йопта ф() { отвечаю \"обещание\"; }
         чутка(() => { сказать(\"таймер\"); }, 0);
         ф().потом((з) => { сказать(з); });
         сказать(\"синхронно\");",
    );
    assert_eq!(out, "синхронно\nобещание\nтаймер\n");
}

fn module_dir(module_src: &str) -> std::path::PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let id = SEQ.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("yps_sink_mod_{}_{id}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("модуль.yopta"), module_src).unwrap();
    dir
}

fn parse(src: &str) -> yps_parser::Program {
    let source = SourceFile::new("test".to_string(), src.to_string());
    let (tokens, lex_diags) = Lexer::new(&source).tokenize();
    assert!(lex_diags.is_empty(), "Ошибки лексера: {lex_diags:?}");
    let (program, parse_diags) = Parser::new(&tokens, &source).parse_program();
    assert!(parse_diags.is_empty(), "Ошибки парсера: {parse_diags:?}");
    program
}

#[test]
fn sink_is_inherited_by_imported_modules() {
    let dir = module_dir("сказать(\"из модуля\");\nпредъява гыы х = 1;");
    let program = parse("спиздить { х } из \"./модуль\";\nсказать(\"из главного\", х);");
    let buffer = BufferSink::new();
    let mut interp = Interpreter::new();
    interp.set_output_sink(Box::new(buffer.clone()));
    interp.set_base_path(dir.clone());

    interp.run(&program).expect("Ошибка интерпретатора");
    let _ = std::fs::remove_dir_all(&dir);

    assert_eq!(buffer.take(), "из модуля\nиз главного 1\n");
}

#[test]
fn stdin_block_is_inherited_by_imported_modules() {
    let dir = module_dir("прочестьСтроку();\nпредъява гыы х = 1;");
    let program = parse("спиздить { х } из \"./модуль\";");
    let mut interp = Interpreter::new();
    interp.block_stdin("stdin занят");
    interp.set_base_path(dir.clone());

    let err = interp.run(&program).expect_err("импорт обязан упасть");
    let _ = std::fs::remove_dir_all(&dir);

    assert!(err.message.contains("stdin занят"), "получено {:?}", err.message);
}
