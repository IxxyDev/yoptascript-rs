use super::*;

#[test]
fn infinite_recursion_returns_error_instead_of_crash() {
    let err = run_code_err("йопта рек(н) { отвечаю рек(н + 1); } рек(0);");
    assert!(err.message.contains("глубина рекурсии"), "ожидалась ошибка о глубине рекурсии, получено: {}", err.message);
}

#[test]
fn recursion_within_limit_succeeds() {
    let i = run_code(
        r#"
        йопта рек(н) {
            вилкойвглаз (н >= 500) { отвечаю н; }
            отвечаю рек(н + 1);
        }
        гыы рез = рек(0);
        "#,
    );
    assert_eq!(i.get("рез"), Some(Value::Number(500.0)));
}

#[test]
fn recursion_limit_error_is_catchable() {
    let i = run_code(
        r#"
        йопта рек(н) { отвечаю рек(н + 1); }
        гыы поймали = лож;
        хапнуть { рек(0); } гоп (е) { поймали = правда; }
        "#,
    );
    assert_eq!(i.get("поймали"), Some(Value::Boolean(true)));
}

#[test]
fn method_recursion_returns_error_instead_of_crash() {
    let err = run_code_err(
        r#"
        клёво К {
            рек(н) { отвечаю тырыпыры.рек(н + 1); }
        }
        гыы к = захуярить К();
        к.рек(0);
        "#,
    );
    assert!(err.message.contains("глубина рекурсии"), "ожидалась ошибка о глубине рекурсии, получено: {}", err.message);
}

#[test]
fn long_binary_chain_evaluates_without_crash() {
    let src = format!("гыы рез = 1{};", " + 1".repeat(5000));
    let i = run_code(&src);
    assert_eq!(i.get("рез"), Some(Value::Number(5001.0)));
}

#[test]
fn json_parse_deeply_nested_returns_error_instead_of_crash() {
    let src = format!("Жсон.разобрать(\"{}\");", "[".repeat(100_000));
    let err = run_code_err(&src);
    assert!(err.message.contains("вложенность JSON"), "ожидалась ошибка о вложенности JSON, получено: {}", err.message);
}

#[test]
fn json_nesting_within_limit_parses() {
    let depth = 100;
    let src = format!("гыы рез = Жсон.разобрать(\"{}1{}\");", "[".repeat(depth), "]".repeat(depth));
    run_code(&src);
}

#[test]
fn iterator_adapter_chain_depth_is_limited() {
    let err = run_code_err(
        r#"
        гыы ит = Итератор.от([1]);
        гыы и = 0;
        потрещим (и < 5000) {
            ит = ит.преобразовать((х) => х);
            и = и + 1;
        }
        ит.вМассив();
        "#,
    );
    assert!(
        err.message.contains("цепочка итераторов"),
        "ожидалась ошибка о цепочке итераторов, получено: {}",
        err.message
    );
}

#[test]
fn iterator_chain_within_limit_works() {
    let i = run_code(
        r#"
        гыы ит = Итератор.от([1, 2, 3]);
        гыы и = 0;
        потрещим (и < 50) {
            ит = ит.преобразовать((х) => х + 1);
            и = и + 1;
        }
        гыы рез = ит.вМассив();
        "#,
    );
    let arr = match i.get("рез") {
        Some(Value::Array(a)) => a.borrow().0.clone(),
        other => panic!("ожидался массив, получено {other:?}"),
    };
    assert_eq!(arr, vec![Value::Number(51.0), Value::Number(52.0), Value::Number(53.0)]);
}

#[test]
fn string_repeat_huge_count_errors() {
    let err = run_code_err(r#""аб".повторить(10000000000);"#);
    assert!(err.message.contains("лимит длины"), "ожидалась ошибка о лимите длины строки, получено: {}", err.message);
}

#[test]
fn string_pad_start_huge_target_errors() {
    let err = run_code_err(r#""х".дополнитьСлева(10000000000);"#);
    assert!(err.message.contains("лимит длины"), "ожидалась ошибка о лимите длины строки, получено: {}", err.message);
}

#[test]
fn string_pad_end_huge_target_errors() {
    let err = run_code_err(r#""х".дополнитьСправа(10000000000);"#);
    assert!(err.message.contains("лимит длины"), "ожидалась ошибка о лимите длины строки, получено: {}", err.message);
}

#[test]
fn string_pad_with_multibyte_fill_respects_byte_limit() {
    let err = run_code_err(r#""х".дополнитьСлева(30000000, "ф");"#);
    assert!(err.message.contains("лимит длины"), "ожидалась ошибка о лимите длины строки, получено: {}", err.message);
}

#[test]
fn generator_reentrant_next_errors_instead_of_panic() {
    let i = run_code(
        r#"
        ясенХуй сам = ноль;
        пиздюли г() { поебалу сам.следующий(); }
        сам = г();
        гыы поймали = лож;
        хапнуть { сам.следующий(); } гоп (е) { поймали = правда; }
        "#,
    );
    assert_eq!(i.get("поймали"), Some(Value::Boolean(true)));
}
