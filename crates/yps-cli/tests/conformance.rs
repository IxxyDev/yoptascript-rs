use std::path::{Path, PathBuf};
use std::process::Command;

fn cli_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_yps-cli"))
}

fn conformance_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests").join("conformance")
}

fn run_case(case_path: &Path) -> String {
    let bin = cli_binary();
    let output = Command::new(&bin)
        .arg(case_path)
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap_or_else(|e| panic!("не удалось запустить {}: {e}", bin.display()));
    let mut combined = String::from_utf8_lossy(&output.stdout).into_owned();
    if !output.status.success() {
        combined.push_str("---STDERR---\n");
        combined.push_str(&String::from_utf8_lossy(&output.stderr));
        combined.push_str(&format!("---EXIT:{}---\n", output.status.code().unwrap_or(-1)));
    }
    combined
}

fn normalize_output(s: &str, case_path: &Path, cases_dir: &Path) -> String {
    let s = s.replace("\r\n", "\n");
    let case_str = case_path.to_string_lossy();
    let cases_str = cases_dir.to_string_lossy();
    let s = s.replace(case_str.as_ref(), "<КЕЙС>");
    s.replace(cases_str.as_ref(), "<КЕЙСЫ>")
}

fn discover_cases(cases_dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(cases_dir)
        .unwrap_or_else(|e| panic!("не удалось прочитать каталог кейсов: {e}"))
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if path.is_file() && path.extension().and_then(|e| e.to_str()) == Some("yopta") {
                path.file_stem().and_then(|s| s.to_str()).map(|s| s.to_owned())
            } else {
                None
            }
        })
        .collect();
    names.sort();
    names
}

#[test]
fn conformance() {
    let dir = conformance_dir();
    let cases_dir = dir.join("cases");
    let golden_dir = dir.join("golden");

    let mut case_names = discover_cases(&cases_dir);
    if let Ok(filter) = std::env::var("YPS_CONFORMANCE_FILTER") {
        let prefixes: Vec<&str> = filter.split(',').filter(|p| !p.is_empty()).collect();
        case_names.retain(|name| prefixes.iter().any(|p| name.starts_with(p)));
        assert!(!case_names.is_empty(), "фильтр '{filter}' не выбрал ни одного кейса");
    }
    let bless = std::env::var("YPS_CONFORMANCE_BLESS").is_ok();
    let mut failures: Vec<String> = Vec::new();

    for name in &case_names {
        let case_path = cases_dir.join(format!("{name}.yopta"));
        let golden_path = golden_dir.join(format!("{name}.txt"));

        assert!(case_path.exists(), "нет кейса: {}", case_path.display());

        let actual = normalize_output(&run_case(&case_path), &case_path, &cases_dir);

        if bless {
            std::fs::write(&golden_path, &actual).expect("запись golden");
            continue;
        }

        let expected = std::fs::read_to_string(&golden_path).unwrap_or_else(|_| {
            panic!("нет golden для кейса '{}': {} (запустите с YPS_CONFORMANCE_BLESS=1)", name, golden_path.display())
        });
        let expected = expected.replace("\r\n", "\n");

        if actual != expected {
            failures
                .push(format!("кейс '{name}':\n--- ожидалось (golden) ---\n{expected}\n--- получено ---\n{actual}"));
        }
    }

    if !failures.is_empty() {
        let report = failures.join("\n\n---\n\n");
        panic!("\nрасхождение conformance в {} кейс(ах):\n\n{report}\n", failures.len());
    }
}
