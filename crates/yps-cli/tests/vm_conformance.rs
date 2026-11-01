use std::path::{Path, PathBuf};
use std::process::Command;

const EXAMPLES_WITHOUT_PARITY: &[(&str, &str)] = &[
    ("io_demo.yopta", "печатает измеренное время, вывод не воспроизводится"),
    ("stack_trace.yopta", "завершается ошибкой, а стек есть только у интерпретатора"),
    ("weak.yopta", "вывод зависит от момента сборки мусора, он не гарантирован"),
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
    cmd.stdin(std::process::Stdio::null());
    let out = cmd.output().expect("не удалось запустить yps-cli");
    BackendRun {
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        code: out.status.code(),
    }
}

fn assert_conformant(file: &Path) {
    let interp = run_backend(file, false);
    let vm = run_backend(file, true);
    assert_eq!(interp.code, Some(0), "интерпретатор завершился с ошибкой на {}: {}", file.display(), interp.stderr);
    assert_eq!(vm.code, Some(0), "VM завершилась с ошибкой на {}: {}", file.display(), vm.stderr);
    assert_eq!(interp.stdout, vm.stdout, "stdout бэкендов расходится для {}", file.display());
    assert_eq!(interp.stderr, vm.stderr, "stderr бэкендов расходится для {}", file.display());
}

fn collect_programs() -> Vec<PathBuf> {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest.parent().and_then(Path::parent).expect("корень воркспейса");

    let mut programs: Vec<PathBuf> = yopta_files_in(&workspace_root.join("examples"))
        .into_iter()
        .filter(|p| !EXAMPLES_WITHOUT_PARITY.iter().any(|(name, _)| p.file_name().is_some_and(|f| f == *name)))
        .collect();
    programs.extend(yopta_files_in(&manifest.join("tests").join("vm_conformance")));
    programs
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
    assert!(programs.len() >= 5, "ожидалось минимум 5 программ, найдено {}", programs.len());
    for program in programs {
        assert_conformant(&program);
    }
}
