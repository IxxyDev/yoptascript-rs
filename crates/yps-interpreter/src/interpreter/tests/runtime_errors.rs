use super::*;

#[test]
fn break_outside_loop_errors() {
    let err = run_code_err("харэ;");
    assert!(err.message.contains("'харэ'"), "got: {}", err.message);
    assert!(err.message.contains("вне цикла"), "got: {}", err.message);
}

#[test]
fn continue_outside_loop_errors() {
    let err = run_code_err("двигай;");
    assert!(err.message.contains("'двигай'"), "got: {}", err.message);
    assert!(err.message.contains("вне цикла"), "got: {}", err.message);
}

#[test]
fn division_by_zero_errors() {
    let err = run_code_err("гыы х = 5 / 0;");
    assert!(err.message.contains("Деление на ноль"), "got: {}", err.message);
}

#[test]
fn array_index_must_be_number() {
    let err = run_code_err(
        r#"
        гыы а = [1, 2, 3];
        а["ключ"] = 9;
        "#,
    );
    assert!(
        err.message.contains("индекс") || err.message.contains("Индекс") || err.message.contains("индексировать"),
        "got: {}",
        err.message
    );
}

#[test]
fn assignment_lhs_must_be_assignable() {
    let err = run_code_err("42 = 7;");
    assert!(err.message.contains("Левая сторона"), "got: {}", err.message);
}

#[test]
fn increment_on_non_variable_errors() {
    let err = run_code_err("42++;");
    assert!(err.message.contains("'++'") || err.message.contains("переменной"), "got: {}", err.message);
}

#[test]
fn this_outside_method_errors() {
    let err = run_code_err("гыы х = тырыпыры;");
    assert!(err.message.contains("тырыпыры") || err.message.contains("this"), "got: {}", err.message);
    assert!(err.message.contains("вне"), "got: {}", err.message);
}

#[test]
fn super_outside_subclass_errors() {
    let err = run_code_err(
        r#"
        клёво А {
            метод() { отвечаю яга.чтото(); }
        }
        гыы а = захуярить А();
        а.метод();
        "#,
    );
    assert!(err.message.contains("яга") || err.message.contains("super"), "got: {}", err.message);
}

#[test]
fn calling_non_function_errors() {
    let err = run_code_err(
        r#"
        гыы х = 5;
        гыы у = х();
        "#,
    );
    assert!(err.message.contains("не является функцией") || err.message.contains("функц"), "got: {}", err.message);
}

#[test]
fn unary_minus_on_string_errors() {
    let err = run_code_err(r#"гыы х = -"абв";"#);
    assert!(err.message.contains("'-'") || err.message.contains("тип"), "got: {}", err.message);
}

#[test]
fn increment_on_string_errors() {
    let err = run_code_err(
        r#"
        гыы х = "стр";
        х++;
        "#,
    );
    assert!(err.message.contains("число") || err.message.contains("'++'"), "got: {}", err.message);
}

#[test]
fn set_property_on_number_errors() {
    let err = run_code_err(
        r#"
        гыы х = 5;
        х.поле = 1;
        "#,
    );
    assert!(err.message.contains("свойство") || err.message.contains("Нельзя"), "got: {}", err.message);
}

#[test]
fn instanceof_operator_requires_class_on_right() {
    let err = run_code_err(
        r#"
        гыы рез = 42 шкура 10;
        "#,
    );
    assert!(err.message.contains("шкура"));
}
