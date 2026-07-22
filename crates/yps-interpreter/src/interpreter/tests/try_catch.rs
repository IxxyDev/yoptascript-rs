use super::*;

#[test]
fn try_catch_catches_runtime_error() {
    let interp = run_code(
        r#"
        гыы результат = 0;
        хапнуть {
            гыы х = 1n / 0n;
        } гоп (е) {
            результат = 1;
        }
        "#,
    );
    assert_eq!(interp.get("результат"), Some(Value::Number(1.0)));
}

#[test]
fn try_catch_catches_throw() {
    let interp = run_code(
        r#"
        гыы результат = "";
        хапнуть {
            кидай "ошибка";
        } гоп (е) {
            результат = е;
        }
        "#,
    );
    assert_eq!(interp.get("результат"), Some(Value::String("ошибка".into())));
}

#[test]
fn try_catch_throw_number() {
    let interp = run_code(
        r#"
        гыы результат = 0;
        хапнуть {
            кидай 42;
        } гоп (е) {
            результат = е;
        }
        "#,
    );
    assert_eq!(interp.get("результат"), Some(Value::Number(42.0)));
}

#[test]
fn try_catch_no_error_skips_catch() {
    let interp = run_code(
        r#"
        гыы результат = 1;
        хапнуть {
            результат = 2;
        } гоп (е) {
            результат = 3;
        }
        "#,
    );
    assert_eq!(interp.get("результат"), Some(Value::Number(2.0)));
}

#[test]
fn try_finally_runs_always() {
    let interp = run_code(
        r#"
        гыы результат = 0;
        хапнуть {
            результат = 1;
        } тюряжка {
            результат = результат + 10;
        }
        "#,
    );
    assert_eq!(interp.get("результат"), Some(Value::Number(11.0)));
}

#[test]
fn try_catch_finally_on_error() {
    let interp = run_code(
        r#"
        гыы шаг1 = 0;
        гыы шаг2 = 0;
        хапнуть {
            кидай "бум";
        } гоп (е) {
            шаг1 = 1;
        } тюряжка {
            шаг2 = 1;
        }
        "#,
    );
    assert_eq!(interp.get("шаг1"), Some(Value::Number(1.0)));
    assert_eq!(interp.get("шаг2"), Some(Value::Number(1.0)));
}

#[test]
fn try_catch_finally_no_error() {
    let interp = run_code(
        r#"
        гыы шаг1 = 0;
        гыы шаг2 = 0;
        хапнуть {
            шаг1 = 1;
        } гоп (е) {
            шаг1 = 99;
        } тюряжка {
            шаг2 = 1;
        }
        "#,
    );
    assert_eq!(interp.get("шаг1"), Some(Value::Number(1.0)));
    assert_eq!(interp.get("шаг2"), Some(Value::Number(1.0)));
}

#[test]
fn uncaught_throw_is_error() {
    let err = run_code_err(
        r#"
        кидай "паника";
        "#,
    );
    assert!(err.message.contains("Необработанное исключение"));
}

#[test]
fn try_catch_without_param() {
    let interp = run_code(
        r#"
        гыы результат = 0;
        хапнуть {
            кидай "бум";
        } гоп {
            результат = 1;
        }
        "#,
    );
    assert_eq!(interp.get("результат"), Some(Value::Number(1.0)));
}

#[test]
fn try_catch_runtime_error_message() {
    let interp = run_code(
        r#"
        гыы результат = "";
        хапнуть {
            гыы х = неизвестная;
        } гоп (е) {
            результат = е.message;
        }
        "#,
    );
    let val = interp.get("результат").unwrap();
    if let Value::String(s) = val {
        assert!(s.contains("не определена"));
    } else {
        panic!("Expected string error message");
    }
}

#[test]
fn nested_try_catch() {
    let interp = run_code(
        r#"
        гыы результат = "";
        хапнуть {
            хапнуть {
                кидай "внутри";
            } гоп (е) {
                кидай "снаружи";
            }
        } гоп (е) {
            результат = е;
        }
        "#,
    );
    assert_eq!(interp.get("результат"), Some(Value::String("снаружи".into())));
}

#[test]
fn try_catch_with_alias_keywords() {
    let interp = run_code(
        r#"
        гыы результат = 0;
        побратски {
            кидай 1;
        } аченетак (е) {
            результат = е;
        }
        "#,
    );
    assert_eq!(interp.get("результат"), Some(Value::Number(1.0)));
}

#[test]
fn finally_runs_after_throw_without_catch() {
    let interp = run_code(
        r#"
        гыы результат = 0;
        хапнуть {
            хапнуть {
                кидай "бум";
            } тюряжка {
                результат = 1;
            }
        } гоп (е) {
            результат = результат + 10;
        }
        "#,
    );
    assert_eq!(interp.get("результат"), Some(Value::Number(11.0)));
}

#[test]
fn finally_runtime_error_preserves_original_via_cause() {
    let err = run_code_err(
        r#"
        хапнуть {
            гыы а = неизвестнаяОригинальная;
        } тюряжка {
            гыы б = неизвестнаяВФинале;
        }
        "#,
    );
    assert!(
        err.message.contains("неизвестнаяВФинале"),
        "ожидается сообщение от финального исключения, получено: {}",
        err.message
    );
    let cause = err.cause.as_deref().expect("ожидается cause с оригинальной ошибкой");
    assert!(
        cause.message.contains("неизвестнаяОригинальная"),
        "cause должен содержать оригинал, получено: {}",
        cause.message
    );
}

#[test]
fn finally_runtime_error_alone_has_no_cause() {
    let err = run_code_err(
        r#"
        хапнуть {
            гыы а = 1;
        } тюряжка {
            гыы б = неизвестная;
        }
        "#,
    );
    assert!(err.message.contains("неизвестная"));
    assert!(err.cause.is_none(), "при отсутствии оригинальной ошибки cause должен быть None");
}

#[test]
fn finally_error_display_shows_cause_chain() {
    let err = run_code_err(
        r#"
        хапнуть {
            гыы а = первая;
        } тюряжка {
            гыы б = вторая;
        }
        "#,
    );
    let s = format!("{err}");
    assert!(s.contains("вторая"), "отображение должно включать финальную ошибку: {s}");
    assert!(s.contains("первая"), "отображение должно включать оригинал: {s}");
    assert!(s.contains("причина"), "отображение должно явно метить причину: {s}");
}
