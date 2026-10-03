mod common;

use common::{Workspace, run, run_in};

#[test]
fn lint_reports_unused_variable_and_exits_with_1() {
    let ws = Workspace::new("lint_unused");
    ws.write("lint_unused.yopta", "йопта ф() {\n  гыы неиспользуемая = 1;\n  отвечаю 0;\n}\nсказать(ф());\n");

    let out = run_in(ws.dir(), &["lint", "lint_unused.yopta"], "");

    assert_eq!(out.code, 1);
    assert!(
        out.stdout.starts_with("lint_unused.yopta:2:7: Предупреждение [unused-variable]: "),
        "stdout: {}",
        out.stdout
    );
}

#[test]
fn lint_exits_with_0_on_a_clean_file() {
    let ws = Workspace::new("lint_clean");
    let path = ws.write("lint_clean.yopta", "сказать(1);\n");

    let out = run(&["lint", path.to_str().unwrap()], "");

    assert_eq!(out.code, 0);
    assert!(out.stdout.is_empty(), "stdout: {}", out.stdout);
}

#[test]
fn lint_reports_parse_errors_with_exit_code_1() {
    let ws = Workspace::new("lint_bad");
    ws.write("lint_bad.yopta", "гыы х = ;\n");

    let out = run_in(ws.dir(), &["lint", "lint_bad.yopta"], "");

    assert_eq!(out.code, 1);
    assert!(out.stderr.starts_with("lint_bad.yopta:1:9: Ошибка: "), "stderr: {}", out.stderr);
}

#[test]
fn lint_reports_a_missing_file() {
    let ws = Workspace::new("lint_missing");

    let out = run_in(ws.dir(), &["lint", "нет.yopta"], "");

    assert_eq!(out.code, 1);
    assert!(out.stderr.contains("Не удалось прочитать файл 'нет.yopta'"), "stderr: {}", out.stderr);
}
