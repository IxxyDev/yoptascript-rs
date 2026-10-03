use std::path::{Path, PathBuf};
use std::process::Command;

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

    let mut programs = vec![workspace_root.join("examples").join("hello.yopta")];

    let dir = manifest.join("tests").join("vm_conformance");
    let mut local: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("каталог vm_conformance")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "yopta"))
        .collect();
    local.sort();
    programs.extend(local);
    programs
}

#[test]
fn vm_matches_interpreter_on_conformance_suite() {
    let programs = collect_programs();
    assert!(programs.len() >= 5, "ожидалось минимум 5 программ, найдено {}", programs.len());
    for program in programs {
        assert_conformant(&program);
    }
}
