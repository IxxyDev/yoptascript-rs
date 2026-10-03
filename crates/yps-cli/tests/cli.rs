mod common;

use common::{Workspace, run, run_in};

const PRINT_ARGS: &str = "го (гыы и = 0; и < длина(Процесс.аргументы); и++) { сказать(Процесс.аргументы[и]); }\n";

#[test]
fn runs_a_program_and_prints_its_output() {
    let ws = Workspace::new("run_ok");
    let prog = ws.write("p.yopta", "сказать(\"привет\", 1 + 2);\n");

    let out = run(&[prog.to_str().unwrap()], "");

    assert_eq!(out.stdout, "привет 3\n");
    assert_eq!(out.code, 0);
}

#[test]
fn reports_a_missing_file_and_exits_with_1() {
    let ws = Workspace::new("missing");
    let absent = ws.path("nope.yopta");

    let out = run(&[absent.to_str().unwrap()], "");

    assert_eq!(out.code, 1);
    assert!(out.stderr.contains("Не удалось прочитать файл"), "stderr: {}", out.stderr);
}

#[test]
fn reports_a_parse_error_with_a_location_and_exits_with_1() {
    let ws = Workspace::new("parse_err");
    let prog = ws.write("bad.yopta", "гыы x = ;\n");

    let out = run(&[prog.to_str().unwrap()], "");

    assert_eq!(out.code, 1);
    assert!(out.stderr.contains(":1:"), "ожидалась позиция в stderr: {}", out.stderr);
}

#[test]
fn diagnostics_label_their_severity_in_russian() {
    let out = run(&["-e", "гыы = ;"], "");

    assert_eq!(out.code, 1);
    assert!(out.stderr.starts_with("<eval>:1:5: Ошибка: "), "stderr: {}", out.stderr);
    assert!(!out.stderr.contains("Error"), "stderr: {}", out.stderr);
}

#[test]
fn reports_an_uncaught_exception_and_exits_with_1() {
    let ws = Workspace::new("throw");
    let prog = ws.write("throw.yopta", "кидай \"бум\";\n");

    let out = run(&[prog.to_str().unwrap()], "");

    assert_eq!(out.code, 1);
    assert!(out.stderr.contains("Необработанное исключение"), "stderr: {}", out.stderr);
    assert!(out.stderr.contains("бум"), "stderr: {}", out.stderr);
}

#[test]
fn version_flag_prints_the_version_and_exits_0() {
    let out = run(&["--version"], "");

    assert_eq!(out.code, 0);
    assert!(out.stdout.starts_with("yps "), "stdout: {}", out.stdout);
}

#[test]
fn short_version_flag_prints_the_version_and_exits_0() {
    let out = run(&["-V"], "");

    assert_eq!(out.code, 0);
    assert!(out.stdout.starts_with("yps "), "stdout: {}", out.stdout);
}

#[test]
fn version_flag_rejects_extra_arguments() {
    let out = run(&["--version", "мусор"], "");

    assert_eq!(out.code, 2);
    assert!(out.stderr.contains("Лишний аргумент: мусор"), "stderr: {}", out.stderr);
    assert!(out.stdout.is_empty(), "stdout: {}", out.stdout);
}

#[test]
fn help_flag_prints_usage_and_exits_0() {
    let out = run(&["--help"], "");

    assert_eq!(out.code, 0);
    assert!(out.stdout.contains("Использование"), "stdout: {}", out.stdout);
    assert!(out.stdout.contains("fmt"), "stdout: {}", out.stdout);
}

#[test]
fn short_help_flag_prints_usage_and_exits_0() {
    let out = run(&["-h"], "");

    assert_eq!(out.code, 0);
    assert!(out.stdout.contains("Использование"), "stdout: {}", out.stdout);
}

#[test]
fn help_flag_works_after_other_flags() {
    let out = run(&["--vm", "--help"], "");

    assert_eq!(out.code, 0);
    assert!(out.stdout.contains("Использование"), "stdout: {}", out.stdout);
}

#[test]
fn help_flag_rejects_extra_arguments() {
    let out = run(&["--help", "мусор"], "");

    assert_eq!(out.code, 2);
    assert!(out.stderr.contains("Лишний аргумент: мусор"), "stderr: {}", out.stderr);
}

#[test]
fn help_documents_script_arguments_and_exit_codes() {
    let out = run(&["--help"], "");

    assert!(out.stdout.contains("АРГУМЕНТЫ"), "stdout: {}", out.stdout);
    assert!(out.stdout.contains("Коды выхода"), "stdout: {}", out.stdout);
}

