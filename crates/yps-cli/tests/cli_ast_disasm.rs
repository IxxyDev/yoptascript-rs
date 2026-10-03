mod common;

use common::{Workspace, run};

#[test]
fn ast_dump_contains_expected_nodes() {
    let ws = Workspace::new("ast_dump");
    let path = ws.write("ast1.yopta", "гыы х = 1;\nсказать(х);\n");

    let out = run(&["ast", path.to_str().unwrap()], "");

    assert_eq!(out.code, 0);
    assert!(out.stdout.contains("Program"));
    assert!(out.stdout.contains("VarDecl"));
    assert!(out.stdout.contains("Call"));
}

#[test]
fn ast_reports_parse_errors_with_exit_code_1() {
    let ws = Workspace::new("ast_bad");
    let path = ws.write("ast_bad.yopta", "гыы х = ;\n");

    let out = run(&["ast", path.to_str().unwrap()], "");

    assert_eq!(out.code, 1);
    assert!(out.stderr.contains(":1:9: Ошибка: "), "stderr: {}", out.stderr);
}

#[test]
fn disasm_contains_expected_opcodes() {
    let ws = Workspace::new("disasm");
    let path = ws.write("disasm1.yopta", "йопта ф(а) { отвечаю а; }\nгыы и = 0;\nпотрещим (и < 2) { и = и + 1; }\n");

    let out = run(&["disasm", path.to_str().unwrap()], "");

    assert_eq!(out.code, 0);
    assert!(out.stdout.contains("proto ф"));
    assert!(out.stdout.contains("Closure"));
    assert!(out.stdout.contains("JumpIfFalse"));
    assert!(out.stdout.contains("Lt"));
}

#[test]
fn file_subcommands_reject_unknown_flags_as_usage_errors() {
    let ws = Workspace::new("sub_flag");
    let path = ws.write("ast_flag.yopta", "гыы х = 1;\n");

    for subcommand in ["ast", "disasm", "lint"] {
        let out = run(&[subcommand, path.to_str().unwrap(), "--bogus"], "");

        assert_eq!(out.code, 2, "{subcommand}: stderr: {}", out.stderr);
        assert!(out.stderr.contains("Неизвестный флаг: --bogus"), "{subcommand}: stderr: {}", out.stderr);
    }
}

#[test]
fn file_subcommands_require_exactly_one_file() {
    let ws = Workspace::new("sub_files");
    let path = ws.write("one.yopta", "гыы х = 1;\n");

    for subcommand in ["ast", "disasm", "lint"] {
        let none = run(&[subcommand], "");
        assert_eq!(none.code, 2, "{subcommand}: stderr: {}", none.stderr);
        assert!(none.stderr.contains(&format!("Использование: yps {subcommand}")), "stderr: {}", none.stderr);

        let two = run(&[subcommand, path.to_str().unwrap(), path.to_str().unwrap()], "");
        assert_eq!(two.code, 2, "{subcommand}: stderr: {}", two.stderr);
        assert!(two.stderr.contains("Указан более чем один файл"), "stderr: {}", two.stderr);
    }
}
