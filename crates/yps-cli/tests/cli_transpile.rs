mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use common::{Workspace, run, run_in};

const EXAMPLES: [&str; 5] = ["hello", "hoisting", "labeled_loops", "destructuring_defaults", "interop"];

fn examples_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("examples")
}

fn node_available() -> bool {
    Command::new("node").arg("--version").output().is_ok_and(|o| o.status.success())
}

#[test]
fn transpile_prints_js_to_stdout() {
    let ws = Workspace::new("tr_basic");
    let path = ws.write("basic.yopta", "гыы х = 1;\nсказать(\"х:\", х);\n");

    let out = run(&["transpile", path.to_str().unwrap()], "");

    assert_eq!(out.code, 0);
    assert_eq!(out.stdout, "let х = 1;\nconsole.log(\"х:\", х);\n");
}

#[test]
fn transpile_writes_output_file() {
    let ws = Workspace::new("tr_out");
    let path = ws.write("out.yopta", "сказать(1);\n");
    let out_path = ws.path("out_result.js");

    let out = run(&["transpile", path.to_str().unwrap(), "-o", out_path.to_str().unwrap()], "");

    assert_eq!(out.code, 0, "stderr: {}", out.stderr);
    assert_eq!(fs::read_to_string(&out_path).unwrap(), "console.log(1);\n");
    assert_eq!(ws.entries(), ["out.yopta", "out_result.js"]);
}

#[test]
fn transpile_replaces_an_existing_output_file() {
    let ws = Workspace::new("tr_replace");
    let path = ws.write("out.yopta", "сказать(1);\n");
    let out_path = ws.write("old.js", "старое содержимое\n");

    let out = run(&["transpile", path.to_str().unwrap(), "--output", out_path.to_str().unwrap()], "");

    assert_eq!(out.code, 0, "stderr: {}", out.stderr);
    assert_eq!(fs::read_to_string(&out_path).unwrap(), "console.log(1);\n");
    assert_eq!(ws.entries(), ["old.js", "out.yopta"]);
}

#[cfg(unix)]
#[test]
fn transpile_writes_to_a_device_file() {
    let ws = Workspace::new("tr_device");
    let path = ws.write("t.yopta", "сказать(1);\n");

    let out = run(&["transpile", path.to_str().unwrap(), "-o", "/dev/null"], "");

    assert_eq!(out.code, 0, "stderr: {}", out.stderr);
    assert!(out.stderr.is_empty(), "stderr: {}", out.stderr);
    assert_eq!(ws.entries(), ["t.yopta"]);
}

#[cfg(unix)]
#[test]
fn transpile_writes_through_a_dangling_symlink() {
    let ws = Workspace::new("tr_dangling");
    let path = ws.write("t.yopta", "сказать(1);\n");
    let link = ws.path("out.js");
    std::os::unix::fs::symlink(ws.path("real.js"), &link).unwrap();

    let out = run(&["transpile", path.to_str().unwrap(), "-o", link.to_str().unwrap()], "");

    assert_eq!(out.code, 0, "stderr: {}", out.stderr);
    assert!(fs::symlink_metadata(&link).unwrap().file_type().is_symlink(), "ссылка должна остаться ссылкой");
    assert_eq!(fs::read_to_string(ws.path("real.js")).unwrap(), "console.log(1);\n");
}

#[cfg(unix)]
fn with_read_only_directory(ws: &Workspace, name: &str, body: impl FnOnce(&Path) -> common::Run) -> common::Run {
    use std::os::unix::fs::PermissionsExt;

    let dir = ws.path(name);
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o555)).unwrap();
    let out = body(&dir);
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).unwrap();
    out
}

#[cfg(unix)]
#[test]
fn transpile_rewrites_a_writable_file_inside_a_read_only_directory() {
    let ws = Workspace::new("tr_ro_dir_file");
    let path = ws.write("t.yopta", "сказать(1);\n");
    fs::create_dir(ws.path("закрыто")).unwrap();
    fs::write(ws.path("закрыто").join("w.js"), "старое\n").unwrap();

    let out = with_read_only_directory(&ws, "закрыто", |dir| {
        run(&["transpile", path.to_str().unwrap(), "-o", dir.join("w.js").to_str().unwrap()], "")
    });

    assert_eq!(out.code, 0, "stderr: {}", out.stderr);
    assert_eq!(fs::read_to_string(ws.path("закрыто").join("w.js")).unwrap(), "console.log(1);\n");
}

#[cfg(unix)]
#[test]
fn transpile_reports_an_unwritable_directory_without_blaming_the_file() {
    use std::os::unix::fs::MetadataExt;

    let ws = Workspace::new("tr_ro_dir_new");
    if fs::metadata(ws.dir()).unwrap().uid() == 0 {
        return;
    }
    let path = ws.write("t.yopta", "сказать(1);\n");
    fs::create_dir(ws.path("закрыто")).unwrap();

    let out = with_read_only_directory(&ws, "закрыто", |dir| {
        run(&["transpile", path.to_str().unwrap(), "-o", dir.join("new.js").to_str().unwrap()], "")
    });

    assert_eq!(out.code, 1);
    assert!(out.stderr.contains("Не удалось записать файл"), "stderr: {}", out.stderr);
    assert!(!out.stderr.contains("только для чтения"), "stderr: {}", out.stderr);
}

