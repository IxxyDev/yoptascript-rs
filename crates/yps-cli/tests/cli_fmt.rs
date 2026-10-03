mod common;

use std::fs;
use std::time::{Duration, SystemTime};

use common::{Workspace, run, run_in};

const MESSY: &str = "гыы    x=1;\n";
const TIDY: &str = "гыы x = 1;\n";

#[test]
fn fmt_without_a_file_prints_usage() {
    let out = run(&["fmt"], "");

    assert_eq!(out.code, 2);
    assert!(out.stderr.contains("Использование: yps fmt"), "stderr: {}", out.stderr);
}

#[test]
fn fmt_rejects_an_unknown_flag() {
    let ws = Workspace::new("fmt_flag");
    let prog = ws.write("f.yopta", TIDY);

    let out = run(&["fmt", prog.to_str().unwrap(), "--bogus"], "");

    assert_eq!(out.code, 2);
    assert!(out.stderr.contains("Неизвестный флаг: --bogus"), "stderr: {}", out.stderr);
}

#[test]
fn fmt_rejects_a_second_file() {
    let ws = Workspace::new("fmt_two");
    let first = ws.write("a.yopta", MESSY);
    let second = ws.write("b.yopta", MESSY);

    let out = run(&["fmt", first.to_str().unwrap(), second.to_str().unwrap()], "");

    assert_eq!(out.code, 2);
    assert!(out.stderr.contains("Указан более чем один файл"), "stderr: {}", out.stderr);
}

#[test]
fn fmt_prints_canonical_form_to_stdout_without_touching_the_file() {
    let ws = Workspace::new("fmt_stdout");
    let prog = ws.write("f.yopta", MESSY);

    let out = run(&["fmt", prog.to_str().unwrap()], "");

    assert_eq!(out.stdout, TIDY);
    assert_eq!(out.code, 0);
    assert_eq!(fs::read_to_string(&prog).unwrap(), MESSY);
}

#[test]
fn fmt_check_fails_on_unformatted_and_passes_on_formatted() {
    let ws = Workspace::new("fmt_check");
    let prog = ws.write("f.yopta", MESSY);

    let unformatted = run(&["fmt", prog.to_str().unwrap(), "--check"], "");
    assert_eq!(unformatted.code, 1);

    ws.write("f.yopta", TIDY);
    let formatted = run(&["fmt", prog.to_str().unwrap(), "--check"], "");
    assert_eq!(formatted.code, 0);
}

#[test]
fn fmt_accepts_flags_before_the_file() {
    let ws = Workspace::new("fmt_flag_first");
    let prog = ws.write("f.yopta", MESSY);

    let checked = run(&["fmt", "--check", prog.to_str().unwrap()], "");
    assert_eq!(checked.code, 1, "stderr: {}", checked.stderr);
    assert!(checked.stderr.is_empty(), "stderr: {}", checked.stderr);

    let written = run(&["fmt", "-w", prog.to_str().unwrap()], "");
    assert_eq!(written.code, 0, "stderr: {}", written.stderr);
    assert_eq!(fs::read_to_string(&prog).unwrap(), TIDY);
}

#[test]
fn fmt_rejects_check_combined_with_write() {
    let ws = Workspace::new("fmt_check_write");
    let prog = ws.write("f.yopta", MESSY);

    let out = run(&["fmt", prog.to_str().unwrap(), "--check", "--write"], "");

    assert_eq!(out.code, 2);
    assert!(out.stderr.contains("--check и --write нельзя использовать вместе"), "stderr: {}", out.stderr);
    assert_eq!(fs::read_to_string(&prog).unwrap(), MESSY);
}

#[test]
fn fmt_rejects_check_combined_with_source_map() {
    let ws = Workspace::new("fmt_check_map");
    let prog = ws.write("f.yopta", MESSY);

    let out = run(&["fmt", prog.to_str().unwrap(), "--check", "--source-map"], "");

    assert_eq!(out.code, 2);
    assert!(out.stderr.contains("--check и --source-map нельзя использовать вместе"), "stderr: {}", out.stderr);
    assert_eq!(ws.entries(), ["f.yopta"]);
}

#[test]
fn fmt_write_rewrites_the_file_in_place() {
    let ws = Workspace::new("fmt_write");
    let prog = ws.write("f.yopta", MESSY);

    let written = run(&["fmt", prog.to_str().unwrap(), "--write"], "");
    assert_eq!(written.code, 0);
    assert_eq!(fs::read_to_string(&prog).unwrap(), TIDY);

    let recheck = run(&["fmt", prog.to_str().unwrap(), "--check"], "");
    assert_eq!(recheck.code, 0);
}

#[test]
fn fmt_write_leaves_no_temporary_files_behind() {
    let ws = Workspace::new("fmt_no_tmp");
    let prog = ws.write("f.yopta", MESSY);

    let out = run(&["fmt", prog.to_str().unwrap(), "--write"], "");

    assert_eq!(out.code, 0, "stderr: {}", out.stderr);
    assert_eq!(ws.entries(), ["f.yopta"]);
}

#[test]
fn fmt_write_leaves_an_unrelated_fmt_tmp_file_alone() {
    let ws = Workspace::new("fmt_tmp_clash");
    let prog = ws.write("f.yopta", MESSY);
    let bystander = ws.write("f.yopta.fmt_tmp", "важное\n");

    let out = run(&["fmt", prog.to_str().unwrap(), "--write"], "");

    assert_eq!(out.code, 0, "stderr: {}", out.stderr);
    assert_eq!(fs::read_to_string(&prog).unwrap(), TIDY);
    assert_eq!(fs::read_to_string(&bystander).unwrap(), "важное\n");
}