#[test]
fn every_file_subcommand_prints_its_own_usage_on_help() {
    for subcommand in ["fmt", "ast", "disasm", "lint", "transpile", "repl"] {
        for flag in ["--help", "-h"] {
            let out = run(&[subcommand, flag], "");

            assert_eq!(out.code, 0, "{subcommand} {flag}: stderr: {}", out.stderr);
            assert!(
                out.stdout.contains(&format!("Использование: yps {subcommand}")),
                "{subcommand} {flag}: stdout: {}",
                out.stdout
            );
        }
    }
}

#[test]
fn eval_runs_an_inline_snippet() {
    let out = run(&["-e", "сказать(\"привет\", 1 + 2);"], "");

    assert_eq!(out.stdout, "привет 3\n");
    assert_eq!(out.code, 0);
}

#[test]
fn eval_long_flag_works_with_the_vm_backend() {
    let out = run(&["--eval", "сказать(\"привет\", 1 + 2);", "--vm"], "");

    assert_eq!(out.stdout, "привет 3\n");
    assert_eq!(out.code, 0);
}

#[test]
fn eval_reports_a_runtime_error_and_exits_with_1() {
    let out = run(&["-e", "кидай \"бум\";"], "");

    assert_eq!(out.code, 1);
    assert!(out.stderr.contains("<eval>"), "stderr: {}", out.stderr);
    assert!(out.stderr.contains("бум"), "stderr: {}", out.stderr);
}

#[test]
fn eval_without_code_is_a_usage_error() {
    let out = run(&["-e"], "");

    assert_eq!(out.code, 2);
    assert!(out.stderr.contains("Флаг -e требует аргумент с кодом"), "stderr: {}", out.stderr);
}

#[test]
fn eval_given_twice_is_rejected() {
    let out = run(&["-e", "сказать(1);", "-e", "сказать(2);"], "");

    assert_eq!(out.code, 2);
    assert!(out.stderr.contains("Флаг -e указан более одного раза"), "stderr: {}", out.stderr);
    assert!(out.stdout.is_empty(), "stdout: {}", out.stdout);
}

#[test]
fn eval_combined_with_a_file_is_rejected() {
    let ws = Workspace::new("eval_and_file");
    let prog = ws.write("p.yopta", "сказать(\"файл\");\n");

    let out = run(&["-e", "сказать(\"eval\");", prog.to_str().unwrap()], "");

    assert_eq!(out.code, 2);
    assert!(out.stderr.contains("Укажите только один источник программы"), "stderr: {}", out.stderr);
    assert!(out.stderr.contains("после --"), "stderr: {}", out.stderr);
    assert!(out.stdout.is_empty(), "stdout: {}", out.stdout);
}

#[test]
fn eval_combined_with_stdin_is_rejected() {
    let out = run(&["-e", "сказать(\"eval\");", "-"], "сказать(\"stdin\");\n");

    assert_eq!(out.code, 2);
    assert!(out.stderr.contains("Укажите только один источник программы"), "stderr: {}", out.stderr);
    assert!(out.stdout.is_empty(), "stdout: {}", out.stdout);
}

#[test]
fn stdin_dash_runs_the_program_read_from_stdin() {
    let out = run(&["-"], "сказать(\"привет\", 1 + 2);\n");

    assert_eq!(out.stdout, "привет 3\n");
    assert_eq!(out.code, 0);
}

#[test]
fn unknown_top_level_flag_is_a_usage_error() {
    let ws = Workspace::new("unknown_flag");
    let prog = ws.write("p.yopta", "сказать(1);\n");

    let out = run(&["--nonsense", prog.to_str().unwrap()], "");

    assert_eq!(out.code, 2);
    assert!(out.stderr.contains("Неизвестный флаг: --nonsense"), "stderr: {}", out.stderr);
}

#[test]
fn requires_a_file_when_only_flags_are_given() {
    let out = run(&["--vm"], "");

    assert_eq!(out.code, 2);
    assert!(out.stderr.contains("Не указан файл"), "stderr: {}", out.stderr);
}

#[test]
fn vm_backend_runs_a_program() {
    let ws = Workspace::new("vm_ok");
    let prog = ws.write("p.yopta", "сказать(\"привет\", 1 + 2);\n");

    let out = run(&["--vm", prog.to_str().unwrap()], "");

    assert_eq!(out.stdout, "привет 3\n");
    assert_eq!(out.code, 0);
}

