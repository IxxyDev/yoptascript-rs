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

fn normalize(s: &str) -> String {
    s.replace("\r\n", "\n")
}

fn check(case_name: &str) {
    let dir = conformance_dir();
    let case_path = dir.join("cases").join(format!("{case_name}.yop"));
    let golden_path = dir.join("golden").join(format!("{case_name}.txt"));
    assert!(case_path.exists(), "нет кейса: {}", case_path.display());

    let actual = normalize(&run_case(&case_path));

    if std::env::var("YPS_CONFORMANCE_BLESS").is_ok() {
        std::fs::write(&golden_path, &actual).expect("запись golden");
        return;
    }

    let expected = std::fs::read_to_string(&golden_path)
        .unwrap_or_else(|_| panic!("нет golden: {} (запустите с YPS_CONFORMANCE_BLESS=1)", golden_path.display()));
    let expected = normalize(&expected);

    assert_eq!(
        actual, expected,
        "\nрасхождение conformance в кейсе '{case_name}'\n--- ожидалось (golden) ---\n{expected}\n--- получено ---\n{actual}\n"
    );
}

macro_rules! conformance_cases {
    ($($name:ident => $case:literal),+ $(,)?) => {
        $(
            #[test]
            fn $name() {
                check($case);
            }
        )+
    };
}

conformance_cases! {
    coercion_equality => "coercion_equality",
    coercion_add => "coercion_add",
    coercion_stringify => "coercion_stringify",
    coercion_user_hooks => "coercion_user_hooks",
    strict_callsites => "strict_callsites",
    example_hello => "example_hello",
    example_hoisting => "example_hoisting",
    example_labeled_loops => "example_labeled_loops",
    example_destructuring_defaults => "example_destructuring_defaults",
    example_tagged_templates => "example_tagged_templates",
    example_decorators => "example_decorators",
    example_for_await_of => "example_for_await_of",
    example_promise_smoke1 => "example_promise_smoke1",
    example_promise_smoke2 => "example_promise_smoke2",
    example_dynamic_import => "example_dynamic_import",
    example_import_json => "example_import_json",
    example_event_loop => "example_event_loop",
    example_async_timers => "example_async_timers",
}
