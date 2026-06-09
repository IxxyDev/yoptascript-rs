use super::*;

#[test]
fn test_stdlib_math_basic() {
    let interp = run_code(
        r#"
        гыы а = Матан.пол(3.7);
        гыы б = Матан.потолок(3.2);
        гыы в = Матан.округлить(3.5);
        гыы г = Матан.модуль(-5);
        гыы д = Матан.мин(1, 2, 3);
        гыы е = Матан.макс(1, 2, 3);
        гыы ё = Матан.степень(2, 10);
        гыы ж = Матан.корень(16);
        "#,
    );
    assert_eq!(interp.get("а"), Some(Value::Number(3.0)));
    assert_eq!(interp.get("б"), Some(Value::Number(4.0)));
    assert_eq!(interp.get("в"), Some(Value::Number(4.0)));
    assert_eq!(interp.get("г"), Some(Value::Number(5.0)));
    assert_eq!(interp.get("д"), Some(Value::Number(1.0)));
    assert_eq!(interp.get("е"), Some(Value::Number(3.0)));
    assert_eq!(interp.get("ё"), Some(Value::Number(1024.0)));
    assert_eq!(interp.get("ж"), Some(Value::Number(4.0)));
}

#[test]
fn test_stdlib_math_constants() {
    let interp = run_code(
        r#"
        гыы пи = Матан.ПИ;
        гыы е = Матан.Е;
        "#,
    );
    assert_eq!(interp.get("пи"), Some(Value::Number(std::f64::consts::PI)));
    assert_eq!(interp.get("е"), Some(Value::Number(std::f64::consts::E)));
}

#[test]
fn test_stdlib_array_push_pop() {
    let interp = run_code(
        r#"
        гыы а = [1, 2];
        а.push(3);
        а.push(4);
        гыы последний = а.pop();
        "#,
    );
    assert_struct_eq(interp.get("а"), Value::array(vec![Value::Number(1.0), Value::Number(2.0), Value::Number(3.0)]));
    assert_eq!(interp.get("последний"), Some(Value::Number(4.0)));
}

#[test]
fn test_stdlib_array_length_property() {
    let interp = run_code(
        r#"
        гыы а = [1, 2, 3, 4, 5];
        гыы д = а.length;
        гыы д2 = а.длина;
        "#,
    );
    assert_eq!(interp.get("д"), Some(Value::Number(5.0)));
    assert_eq!(interp.get("д2"), Some(Value::Number(5.0)));
}

#[test]
fn test_stdlib_array_map_filter_reduce() {
    let interp = run_code(
        r#"
        гыы а = [1, 2, 3, 4, 5];
        гыы у = а.map((x) => x * 2);
        гыы ф = а.filter((x) => x > 2);
        гыы с = а.reduce((а, б) => а + б, 0);
        "#,
    );
    assert_struct_eq(
        interp.get("у"),
        Value::array(vec![
            Value::Number(2.0),
            Value::Number(4.0),
            Value::Number(6.0),
            Value::Number(8.0),
            Value::Number(10.0),
        ]),
    );
    assert_struct_eq(interp.get("ф"), Value::array(vec![Value::Number(3.0), Value::Number(4.0), Value::Number(5.0)]));
    assert_eq!(interp.get("с"), Some(Value::Number(15.0)));
}

