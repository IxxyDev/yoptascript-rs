use super::*;

fn eval_bool(src: &str) -> bool {
    let interp = run_code(&format!("гыы рез = {src};"));
    match interp.get("рез").unwrap() {
        Value::Boolean(b) => b,
        other => panic!("ожидалось булево, получено {other:?}"),
    }
}

#[test]
fn abstract_equals_number_string() {
    assert!(eval_bool("1 == \"1\""));
    assert!(!eval_bool("1 === \"1\""));
    assert!(eval_bool("\"1\" == 1"));
}

#[test]
fn abstract_equals_null_undefined() {
    assert!(eval_bool("ноль == неибу"));
    assert!(!eval_bool("ноль === неибу"));
    assert!(eval_bool("неибу == ноль"));
    assert!(!eval_bool("ноль == 0"));
    assert!(!eval_bool("неибу == 0"));
}

#[test]
fn abstract_equals_boolean() {
    assert!(eval_bool("правда == 1"));
    assert!(eval_bool("0 == лож"));
    assert!(eval_bool("лож == 0"));
    assert!(!eval_bool("правда == 2"));
    assert!(eval_bool("\"\" == 0"));
}

#[test]
fn abstract_equals_object_to_primitive() {
    let interp = run_code(
        r#"
        гыы об = {};
        гыы рез = ("" + об) == "[object Object]";
        "#,
    );
    assert_eq!(interp.get("рез").unwrap(), Value::Boolean(true));
}

#[test]
fn abstract_not_equals() {
    assert!(!eval_bool("1 != \"1\""));
    assert!(eval_bool("1 !== \"1\""));
}

#[test]
fn add_string_concat_via_ecma_string() {
    let interp = run_code(
        r#"
        гыы об = {};
        гыы мас = [1, 2, 3];
        гыы со = "об=" + об;
        гыы см = "мас=" + мас;
        гыы нп = "" + ноль;
        гыы уп = "" + неибу;
        "#,
    );
    assert_eq!(interp.get("со").unwrap(), Value::String("об=[object Object]".to_string()));
    assert_eq!(interp.get("см").unwrap(), Value::String("мас=1,2,3".to_string()));
    assert_eq!(interp.get("нп").unwrap(), Value::String("null".to_string()));
    assert_eq!(interp.get("уп").unwrap(), Value::String("undefined".to_string()));
}

#[test]
fn add_numeric_and_mixed() {
    let interp = run_code(
        r#"
        гыы а = 1 + 2;
        гыы б = 10 + 5 + "px";
        гыы в = "px" + 10 + 5;
        "#,
    );
    assert_eq!(interp.get("а").unwrap(), Value::Number(3.0));
    assert_eq!(interp.get("б").unwrap(), Value::String("15px".to_string()));
    assert_eq!(interp.get("в").unwrap(), Value::String("px105".to_string()));
}

#[test]
fn switch_uses_strict_equality_not_abstract() {
    let interp = run_code(
        r#"
        гыы рез = "нет";
        базарпо (1) {
            тема "1": { рез = "строка"; }
            тема 1: { рез = "число"; }
            нуичо { рез = "дефолт"; }
        }
        "#,
    );
    assert_eq!(interp.get("рез").unwrap(), Value::String("число".to_string()));
}

#[test]
fn array_includes_index_use_strict_not_abstract() {
    let interp = run_code(
        r#"
        гыы мас = [1, 2, 3];
        гыы вкл_число = мас.включает(1);
        гыы вкл_строка = мас.включает("1");
        гыы идкс_число = мас.найтиИндекс(2);
        гыы идкс_строка = мас.найтиИндекс("2");
        "#,
    );
    assert_eq!(interp.get("вкл_число").unwrap(), Value::Boolean(true));
    assert_eq!(interp.get("вкл_строка").unwrap(), Value::Boolean(false));
    assert_eq!(interp.get("идкс_число").unwrap(), Value::Number(1.0));
    assert_eq!(interp.get("идкс_строка").unwrap(), Value::Number(-1.0));
}

#[test]
fn user_value_of_in_add_and_equals() {
    let interp = run_code(
        r#"
        гыы об = { вЧисло: йопта() { отвечаю 42; } };
        гыы сум = об + 0;
        гыы равн = об == 42;
        "#,
    );
    assert_eq!(interp.get("сум").unwrap(), Value::Number(42.0));
    assert_eq!(interp.get("равн").unwrap(), Value::Boolean(true));
}

#[test]
fn user_to_string_in_concat() {
    let interp = run_code(
        r#"
        гыы об = { вСтроку: йопта() { отвечаю "привет"; } };
        гыы рез = "" + об;
        "#,
    );
    assert_eq!(interp.get("рез").unwrap(), Value::String("привет".to_string()));
}

#[test]
fn to_primitive_hook_has_priority_over_value_of() {
    let interp = run_code(
        r#"
        гыы об = {
            вПримитив: йопта(п) { отвечаю 100; },
            вЧисло: йопта() { отвечаю 1; }
        };
        гыы рез = об + 0;
        "#,
    );
    assert_eq!(interp.get("рез").unwrap(), Value::Number(100.0));
}

#[test]
fn default_hint_prefers_value_of() {
    let interp = run_code(
        r#"
        гыы об = {
            вЧисло: йопта() { отвечаю 7; },
            вСтроку: йопта() { отвечаю "семь"; }
        };
        гыы рез = об + 1;
        "#,
    );
    assert_eq!(interp.get("рез").unwrap(), Value::Number(8.0));
}

#[test]
fn recursive_value_of_hits_depth_limit_not_stack_overflow() {
    let message = std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(|| {
            run_code_err(
                r#"
                гыы об = {};
                об.вЧисло = йопта() { отвечаю об == 5; };
                гыы рез = об + 1;
                "#,
            )
            .message
        })
        .unwrap()
        .join()
        .unwrap();
    assert!(message.contains("глубин"), "ожидалось сообщение о глубине коэрции, получено: {message}");
}

#[test]
fn value_of_returning_object_is_type_error() {
    let err = run_code_err(
        r#"
        гыы об = { вЧисло: йопта() { отвечаю {}; }, вСтроку: йопта() { отвечаю {}; } };
        гыы рез = об + 1;
        "#,
    );
    assert!(err.message.contains("примитив"), "ожидался TypeError о примитиве, получено: {}", err.message);
}

#[test]
fn to_primitive_hook_returning_object_is_type_error() {
    let err = run_code_err(
        r#"
        гыы об = { вПримитив: йопта(п) { отвечаю {}; } };
        гыы рез = об + 1;
        "#,
    );
    assert!(err.message.contains("вПримитив"), "ожидался TypeError о вПримитив, получено: {}", err.message);
}
