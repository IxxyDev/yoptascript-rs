mod common;

use std::process::Command;

use common::{Workspace, run, run_command, run_in};

#[test]
fn repl_evaluates_and_prints_an_expression_value() {
    let out = run(&["repl"], "1 + 2;\n");

    assert_eq!(out.stdout, "3\n");
    assert_eq!(out.code, 0);
}

#[test]
fn repl_runs_builtin_side_effects() {
    let out = run(&["repl"], "сказать(\"эхо\");\n");

    assert!(out.stdout.contains("эхо"), "stdout: {}", out.stdout);
    assert_eq!(out.code, 0);
}

#[test]
fn repl_accumulates_multiline_input_until_complete() {
    let out = run(&["repl"], "йопта f() {\nотвечаю 42;\n}\nсказать(f());\n");

    assert!(out.stdout.contains("42"), "stdout: {}", out.stdout);
    assert_eq!(out.code, 0);
}

#[test]
fn repl_accepts_a_template_literal_spanning_lines() {
    let out = run(&["repl"], "гыы с = `а\nб`;\nсказать(с);\n");

    assert!(out.stderr.is_empty(), "stderr: {}", out.stderr);
    assert!(out.stdout.starts_with("а\nб\n"), "stdout: {}", out.stdout);
    assert_eq!(out.code, 0);
}

#[test]
fn repl_accepts_a_block_comment_spanning_lines() {
    let out = run(&["repl"], "/* начало\nконец */\n1 + 2;\n");

    assert!(out.stderr.is_empty(), "stderr: {}", out.stderr);
    assert_eq!(out.stdout, "3\n");
    assert_eq!(out.code, 0);
}

#[test]
fn repl_accepts_a_string_literal_spanning_lines() {
    let out = run(&["repl"], "гыы с = \"а\nб\";\nдлина(с);\n");

    assert!(out.stderr.is_empty(), "stderr: {}", out.stderr);
    assert_eq!(out.stdout, "3\n");
    assert_eq!(out.code, 0);
}

#[test]
fn repl_reports_an_unterminated_literal_left_at_end_of_input() {
    let out = run(&["repl"], "гыы с = `а\n");

    assert_eq!(out.code, 1);
    assert!(out.stderr.contains("Незакрытая шаблонная строка"), "stderr: {}", out.stderr);
}

#[test]
fn repl_reset_clears_interpreter_state() {
    let out = run(&["repl"], "гыы z = 5;\n:сброс\nсказать(z);\n");

    assert!(out.stderr.contains("не определена"), "ожидалось, что z исчезнет: {}", out.stderr);
}

#[test]
fn repl_repeats_a_history_entry() {
    let out = run(&["repl"], "10 + 1;\n!1\n");

    assert_eq!(out.stdout, "11\n11\n");
    assert_eq!(out.code, 0);
}

#[test]
fn repl_lists_history() {
    let out = run(&["repl"], "1 + 1;\n2 + 2;\n:история\n");

    assert!(out.stdout.contains("1: 1 + 1;"), "stdout: {}", out.stdout);
    assert!(out.stdout.contains("2: 2 + 2;"), "stdout: {}", out.stdout);
    assert_eq!(out.code, 0);
}

#[test]
fn repl_history_keeps_an_input_rejected_by_the_lexer() {
    let out = run(&["repl"], "гыы а = §;\n:история\n");

    assert!(out.stderr.contains("Неизвестный символ"), "stderr: {}", out.stderr);
    assert_eq!(out.stdout, "1: гыы а = §;\n");
}

#[test]
fn repl_history_keeps_an_input_rejected_by_the_parser() {
    let out = run(&["repl"], "гыы = ;\n:история\n");

    assert_eq!(out.stdout, "1: гыы = ;\n");
}

#[test]
fn repl_exit_command_stops_processing_remaining_input() {
    let out = run(&["repl"], ":выход\nсказать(\"после\");\n");

    assert!(!out.stdout.contains("после"), "ввод после :выход не должен исполняться: {}", out.stdout);
    assert_eq!(out.code, 0);
}

#[test]
fn repl_exit_command_works_inside_an_unfinished_block() {
    let out = run(&["repl"], "йопта f() {\n:выход\nсказать(\"после\");\n");

    assert!(out.stderr.is_empty(), "stderr: {}", out.stderr);
    assert!(!out.stdout.contains("после"), "stdout: {}", out.stdout);
    assert_eq!(out.code, 0);
}

#[test]
fn repl_reset_command_discards_an_unfinished_block() {
    let out = run(&["repl"], "йопта f() {\n:сброс\n1 + 2;\n");

    assert!(out.stderr.is_empty(), "stderr: {}", out.stderr);
    assert_eq!(out.stdout, "3\n");
    assert_eq!(out.code, 0);
}

#[test]
fn repl_history_command_keeps_an_unfinished_block() {
    let out = run(&["repl"], "1 + 1;\nйопта f() {\n:история\nотвечаю 5;\n}\nf();\n");

    assert!(out.stderr.is_empty(), "stderr: {}", out.stderr);
    assert_eq!(out.stdout, "2\n1: 1 + 1;\n5\n");
    assert_eq!(out.code, 0);
}

#[test]
fn repl_cancel_command_discards_an_unfinished_block() {
    let out = run(&["repl"], "йопта f() {\n:отмена\n1 + 2;\n");

    assert!(out.stderr.is_empty(), "stderr: {}", out.stderr);
    assert_eq!(out.stdout, "3\n");
    assert_eq!(out.code, 0);
}

