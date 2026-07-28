use std::fs;
use std::process::Command;

fn write_temp(name: &str, contents: &str) -> std::path::PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!("yps_cli_test_{}_{name}", std::process::id()));
    fs::write(&path, contents).unwrap();
    path
}

#[test]
fn ast_dump_contains_expected_nodes() {
    let path = write_temp("ast1.yopta", "гыы х = 1;\nсказать(х);\n");
    let output = Command::new(env!("CARGO_BIN_EXE_yps-cli")).args(["ast", path.to_str().unwrap()]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Program"));
    assert!(stdout.contains("VarDecl"));
    assert!(stdout.contains("Call"));
    let _ = fs::remove_file(&path);
}

#[test]
fn ast_reports_parse_errors_with_nonzero_exit() {
    let path = write_temp("ast_bad.yopta", "гыы х = ;\n");
    let output = Command::new(env!("CARGO_BIN_EXE_yps-cli")).args(["ast", path.to_str().unwrap()]).output().unwrap();
    assert!(!output.status.success());
    let _ = fs::remove_file(&path);
}

#[test]
fn disasm_contains_expected_opcodes() {
    let path = write_temp("disasm1.yopta", "йопта ф(а) { отвечаю а; }\nгыы и = 0;\nпотрещим (и < 2) { и = и + 1; }\n");
    let output = Command::new(env!("CARGO_BIN_EXE_yps-cli")).args(["disasm", path.to_str().unwrap()]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("proto ф"));
    assert!(stdout.contains("Closure"));
    assert!(stdout.contains("JumpIfFalse"));
    assert!(stdout.contains("Lt"));
    let _ = fs::remove_file(&path);
}

#[test]
fn subcommands_reject_unknown_flags() {
    let path = write_temp("ast_flag.yopta", "гыы х = 1;\n");
    let output =
        Command::new(env!("CARGO_BIN_EXE_yps-cli")).args(["ast", path.to_str().unwrap(), "--bogus"]).output().unwrap();
    assert!(!output.status.success());
    let _ = fs::remove_file(&path);
}

#[test]
fn lint_reports_unused_variable_and_exits_with_1() {
    let path =
        write_temp("lint_unused.yopta", "йопта ф() {\n  гыы неиспользуемая = 1;\n  отвечаю 0;\n}\nсказать(ф());\n");
    let output = Command::new(env!("CARGO_BIN_EXE_yps-cli")).args(["lint", path.to_str().unwrap()]).output().unwrap();
    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("unused-variable"));
    let _ = fs::remove_file(&path);
}

#[test]
fn lint_exits_with_0_on_a_clean_file() {
    let path = write_temp("lint_clean.yopta", "сказать(1);\n");
    let output = Command::new(env!("CARGO_BIN_EXE_yps-cli")).args(["lint", path.to_str().unwrap()]).output().unwrap();
    assert!(output.status.success());
    let _ = fs::remove_file(&path);
}

#[test]
fn lint_reports_parse_errors_with_nonzero_exit() {
    let path = write_temp("lint_bad.yopta", "гыы х = ;\n");
    let output = Command::new(env!("CARGO_BIN_EXE_yps-cli")).args(["lint", path.to_str().unwrap()]).output().unwrap();
    assert!(!output.status.success());
    let _ = fs::remove_file(&path);
}
