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

#[test]
fn string_index_of_from_index() {
    let interp = run_code(
        r#"
        гыы a = "abcabc".indexOf("bc", 2);
        гыы b = "abcabc".indexOf("", 3);
        "#,
    );
    assert_eq!(interp.get("a"), Some(Value::Number(4.0)));
    assert_eq!(interp.get("b"), Some(Value::Number(3.0)));
}

#[test]
fn string_last_index_of() {
    let interp = run_code(
        r#"
        гыы a = "abcabc".lastIndexOf("bc");
        гыы b = "abcabc".lastIndexOf("bc", 2);
        гыы c = "abcabc".lastIndexOf("x");
        гыы d = "abcabc".lastIndexOf("");
        "#,
    );
    assert_eq!(interp.get("a"), Some(Value::Number(4.0)));
    assert_eq!(interp.get("b"), Some(Value::Number(1.0)));
    assert_eq!(interp.get("c"), Some(Value::Number(-1.0)));
    assert_eq!(interp.get("d"), Some(Value::Number(6.0)));
}

#[test]
fn string_includes_from_index() {
    let interp = run_code(
        r#"
        гыы a = "abcabc".содержит("cab", 3);
        гыы b = "abcabc".содержит("abc", 1);
        "#,
    );
    assert_eq!(interp.get("a"), Some(Value::Boolean(false)));
    assert_eq!(interp.get("b"), Some(Value::Boolean(true)));
}