#[test]
fn transpile_refuses_to_overwrite_its_own_input() {
    let ws = Workspace::new("tr_same");
    ws.write("t.yopta", "сказать(1);\n");

    let out = run_in(ws.dir(), &["transpile", "t.yopta", "-o", "./t.yopta"], "");

    assert_eq!(out.code, 2);
    assert!(out.stderr.contains("Выходной файл совпадает с исходным"), "stderr: {}", out.stderr);
    assert_eq!(fs::read_to_string(ws.path("t.yopta")).unwrap(), "сказать(1);\n");
}

#[test]
fn transpile_output_flag_requires_a_path() {
    let ws = Workspace::new("tr_no_path");
    let path = ws.write("t.yopta", "сказать(1);\n");

    let out = run(&["transpile", path.to_str().unwrap(), "-o"], "");

    assert_eq!(out.code, 2);
    assert!(out.stderr.contains("Флаг -o требует путь к файлу"), "stderr: {}", out.stderr);
}

#[test]
fn transpile_rejects_a_repeated_output_flag() {
    let ws = Workspace::new("tr_two_outputs");
    let path = ws.write("t.yopta", "сказать(1);\n");
    let first = ws.path("a.js");
    let second = ws.path("b.js");

    let out =
        run(&["transpile", path.to_str().unwrap(), "-o", first.to_str().unwrap(), "-o", second.to_str().unwrap()], "");

    assert_eq!(out.code, 2);
    assert!(out.stderr.contains("Флаг -o указан более одного раза"), "stderr: {}", out.stderr);
    assert_eq!(ws.entries(), ["t.yopta"]);
}

#[test]
fn transpile_reports_unknown_member_of_a_supported_namespace() {
    let ws = Workspace::new("tr_member");
    let path = ws.write("unknown_member.yopta", "сказать(Матан.пи);\n");

    let out = run(&["transpile", path.to_str().unwrap()], "");

    assert_eq!(out.code, 1);
    assert!(out.stderr.contains("Матан"), "stderr: {}", out.stderr);
    assert!(out.stderr.contains("пи"), "stderr: {}", out.stderr);
    assert!(out.stderr.contains("нет члена"), "stderr: {}", out.stderr);
    assert!(!out.stderr.contains("глобальный объект"), "stderr: {}", out.stderr);
    assert!(out.stderr.contains(":1:9:"), "stderr: {}", out.stderr);
}

#[test]
fn transpile_reports_unsupported_global_with_position() {
    let ws = Workspace::new("tr_global");
    let path = ws.write("unsupported.yopta", "сказать(Помойка.ключи(о));\n");

    let out = run(&["transpile", path.to_str().unwrap()], "");

    assert_eq!(out.code, 1);
    assert!(out.stderr.contains("Помойка"), "stderr: {}", out.stderr);
    assert!(out.stderr.contains(":1:9:"), "stderr: {}", out.stderr);
}

#[test]
fn transpile_reports_date_used_as_a_namespace() {
    let ws = Workspace::new("tr_date");
    let path = ws.write("date_ns.yopta", "сказать(Дата.сейчас());\n");

    let out = run(&["transpile", path.to_str().unwrap()], "");

    assert_eq!(out.code, 1);
    assert!(out.stderr.contains("Дата"), "stderr: {}", out.stderr);
    assert!(out.stderr.contains(":1:9:"), "stderr: {}", out.stderr);
}

#[test]
fn transpile_rejects_unknown_flags() {
    let ws = Workspace::new("tr_flag");
    let path = ws.write("flag.yopta", "сказать(1);\n");

    let out = run(&["transpile", path.to_str().unwrap(), "--bogus"], "");

    assert_eq!(out.code, 2);
    assert!(out.stderr.contains("Неизвестный флаг: --bogus"), "stderr: {}", out.stderr);
}

#[test]
fn transpile_reports_parse_errors() {
    let ws = Workspace::new("tr_parse");
    let path = ws.write("bad.yopta", "гыы х = ;\n");

    let out = run(&["transpile", path.to_str().unwrap()], "");

    assert_eq!(out.code, 1);
    assert!(out.stderr.contains("Неожиданный токен"), "stderr: {}", out.stderr);
    assert!(out.stderr.contains(":1:9:"), "stderr: {}", out.stderr);
}

fn assert_node_matches_interpreter_file(ws: &Workspace, name: &str, source: &Path) {
    let js_path = ws.path(&format!("{name}.js"));

    let transpiled = run(&["transpile", source.to_str().unwrap(), "-o", js_path.to_str().unwrap()], "");
    assert_eq!(transpiled.code, 0, "{name}: транспиляция упала: {}", transpiled.stderr);

    let node = Command::new("node").arg(&js_path).output().unwrap();
    assert!(node.status.success(), "{name}: node упал: {}", String::from_utf8_lossy(&node.stderr));

    let interpreted = run(&[source.to_str().unwrap()], "");
    assert_eq!(interpreted.code, 0, "{name}: интерпретатор упал");

    assert_eq!(
        String::from_utf8_lossy(&node.stdout),
        interpreted.stdout,
        "{name}: вывод node и интерпретатора разошёлся"
    );
}

