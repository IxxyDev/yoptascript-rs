use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

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
fn stdin_dash_runs_the_program_read_from_stdin() {
    let out = run(&["-"], "сказать(\"привет\", 1 + 2);\n");

    assert_eq!(out.stdout, "привет 3\n");
    assert_eq!(out.code, 0);
}

#[test]
fn unknown_top_level_flag_is_rejected() {
    let ws = Workspace::new("unknown_flag");
    let prog = ws.write("p.yopta", "сказать(1);\n");

    let out = run(&["--nonsense", prog.to_str().unwrap()], "");

    assert_eq!(out.code, 1);
    assert!(out.stderr.contains("Неизвестный флаг"), "stderr: {}", out.stderr);
}

#[test]
fn fmt_unknown_flag_is_rejected() {
    let ws = Workspace::new("fmt_unknown_flag");
    let prog = ws.write("p.yopta", "гыы x = 1;\n");

    let out = run(&["fmt", prog.to_str().unwrap(), "--nonsense"], "");

    assert_eq!(out.code, 1);
    assert!(out.stderr.contains("Неизвестный флаг"), "stderr: {}", out.stderr);
}

#[test]
fn requires_a_file_when_only_flags_are_given() {
    let out = run(&["--vm"], "");

    assert_eq!(out.code, 1);
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
fn fmt_without_a_file_prints_usage() {
    let out = run(&["fmt"], "");

    assert_eq!(out.code, 1);
    assert!(out.stderr.contains("Использование"), "stderr: {}", out.stderr);
}

#[test]
fn fmt_rejects_an_unknown_flag() {
    let ws = Workspace::new("fmt_flag");
    let prog = ws.write("f.yopta", "гыы x = 1;\n");

    let out = run(&["fmt", prog.to_str().unwrap(), "--bogus"], "");

    assert_eq!(out.code, 1);
    assert!(out.stderr.contains("Неизвестный флаг"), "stderr: {}", out.stderr);
}

#[test]
fn fmt_prints_canonical_form_to_stdout_without_touching_the_file() {
    let ws = Workspace::new("fmt_stdout");
    let messy = "гыы    x=1;\n";
    let prog = ws.write("f.yopta", messy);

    let out = run(&["fmt", prog.to_str().unwrap()], "");

    assert_eq!(out.stdout, "гыы x = 1;\n");
    assert_eq!(out.code, 0);
    assert_eq!(std::fs::read_to_string(&prog).unwrap(), messy);
}

#[test]
fn fmt_check_fails_on_unformatted_and_passes_on_formatted() {
    let ws = Workspace::new("fmt_check");
    let prog = ws.write("f.yopta", "гыы    x=1;\n");

    let unformatted = run(&["fmt", prog.to_str().unwrap(), "--check"], "");
    assert_eq!(unformatted.code, 1);

    ws.write("f.yopta", "гыы x = 1;\n");
    let formatted = run(&["fmt", prog.to_str().unwrap(), "--check"], "");
    assert_eq!(formatted.code, 0);
}

#[test]
fn fmt_write_rewrites_the_file_in_place() {
    let ws = Workspace::new("fmt_write");
    let prog = ws.write("f.yopta", "гыы    x=1;\n");

    let written = run(&["fmt", prog.to_str().unwrap(), "--write"], "");
    assert_eq!(written.code, 0);
    assert_eq!(std::fs::read_to_string(&prog).unwrap(), "гыы x = 1;\n");

    let recheck = run(&["fmt", prog.to_str().unwrap(), "--check"], "");
    assert_eq!(recheck.code, 0);
}

#[test]
fn fmt_source_map_emits_a_mapping_alongside_the_code() {
    let ws = Workspace::new("fmt_map");
    let prog = ws.write("g.yopta", "гыы y=2;\n");

    let out = run(&["fmt", prog.to_str().unwrap(), "--source-map"], "");

    assert_eq!(out.code, 0);
    assert!(out.stdout.contains("гыы y = 2;"), "stdout: {}", out.stdout);
    assert!(out.stdout.contains("\"version\":3"), "ожидался source map: {}", out.stdout);
}

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
fn repl_reset_clears_interpreter_state() {
    let out = run(&["repl"], "гыы z = 5;\n:сброс\nсказать(z);\n");

    assert!(out.stderr.contains("не определена"), "ожидалось, что z исчезнет: {}", out.stderr);
    assert_eq!(out.code, 0);
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
fn repl_exit_command_stops_processing_remaining_input() {
    let out = run(&["repl"], ":выход\nсказать(\"после\");\n");

    assert!(!out.stdout.contains("после"), "ввод после :выход не должен исполняться: {}", out.stdout);
    assert_eq!(out.code, 0);
}

#[test]
fn repl_incomplete_input_at_eof_fails() {
    let out = run(&["repl"], "йопта f() {\n");

    assert_eq!(out.code, 1);
    assert!(!out.stderr.is_empty(), "ожидалась диагностика незакрытого ввода");
}

#[test]
fn repl_recovers_after_a_parse_error() {
    let out = run(&["repl"], "гыы = ;\n7 + 0;\n");

    assert!(out.stdout.contains("7"), "REPL должен продолжить после ошибки: {}", out.stdout);
    assert_eq!(out.code, 0);
}

fn run(args: &[&str], stdin: &str) -> Run {
    let mut child = Command::new(env!("CARGO_BIN_EXE_yps-cli"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("не удалось запустить yps-cli");

    {
        let mut handle = child.stdin.take().expect("stdin недоступен");
        let _ = handle.write_all(stdin.as_bytes());
    }

    let out = child.wait_with_output().expect("ожидание завершения yps-cli");
    Run {
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        code: out.status.code().unwrap_or(-1),
    }
}

struct Run {
    stdout: String,
    stderr: String,
    code: i32,
}

struct Workspace {
    dir: PathBuf,
}

impl Workspace {
    fn new(tag: &str) -> Workspace {
        let dir = std::env::temp_dir().join(format!("yps_cli_it_{}_{}", tag, std::process::id()));
        std::fs::create_dir_all(&dir).expect("создать временный каталог");
        Workspace { dir }
    }

    fn write(&self, name: &str, contents: &str) -> PathBuf {
        let path = self.dir.join(name);
        std::fs::write(&path, contents).expect("записать тестовый файл");
        path
    }

    fn path(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }
}

impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}
