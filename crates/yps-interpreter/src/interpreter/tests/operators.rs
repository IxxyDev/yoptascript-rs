use super::*;

#[test]
fn exponent_basic() {
    let interp = run_code("гыы р = 2 ** 3;");
    assert_eq!(interp.get("р"), Some(Value::Number(8.0)));
}

#[test]
fn exponent_right_associative() {
    let interp = run_code("гыы р = 2 ** 3 ** 2;");
    assert_eq!(interp.get("р"), Some(Value::Number(512.0)));
}

#[test]
fn exponent_with_multiply() {
    let interp = run_code("гыы р = 3 * 2 ** 3;");
    assert_eq!(interp.get("р"), Some(Value::Number(24.0)));
}

#[test]
fn exponent_assign() {
    let interp = run_code(
        r#"
        гыы р = 2;
        р **= 10;
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::Number(1024.0)));
}

#[test]
fn nullish_coalescing_null() {
    let interp = run_code("гыы р = ноль ?? 42;");
    assert_eq!(interp.get("р"), Some(Value::Number(42.0)));
}

#[test]
fn nullish_coalescing_undefined() {
    let interp = run_code("гыы р = неибу ?? 42;");
    assert_eq!(interp.get("р"), Some(Value::Number(42.0)));
}

#[test]
fn nullish_coalescing_non_null() {
    let interp = run_code("гыы р = 0 ?? 42;");
    assert_eq!(interp.get("р"), Some(Value::Number(0.0)));
}

#[test]
fn nullish_coalescing_false_is_not_nullish() {
    let interp = run_code("гыы р = лож ?? 42;");
    assert_eq!(interp.get("р"), Some(Value::Boolean(false)));
}

#[test]
fn nullish_coalescing_chain() {
    let interp = run_code("гыы р = ноль ?? неибу ?? 7;");
    assert_eq!(interp.get("р"), Some(Value::Number(7.0)));
}

#[test]
fn nullish_assign_null() {
    let interp = run_code(
        r#"
        гыы р = ноль;
        р ??= 99;
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::Number(99.0)));
}

#[test]
fn nullish_assign_non_null() {
    let interp = run_code(
        r#"
        гыы р = 5;
        р ??= 99;
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::Number(5.0)));
}

#[test]
fn optional_chain_member_on_object() {
    let interp = run_code(
        r#"
        гыы чел = { имя: "Вася" };
        гыы р = чел?.имя;
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::String("Вася".to_string())));
}

#[test]
fn optional_chain_member_on_null() {
    let interp = run_code(
        r#"
        гыы чел = ноль;
        гыы р = чел?.имя;
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::Undefined));
}

#[test]
fn optional_chain_member_on_undefined() {
    let interp = run_code(
        r#"
        гыы чел = неибу;
        гыы р = чел?.имя;
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::Undefined));
}

#[test]
fn optional_chain_nested() {
    let interp = run_code(
        r#"
        гыы данные = { а: { б: 42 } };
        гыы р = данные?.а?.б;
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::Number(42.0)));
}

#[test]
fn optional_chain_nested_null() {
    let interp = run_code(
        r#"
        гыы данные = { а: ноль };
        гыы р = данные?.а?.б;
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::Undefined));
}

#[test]
fn optional_chain_index() {
    let interp = run_code(
        r#"
        гыы арр = [10, 20, 30];
        гыы р = арр?.[1];
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::Number(20.0)));
}

#[test]
fn optional_chain_index_on_null() {
    let interp = run_code(
        r#"
        гыы арр = ноль;
        гыы р = арр?.[0];
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::Undefined));
}

#[test]
fn optional_chain_call() {
    let interp = run_code(
        r#"
        гыы ф = () => { отвечаю 42; };
        гыы р = ф?.();
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::Number(42.0)));
}

#[test]
fn optional_chain_call_on_null() {
    let interp = run_code(
        r#"
        гыы ф = ноль;
        гыы р = ф?.();
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::Undefined));
}

#[test]
fn logical_and_assign_truthy() {
    let interp = run_code(
        r#"
        гыы а = 1;
        а &&= 42;
        "#,
    );
    assert_eq!(interp.get("а"), Some(Value::Number(42.0)));
}

#[test]
fn logical_and_assign_falsy() {
    let interp = run_code(
        r#"
        гыы а = 0;
        а &&= 42;
        "#,
    );
    assert_eq!(interp.get("а"), Some(Value::Number(0.0)));
}

#[test]
fn logical_or_assign_falsy() {
    let interp = run_code(
        r#"
        гыы а = 0;
        а ||= 42;
        "#,
    );
    assert_eq!(interp.get("а"), Some(Value::Number(42.0)));
}