fn assert_node_matches_interpreter(name: &str, source: &str) {
    let ws = Workspace::new(&format!("tr_node_{name}"));
    let path = ws.write(&format!("{name}.yopta"), source);
    assert_node_matches_interpreter_file(&ws, name, &path);
}

#[test]
#[ignore = "requires node on PATH; run with `cargo test -- --ignored`"]
fn switch_break_inside_loop_matches_interpreter_under_node() {
    assert!(node_available(), "node не найден — запустите `cargo test -- --ignored` на машине с node");
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
#[ignore = "requires node on PATH; run with `cargo test -- --ignored`"]
fn typeof_of_a_class_matches_interpreter_under_node() {
    assert!(node_available(), "node не найден — запустите `cargo test -- --ignored` на машине с node");
    assert_node_matches_interpreter(
        "typeof_class",
        "клёво К {}\nйопта ф() {}\nсказать(тип(К));\nсказать(тип(ф));\nсказать(тип(Косяк));\n",
    );
}

#[test]
#[ignore = "requires node on PATH; run with `cargo test -- --ignored`"]
fn math_namespace_matches_interpreter_under_node() {
    assert!(node_available(), "node не найден — запустите `cargo test -- --ignored` на машине с node");
    assert_node_matches_interpreter(
        "math_ns",
        "сказать(Матан.ПИ, Матан.КОРЕНЬ2);\nсказать(Матан.округлить(1.5), Матан.пол(-1.2), Матан.потолок(1.2));\nсказать(Матан.степень(2, 10), Матан.корень(144), Матан.модуль(-7));\nсказать(Матан.мин(3, 1, 2), Матан.макс(3, 1, 2), Матан.гипотенуза(3, 4));\nсказать(Матан.арктангенс2(1, 1), Матан.знак(-5), Матан.обрезать(4.9));\nсказать(Матан.умножить32(3, 4), Матан.нулиСлева32(1), Матан.дробь32(1.5));\n",
    );
}

#[test]
#[ignore = "requires node on PATH; run with `cargo test -- --ignored`"]
fn math_round_and_hypot_shims_match_interpreter_under_node() {
    assert!(node_available(), "node не найден — запустите `cargo test -- --ignored` на машине с node");
    assert_node_matches_interpreter(
        "math_round_hypot",
        "сказать(Матан.округлить(-1.5), Матан.округлить(-0.5), Матан.округлить(-2.5));\nсказать(Матан.округлить(1.5), Матан.округлить(0.5), Матан.округлить(2.5));\nсказать(Матан.округлить(-1.4), Матан.округлить(-1.6), Матан.округлить(0));\nсказать(Матан.гипотенуза(1e200, 1e200));\nсказать(Матан.гипотенуза(3e150, 4e150), Матан.гипотенуза(3, 4));\nсказать(Матан.гипотенуза(1e-200, 1e-200));\n",
    );
}

#[test]
#[ignore = "requires node on PATH; run with `cargo test -- --ignored`"]
fn json_namespace_matches_interpreter_under_node() {
    assert!(node_available(), "node не найден — запустите `cargo test -- --ignored` на машине с node");
    assert_node_matches_interpreter(
        "json_ns",
        "гыы о = { а: 1, б: [1, 2, 3], в: \"текст\" };\nгыы с = Жсон.вСтроку(о);\nсказать(с);\nгыы з = Жсон.разобрать(с);\nсказать(з.а, з.в, Жсон.вСтроку(з.б));\n",
    );
}

#[test]
#[ignore = "requires node on PATH; run with `cargo test -- --ignored`"]
fn reflect_namespace_matches_interpreter_under_node() {
    assert!(node_available(), "node не найден — запустите `cargo test -- --ignored` на машине с node");
    assert_node_matches_interpreter(
        "reflect_ns",
        "гыы о = { а: 1, б: 2 };\nсказать(Отражение.получить(о, \"а\"));\nОтражение.установить(о, \"в\", 3);\nсказать(Отражение.есть(о, \"в\"), Отражение.есть(о, \"г\"));\nсказать(длина(Отражение.собственныеКлючи(о)));\nОтражение.удалить(о, \"б\");\nсказать(Отражение.есть(о, \"б\"), Отражение.расширяем(о));\n",
    );
}

#[test]
#[ignore = "requires node on PATH; run with `cargo test -- --ignored`"]
fn transpiled_examples_match_interpreter_output_under_node() {
    assert!(node_available(), "node не найден — запустите `cargo test -- --ignored` на машине с node");

    let ws = Workspace::new("tr_node_examples");
    for name in EXAMPLES {
        assert_node_matches_interpreter_file(&ws, name, &examples_dir().join(format!("{name}.yopta")));
    }
}
