use super::*;

fn run_code_with_step_limit(src: &str, limit: u64) -> Result<Interpreter, RuntimeError> {
    let source = yps_lexer::SourceFile::new("test".to_string(), src.to_string());
    let (tokens, lex_diags) = yps_lexer::Lexer::new(&source).tokenize();
    assert!(lex_diags.is_empty(), "Ошибки лексера: {lex_diags:?}");
    let (program, parse_diags) = yps_parser::Parser::new(&tokens, &source).parse_program();
    assert!(parse_diags.is_empty(), "Ошибки парсера: {parse_diags:?}");
    let mut interp = Interpreter::new();
    interp.set_step_limit(limit);
    match interp.run(&program) {
        Ok(()) => Ok(interp),
        Err(e) => Err(e),
    }
}

#[test]
fn step_limit_stops_infinite_for_loop() {
    let err = match run_code_with_step_limit("го(;;){}", 10_000) {
        Err(e) => e,
        Ok(_) => panic!("ожидалась ошибка лимита шагов"),
    };
    assert!(err.message.contains("превышен лимит шагов"), "неожиданное сообщение: {}", err.message);
}

#[test]
fn step_limit_stops_infinite_while_loop() {
    let err = match run_code_with_step_limit("потрещим (правда) {}", 10_000) {
        Err(e) => e,
        Ok(_) => panic!("ожидалась ошибка лимита шагов"),
    };
    assert!(err.message.contains("превышен лимит шагов"), "неожиданное сообщение: {}", err.message);
}

#[test]
fn step_limit_allows_normal_programs() {
    let i = run_code_with_step_limit("гыы с = 0; го (гыы и = 0; и < 100; и += 1) { с += и; }", 10_000).unwrap();
    assert_eq!(i.get("с"), Some(Value::Number(4950.0)));
}

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
    let src = format!("гыы рез = Жсон.разобрать(\"{}1{}\");\nгыы глуб = 0;", "[".repeat(depth), "]".repeat(depth));
    let i = run_code(&src);
    let mut current = i.get("рез").expect("рез должен быть определён");
    for _ in 0..depth {
        let Value::Array(items) = current else {
            panic!("ожидался массив, получено {current:?}")
        };
        let inner = items.borrow()[0].clone();
        current = inner;
    }
    assert_eq!(current, Value::Number(1.0));
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
        участковый сам = ноль;
        пиздюли г() { поебалу сам.следующий(); }
        сам = г();
        гыы поймали = лож;
        хапнуть { сам.следующий(); } гоп (е) { поймали = правда; }
        "#,
    );
    assert_eq!(i.get("поймали"), Some(Value::Boolean(true)));
}