#[test]
fn script_arguments_follow_the_file() {
    let ws = Workspace::new("script_args");
    ws.write("argv.yopta", PRINT_ARGS);

    let out = run_in(ws.dir(), &["argv.yopta", "первый", "--флаг", "-"], "");

    assert_eq!(out.stdout, "argv.yopta\nпервый\n--флаг\n-\n", "stderr: {}", out.stderr);
    assert_eq!(out.code, 0);
}

#[test]
fn both_backends_see_the_same_script_arguments() {
    let ws = Workspace::new("script_args_vm");
    ws.write("argv.yopta", PRINT_ARGS);

    let interpreted = run_in(ws.dir(), &["argv.yopta", "х"], "");
    let compiled = run_in(ws.dir(), &["--vm", "argv.yopta", "х"], "");

    assert_eq!(interpreted.stdout, "argv.yopta\nх\n", "stderr: {}", interpreted.stderr);
    assert_eq!(compiled.stdout, interpreted.stdout, "stderr: {}", compiled.stderr);
}

#[test]
fn eval_takes_script_arguments_after_a_double_dash() {
    let out = run(&["-e", PRINT_ARGS, "--", "а", "--б"], "");

    assert_eq!(out.stdout, "-e\nа\n--б\n", "stderr: {}", out.stderr);
    assert_eq!(out.code, 0);
}

#[test]
fn stdin_program_takes_script_arguments() {
    let out = run(&["-", "а", "б"], PRINT_ARGS);

    assert_eq!(out.stdout, "-\nа\nб\n", "stderr: {}", out.stderr);
    assert_eq!(out.code, 0);
}

#[test]
fn double_dash_runs_a_file_whose_name_starts_with_a_dash() {
    let ws = Workspace::new("dash_file");
    ws.write("-странный.yopta", "сказать(\"из файла\");\n");

    let out = run_in(ws.dir(), &["--", "-странный.yopta"], "");

    assert_eq!(out.stdout, "из файла\n", "stderr: {}", out.stderr);
    assert_eq!(out.code, 0);
}

#[cfg(unix)]
#[test]
fn a_non_utf8_argument_is_reported_without_a_panic() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;
    use std::process::Command;

    let ws = Workspace::new("non_utf8");
    let mut command = Command::new(common::BIN);
    command.current_dir(ws.dir()).arg(OsStr::from_bytes(b"\xff.yopta"));

    let out = common::run_command(&mut command, b"");

    assert_eq!(out.code, 1, "stderr: {}", out.stderr);
    assert!(out.stderr.contains("Не удалось прочитать файл"), "stderr: {}", out.stderr);
    assert!(!out.stderr.contains("panicked"), "stderr: {}", out.stderr);
}

#[test]
fn a_runtime_error_inside_an_imported_module_names_the_module_file() {
    let ws = Workspace::new("module_error");
    ws.write("mod.yopta", "\nпредъява йопта упасть() {\n  гыы о = ноль;\n  отвечаю о.поле;\n}\n");
    ws.write("main.yopta", "спиздить { упасть } из \"./mod\";\n\nупасть();\n");

    for backend in [&["main.yopta"][..], &["--vm", "main.yopta"][..]] {
        let out = run_in(ws.dir(), backend, "");

        assert_eq!(out.code, 1, "{backend:?}: stderr: {}", out.stderr);
        assert!(out.stderr.contains("mod.yopta:4:11: "), "{backend:?}: stderr: {}", out.stderr);
        assert!(!out.stderr.contains("main.yopta:4:11"), "{backend:?}: stderr: {}", out.stderr);
    }
}

#[test]
fn a_module_error_keeps_the_caller_frame_in_the_main_file() {
    let ws = Workspace::new("module_frame");
    ws.write("mod.yopta", "\nпредъява йопта упасть() {\n  гыы о = ноль;\n  отвечаю о.поле;\n}\n");
    ws.write("main.yopta", "спиздить { упасть } из \"./mod\";\n\nупасть();\n");

    let out = run_in(ws.dir(), &["main.yopta"], "");

    assert!(out.stderr.contains("  в упасть:main.yopta:3:1"), "stderr: {}", out.stderr);
}

