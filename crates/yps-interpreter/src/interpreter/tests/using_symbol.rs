use super::*;

#[test]
fn using_disposes_on_scope_exit() {
    let interp = run_code(
        r#"
        гыы счёт = 0;
        {
            юзай р = { расход: () => { счёт = счёт + 1; } };
        }
        "#,
    );
    assert_eq!(interp.get("счёт"), Some(Value::Number(1.0)));
}

#[test]
fn using_disposes_in_lifo_order() {
    let interp = run_code(
        r#"
        гыы лог = [];
        {
            юзай а = { расход: () => { лог.push("а"); } };
            юзай б = { расход: () => { лог.push("б"); } };
            юзай в = { расход: () => { лог.push("в"); } };
        }
        "#,
    );
    let log = interp.get("лог").unwrap();
    let Value::Array(items) = log else { panic!("expected array") };
    let items = items.borrow();
    assert_eq!(items.len(), 3);
    assert_eq!(items[0], Value::String("в".to_string()));
    assert_eq!(items[1], Value::String("б".to_string()));
    assert_eq!(items[2], Value::String("а".to_string()));
}

#[test]
fn using_skips_null_resource() {
    let interp = run_code(
        r#"
        гыы счёт = 0;
        {
            юзай р = ноль;
        }
        "#,
    );
    assert_eq!(interp.get("счёт"), Some(Value::Number(0.0)));
}

#[test]
fn using_requires_dispose_method() {
    let err = run_code_err(
        r#"
        {
            юзай р = { данные: 42 };
        }
        "#,
    );
    assert!(err.message.contains("расход"));
}

#[test]
fn using_with_class_instance() {
    let interp = run_code(
        r#"
        гыы счёт = 0;
        клёво Файл {
            расход() {
                счёт = счёт + 10;
            }
        }
        {
            юзай ф = захуярить Файл();
        }
        "#,
    );
    assert_eq!(interp.get("счёт"), Some(Value::Number(10.0)));
}

#[test]
fn symbol_create_and_typeof() {
    let interp = run_code(
        r#"
        гыы с = Симбол("привет");
        гыы т = чезажижан с;
        "#,
    );
    assert_eq!(interp.get("т"), Some(Value::String("символ".to_string())));
}

#[test]
fn symbol_unique_identity() {
    let interp = run_code(
        r#"
        гыы а = Симбол("ключ");
        гыы б = Симбол("ключ");
        гыы равны = а === б;
        гыы самСебя = а === а;
        "#,
    );
    assert_eq!(interp.get("равны"), Some(Value::Boolean(false)));
    assert_eq!(interp.get("самСебя"), Some(Value::Boolean(true)));
}

#[test]
fn symbol_for_returns_shared() {
    let interp = run_code(
        r#"
        гыы а = Симбол.для("общий");
        гыы б = Симбол.для("общий");
        гыы в = Симбол.для("другой");
        гыы равны1 = а === б;
        гыы равны2 = а === в;
        "#,
    );
    assert_eq!(interp.get("равны1"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("равны2"), Some(Value::Boolean(false)));
}

#[test]
fn symbol_description_property() {
    let interp = run_code(
        r#"
        гыы с = Симбол("моёОписание");
        гыы оп = с.описание;
        "#,
    );
    assert_eq!(interp.get("оп"), Some(Value::String("моёОписание".to_string())));
}

#[test]
fn symbol_well_known_iterator_dispose() {
    let interp = run_code(
        r#"
        гыы и1 = Симбол.итератор;
        гыы и2 = Симбол.итератор;
        гыы р1 = Симбол.расход;
        гыы итерРасх = и1 === р1;
        гыы итерИтер = и1 === и2;
        "#,
    );
    assert_eq!(interp.get("итерРасх"), Some(Value::Boolean(false)));
    assert_eq!(interp.get("итерИтер"), Some(Value::Boolean(true)));
}

#[test]
fn symbol_to_string_method() {
    let interp = run_code(
        r#"
        гыы с = Симбол("м");
        гыы стр = с.вСтроку();
        "#,
    );
    assert_eq!(interp.get("стр"), Some(Value::String("Симбол(м)".to_string())));
}
