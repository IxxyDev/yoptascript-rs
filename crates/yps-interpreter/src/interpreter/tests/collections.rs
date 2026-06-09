use super::*;

#[test]
fn test_karta_get_or_insert() {
    let interp = run_code(
        r#"
        гыы м = захуярить Карта();
        м.set("а", 1);
        гыы существ = м.getOrInsert("а", 99);
        гыы новое = м.getOrInsert("б", 7);
        гыы итог = м.get("б");
        "#,
    );
    assert_eq!(interp.get("существ"), Some(Value::Number(1.0)));
    assert_eq!(interp.get("новое"), Some(Value::Number(7.0)));
    assert_eq!(interp.get("итог"), Some(Value::Number(7.0)));
}

#[test]
fn test_karta_get_or_insert_computed() {
    let interp = run_code(
        r#"
        гыы м = захуярить Карта();
        гыы вызовов = 0;
        гыы вычислить = (к) => {
            вызовов += 1;
            отвечаю к + "!";
        };
        гыы а = м.getOrInsertComputed("привет", вычислить);
        гыы б = м.getOrInsertComputed("привет", вычислить);
        "#,
    );
    assert_eq!(interp.get("а"), Some(Value::String("привет!".to_string())));
    assert_eq!(interp.get("б"), Some(Value::String("привет!".to_string())));
    assert_eq!(interp.get("вызовов"), Some(Value::Number(1.0)));
}

#[test]
fn test_kosyak_construct() {
    let interp = run_code(
        r#"
        гыы е = захуярить Косяк("плохо");
        гыы имя = е.name;
        гыы сообщ = е.message;
        "#,
    );
    assert_eq!(interp.get("имя"), Some(Value::String("Косяк".to_string())));
    assert_eq!(interp.get("сообщ"), Some(Value::String("плохо".to_string())));
}

#[test]
fn test_kosyak_without_new() {
    let interp = run_code(
        r#"
        гыы е = Косяк("сообщение");
        гыы имя = е.name;
        "#,
    );
    assert_eq!(interp.get("имя"), Some(Value::String("Косяк".to_string())));
}

#[test]
fn test_kosyak_throw_catch() {
    let interp = run_code(
        r#"
        гыы пойман = ноль;
        хапнуть {
            кидай захуярить Косяк("упало");
        } гоп (е) {
            пойман = е.message;
        }
        "#,
    );
    assert_eq!(interp.get("пойман"), Some(Value::String("упало".to_string())));
}

#[test]
fn test_kosyak_with_cause() {
    let interp = run_code(
        r#"
        гыы первый = захуярить Косяк("первая ошибка");
        гыы второй = захуярить Косяк("обёртка", { cause: первый });
        гыы причина = второй.cause;
        гыы сообщ = причина.message;
        "#,
    );
    assert_eq!(interp.get("сообщ"), Some(Value::String("первая ошибка".to_string())));
}

#[test]
fn test_kent_group_by() {
    let interp = run_code(
        r#"
        гыы числа = [1, 2, 3, 4, 5, 6, 7];
        гыы по_чётности = Кент.группировать(числа, (n) => n % 2 === 0 ? "чётные" : "нечётные");
        гыы чётные = по_чётности["чётные"];
        гыы нечётные = по_чётности["нечётные"];
        "#,
    );
    assert_struct_eq(
        interp.get("чётные"),
        Value::array(vec![Value::Number(2.0), Value::Number(4.0), Value::Number(6.0)]),
    );
    assert_struct_eq(
        interp.get("нечётные"),
        Value::array(vec![Value::Number(1.0), Value::Number(3.0), Value::Number(5.0), Value::Number(7.0)]),
    );
}