#[test]
fn an_uncaught_throw_inside_an_imported_module_names_the_module_file() {
    let ws = Workspace::new("module_throw");
    ws.write("mod.yopta", "\nпредъява йопта упасть() {\n  кидай \"из модуля\";\n}\n");
    ws.write("main.yopta", "спиздить { упасть } из \"./mod\";\n\nупасть();\n");

    for backend in [&["main.yopta"][..], &["--vm", "main.yopta"][..]] {
        let out = run_in(ws.dir(), backend, "");

        assert_eq!(out.code, 1, "{backend:?}: stderr: {}", out.stderr);
        assert!(out.stderr.contains("mod.yopta:3:3: "), "{backend:?}: stderr: {}", out.stderr);
        assert!(out.stderr.contains("Необработанное исключение: из модуля"), "{backend:?}: stderr: {}", out.stderr);
    }
}

#[test]
fn an_uncaught_top_level_throw_reports_its_own_line() {
    let ws = Workspace::new("throw_line");
    ws.write("throw.yopta", "гыы а = 1;\n\nкидай \"бум\";\n");

    for backend in [&["throw.yopta"][..], &["--vm", "throw.yopta"][..]] {
        let out = run_in(ws.dir(), backend, "");

        assert_eq!(out.code, 1, "{backend:?}: stderr: {}", out.stderr);
        assert!(out.stderr.starts_with("throw.yopta:3:1: "), "{backend:?}: stderr: {}", out.stderr);
    }
}

#[test]
fn a_syntax_error_in_an_imported_module_is_located_and_labelled() {
    let ws = Workspace::new("module_syntax");
    ws.write("bad.yopta", "предъява гыы х = 1;\nгыы у = ;\n");
    ws.write("main.yopta", "спиздить { х } из \"./bad\";\n");

    for backend in [&["main.yopta"][..], &["--vm", "main.yopta"][..]] {
        let out = run_in(ws.dir(), backend, "");

        assert_eq!(out.code, 1, "{backend:?}: stderr: {}", out.stderr);
        assert!(out.stderr.contains("bad.yopta:2:9: Ошибка: "), "{backend:?}: stderr: {}", out.stderr);
        assert!(!out.stderr.contains("Diagnostic {"), "{backend:?}: stderr: {}", out.stderr);
    }
}

#[cfg(unix)]
mod broken_pipe {
    use super::common::{Workspace, run_until_first_line};

    const SIGPIPE: i32 = 13;
    const NOISY_LOOP: &str = "го (гыы и = 0; и < 200000; и++) { сказать(и); }\n";

    fn assert_quiet_sigpipe(label: &str, out: &super::common::Run) {
        assert_eq!(out.signal, Some(SIGPIPE), "{label}: код {}, stderr: {}", out.code, out.stderr);
        assert!(out.stderr.is_empty(), "{label}: stderr: {}", out.stderr);
    }

    fn big_program(ws: &Workspace) -> String {
        let path = ws.write("big.yopta", &"сказать(1);\n".repeat(50_000));
        path.to_str().unwrap().to_string()
    }

    #[test]
    fn interpreter_dies_quietly_when_stdout_is_closed() {
        let out = run_until_first_line(&["-e", NOISY_LOOP], "");

        assert_eq!(out.stdout, "0\n");
        assert_quiet_sigpipe("интерпретатор", &out);
    }

    #[test]
    fn vm_dies_quietly_when_stdout_is_closed() {
        let out = run_until_first_line(&["--vm", "-e", NOISY_LOOP], "");

        assert_eq!(out.stdout, "0\n");
        assert_quiet_sigpipe("vm", &out);
    }

    #[test]
    fn repl_dies_quietly_when_stdout_is_closed() {
        let out = run_until_first_line(&["repl"], NOISY_LOOP);

        assert_eq!(out.stdout, "0\n");
        assert_quiet_sigpipe("repl", &out);
    }

    #[test]
    fn ast_dies_quietly_when_stdout_is_closed() {
        let ws = Workspace::new("pipe_ast");
        let big = big_program(&ws);

        let out = run_until_first_line(&["ast", &big], "");

        assert_quiet_sigpipe("ast", &out);
    }

    #[test]
    fn disasm_dies_quietly_when_stdout_is_closed() {
        let ws = Workspace::new("pipe_disasm");
        let big = big_program(&ws);

        let out = run_until_first_line(&["disasm", &big], "");

        assert_quiet_sigpipe("disasm", &out);
    }

    #[test]
    fn fmt_dies_quietly_when_stdout_is_closed() {
        let ws = Workspace::new("pipe_fmt");
        let big = big_program(&ws);

        let out = run_until_first_line(&["fmt", &big], "");

        assert_quiet_sigpipe("fmt", &out);
    }
}
