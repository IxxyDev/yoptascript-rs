use super::*;

#[test]
fn test_async_function_then_runs_without_explicit_await() {
    let interp = run_code(
        r#"
        ассо йопта f() { отвечаю 42; }
        гыы итог = 0;
        f().потом((v) => { итог = v; });
        "#,
    );
    assert_eq!(interp.get("итог"), Some(Value::Number(42.0)));
}

#[test]
fn test_async_function_returns_promise() {
    let interp = run_code(
        r#"
        ассо йопта f() { отвечаю 42; }
        гыы p = f();
        гыы т = тип(p);
        "#,
    );
    assert_eq!(interp.get("т"), Some(Value::String("обещание".into())));
    match interp.get("p") {
        Some(Value::Promise { .. }) => {}
        other => panic!("Ожидался Promise, получено {other:?}"),
    }
}

#[test]
fn test_async_await_chain_then() {
    let interp = run_code(
        r#"
        ассо йопта f() { отвечаю 1; }
        ассо йопта g() {
            гыы x = сидетьНахуй f();
            отвечаю x + 1;
        }
        гыы итог = 0;
        g().потом((v) => { итог = v; });
        "#,
    );
    assert_eq!(interp.get("итог"), Some(Value::Number(2.0)));
}

#[test]
fn test_promise_resolve_then() {
    let interp = run_code(
        r#"
        гыы итог = 0;
        СловоПацана.решить(5).потом((v) => { итог = v; });
        "#,
    );
    assert_eq!(interp.get("итог"), Some(Value::Number(5.0)));
}

#[test]
fn test_promise_all_resolved() {
    let interp = run_code(
        r#"
        гыы итог = ноль;
        СловоПацана.всех([СловоПацана.решить(1), СловоПацана.решить(2)]).потом((v) => { итог = v; });
        "#,
    );
    assert_struct_eq(interp.get("итог"), Value::array(vec![Value::Number(1.0), Value::Number(2.0)]));
}

#[test]
fn test_await_rejected_throws_catchable() {
    let interp = run_code(
        r#"
        ассо йопта плохо() {
            кидай "беда";
        }
        ассо йопта тест() {
            хапнуть {
                сидетьНахуй плохо();
                отвечаю "ок";
            } гоп (e) {
                отвечаю "поймал";
            }
        }
        гыы итог = "пусто";
        тест().потом((v) => { итог = v; });
        "#,
    );
    assert_eq!(interp.get("итог"), Some(Value::String("поймал".into())));
}

#[test]
fn test_promise_then_on_resolved_executes_once() {
    let interp = run_code(
        r#"
        гыы счёт = 0;
        гыы p = СловоПацана.решить(1);
        p.потом((v) => { счёт = счёт + v; });
        "#,
    );
    assert_eq!(interp.get("счёт"), Some(Value::Number(1.0)));
}

#[test]
fn test_promise_then_stored_then_chained() {
    let interp = run_code(
        r#"
        гыы итог = 0;
        гыы p = СловоПацана.решить(10);
        p.потом((v) => { итог = v; });
        "#,
    );
    assert_eq!(interp.get("итог"), Some(Value::Number(10.0)));
}

#[test]
fn test_promise_catch_on_rejected() {
    let interp = run_code(
        r#"
        гыы итог = "нет";
        СловоПацана.отвергнуть("ошибка").ловить((e) => { итог = e; });
        "#,
    );
    assert_eq!(interp.get("итог"), Some(Value::String("ошибка".into())));
}

#[test]
fn test_promise_finally_on_fulfilled() {
    let interp = run_code(
        r#"
        гыы итог = 0;
        СловоПацана.решить(7).наконец(() => { итог = 1; });
        "#,
    );
    assert_eq!(interp.get("итог"), Some(Value::Number(1.0)));
}

#[test]
fn test_promise_then_chained_twice() {
    let interp = run_code(
        r#"
        гыы итог = 0;
        СловоПацана.решить(3)
            .потом((v) => { отвечаю v + 1; })
            .потом((v) => { итог = v; });
        "#,
    );
    assert_eq!(interp.get("итог"), Some(Value::Number(4.0)));
}

#[test]
fn promise_catch_receives_thrown_object_from_then() {
    let i = run_code(
        r#"
        гыы тип_е = "";
        гыы код = 0;
        СловоПацана.решить(1)
            .потом((з) => { кидай { код: 7 }; })
            .ловить((е) => { тип_е = тип(е); код = е.код; });
        "#,
    );
    assert_eq!(i.get("тип_е"), Some(Value::String("объект".into())));
    assert_eq!(i.get("код"), Some(Value::Number(7.0)));
}

#[test]
fn promise_try_rejects_with_thrown_value() {
    let i = run_code(
        r#"
        гыы код = 0;
        СловоПацана.попробовать(() => { кидай { код: 5 }; }).ловить((е) => { код = е.код; });
        "#,
    );
    assert_eq!(i.get("код"), Some(Value::Number(5.0)));
}

#[test]
fn throw_in_promise_executor_rejects_promise() {
    let i = run_code(
        r#"
        гыы код = 0;
        ясенХуй п = захуярить СловоПацана((реш, отв) => { кидай { код: 42 }; });
        п.ловить((е) => { код = е.код; });
        "#,
    );
    assert_eq!(i.get("код"), Some(Value::Number(42.0)));
}

#[test]
fn promise_rejects_engine_error_as_object_like_catch() {
    let i = run_code(
        r#"
        гыы имя = "";
        гыы сообщение = "";
        СловоПацана.решить(1)
            .потом((з) => { несуществуетТакого(); })
            .ловить((е) => { имя = е.name; сообщение = е.message; });
        "#,
    );
    assert_eq!(i.get("имя"), Some(Value::String("Косяк".into())));
    let msg = match i.get("сообщение") {
        Some(Value::String(s)) => s,
        other => panic!("ожидалась строка, получено {other:?}"),
    };
    assert!(msg.contains("не определена"), "{msg}");
}
