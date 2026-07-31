use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const EXAMPLES: [&str; 5] = ["hello", "hoisting", "labeled_loops", "destructuring_defaults", "interop"];

fn examples_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("examples")
}

fn temp_path(name: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!("yps_transpile_test_{}_{name}", std::process::id()));
    path
}

fn write_temp(name: &str, contents: &str) -> PathBuf {
    let path = temp_path(name);
    fs::write(&path, contents).unwrap();
    path
}

fn node_available() -> bool {
    Command::new("node").arg("--version").output().is_ok_and(|o| o.status.success())
}

#[test]
fn transpile_prints_js_to_stdout() {
    let path = write_temp("basic.yopta", "гыы х = 1;\nсказать(\"х:\", х);\n");
    let output =
        Command::new(env!("CARGO_BIN_EXE_yps-cli")).args(["transpile", path.to_str().unwrap()]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, "let х = 1;\nconsole.log(\"х:\", х);\n");
    let _ = fs::remove_file(&path);
}

#[test]
fn transpile_writes_output_file() {
    let path = write_temp("out.yopta", "сказать(1);\n");
    let out_path = temp_path("out_result.js");
    let output = Command::new(env!("CARGO_BIN_EXE_yps-cli"))
        .args(["transpile", path.to_str().unwrap(), "-o", out_path.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(fs::read_to_string(&out_path).unwrap(), "console.log(1);\n");
    let _ = fs::remove_file(&path);
    let _ = fs::remove_file(&out_path);
}

#[test]
fn transpile_reports_unsupported_global_with_position() {
    let path = write_temp("unsupported.yopta", "сказать(Матан.пи);\n");
    let output =
        Command::new(env!("CARGO_BIN_EXE_yps-cli")).args(["transpile", path.to_str().unwrap()]).output().unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Матан"), "stderr: {stderr}");
    assert!(stderr.contains(":1:9:"), "stderr: {stderr}");
    let _ = fs::remove_file(&path);
}

#[test]
fn transpile_rejects_unknown_flags() {
    let path = write_temp("flag.yopta", "сказать(1);\n");
    let output = Command::new(env!("CARGO_BIN_EXE_yps-cli"))
        .args(["transpile", path.to_str().unwrap(), "--bogus"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let _ = fs::remove_file(&path);
}

#[test]
fn transpile_reports_parse_errors() {
    let path = write_temp("bad.yopta", "гыы х = ;\n");
    let output =
        Command::new(env!("CARGO_BIN_EXE_yps-cli")).args(["transpile", path.to_str().unwrap()]).output().unwrap();
    assert!(!output.status.success());
    let _ = fs::remove_file(&path);
}

fn assert_node_matches_interpreter_file(name: &str, source: &Path) {
    let js_path = temp_path(&format!("{name}.js"));

    let transpiled = Command::new(env!("CARGO_BIN_EXE_yps-cli"))
        .args(["transpile", source.to_str().unwrap(), "-o", js_path.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(transpiled.status.success(), "{name}: транспиляция упала: {transpiled:?}");

    let node = Command::new("node").arg(&js_path).output().unwrap();
    assert!(node.status.success(), "{name}: node упал: {}", String::from_utf8_lossy(&node.stderr));

    let interpreted = Command::new(env!("CARGO_BIN_EXE_yps-cli")).arg(source.to_str().unwrap()).output().unwrap();
    assert!(interpreted.status.success(), "{name}: интерпретатор упал");

    assert_eq!(
        String::from_utf8_lossy(&node.stdout),
        String::from_utf8_lossy(&interpreted.stdout),
        "{name}: вывод node и интерпретатора разошёлся"
    );

    let _ = fs::remove_file(&js_path);
}

fn assert_node_matches_interpreter(name: &str, source: &str) {
    let path = write_temp(&format!("{name}.yopta"), source);
    assert_node_matches_interpreter_file(name, &path);
    let _ = fs::remove_file(&path);
}

#[test]
fn switch_break_inside_loop_matches_interpreter_under_node() {
    if !node_available() {
        eprintln!("node не найден — сверка вывода пропущена");
        return;
    }
    assert_node_matches_interpreter(
        "switch_break",
        "го (гыы и = 0; и < 4; и++) { базарпо (и) { тема 2: { харэ; } нуичо { сказать(и); } } }\nсказать(\"конец\");\n",
    );
    assert_node_matches_interpreter(
        "switch_nested",
        "базарпо (1) {\n  тема 1: {\n    базарпо (2) { тема 2: { сказать(\"в\"); } нуичо { сказать(\"плохо\"); } }\n    сказать(\"после\");\n  }\n  нуичо { сказать(\"деф\"); }\n}\nбазарпо (\"нет\") { тема 1: { сказать(\"нет\"); } нуичо { сказать(\"деф2\"); } }\n",
    );
}

#[test]
fn typeof_of_a_class_matches_interpreter_under_node() {
    if !node_available() {
        eprintln!("node не найден — сверка вывода пропущена");
        return;
    }
    assert_node_matches_interpreter(
        "typeof_class",
        "клёво К {}\nйопта ф() {}\nсказать(тип(К));\nсказать(тип(ф));\nсказать(тип(Косяк));\n",
    );
}

#[test]
fn transpile_reports_date_used_as_a_namespace() {
    let path = write_temp("date_ns.yopta", "сказать(Дата.сейчас());\n");
    let output =
        Command::new(env!("CARGO_BIN_EXE_yps-cli")).args(["transpile", path.to_str().unwrap()]).output().unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Дата"), "stderr: {stderr}");
    assert!(stderr.contains(":1:9:"), "stderr: {stderr}");
    let _ = fs::remove_file(&path);
}

#[test]
fn transpiled_examples_match_interpreter_output_under_node() {
    if !node_available() {
        eprintln!("node не найден — сверка вывода пропущена");
        return;
    }

    for name in EXAMPLES {
        assert_node_matches_interpreter_file(name, &examples_dir().join(format!("{name}.yopta")));
    }
}
