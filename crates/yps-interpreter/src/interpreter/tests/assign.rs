use super::*;

#[test]
fn assign_array_index() {
    let interp = run_code(
        r#"
        гыы арр = [1, 2, 3];
        арр[0] = 10;
        гыы результат = арр[0];
        "#,
    );
    assert_eq!(interp.get("результат"), Some(Value::Number(10.0)));
}

#[test]
fn assign_array_index_middle() {
    let interp = run_code(
        r#"
        гыы арр = [1, 2, 3];
        арр[1] = 42;
        гыы результат = арр[1];
        "#,
    );
    assert_eq!(interp.get("результат"), Some(Value::Number(42.0)));
}

#[test]
fn assign_array_index_preserves_other_elements() {
    let interp = run_code(
        r#"
        гыы арр = [10, 20, 30];
        арр[1] = 99;
        гыы а = арр[0];
        гыы б = арр[2];
        "#,
    );
    assert_eq!(interp.get("а"), Some(Value::Number(10.0)));
    assert_eq!(interp.get("б"), Some(Value::Number(30.0)));
}

#[test]
fn assign_array_index_out_of_bounds() {
    let err = run_code_err(
        r#"
        гыы арр = [1, 2];
        арр[5] = 10;
        "#,
    );
    assert!(err.message.contains("вне диапазона") || err.message.contains("Индекс"));
}

#[test]
fn assign_object_member() {
    let interp = run_code(
        r#"
        гыы чел = { имя: "Вася", возраст: 25 };
        чел.имя = "Петя";
        гыы результат = чел.имя;
        "#,
    );
    assert_eq!(interp.get("результат"), Some(Value::String("Петя".to_string())));
}

#[test]
fn assign_object_member_new_property() {
    let interp = run_code(
        r#"
        гыы чел = { имя: "Вася" };
        чел.возраст = 30;
        гыы результат = чел.возраст;
        "#,
    );
    assert_eq!(interp.get("результат"), Some(Value::Number(30.0)));
}

#[test]
fn assign_object_bracket_notation() {
    let interp = run_code(
        r#"
        гыы чел = { имя: "Вася" };
        чел["имя"] = "Коля";
        гыы результат = чел.имя;
        "#,
    );
    assert_eq!(interp.get("результат"), Some(Value::String("Коля".to_string())));
}

#[test]
fn assign_member_on_non_object_fails() {
    let err = run_code_err(
        r#"
        гыы х = 5;
        х.поле = 10;
        "#,
    );
    assert!(err.message.contains("свойство") || err.message.contains("объект"));
}

#[test]
fn compound_assign_array_index() {
    let interp = run_code(
        r#"
        гыы арр = [10, 20, 30];
        арр[0] += 5;
        гыы результат = арр[0];
        "#,
    );
    assert_eq!(interp.get("результат"), Some(Value::Number(15.0)));
}

#[test]
fn compound_assign_object_member() {
    let interp = run_code(
        r#"
        гыы чел = { баланс: 100 };
        чел.баланс -= 30;
        гыы результат = чел.баланс;
        "#,
    );
    assert_eq!(interp.get("результат"), Some(Value::Number(70.0)));
}

#[test]
fn assign_nested_array() {
    let interp = run_code(
        r#"
        гыы матрица = [[1, 2], [3, 4]];
        матрица[0][1] = 99;
        гыы результат = матрица[0][1];
        "#,
    );
    assert_eq!(interp.get("результат"), Some(Value::Number(99.0)));
}

#[test]
fn assign_nested_object() {
    let interp = run_code(
        r#"
        гыы данные = { внутри: { значение: 1 } };
        данные.внутри.значение = 42;
        гыы результат = данные.внутри.значение;
        "#,
    );
    assert_eq!(interp.get("результат"), Some(Value::Number(42.0)));
}

#[test]
fn assign_object_in_array() {
    let interp = run_code(
        r#"
        гыы список = [{ имя: "А" }, { имя: "Б" }];
        список[0].имя = "В";
        гыы результат = список[0].имя;
        "#,
    );
    assert_eq!(interp.get("результат"), Some(Value::String("В".to_string())));
}

#[test]
fn assign_array_in_object() {
    let interp = run_code(
        r#"
        гыы данные = { список: [1, 2, 3] };
        данные.список[2] = 99;
        гыы результат = данные.список[2];
        "#,
    );
    assert_eq!(interp.get("результат"), Some(Value::Number(99.0)));
}
