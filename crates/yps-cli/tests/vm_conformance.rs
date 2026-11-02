use std::path::{Path, PathBuf};
use std::process::Command;

const PROGRAMS_WITHOUT_PARITY: &[(&str, &str)] = &[
    ("io_demo.yopta", "печатает измеренное время, вывод не воспроизводится"),
    ("weak.yopta", "вывод зависит от момента сборки мусора, он не гарантирован"),
];

const PROGRAMS_THAT_FAIL: &[(&str, &str)] = &[
    ("stack_trace.yopta", "показывает отчёт о необработанном исключении"),
    ("negative_throw.yopta", "проверяет выход с необработанным исключением"),
];

struct BackendRun {
    stdout: String,
    stderr: String,
    code: Option<i32>,
}

fn run_backend(file: &Path, vm: bool) -> BackendRun {
    let bin = env!("CARGO_BIN_EXE_yps-cli");
    let mut cmd = Command::new(bin);
    if vm {
        cmd.arg("--vm");
    }
    cmd.arg(file);
    cmd.env("TZ", "UTC");
    cmd.stdin(std::process::Stdio::null());
    let out = cmd.output().expect("не удалось запустить yps-cli");
    BackendRun {
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        code: out.status.code(),
    }
}

fn is_listed(file: &Path, list: &[(&str, &str)]) -> bool {
    list.iter().any(|(name, _)| file.file_name().is_some_and(|f| f == *name))
}

fn assert_conformant(file: &Path) {
    let interp = run_backend(file, false);
    let vm = run_backend(file, true);
    assert_eq!(interp.stdout, vm.stdout, "stdout бэкендов расходится для {}", file.display());
    assert_eq!(interp.code, vm.code, "коды выхода расходятся для {}: {}", file.display(), vm.stderr);
    if is_listed(file, PROGRAMS_THAT_FAIL) {
        assert_eq!(interp.code, Some(1), "программа должна завершиться ошибкой: {}", file.display());
    } else {
        assert_eq!(interp.code, Some(0), "программа завершилась с ошибкой: {}: {}", file.display(), interp.stderr);
        assert_eq!(interp.stderr, vm.stderr, "stderr бэкендов расходится для {}", file.display());
    }
}

fn collect_programs() -> Vec<PathBuf> {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest.parent().and_then(Path::parent).expect("корень воркспейса");
    let tests = manifest.join("tests");
    [workspace_root.join("examples"), tests.join("vm_conformance"), tests.join("conformance").join("cases")]
        .iter()
        .flat_map(|dir| yopta_files_in(dir))
        .filter(|program| !is_listed(program, PROGRAMS_WITHOUT_PARITY))
        .collect()
}

fn yopta_files_in(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("каталог {}: {e}", dir.display()))
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "yopta"))
        .collect();
    files.sort();
    files
}

#[test]
fn vm_matches_interpreter_on_conformance_suite() {
    let programs = collect_programs();
    assert!(programs.len() >= 200, "ожидалось минимум 200 программ, найдено {}", programs.len());
    for program in programs {
        assert_conformant(&program);
    }
}