#[test]
fn test_stdlib_array_find_includes_indexof() {
    let interp = run_code(
        r#"
        гыы а = [10, 20, 30, 40];
        гыы н = а.find((x) => x > 15);
        гыы и = а.includes(30);
        гыы ин = а.indexOf(40);
        гыы ин2 = а.indexOf(99);
        "#,
    );
    assert_eq!(interp.get("н"), Some(Value::Number(20.0)));
    assert_eq!(interp.get("и"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("ин"), Some(Value::Number(3.0)));
    assert_eq!(interp.get("ин2"), Some(Value::Number(-1.0)));
}

#[test]
fn test_stdlib_array_join_slice_reverse() {
    let interp = run_code(
        r#"
        гыы а = [1, 2, 3];
        гыы дж = а.join("-");
        гыы ср = а.slice(1, 3);
        гыы пр = а.toReversed();
        "#,
    );
    assert_eq!(interp.get("дж"), Some(Value::String("1-2-3".to_string())));
    assert_struct_eq(interp.get("ср"), Value::array(vec![Value::Number(2.0), Value::Number(3.0)]));
    assert_struct_eq(interp.get("пр"), Value::array(vec![Value::Number(3.0), Value::Number(2.0), Value::Number(1.0)]));
}

#[test]
fn test_stdlib_array_at() {
    let interp = run_code(
        r#"
        гыы а = [10, 20, 30];
        гыы п = а.at(0);
        гыы пос = а.at(-1);
        гыы внеДиапазона = а.at(99);
        "#,
    );
    assert_eq!(interp.get("п"), Some(Value::Number(10.0)));
    assert_eq!(interp.get("пос"), Some(Value::Number(30.0)));
    assert_eq!(interp.get("внеДиапазона"), Some(Value::Undefined));
}

#[test]
fn test_stdlib_array_flat() {
    let interp = run_code(
        r#"
        гыы а = [1, [2, [3, [4]]]];
        гыы пл1 = а.flat();
        гыы пл2 = а.flat(2);
        "#,
    );
    assert_struct_eq(
        interp.get("пл1"),
        Value::array(vec![
            Value::Number(1.0),
            Value::Number(2.0),
            Value::array(vec![Value::Number(3.0), Value::array(vec![Value::Number(4.0)])]),
        ]),
    );
    assert_struct_eq(
        interp.get("пл2"),
        Value::array(vec![
            Value::Number(1.0),
            Value::Number(2.0),
            Value::Number(3.0),
            Value::array(vec![Value::Number(4.0)]),
        ]),
    );
}

#[test]
fn test_stdlib_string_basic() {
    let interp = run_code(
        r#"
        гыы с = "Привет, Мир";
        гыы в = с.toUpperCase();
        гыы н = с.toLowerCase();
        гыы и = с.indexOf("Мир");
        гыы вкл = с.includes("Привет");
        "#,
    );
    assert_eq!(interp.get("в"), Some(Value::String("ПРИВЕТ, МИР".to_string())));
    assert_eq!(interp.get("н"), Some(Value::String("привет, мир".to_string())));
    assert_eq!(interp.get("и"), Some(Value::Number(8.0)));
    assert_eq!(interp.get("вкл"), Some(Value::Boolean(true)));
}

#[test]
fn test_stdlib_string_slice_trim_split() {
    let interp = run_code(
        r#"
        гыы с = "  привет  ";
        гыы об = с.trim();
        гыы сл = "a,b,c".split(",");
        гыы отр = "hello".slice(1, 4);
        "#,
    );
    assert_eq!(interp.get("об"), Some(Value::String("привет".to_string())));
    assert_struct_eq(
        interp.get("сл"),
        Value::array(vec![
            Value::String("a".to_string()),
            Value::String("b".to_string()),
            Value::String("c".to_string()),
        ]),
    );
    assert_eq!(interp.get("отр"), Some(Value::String("ell".to_string())));
}

#[test]
fn test_stdlib_string_length() {
    let interp = run_code(
        r#"
        гыы с = "Привет";
        гыы д = с.length;
        гыы д2 = с.длина;
        "#,
    );
    assert_eq!(interp.get("д"), Some(Value::Number(6.0)));
    assert_eq!(interp.get("д2"), Some(Value::Number(6.0)));
}

#[test]
fn test_stdlib_string_repeat_pad() {
    let interp = run_code(
        r#"
        гыы а = "abc".repeat(3);
        гыы б = "5".padStart(3, "0");
        гыы в = "5".padEnd(4, "-");
        "#,
    );
    assert_eq!(interp.get("а"), Some(Value::String("abcabcabc".to_string())));
    assert_eq!(interp.get("б"), Some(Value::String("005".to_string())));
    assert_eq!(interp.get("в"), Some(Value::String("5---".to_string())));
}

#[test]
fn test_stdlib_object_keys_values_entries() {
    let interp = run_code(
        r#"
        гыы о = { а: 1, б: 2 };
        гыы к = Кент.ключи(о);
        гыы з = Кент.значения(о);
        "#,
    );
    if let Some(Value::Array(keys)) = interp.get("к") {
        let mut keys = keys.borrow().clone();
        keys.sort_by_key(|v| v.to_string());
        assert_eq!(keys, vec![Value::String("а".to_string()), Value::String("б".to_string())]);
    } else {
        panic!("Expected Array");
    }
    if let Some(Value::Array(values)) = interp.get("з") {
        let mut values = values.borrow().clone();
        values.sort_by_key(|v| v.to_string());
        assert_eq!(values, vec![Value::Number(1.0), Value::Number(2.0)]);
    } else {
        panic!("Expected Array");
    }
}

#[test]
fn test_stdlib_json_stringify_parse_roundtrip() {
    let interp = run_code(
        r#"
        гыы о = { имя: "Саня", возраст: 25, активен: правда };
        гыы с = Жсон.вСтроку(о);
        гыы об = Жсон.разобрать(с);
        гыы имя = об.имя;
        гыы возраст = об.возраст;
        гыы активен = об.активен;
        "#,
    );
    assert_eq!(interp.get("имя"), Some(Value::String("Саня".to_string())));
    assert_eq!(interp.get("возраст"), Some(Value::Number(25.0)));
    assert_eq!(interp.get("активен"), Some(Value::Boolean(true)));
}

#[test]
fn test_stdlib_json_parse_array() {
    let interp = run_code(
        r#"
        гыы а = Жсон.разобрать("[1, 2, 3]");
        "#,
    );
    assert_struct_eq(interp.get("а"), Value::array(vec![Value::Number(1.0), Value::Number(2.0), Value::Number(3.0)]));
}

#[test]
fn json_parse_rejects_trailing_chars() {
    let err = run_code_err(r#"гыы а = Жсон.разобрать("{} мусор");"#);
    assert!(err.message.contains("Лишние символы"), "got: {}", err.message);
}

#[test]
fn json_parse_object_missing_colon() {
    let err = run_code_err(r#"гыы а = Жсон.разобрать("{\"к\" 1}");"#);
    assert!(err.message.contains("':'") || err.message.contains("JSON"), "got: {}", err.message);
}

#[test]
fn json_parse_array_missing_comma() {
    let err = run_code_err(r#"гыы а = Жсон.разобрать("[1 2]");"#);
    assert!(err.message.contains("','") || err.message.contains("']'"), "got: {}", err.message);
}

#[test]
fn json_parse_unexpected_token() {
    let err = run_code_err(r#"гыы а = Жсон.разобрать("чушь");"#);
    assert!(err.message.contains("JSON"), "got: {}", err.message);
}

#[test]
fn json_parse_incomplete_unicode_escape() {
    let err = run_code_err(r#"гыы а = Жсон.разобрать("\"\\u00\"");"#);
    assert!(err.message.contains("\\u") || err.message.contains("escape"), "got: {}", err.message);
}

#[test]
fn json_stringify_rejects_function() {
    let err = run_code_err(
        r#"
        гыы ф = () => 1;
        гыы с = Жсон.вСтроку(ф);
        "#,
    );
    assert!(err.message.contains("Функции") || err.message.contains("JSON"), "got: {}", err.message);
}

#[test]
fn json_stringify_rejects_symbol() {
    let err = run_code_err(
        r#"
        гыы с = Симбол("х");
        гыы стр = Жсон.вСтроку(с);
        "#,
    );
    assert!(err.message.contains("Символ") || err.message.contains("JSON"), "got: {}", err.message);
}

#[test]
fn object_keys_rejects_non_object() {
    let err = run_code_err(r#"гыы к = Кент.ключи(42);"#);
    assert!(err.message.contains("Кент.ключи"), "got: {}", err.message);
}

#[test]
fn promise_constructor_requires_function() {
    let err = run_code_err(r#"гыы p = захуярить СловоПацана(5);"#);
    assert!(err.message.contains("исполнитель") || err.message.contains("СловоПацана"), "got: {}", err.message);
}

#[test]
fn promise_race_rejects_empty_array() {
    let err = run_code_err(r#"гыы p = СловоПацана.гонка([]);"#);
    assert!(err.message.contains("гонка") || err.message.contains("пуст"), "got: {}", err.message);
}

#[test]
fn array_reduce_empty_without_initial_errors() {
    let err = run_code_err(
        r#"
        гыы а = [];
        гыы р = а.свернуть((а, в) => а + в);
        "#,
    );
    assert!(err.message.contains("reduce") || err.message.contains("пуст"), "got: {}", err.message);
}

#[test]
fn iterator_reduce_empty_without_initial_errors() {
    let err = run_code_err(
        r#"
        гыы и = Итератор.от([]);
        гыы р = и.свернуть((а, в) => а + в);
        "#,
    );
    assert!(err.message.contains("reduce") || err.message.contains("пуст"), "got: {}", err.message);
}

#[test]
fn string_repeat_negative_count_errors() {
    let err = run_code_err(
        r#"
        гыы с = "а";
        гыы р = с.повторить(-1);
        "#,
    );
    assert!(err.message.contains("повторений") || err.message.contains("Некорректное"), "got: {}", err.message);
}

#[test]
fn test_stdlib_number_checks() {
    let interp = run_code(
        r#"
        гыы кон = Хуйня.конечна(5);
        гыы кон2 = Хуйня.конечна(5.5);
        гыы цел = Хуйня.целая(5);
        гыы цел2 = Хуйня.целая(5.5);
        "#,
    );
    assert_eq!(interp.get("кон"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("кон2"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("цел"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("цел2"), Some(Value::Boolean(false)));
}

#[test]
fn test_stdlib_array_is_array() {
    let interp = run_code(
        r#"
        гыы а = Помойка.являетсяПомойкой([1, 2]);
        гыы б = Помойка.являетсяПомойкой("строка");
        "#,
    );
    assert_eq!(interp.get("а"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("б"), Some(Value::Boolean(false)));
}

#[test]
fn test_stdlib_array_sort() {
    let interp = run_code(
        r#"
        гыы а = [3, 1, 4, 1, 5, 9, 2, 6];
        а.sort((л, п) => л - п);
        "#,
    );
    assert_struct_eq(
        interp.get("а"),
        Value::array(vec![
            Value::Number(1.0),
            Value::Number(1.0),
            Value::Number(2.0),
            Value::Number(3.0),
            Value::Number(4.0),
            Value::Number(5.0),
            Value::Number(6.0),
            Value::Number(9.0),
        ]),
    );
}

#[test]
fn test_stdlib_array_splice() {
    let interp = run_code(
        r#"
        гыы а = [1, 2, 3, 4, 5];
        гыы удалённые = а.splice(1, 2, 9, 9);
        "#,
    );
    assert_struct_eq(
        interp.get("а"),
        Value::array(vec![
            Value::Number(1.0),
            Value::Number(9.0),
            Value::Number(9.0),
            Value::Number(4.0),
            Value::Number(5.0),
        ]),
    );
    assert_struct_eq(interp.get("удалённые"), Value::array(vec![Value::Number(2.0), Value::Number(3.0)]));
}

#[test]
fn test_stdlib_array_to_spliced() {
    let interp = run_code(
        r#"
        гыы а = [1, 2, 3, 4];
        гыы б = а.toSpliced(1, 1, 8, 9);
        "#,
    );
    assert_struct_eq(
        interp.get("а"),
        Value::array(vec![Value::Number(1.0), Value::Number(2.0), Value::Number(3.0), Value::Number(4.0)]),
    );
    assert_struct_eq(
        interp.get("б"),
        Value::array(vec![
            Value::Number(1.0),
            Value::Number(8.0),
            Value::Number(9.0),
            Value::Number(3.0),
            Value::Number(4.0),
        ]),
    );
}

#[test]
fn test_stdlib_chained_methods() {
    let interp = run_code(
        r#"
        гыы рез = [1, 2, 3, 4, 5]
            .filter((x) => x > 1)
            .map((x) => x * x)
            .reduce((а, б) => а + б, 0);
        "#,
    );
    assert_eq!(interp.get("рез"), Some(Value::Number(54.0)));
}