#[test]
fn logical_or_assign_truthy() {
    let interp = run_code(
        r#"
        гыы а = 1;
        а ||= 42;
        "#,
    );
    assert_eq!(interp.get("а"), Some(Value::Number(1.0)));
}

#[test]
fn numeric_separator() {
    let interp = run_code(
        r#"
        гыы а = 1_000_000;
        гыы б = 1.23_45;
        "#,
    );
    assert_eq!(interp.get("а"), Some(Value::Number(1_000_000.0)));
    assert_eq!(interp.get("б"), Some(Value::Number(1.2345)));
}

#[test]
fn typeof_basic() {
    let interp = run_code(
        r#"
        гыы а = чезажижан 42;
        гыы б = чезажижан "привет";
        гыы в = чезажижан правда;
        гыы г = чезажижан ноль;
        гыы д = чезажижан неибу;
        "#,
    );
    assert_eq!(interp.get("а"), Some(Value::String("число".to_string())));
    assert_eq!(interp.get("б"), Some(Value::String("строка".to_string())));
    assert_eq!(interp.get("в"), Some(Value::String("булево".to_string())));
    assert_eq!(interp.get("г"), Some(Value::String("объект".to_string())));
    assert_eq!(interp.get("д"), Some(Value::String("неопределено".to_string())));
}

#[test]
fn typeof_undefined_variable() {
    let interp = run_code(
        r#"
        гыы р = чезажижан несуществует;
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::String("неопределено".to_string())));
}

#[test]
fn typeof_function() {
    let interp = run_code(
        r#"
        йопта ф() {}
        гыы р = чезажижан ф;
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::String("функция".to_string())));
}

#[test]
fn delete_object_property() {
    let i = run_code(
        r#"
        гыы о = {а: 1, б: 2};
        ёбнуть о.а;
        гыы рез = чезажижан о["а"];
        "#,
    );
    assert_eq!(i.get("рез"), Some(Value::String("неопределено".to_string())));
}

#[test]
fn delete_array_index_creates_undefined_hole() {
    let i = run_code(
        r#"
        гыы а = [10, 20, 30];
        ёбнуть а[1];
        гыы н = а[1];
        гыы д = длина(а);
        "#,
    );
    assert_eq!(i.get("н"), Some(Value::Undefined));
    assert_eq!(i.get("д"), Some(Value::Number(3.0)));
}

#[test]
fn delete_array_preserves_other_elements() {
    let i = run_code(
        r#"
        гыы а = [10, 20, 30];
        ёбнуть а[1];
        гыы х = а[0];
        гыы з = а[2];
        "#,
    );
    assert_eq!(i.get("х"), Some(Value::Number(10.0)));
    assert_eq!(i.get("з"), Some(Value::Number(30.0)));
}

#[test]
fn delete_array_out_of_bounds_is_noop() {
    let i = run_code(
        r#"
        гыы а = [10, 20];
        ёбнуть а[10];
        гыы д = длина(а);
        "#,
    );
    assert_eq!(i.get("д"), Some(Value::Number(2.0)));
}

#[test]
fn delete_string_index_is_runtime_error() {
    let err = run_code_err(
        r#"
        гыы с = "абв";
        ёбнуть с[0];
        "#,
    );
    assert!(err.message.to_lowercase().contains("стро"), "ошибка должна упоминать строки, получено: {}", err.message);
}

#[test]
fn in_operator() {
    let i = run_code(
        r#"
        гыы о = {х: 1, у: 2};
        гыы р1 = "х" из о;
        гыы р2 = "з" из о;
        "#,
    );
    assert_eq!(i.get("р1"), Some(Value::Boolean(true)));
    assert_eq!(i.get("р2"), Some(Value::Boolean(false)));
}

#[test]
fn pipeline_operator() {
    let i = run_code(
        r#"
        йопта удвоить(н) {
            отвечаю н * 2;
        }
        йопта прибавитьОдин(н) {
            отвечаю н + 1;
        }
        гыы рез = 5 |> удвоить |> прибавитьОдин;
        "#,
    );
    assert_eq!(i.get("рез"), Some(Value::Number(11.0)));
}

#[test]
fn void_operator() {
    let i = run_code(
        r#"
        гыы рез = куку 42;
        "#,
    );
    assert_eq!(i.get("рез"), Some(Value::Undefined));
}

#[test]
fn string_key_in_object() {
    let i = run_code(
        r#"
        гыы о = {"моё имя": 42};
        гыы рез = о["моё имя"];
        "#,
    );
    assert_eq!(i.get("рез"), Some(Value::Number(42.0)));
}