#[test]
fn test_huynya_parse_int() {
    let interp = run_code(
        r#"
        гыы а = Хуйня.разобратьЦелое("42");
        гыы б = Хуйня.разобратьЦелое("  -17  ");
        гыы в = Хуйня.разобратьЦелое("1010", 2);
        гыы г = Хуйня.разобратьЦелое("ff", 16);
        гыы д = Хуйня.разобратьЦелое("123abc");
        гыы е = Хуйня.разобратьЦелое("xyz");
        "#,
    );
    assert_eq!(interp.get("а"), Some(Value::Number(42.0)));
    assert_eq!(interp.get("б"), Some(Value::Number(-17.0)));
    assert_eq!(interp.get("в"), Some(Value::Number(10.0)));
    assert_eq!(interp.get("г"), Some(Value::Number(255.0)));
    assert_eq!(interp.get("д"), Some(Value::Number(123.0)));
    if let Some(Value::Number(n)) = interp.get("е") {
        assert!(n.is_nan(), "ожидалось NaN, получено {n}");
    } else {
        panic!("ожидалось Number(NaN)");
    }
}

#[test]
fn test_huynya_parse_float() {
    let interp = run_code(
        r#"
        гыы а = Хуйня.разобратьЧисло("2.5");
        гыы б = Хуйня.разобратьЧисло("  -2.5e2  ");
        гыы в = Хуйня.разобратьЧисло("123abc");
        гыы г = Хуйня.разобратьЧисло("");
        "#,
    );
    assert_eq!(interp.get("а"), Some(Value::Number(2.5)));
    assert_eq!(interp.get("б"), Some(Value::Number(-250.0)));
    if let Some(Value::Number(n)) = interp.get("г") {
        assert!(n.is_nan());
    } else {
        panic!("ожидалось NaN");
    }
    let _ = interp.get("в");
}

#[test]
fn test_spread_set_into_array() {
    let interp = run_code(
        r#"
        гыы н = захуярить Набор([1, 2, 3]);
        гыы а = [0, ...н, 4];
        "#,
    );
    assert_struct_eq(
        interp.get("а"),
        Value::array(vec![
            Value::Number(0.0),
            Value::Number(1.0),
            Value::Number(2.0),
            Value::Number(3.0),
            Value::Number(4.0),
        ]),
    );
}

#[test]
fn test_spread_map_into_array() {
    let interp = run_code(
        r#"
        гыы к = захуярить Карта([["а", 1], ["б", 2]]);
        гыы а = [...к];
        "#,
    );
    assert_struct_eq(
        interp.get("а"),
        Value::array(vec![
            Value::array(vec![Value::String("а".to_string()), Value::Number(1.0)]),
            Value::array(vec![Value::String("б".to_string()), Value::Number(2.0)]),
        ]),
    );
}

#[test]
fn test_spread_string_into_array() {
    let interp = run_code(
        r#"
        гыы а = [..."абв"];
        "#,
    );
    assert_struct_eq(
        interp.get("а"),
        Value::array(vec![
            Value::String("а".to_string()),
            Value::String("б".to_string()),
            Value::String("в".to_string()),
        ]),
    );
}

#[test]
fn test_spread_set_into_args() {
    let interp = run_code(
        r#"
        гыы н = захуярить Набор([1, 5, 3, 9, 2]);
        гыы макс = Матан.макс(...н);
        "#,
    );
    assert_eq!(interp.get("макс"), Some(Value::Number(9.0)));
}

#[test]
fn test_for_of_set() {
    let interp = run_code(
        r#"
        гыы н = захуярить Набор([10, 20, 30]);
        гыы сумма = 0;
        го (х сашаГрей н) {
            сумма += х;
        }
        "#,
    );
    assert_eq!(interp.get("сумма"), Some(Value::Number(60.0)));
}

#[test]
fn test_for_of_map_yields_pairs() {
    let interp = run_code(
        r#"
        гыы к = захуярить Карта([["а", 1], ["б", 2], ["в", 3]]);
        гыы ключи = [];
        гыы суммаЗнч = 0;
        го (пара сашаГрей к) {
            ключи.push(пара[0]);
            суммаЗнч += пара[1];
        }
        "#,
    );
    assert_struct_eq(
        interp.get("ключи"),
        Value::array(vec![
            Value::String("а".to_string()),
            Value::String("б".to_string()),
            Value::String("в".to_string()),
        ]),
    );
    assert_eq!(interp.get("суммаЗнч"), Some(Value::Number(6.0)));
}

