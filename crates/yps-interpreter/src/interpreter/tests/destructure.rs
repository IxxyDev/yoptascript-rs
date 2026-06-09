use super::*;

#[test]
fn destructure_array_basic() {
    let interp = run_code(
        r#"
        гыы [а, б, в] = [1, 2, 3];
        "#,
    );
    assert_eq!(interp.get("а"), Some(Value::Number(1.0)));
    assert_eq!(interp.get("б"), Some(Value::Number(2.0)));
    assert_eq!(interp.get("в"), Some(Value::Number(3.0)));
}

#[test]
fn destructure_array_fewer_elements() {
    let interp = run_code(
        r#"
        гыы [а, б] = [1];
        "#,
    );
    assert_eq!(interp.get("а"), Some(Value::Number(1.0)));
    assert_eq!(interp.get("б"), Some(Value::Undefined));
}

#[test]
fn destructure_array_skip_elements() {
    let interp = run_code(
        r#"
        гыы [, , в] = [1, 2, 3];
        "#,
    );
    assert_eq!(interp.get("в"), Some(Value::Number(3.0)));
}

#[test]
fn destructure_array_rest() {
    let interp = run_code(
        r#"
        гыы [а, ...остаток] = [1, 2, 3, 4];
        гыы длинна = длина(остаток);
        гыы б = остаток[0];
        гыы в = остаток[1];
        гыы г = остаток[2];
        "#,
    );
    assert_eq!(interp.get("а"), Some(Value::Number(1.0)));
    assert_eq!(interp.get("длинна"), Some(Value::Number(3.0)));
    assert_eq!(interp.get("б"), Some(Value::Number(2.0)));
    assert_eq!(interp.get("в"), Some(Value::Number(3.0)));
    assert_eq!(interp.get("г"), Some(Value::Number(4.0)));
}

#[test]
fn destructure_array_rest_empty() {
    let interp = run_code(
        r#"
        гыы [а, ...остаток] = [1];
        гыы длинна = длина(остаток);
        "#,
    );
    assert_eq!(interp.get("а"), Some(Value::Number(1.0)));
    assert_eq!(interp.get("длинна"), Some(Value::Number(0.0)));
}

#[test]
fn destructure_array_non_array_fails() {
    let err = run_code_err(
        r#"
        гыы [а, б] = 42;
        "#,
    );
    assert!(err.message.contains("деструктурировать"));
}

#[test]
fn destructure_object_shorthand() {
    let interp = run_code(
        r#"
        гыы {х, у} = { х: 10, у: 20 };
        "#,
    );
    assert_eq!(interp.get("х"), Some(Value::Number(10.0)));
    assert_eq!(interp.get("у"), Some(Value::Number(20.0)));
}

#[test]
fn destructure_object_rename() {
    let interp = run_code(
        r#"
        гыы {х: а, у: б} = { х: 10, у: 20 };
        "#,
    );
    assert_eq!(interp.get("а"), Some(Value::Number(10.0)));
    assert_eq!(interp.get("б"), Some(Value::Number(20.0)));
}

#[test]
fn destructure_object_missing_key() {
    let interp = run_code(
        r#"
        гыы {х, з} = { х: 10, у: 20 };
        "#,
    );
    assert_eq!(interp.get("х"), Some(Value::Number(10.0)));
    assert_eq!(interp.get("з"), Some(Value::Undefined));
}

#[test]
fn destructure_object_rest() {
    let interp = run_code(
        r#"
        гыы {х, ...остаток} = { х: 1, у: 2, з: 3 };
        "#,
    );
    assert_eq!(interp.get("х"), Some(Value::Number(1.0)));
    let rest = interp.get("остаток").unwrap();
    if let Value::Object(map) = rest {
        let map = map.borrow();
        assert_eq!(map.get("у"), Some(&Value::Number(2.0)));
        assert_eq!(map.get("з"), Some(&Value::Number(3.0)));
        assert_eq!(map.len(), 2);
    } else {
        panic!("Ожидался объект");
    }
}

#[test]
fn destructure_object_non_object_fails() {
    let err = run_code_err(
        r#"
        гыы {х} = 42;
        "#,
    );
    assert!(err.message.contains("деструктурировать"));
}

#[test]
fn destructure_nested_array_in_array() {
    let interp = run_code(
        r#"
        гыы [а, [б, в]] = [1, [2, 3]];
        "#,
    );
    assert_eq!(interp.get("а"), Some(Value::Number(1.0)));
    assert_eq!(interp.get("б"), Some(Value::Number(2.0)));
    assert_eq!(interp.get("в"), Some(Value::Number(3.0)));
}

#[test]
fn destructure_object_in_array() {
    let interp = run_code(
        r#"
        гыы [а, {б}] = [1, { б: 2 }];
        "#,
    );
    assert_eq!(interp.get("а"), Some(Value::Number(1.0)));
    assert_eq!(interp.get("б"), Some(Value::Number(2.0)));
}

#[test]
fn destructure_array_in_object() {
    let interp = run_code(
        r#"
        гыы {данные: [а, б]} = { данные: [10, 20] };
        "#,
    );
    assert_eq!(interp.get("а"), Some(Value::Number(10.0)));
    assert_eq!(interp.get("б"), Some(Value::Number(20.0)));
}

#[test]
fn destructure_const_array() {
    let err = run_code_err(
        r#"
        участковый [а, б] = [1, 2];
        а = 10;
        "#,
    );
    assert!(err.message.contains("константу") || err.message.contains("const"));
}

#[test]
fn destructure_const_object() {
    let err = run_code_err(
        r#"
        участковый {х, у} = { х: 1, у: 2 };
        х = 10;
        "#,
    );
    assert!(err.message.contains("константу") || err.message.contains("const"));
}

#[test]
fn destructure_object_default_applied() {
    let interp = run_code(
        r#"
        гыы { х = 5, у = 10 } = { у: 20 };
        "#,
    );
    assert_eq!(interp.get("х"), Some(Value::Number(5.0)));
    assert_eq!(interp.get("у"), Some(Value::Number(20.0)));
}

#[test]
fn destructure_object_default_with_rename() {
    let interp = run_code(
        r#"
        гыы { а: б = 7 } = {};
        "#,
    );
    assert_eq!(interp.get("б"), Some(Value::Number(7.0)));
}

#[test]
fn destructure_array_default_applied() {
    let interp = run_code(
        r#"
        гыы [а = 1, б = 2, в = 3] = [100, ноль];
        "#,
    );
    assert_eq!(interp.get("а"), Some(Value::Number(100.0)));
    assert_eq!(interp.get("б"), Some(Value::Null));
    assert_eq!(interp.get("в"), Some(Value::Number(3.0)));
}

#[test]
fn destructure_array_default_missing_element() {
    let interp = run_code(
        r#"
        гыы [п, в = 42] = [1];
        "#,
    );
    assert_eq!(interp.get("п"), Some(Value::Number(1.0)));
    assert_eq!(interp.get("в"), Some(Value::Number(42.0)));
}

#[test]
fn destructure_default_expression_references_value() {
    let interp = run_code(
        r#"
        гыы { ширина = 3, площадь = ширина * ширина } = { ширина: 4 };
        "#,
    );
    assert_eq!(interp.get("ширина"), Some(Value::Number(4.0)));
    assert_eq!(interp.get("площадь"), Some(Value::Number(16.0)));
}
