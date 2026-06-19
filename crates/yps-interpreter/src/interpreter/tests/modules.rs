use super::*;

fn run_with_module(module_src: &str, main_src: &str) -> Interpreter {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let id = SEQ.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("yps_dynimp_{}_{id}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("модуль.yop"), module_src).unwrap();

    let source = SourceFile::new("test".to_string(), main_src.to_string());
    let (tokens, lex_diags) = Lexer::new(&source).tokenize();
    assert!(lex_diags.is_empty(), "Ошибки лексера: {lex_diags:?}");
    let (program, parse_diags) = Parser::new(&tokens, &source).parse_program();
    assert!(parse_diags.is_empty(), "Ошибки парсера: {parse_diags:?}");
    let mut interp = Interpreter::new();
    interp.set_base_path(dir.clone());
    interp.run(&program).expect("Ошибка интерпретатора");
    let _ = std::fs::remove_dir_all(&dir);
    interp
}

#[test]
fn dynamic_import_returns_namespace() {
    let module = r#"
        предъява гыы х = 42;
        предъява йопта удвоить(а) { отвечаю а * 2; }
    "#;
    let main = r#"
        ассо йопта главная() {
            гыы м = сидетьНахуй спиздить("./модуль");
            результат_х = м.х;
            результат_удв = м.удвоить(10);
        }
        гыы результат_х = 0;
        гыы результат_удв = 0;
        главная();
    "#;
    let interp = run_with_module(module, main);
    assert_eq!(interp.get("результат_х"), Some(Value::Number(42.0)));
    assert_eq!(interp.get("результат_удв"), Some(Value::Number(20.0)));
}

#[test]
fn dynamic_import_missing_module_rejects() {
    let source = SourceFile::new(
        "test".to_string(),
        r#"
        ассо йопта главная() {
            хапнуть {
                сидетьНахуй спиздить("./нету_такого");
            } гоп (е) {
                поймано = правда;
            }
        }
        гыы поймано = лож;
        главная();
        "#
        .to_string(),
    );
    let (tokens, _) = Lexer::new(&source).tokenize();
    let (program, _) = Parser::new(&tokens, &source).parse_program();
    let mut interp = Interpreter::new();
    interp.set_base_path(std::env::temp_dir());
    interp.run(&program).expect("await rejected promise превращается в ошибку, поймано в try/catch");
    assert_eq!(interp.get("поймано"), Some(Value::Boolean(true)));
}

#[test]
fn dynamic_import_at_expr_position_parses() {
    let source = SourceFile::new("test".to_string(), r#"гыы пром = спиздить("./нет");"#.to_string());
    let (tokens, lex_diags) = Lexer::new(&source).tokenize();
    assert!(lex_diags.is_empty());
    let (_program, parse_diags) = Parser::new(&tokens, &source).parse_program();
    assert!(parse_diags.is_empty(), "должно парситься как выражение: {parse_diags:?}");
}

fn run_with_data_file(filename: &str, content: &str, main_src: &str) -> Interpreter {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let id = SEQ.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("yps_import_attr_{}_{id}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(filename), content).unwrap();

    let source = SourceFile::new("test".to_string(), main_src.to_string());
    let (tokens, lex_diags) = Lexer::new(&source).tokenize();
    assert!(lex_diags.is_empty(), "Ошибки лексера: {lex_diags:?}");
    let (program, parse_diags) = Parser::new(&tokens, &source).parse_program();
    assert!(parse_diags.is_empty(), "Ошибки парсера: {parse_diags:?}");
    let mut interp = Interpreter::new();
    interp.set_base_path(dir.clone());
    interp.run(&program).expect("Ошибка интерпретатора");
    let _ = std::fs::remove_dir_all(&dir);
    interp
}

#[test]
fn import_json_with_type_attribute() {
    let json = r#"{ "имя": "Вася", "возраст": 25, "хобби": ["а", "б"] }"#;
    let main = r#"
        спиздить data из "./d.json" with { type: "json" };
        гыы имя = data.имя;
        гыы возраст = data.возраст;
        гыы первое = data.хобби[0];
    "#;
    let interp = run_with_data_file("d.json", json, main);
    assert_eq!(interp.get("имя"), Some(Value::String("Вася".to_string())));
    assert_eq!(interp.get("возраст"), Some(Value::Number(25.0)));
    assert_eq!(interp.get("первое"), Some(Value::String("а".to_string())));
}

fn run_with_files(files: &[(&str, &str)], entry: &str, main_src: &str) -> Interpreter {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let id = SEQ.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("yps_cyclic_{}_{id}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for (name, content) in files {
        std::fs::write(dir.join(name), content).unwrap();
    }
    let _ = entry;

    let source = SourceFile::new("test".to_string(), main_src.to_string());
    let (tokens, lex_diags) = Lexer::new(&source).tokenize();
    assert!(lex_diags.is_empty(), "Ошибки лексера: {lex_diags:?}");
    let (program, parse_diags) = Parser::new(&tokens, &source).parse_program();
    assert!(parse_diags.is_empty(), "Ошибки парсера: {parse_diags:?}");
    let mut interp = Interpreter::new();
    interp.set_base_path(dir.clone());
    interp.run(&program).expect("Ошибка интерпретатора");
    let _ = std::fs::remove_dir_all(&dir);
    interp
}

#[test]
fn cyclic_import_live_binding_deferred_access() {
    let a = r#"
        спиздить { получитьБ } из "./b";
        предъява гыы значениеА = 100;
        предъява йопта вызватьБ() { отвечаю получитьБ(); }
    "#;
    let b = r#"
        спиздить { значениеА } из "./a";
        предъява йопта получитьБ() { отвечаю значениеА * 2; }
    "#;
    let main = r#"
        спиздить { вызватьБ } из "./a";
        гыы итог = вызватьБ();
    "#;
    let interp = run_with_files(&[("a.yop", a), ("b.yop", b)], "main", main);
    assert_eq!(interp.get("итог"), Some(Value::Number(200.0)));
}

#[test]
fn import_attributes_russian_alias_satr() {
    let json = r#"{ "ключ": 7 }"#;
    let main = r#"
        спиздить д из "./a.json" сатр { type: "json" };
        гыы значение = д.ключ;
    "#;
    let interp = run_with_data_file("a.json", json, main);
    assert_eq!(interp.get("значение"), Some(Value::Number(7.0)));
}
