use super::*;

#[test]
fn string_escape_newline() {
    let interp = run_code(
        r#"
        гыы с = "привет\nмир";
        "#,
    );
    assert_eq!(interp.get("с"), Some(Value::String("привет\nмир".to_string())));
}

#[test]
fn string_escape_tab() {
    let interp = run_code(
        r#"
        гыы с = "а\tб";
        "#,
    );
    assert_eq!(interp.get("с"), Some(Value::String("а\tб".to_string())));
}

#[test]
fn string_escape_backslash() {
    let interp = run_code(
        r#"
        гыы с = "путь\\файл";
        "#,
    );
    assert_eq!(interp.get("с"), Some(Value::String("путь\\файл".to_string())));
}

#[test]
fn string_escape_quote() {
    let interp = run_code(
        r#"
        гыы с = "он сказал \"да\"";
        "#,
    );
    assert_eq!(interp.get("с"), Some(Value::String("он сказал \"да\"".to_string())));
}

#[test]
fn string_escape_combined() {
    let interp = run_code(
        r#"
        гыы с = "строка1\nстрока2\tтаб";
        "#,
    );
    assert_eq!(interp.get("с"), Some(Value::String("строка1\nстрока2\tтаб".to_string())));
}

#[test]
fn template_no_substitution() {
    let interp = run_code("гыы р = `привет мир`;");
    assert_eq!(interp.get("р"), Some(Value::String("привет мир".to_string())));
}

#[test]
fn template_empty() {
    let interp = run_code("гыы р = ``;");
    assert_eq!(interp.get("р"), Some(Value::String(String::new())));
}

#[test]
fn template_single_interpolation() {
    let interp = run_code(
        r#"
        гыы имя = "Вася";
        гыы р = `привет, ${имя}!`;
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::String("привет, Вася!".to_string())));
}

#[test]
fn template_multiple_interpolations() {
    let interp = run_code(
        r#"
        гыы а = 1;
        гыы б = 2;
        гыы р = `${а} + ${б} = ${а + б}`;
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::String("1 + 2 = 3".to_string())));
}

#[test]
fn template_expression_interpolation() {
    let interp = run_code("гыы р = `результат: ${2 + 3 * 4}`;");
    assert_eq!(interp.get("р"), Some(Value::String("результат: 14".to_string())));
}

#[test]
fn template_with_escape() {
    let interp = run_code("гыы р = `строка1\\nстрока2`;");
    assert_eq!(interp.get("р"), Some(Value::String("строка1\nстрока2".to_string())));
}

#[test]
fn template_multiline() {
    let interp = run_code("гыы р = `строка1\nстрока2`;");
    assert_eq!(interp.get("р"), Some(Value::String("строка1\nстрока2".to_string())));
}

#[test]
fn template_nested() {
    let interp = run_code(
        r#"
        гыы х = 5;
        гыы р = `внешний ${`внутренний ${х}`}`;
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::String("внешний внутренний 5".to_string())));
}

#[test]
fn template_with_object_in_braces() {
    let interp = run_code(
        r#"
        гыы а = [1, 2, 3];
        гыы р = `длина: ${длина(а)}`;
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::String("длина: 3".to_string())));
}

#[test]
fn template_only_interpolation() {
    let interp = run_code(
        r#"
        гыы х = 42;
        гыы р = `${х}`;
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::String("42".to_string())));
}

#[test]
fn template_escaped_dollar() {
    let interp = run_code("гыы р = `цена: \\${100}`;");
    assert_eq!(interp.get("р"), Some(Value::String("цена: ${100}".to_string())));
}

#[test]
fn template_ternary_inside() {
    let interp = run_code(
        r#"
        гыы х = 10;
        гыы р = `число ${х > 5 ? "большое" : "маленькое"}`;
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::String("число большое".to_string())));
}

#[test]
fn tagged_template_basic() {
    let interp = run_code(
        r#"
        йопта тег(строки, ...значения) {
            гыы р = "";
            го (гыы и = 0; и < строки.длина; и += 1) {
                р += строки[и];
                вилкойвглаз (и < значения.длина) {
                    р += "<" + значения[и] + ">";
                }
            }
            отвечаю р;
        }
        гыы имя = "Мир";
        гыы возраст = 42;
        гыы результат = тег`Привет, ${имя}! Тебе ${возраст}.`;
        "#,
    );
    assert_eq!(interp.get("результат"), Some(Value::String("Привет, <Мир>! Тебе <42>.".to_string())));
}

#[test]
fn tagged_template_no_substitutions() {
    let interp = run_code(
        r#"
        йопта тег(строки) { отвечаю строки[0]; }
        гыы р = тег`просто текст`;
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::String("просто текст".to_string())));
}

#[test]
fn tagged_template_raw_vs_cooked() {
    let interp = run_code(
        r#"
        йопта сырой(строки) { отвечаю строки.сырьё[0]; }
        йопта готовый(строки) { отвечаю строки[0]; }
        гыы r = сырой`a\nb`;
        гыы c = готовый`a\nb`;
        "#,
    );
    assert_eq!(interp.get("r"), Some(Value::String("a\\nb".to_string())));
    assert_eq!(interp.get("c"), Some(Value::String("a\nb".to_string())));
}

#[test]
fn tagged_template_strings_length() {
    let interp = run_code(
        r#"
        йопта тег(строки, ...значения) { отвечаю строки.длина; }
        гыы н = тег`${1}${2}${3}`;
        "#,
    );
    assert_eq!(interp.get("н"), Some(Value::Number(4.0)));
}

#[test]
fn tagged_template_builtin_dlina_on_strings() {
    let interp = run_code(
        r#"
        йопта тег(строки) { отвечаю длина(строки); }
        гыы н = тег`a${1}b${2}c`;
        "#,
    );
    assert_eq!(interp.get("н"), Some(Value::Number(3.0)));
}
