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