#[test]
fn repl_incomplete_input_at_eof_fails() {
    let out = run(&["repl"], "йопта f() {\n");

    assert_eq!(out.code, 1);
    assert!(out.stderr.contains("Ожидалась '}'"), "stderr: {}", out.stderr);
}

#[test]
fn repl_recovers_after_a_parse_error() {
    let out = run(&["repl"], "гыы = ;\n7 + 0;\n");

    assert!(out.stdout.contains("7"), "REPL должен продолжить после ошибки: {}", out.stdout);
}

#[test]
fn piped_repl_exits_with_1_after_a_parse_error() {
    let out = run(&["repl"], "гыы = ;\n7 + 0;\n");

    assert_eq!(out.code, 1);
}

#[test]
fn piped_repl_exits_with_1_after_a_runtime_error() {
    let out = run(&["repl"], "кидай \"бум\";\n1 + 1;\n");

    assert_eq!(out.stdout, "2\n");
    assert_eq!(out.code, 1);
}

#[test]
fn piped_repl_exits_with_1_after_a_lexer_error() {
    let out = run(&["repl"], "гыы а = §;\n");

    assert_eq!(out.code, 1);
}

#[test]
fn piped_repl_keeps_the_failure_code_when_leaving_through_the_exit_command() {
    let out = run(&["repl"], "кидай \"бум\";\n:выход\n");

    assert_eq!(out.code, 1);
}

#[test]
fn repl_locates_a_runtime_error_in_the_input_that_defined_the_code() {
    let out = run(&["repl"], "гыы о = ноль;\nйопта г() { отвечаю о.поле; }\nг();\n");

    assert!(out.stderr.contains("<repl#2>:1:21: Ошибка: "), "stderr: {}", out.stderr);
    assert!(out.stderr.contains("  в г:<repl>:1:1"), "stderr: {}", out.stderr);
}

#[test]
fn repl_locates_an_error_in_the_current_input_without_a_number() {
    let out = run(&["repl"], "1 + 1;\n\nгыы о = ноль;\nо.поле;\n");

    assert!(out.stderr.starts_with("<repl>:1:1: Ошибка: "), "stderr: {}", out.stderr);
}

#[test]
fn repl_reports_parse_errors_relative_to_the_current_input() {
    let out = run(&["repl"], "1 + 1;\nгыы = ;\n");

    assert!(out.stderr.starts_with("<repl>:1:5: Ошибка: "), "stderr: {}", out.stderr);
}

#[test]
fn repl_reports_a_top_level_throw_in_the_current_input() {
    let out = run(&["repl"], "гыы а = 1;\nгыы б = 2;\nкидай 5;\n");

    assert!(out.stderr.starts_with("<repl>:1:1: Ошибка: Необработанное исключение: 5"), "stderr: {}", out.stderr);
}

#[test]
fn repl_reports_break_outside_a_loop_in_the_current_input() {
    let out = run(&["repl"], "гыы а = 1;\nхарэ;\n");

    assert!(out.stderr.starts_with("<repl>:1:1: Ошибка: 'харэ' вне цикла"), "stderr: {}", out.stderr);
}

#[test]
fn piped_repl_exits_with_1_when_its_input_cannot_be_read() {
    let out = run_command(Command::new(common::BIN).arg("repl"), b"1 + 1;\n\xff;\n2 + 2;\n");

    assert_eq!(out.stdout, "2\n");
    assert!(out.stderr.contains("Ошибка чтения ввода"), "stderr: {}", out.stderr);
    assert_eq!(out.code, 1);
}

#[test]
fn repl_programs_see_no_script_arguments() {
    let out = run(&["repl"], "длина(Процесс.аргументы);\n");

    assert_eq!(out.stdout, "0\n", "stderr: {}", out.stderr);
}

#[test]
fn repl_keeps_a_command_word_inside_a_multiline_literal_as_text() {
    let out = run(&["repl"], "гыы с = `а\n:история\nб`;\nдлина(с);\n");

    assert!(out.stderr.is_empty(), "stderr: {}", out.stderr);
    assert_eq!(out.stdout, "12\n");
}

#[test]
fn repl_cancel_command_still_escapes_a_multiline_literal() {
    let out = run(&["repl"], "гыы с = `а\n:отмена\n1 + 2;\n");

    assert!(out.stderr.is_empty(), "stderr: {}", out.stderr);
    assert_eq!(out.stdout, "3\n");
}

#[test]
fn repl_resolves_relative_imports_against_the_working_directory() {
    let ws = Workspace::new("repl_import");
    ws.write("mod.yopta", "предъява гыы ч = 7;\n");

    let out = run_in(ws.dir(), &["repl"], "спиздить { ч } из \"./mod\";\nч + 1;\n");

    assert!(out.stderr.is_empty(), "stderr: {}", out.stderr);
    assert_eq!(out.stdout, "8\n");
}

#[test]
fn repl_rejects_extra_arguments() {
    let out = run(&["repl", "--vm"], "1 + 1;\n");

    assert_eq!(out.code, 2);
    assert!(out.stderr.contains("Лишний аргумент: --vm"), "stderr: {}", out.stderr);
    assert!(out.stdout.is_empty(), "stdout: {}", out.stdout);
}