#[test]
fn fmt_write_does_not_touch_an_already_formatted_file() {
    let ws = Workspace::new("fmt_untouched");
    let prog = ws.write("f.yopta", TIDY);
    let old = SystemTime::now() - Duration::from_secs(3600);
    fs::File::options().write(true).open(&prog).unwrap().set_modified(old).unwrap();
    let before = fs::metadata(&prog).unwrap().modified().unwrap();

    let out = run(&["fmt", prog.to_str().unwrap(), "--write"], "");

    assert_eq!(out.code, 0, "stderr: {}", out.stderr);
    assert_eq!(fs::metadata(&prog).unwrap().modified().unwrap(), before);
}

#[cfg(unix)]
#[test]
fn fmt_write_preserves_file_permissions() {
    use std::os::unix::fs::PermissionsExt;

    let ws = Workspace::new("fmt_perms");
    let prog = ws.write("f.yopta", MESSY);
    fs::set_permissions(&prog, fs::Permissions::from_mode(0o755)).unwrap();

    let out = run(&["fmt", prog.to_str().unwrap(), "--write"], "");

    assert_eq!(out.code, 0, "stderr: {}", out.stderr);
    assert_eq!(fs::read_to_string(&prog).unwrap(), TIDY);
    assert_eq!(fs::metadata(&prog).unwrap().permissions().mode() & 0o777, 0o755);
}

#[cfg(unix)]
#[test]
fn fmt_write_refuses_a_read_only_file() {
    use std::os::unix::fs::PermissionsExt;

    let ws = Workspace::new("fmt_readonly");
    let prog = ws.write("f.yopta", MESSY);
    fs::set_permissions(&prog, fs::Permissions::from_mode(0o444)).unwrap();

    let out = run(&["fmt", prog.to_str().unwrap(), "--write"], "");

    assert_eq!(out.code, 1);
    assert!(out.stderr.contains("доступен только для чтения"), "stderr: {}", out.stderr);
    assert_eq!(fs::read_to_string(&prog).unwrap(), MESSY);
    assert_eq!(fs::metadata(&prog).unwrap().permissions().mode() & 0o777, 0o444);
    assert_eq!(ws.entries(), ["f.yopta"]);
}

#[cfg(unix)]
#[test]
fn fmt_write_through_a_symlink_formats_the_target_and_keeps_the_link() {
    let ws = Workspace::new("fmt_symlink");
    let target = ws.write("real.yopta", MESSY);
    let link = ws.path("link.yopta");
    std::os::unix::fs::symlink(&target, &link).unwrap();

    let out = run(&["fmt", link.to_str().unwrap(), "--write"], "");

    assert_eq!(out.code, 0, "stderr: {}", out.stderr);
    assert!(fs::symlink_metadata(&link).unwrap().file_type().is_symlink(), "ссылка должна остаться ссылкой");
    assert_eq!(fs::read_to_string(&target).unwrap(), TIDY);
}

#[test]
fn fmt_source_map_requires_write() {
    let ws = Workspace::new("fmt_map");
    ws.write("g.yopta", "гыы y=2;\n");

    let out = run_in(ws.dir(), &["fmt", "g.yopta", "--source-map"], "");

    assert_eq!(out.code, 2);
    assert!(out.stderr.contains("Флаг --source-map требует --write"), "stderr: {}", out.stderr);
    assert!(out.stdout.is_empty(), "stdout: {}", out.stdout);
    assert_eq!(ws.entries(), ["g.yopta"]);
}

#[cfg(unix)]
#[test]
fn fmt_source_map_is_not_written_when_the_file_cannot_be_rewritten() {
    use std::os::unix::fs::PermissionsExt;

    let ws = Workspace::new("fmt_map_readonly");
    let prog = ws.write("f.yopta", MESSY);
    fs::set_permissions(&prog, fs::Permissions::from_mode(0o444)).unwrap();

    let out = run(&["fmt", prog.to_str().unwrap(), "--write", "--source-map"], "");

    assert_eq!(out.code, 1);
    assert_eq!(ws.entries(), ["f.yopta"]);
}

#[test]
fn fmt_source_map_names_the_formatted_file_not_the_map() {
    let ws = Workspace::new("fmt_map_file");
    let prog = ws.write("g.yopta", "гыы y=2;\n");

    let out = run(&["fmt", prog.to_str().unwrap(), "--source-map", "--write"], "");

    assert_eq!(out.code, 0, "stderr: {}", out.stderr);
    let map = fs::read_to_string(ws.path("g.yopta.map")).unwrap();
    assert!(map.contains("\"file\":\"g.yopta\""), "map: {map}");
    assert!(map.contains("\"sources\":[\"g.yopta\"]"), "map: {map}");
    assert_eq!(fs::read_to_string(&prog).unwrap(), "гыы y = 2;\n");
    assert_eq!(ws.entries(), ["g.yopta", "g.yopta.map"]);
}

#[test]
fn fmt_reports_a_syntax_error_with_a_russian_severity() {
    let ws = Workspace::new("fmt_syntax");
    ws.write("bad.yopta", "гыы x = ;\n");

    let out = run_in(ws.dir(), &["fmt", "bad.yopta"], "");

    assert_eq!(out.code, 1);
    assert!(out.stderr.contains("bad.yopta:1:9: Ошибка: "), "stderr: {}", out.stderr);
    assert!(out.stderr.contains("Форматирование отклонено"), "stderr: {}", out.stderr);
}
