use super::*;

#[test]
fn ternary_true_branch() {
    let interp = run_code(
        r#"
        гыы р = правда ? 10 : 20;
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::Number(10.0)));
}

#[test]
fn ternary_false_branch() {
    let interp = run_code(
        r#"
        гыы р = лож ? 10 : 20;
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::Number(20.0)));
}

#[test]
fn ternary_with_expression_condition() {
    let interp = run_code(
        r#"
        гыы x = 7;
        гыы р = x > 5 ? "да" : "нет";
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::String("да".to_string())));
}

#[test]
fn ternary_nested() {
    let interp = run_code(
        r#"
        гыы x = 3;
        гыы р = x > 10 ? "большое" : x > 5 ? "среднее" : "маленькое";
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::String("маленькое".to_string())));
}

#[test]
fn ternary_with_function_call() {
    let interp = run_code(
        r#"
        гыы arr = [1, 2, 3];
        гыы р = длина(arr) > 0 ? arr[0] : ноль;
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::Number(1.0)));
}

#[test]
fn function_without_return_gives_undefined() {
    let interp = run_code(
        r#"
        йопта ф() {}
        гыы р = ф();
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::Undefined));
}

#[test]
fn return_without_value_gives_undefined() {
    let interp = run_code(
        r#"
        йопта ф() { отвечаю; }
        гыы р = ф();
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::Undefined));
}

#[test]
fn missing_object_property_gives_undefined() {
    let interp = run_code(
        r#"
        гыы о = { а: 1 };
        гыы р = о.б;
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::Undefined));
}

#[test]
fn array_index_out_of_bounds_gives_undefined() {
    let interp = run_code(
        r#"
        гыы м = [1, 2, 3];
        гыы р = м[10];
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::Undefined));
}

#[test]
fn typeof_undefined() {
    let interp = run_code(
        r#"
        йопта ф() {}
        гыы р = тип(ф());
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::String("неопределено".to_string())));
}

#[test]
fn null_abstract_equal_undefined() {
    let interp = run_code(
        r#"
        йопта ф() {}
        гыы р = ф() == ноль;
        гыы с = ф() === ноль;
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("с"), Some(Value::Boolean(false)));
}

#[test]
fn spread_in_array() {
    let i = run_code(
        r#"
        гыы а = [1, 2, 3];
        гыы б = [0, ...а, 4];
        гыы длн = 0;
        го (гыы и = 0; и < 5; и++) {
            длн = длн + 1;
        }
        гыы рез = б[0] + б[1] + б[2] + б[3] + б[4];
        "#,
    );
    assert_eq!(i.get("рез"), Some(Value::Number(10.0)));
}

#[test]
fn spread_in_call() {
    let i = run_code(
        r#"
        йопта сумма(а, б, в) {
            отвечаю а + б + в;
        }
        гыы арг = [1, 2, 3];
        гыы рез = сумма(...арг);
        "#,
    );
    assert_eq!(i.get("рез"), Some(Value::Number(6.0)));
}

#[test]
fn spread_in_object() {
    let i = run_code(
        r#"
        гыы а = {x: 1, y: 2};
        гыы б = {...а, z: 3};
        гыы рез = б["x"] + б["y"] + б["z"];
        "#,
    );
    assert_eq!(i.get("рез"), Some(Value::Number(6.0)));
}

#[test]
fn computed_property_name() {
    let i = run_code(
        r#"
        гыы ключ = "привет";
        гыы о = {[ключ]: 42};
        гыы рез = о["привет"];
        "#,
    );
    assert_eq!(i.get("рез"), Some(Value::Number(42.0)));
}

#[test]
fn shorthand_property() {
    let i = run_code(
        r#"
        гыы х = 10;
        гыы у = 20;
        гыы о = {х, у};
        гыы рез = о["х"] + о["у"];
        "#,
    );
    assert_eq!(i.get("рез"), Some(Value::Number(30.0)));
}

#[test]
fn method_shorthand_in_object() {
    let i = run_code(
        r#"
        гыы о = {
            удвоить(н) {
                отвечаю н * 2;
            }
        };
        гыы рез = о.удвоить(5);
        "#,
    );
    assert_eq!(i.get("рез"), Some(Value::Number(10.0)));
}