#[test]
fn string_starts_ends_with_position() {
    let interp = run_code(
        r#"
        гыы a = "hello".начинаетсяС("ll", 2);
        гыы b = "hello".начинаетсяС("he", 1);
        гыы c = "hello".заканчиваетсяНа("ell", 4);
        гыы d = "hello".заканчиваетсяНа("lo", 4);
        "#,
    );
    assert_eq!(interp.get("a"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("b"), Some(Value::Boolean(false)));
    assert_eq!(interp.get("c"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("d"), Some(Value::Boolean(false)));
}

#[test]
fn string_split_limit() {
    let interp = run_code(
        r#"
        гыы a = "a,b,c".разбить(",", 2).join("|");
        гыы b = "a-b-c".разбить("-", 0).длина;
        гыы c = "hello".разбить("", 3).join("|");
        "#,
    );
    assert_eq!(interp.get("a"), Some(Value::String("a|b".to_string())));
    assert_eq!(interp.get("b"), Some(Value::Number(0.0)));
    assert_eq!(interp.get("c"), Some(Value::String("h|e|l".to_string())));
}

#[test]
fn string_replace_dollar_patterns() {
    let interp = run_code(
        r#"
        гыы a = "price: 100".заменить("100", "$$");
        гыы b = "name".заменить("name", "[$&]");
        гыы c = "a.b.c".заменитьВсе(".", "$$");
        "#,
    );
    assert_eq!(interp.get("a"), Some(Value::String("price: $".to_string())));
    assert_eq!(interp.get("b"), Some(Value::String("[name]".to_string())));
    assert_eq!(interp.get("c"), Some(Value::String("a$b$c".to_string())));
}

#[test]
fn string_length_utf16_emoji() {
    let interp = run_code(
        r#"
        гыы a = "😀".длина;
        гыы b = "a😀b".длина;
        "#,
    );
    assert_eq!(interp.get("a"), Some(Value::Number(2.0)));
    assert_eq!(interp.get("b"), Some(Value::Number(4.0)));
}

#[test]
fn string_length_utf16_bmp() {
    let interp = run_code(
        r#"
        гыы a = "café".длина;
        гыы b = "привет".длина;
        "#,
    );
    assert_eq!(interp.get("a"), Some(Value::Number(4.0)));
    assert_eq!(interp.get("b"), Some(Value::Number(6.0)));
}

#[test]
fn string_slice_utf16_code_units() {
    let interp = run_code(
        r#"
        гыы a = "a😀b".отрезать(0, 1);
        гыы b = "a😀b".отрезать(3, 4);
        гыы c = "a😀b".подстрока(1, 3);
        "#,
    );
    assert_eq!(interp.get("a"), Some(Value::String("a".to_string())));
    assert_eq!(interp.get("b"), Some(Value::String("b".to_string())));
    assert_eq!(interp.get("c"), Some(Value::String("😀".to_string())));
}

#[test]
fn string_at_utf16_code_units() {
    let interp = run_code(
        r#"
        гыы a = "a😀b".поИндексу(0);
        гыы b = "a😀b".поИндексу(3);
        гыы c = "a😀b".поИндексу(-1);
        "#,
    );
    assert_eq!(interp.get("a"), Some(Value::String("a".to_string())));
    assert_eq!(interp.get("b"), Some(Value::String("b".to_string())));
    assert_eq!(interp.get("c"), Some(Value::String("b".to_string())));
}

#[test]
fn string_char_code_at_basic() {
    let interp = run_code(
        r#"
        гыы a = "A".charCodeAt(0);
        гыы b = "b".charCodeAt(0);
        "#,
    );
    assert_eq!(interp.get("a"), Some(Value::Number(65.0)));
    assert_eq!(interp.get("b"), Some(Value::Number(98.0)));
}

#[test]
fn string_index_of_utf16() {
    let interp = run_code(
        r#"
        гыы a = "a😀b".найтиПодстроку("b");
        гыы b = "a😀b".содержит("b");
        "#,
    );
    assert_eq!(interp.get("a"), Some(Value::Number(3.0)));
    assert_eq!(interp.get("b"), Some(Value::Boolean(true)));
}

#[test]
fn string_index_access_utf16() {
    let interp = run_code(
        r#"
        гыы a = "a😀b"[0];
        гыы b = "a😀b"[3];
        "#,
    );
    assert_eq!(interp.get("a"), Some(Value::String("a".to_string())));
    assert_eq!(interp.get("b"), Some(Value::String("b".to_string())));
}

#[test]
fn bound_string_method_extract_and_call() {
    let interp = run_code(
        r#"
        гыы в = "абв".toUpperCase;
        гыы рез = в();
        "#,
    );
    assert_eq!(interp.get("рез"), Some(Value::String("АБВ".to_string())));
}

#[test]
fn bound_string_method_extract_russian_alias() {
    let interp = run_code(
        r#"
        гыы в = "аБв".вНижнийРегистр;
        гыы рез = в();
        "#,
    );
    assert_eq!(interp.get("рез"), Some(Value::String("абв".to_string())));
}

#[test]
fn bound_string_unknown_property_is_undefined() {
    let interp = run_code(
        r#"
        гыы с = "абв";
        гыы м = с.нетТакогоМетода;
        "#,
    );
    assert_eq!(interp.get("м"), Some(Value::Undefined));
}

#[test]
fn code_point_at_basic() {
    let interp = run_code(
        r#"
        гыы a = "abc".codePointAt(0);
        гыы b = "abc".кодТочки(1);
        "#,
    );
    assert_eq!(interp.get("a"), Some(Value::Number(97.0)));
    assert_eq!(interp.get("b"), Some(Value::Number(98.0)));
}

#[test]
fn code_point_at_surrogate_pair() {
    let interp = run_code(
        r#"
        гыы a = "😀".codePointAt(0);
        "#,
    );
    assert_eq!(interp.get("a"), Some(Value::Number(128512.0)));
}

#[test]
fn code_point_at_out_of_range_is_undefined() {
    let interp = run_code(
        r#"
        гыы a = "x".codePointAt(5);
        "#,
    );
    assert_eq!(interp.get("a"), Some(Value::Undefined));
}

#[test]
fn string_namespace_from_char_code() {
    let interp = run_code(
        r#"
        гыы a = Строка.изСимволов(72, 105);
        гыы b = Строка.fromCharCode(72, 105);
        "#,
    );
    assert_eq!(interp.get("a"), Some(Value::String("Hi".to_string())));
    assert_eq!(interp.get("b"), Some(Value::String("Hi".to_string())));
}

#[test]
fn string_namespace_from_code_point() {
    let interp = run_code(
        r#"
        гыы a = Строка.изКодовТочек(128512);
        гыы b = Строка.fromCodePoint(97, 98);
        "#,
    );
    assert_eq!(interp.get("a"), Some(Value::String("😀".to_string())));
    assert_eq!(interp.get("b"), Some(Value::String("ab".to_string())));
}

#[test]
fn string_namespace_from_code_point_invalid_errors() {
    let err = run_code_err(
        r#"
        Строка.изКодовТочек(-1);
        "#,
    );
    assert!(err.message.contains("кодовая точка") || err.message.contains("Некорректная"));
}

#[test]
fn string_namespace_raw() {
    let interp = run_code(
        r#"
        йопта тег(строки, ...значения) {
            отвечаю Строка.raw(строки, ...значения);
        }
        гыы имя = "Мир";
        гыы rez = тег`Привет, ${имя}!\n`;
        "#,
    );
    assert_eq!(interp.get("rez"), Some(Value::String("Привет, Мир!\\n".to_string())));
}

#[test]
fn normalize_default_form_is_nfc() {
    let interp = run_code(
        r#"
        гыы composed = "Й";
        гыы decomposed = "Й";
        гыы a = composed.normalize();
        гыы b = decomposed.normalize();
        гыы equal = a === b;
        "#,
    );
    assert_eq!(interp.get("equal"), Some(Value::Boolean(true)));
}

#[test]
fn normalize_cyrillic_composed_decomposed_round_trip() {
    let interp = run_code(
        r#"
        гыы composed = "Й";
        гыы decomposed = "Й";
        гыы a = composed.normalize("NFD") === decomposed.normalize("NFD");
        гыы b = composed.normalize("NFC") === decomposed.normalize("NFC");
        гыы roundTrip = composed.normalize("NFD").normalize("NFC") === composed;
        "#,
    );
    assert_eq!(interp.get("a"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("b"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("roundTrip"), Some(Value::Boolean(true)));
}

#[test]
fn normalize_nfkc_and_nfkd_forms() {
    let interp = run_code(
        r#"
        гыы a = "ﬁ".normalize("NFKC");
        гыы b = "ﬁ".нормализовать("NFKD");
        "#,
    );
    assert_eq!(interp.get("a"), Some(Value::String("fi".to_string())));
    assert_eq!(interp.get("b"), Some(Value::String("fi".to_string())));
}

#[test]
fn normalize_russian_alias() {
    let interp = run_code(
        r#"
        гыы composed = "Й";
        гыы decomposed = "Й";
        гыы equal = composed.нормализовать("NFD") === decomposed.нормализовать("NFD");
        "#,
    );
    assert_eq!(interp.get("equal"), Some(Value::Boolean(true)));
}

#[test]
fn normalize_invalid_form_errors() {
    let err = run_code_err(
        r#"
        "x".normalize("BOGUS");
        "#,
    );
    assert!(err.message.contains("нормализации"), "got: {}", err.message);
}