#[test]
fn test_nabor_basic_operations() {
    let interp = run_code(
        r#"
        гыы н = захуярить Набор();
        н.add(1);
        н.add(2);
        н.add(2);
        гыы есть = н.has(1);
        гыы размер = н.size;
        "#,
    );
    assert_eq!(interp.get("есть"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("размер"), Some(Value::Number(2.0)));
}

#[test]
fn test_nabor_construct_from_array() {
    let interp = run_code(
        r#"
        гыы н = захуярить Набор([1, 2, 2, 3, 3, 3]);
        гыы размер = н.size;
        "#,
    );
    assert_eq!(interp.get("размер"), Some(Value::Number(3.0)));
}

#[test]
fn test_nabor_delete_clear() {
    let interp = run_code(
        r#"
        гыы н = захуярить Набор([1, 2, 3]);
        гыы убрал = н.delete(2);
        гыы естьЛи = н.has(2);
        н.clear();
        гыы пустой = н.size;
        "#,
    );
    assert_eq!(interp.get("убрал"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("естьЛи"), Some(Value::Boolean(false)));
    assert_eq!(interp.get("пустой"), Some(Value::Number(0.0)));
}

#[test]
fn test_nabor_values() {
    let interp = run_code(
        r#"
        гыы н = захуярить Набор([3, 1, 2]);
        гыы зн = н.values();
        "#,
    );
    assert_struct_eq(interp.get("зн"), Value::array(vec![Value::Number(3.0), Value::Number(1.0), Value::Number(2.0)]));
}

#[test]
fn test_nabor_for_each() {
    let interp = run_code(
        r#"
        гыы н = захуярить Набор([10, 20, 30]);
        гыы сумма = 0;
        н.forEach((x) => { сумма += x; });
        "#,
    );
    assert_eq!(interp.get("сумма"), Some(Value::Number(60.0)));
}

#[test]
fn test_nabor_set_operations() {
    let interp = run_code(
        r#"
        гыы а = захуярить Набор([1, 2, 3]);
        гыы б = захуярить Набор([3, 4, 5]);
        гыы пересечение = а.intersection(б).size;
        гыы объединение = а.union(б).size;
        гыы разница = а.difference(б).size;
        гыы симРазн = а.symmetricDifference(б).size;
        "#,
    );
    assert_eq!(interp.get("пересечение"), Some(Value::Number(1.0)));
    assert_eq!(interp.get("объединение"), Some(Value::Number(5.0)));
    assert_eq!(interp.get("разница"), Some(Value::Number(2.0)));
    assert_eq!(interp.get("симРазн"), Some(Value::Number(4.0)));
}

#[test]
fn test_nabor_subset_superset_disjoint() {
    let interp = run_code(
        r#"
        гыы малый = захуярить Набор([1, 2]);
        гыы большой = захуярить Набор([1, 2, 3, 4]);
        гыы отдельный = захуярить Набор([99]);
        гыы под = малый.isSubsetOf(большой);
        гыы над = большой.isSupersetOf(малый);
        гыы непер = малый.isDisjointFrom(отдельный);
        "#,
    );
    assert_eq!(interp.get("под"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("над"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("непер"), Some(Value::Boolean(true)));
}

#[test]
fn test_karta_basic_operations() {
    let interp = run_code(
        r#"
        гыы к = захуярить Карта();
        к.set("а", 1);
        к.set("б", 2);
        гыы есть = к.has("а");
        гыы значение = к.get("б");
        гыы размер = к.size;
        "#,
    );
    assert_eq!(interp.get("есть"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("значение"), Some(Value::Number(2.0)));
    assert_eq!(interp.get("размер"), Some(Value::Number(2.0)));
}

#[test]
fn test_karta_construct_from_pairs() {
    let interp = run_code(
        r#"
        гыы к = захуярить Карта([["а", 1], ["б", 2]]);
        гыы а = к.get("а");
        гыы б = к.get("б");
        "#,
    );
    assert_eq!(interp.get("а"), Some(Value::Number(1.0)));
    assert_eq!(interp.get("б"), Some(Value::Number(2.0)));
}

#[test]
fn test_karta_delete_clear() {
    let interp = run_code(
        r#"
        гыы к = захуярить Карта([["а", 1], ["б", 2], ["в", 3]]);
        к.delete("б");
        гыы есть = к.has("б");
        гыы рдо = к.size;
        к.clear();
        гыы рпосле = к.size;
        "#,
    );
    assert_eq!(interp.get("есть"), Some(Value::Boolean(false)));
    assert_eq!(interp.get("рдо"), Some(Value::Number(2.0)));
    assert_eq!(interp.get("рпосле"), Some(Value::Number(0.0)));
}

#[test]
fn test_karta_keys_values_entries_preserve_insertion_order() {
    let interp = run_code(
        r#"
        гыы к = захуярить Карта();
        к.set("первый", 1);
        к.set("второй", 2);
        к.set("третий", 3);
        гыы клч = к.keys();
        гыы знч = к.values();
        "#,
    );
    assert_struct_eq(
        interp.get("клч"),
        Value::array(vec![
            Value::String("первый".to_string()),
            Value::String("второй".to_string()),
            Value::String("третий".to_string()),
        ]),
    );
    assert_struct_eq(interp.get("знч"), Value::array(vec![Value::Number(1.0), Value::Number(2.0), Value::Number(3.0)]));
}

#[test]
fn test_karta_overwrite_keeps_position() {
    let interp = run_code(
        r#"
        гыы к = захуярить Карта();
        к.set("а", 1);
        к.set("б", 2);
        к.set("а", 99);
        гыы знч = к.values();
        "#,
    );
    assert_struct_eq(interp.get("знч"), Value::array(vec![Value::Number(99.0), Value::Number(2.0)]));
}

#[test]
fn test_karta_static_ot_par() {
    let interp = run_code(
        r#"
        гыы к = Карта.отПар([["x", 10], ["y", 20]]);
        гыы x = к.get("x");
        "#,
    );
    assert_eq!(interp.get("x"), Some(Value::Number(10.0)));
}

#[test]
fn test_karta_for_each() {
    let interp = run_code(
        r#"
        гыы к = захуярить Карта([["а", 1], ["б", 2]]);
        гыы сумма = 0;
        к.forEach((значение, ключ) => {
            сумма += значение;
        });
        "#,
    );
    assert_eq!(interp.get("сумма"), Some(Value::Number(3.0)));
}

#[test]
fn test_karta_keys_supports_non_string() {
    let interp = run_code(
        r#"
        гыы к = захуярить Карта();
        к.set(1, "один");
        к.set(2, "два");
        гыы а = к.get(1);
        гыы б = к.get(2);
        "#,
    );
    assert_eq!(interp.get("а"), Some(Value::String("один".to_string())));
    assert_eq!(interp.get("б"), Some(Value::String("два".to_string())));
}

#[test]
fn test_eto_kosyak() {
    let interp = run_code(
        r#"
        гыы а = этоКосяк(захуярить Косяк("ой"));
        гыы б = этоКосяк("просто строка");
        гыы в = этоКосяк({ name: "Другое" });
        "#,
    );
    assert_eq!(interp.get("а"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("б"), Some(Value::Boolean(false)));
    assert_eq!(interp.get("в"), Some(Value::Boolean(false)));
}

#[test]
fn test_kosyak_static_is_error() {
    let interp = run_code(
        r#"
        гыы а = Косяк.этоКосяк(захуярить Косяк("ой"));
        гыы б = Косяк.этоКосяк("строка");
        гыы в = Косяк.этоКосяк({ name: "Другое" });
        гыы г = Косяк.этоКосяк(ноль);
        "#,
    );
    assert_eq!(interp.get("а"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("б"), Some(Value::Boolean(false)));
    assert_eq!(interp.get("в"), Some(Value::Boolean(false)));
    assert_eq!(interp.get("г"), Some(Value::Boolean(false)));
}

#[test]
fn test_kosyak_static_is_error_english_alias() {
    let interp = run_code(
        r#"
        гыы а = Косяк.isError(захуярить Косяк("ой"));
        гыы б = Косяк.isError(42);
        "#,
    );
    assert_eq!(interp.get("а"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("б"), Some(Value::Boolean(false)));
}

#[test]
fn test_kosyak_unknown_static_method() {
    let err = run_code_err(
        r#"
        Косяк.несуществует("х");
        "#,
    );
    assert!(
        err.message.contains("Косяк") && err.message.contains("несуществует"),
        "Сообщение должно упоминать неизвестный метод, получено: {}",
        err.message
    );
}
