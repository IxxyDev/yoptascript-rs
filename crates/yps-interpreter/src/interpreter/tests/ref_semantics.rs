use super::*;

#[test]
fn ref_semantics_object_alias_sees_mutation() {
    let interp = run_code(
        r#"
        гыы о1 = { х: 0 };
        гыы о2 = о1;
        о1.х = 1;
        гыы рез = о2.х;
        "#,
    );
    assert_eq!(interp.get("рез"), Some(Value::Number(1.0)));
}

#[test]
fn ref_semantics_array_alias_sees_push() {
    let interp = run_code(
        r#"
        гыы а1 = [1, 2];
        гыы а2 = а1;
        а1.втолкнуть(9);
        гыы дл = длина(а2);
        гыы посл = а2[2];
        "#,
    );
    assert_eq!(interp.get("дл"), Some(Value::Number(3.0)));
    assert_eq!(interp.get("посл"), Some(Value::Number(9.0)));
}

#[test]
fn ref_semantics_nested_mutation_visible_through_alias() {
    let interp = run_code(
        r#"
        гыы о = { вложен: { б: 0 } };
        гыы алиас = о.вложен;
        о.вложен.б = 7;
        гыы рез = алиас.б;
        "#,
    );
    assert_eq!(interp.get("рез"), Some(Value::Number(7.0)));
}

#[test]
fn ref_semantics_class_instance_is_reference() {
    let interp = run_code(
        r#"
        клёво Точка {
            конструктор() { тырыпыры.поле = 0; }
        }
        гыы а = захуярить Точка();
        гыы б = а;
        а.поле = 5;
        гыы рез = б.поле;
        "#,
    );
    assert_eq!(interp.get("рез"), Some(Value::Number(5.0)));
}

#[test]
fn eq_array_identity_true_structural_false() {
    let interp = run_code(
        r#"
        гыы а = [1];
        гыы сам = (а == а);
        гыы структ = ([1] == [1]);
        "#,
    );
    assert_eq!(interp.get("сам"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("структ"), Some(Value::Boolean(false)));
}

#[test]
fn eq_object_identity_true_structural_false() {
    let interp = run_code(
        r#"
        гыы о = { х: 1 };
        гыы сам = (о == о);
        гыы структ = ({} == {});
        "#,
    );
    assert_eq!(interp.get("сам"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("структ"), Some(Value::Boolean(false)));
}

#[test]
fn eq_switch_case_uses_reference_identity() {
    let interp = run_code(
        r#"
        гыы цель = { к: 1 };
        гыы другой = { к: 1 };
        гыы рез = "нет";
        базарпо (цель) {
            тема другой: { рез = "структ"; }
            тема цель: { рез = "идент"; }
        }
        "#,
    );
    assert_eq!(interp.get("рез"), Some(Value::String("идент".into())));
}

#[test]
fn svz_index_of_primitive_and_reference() {
    let interp = run_code(
        r#"
        гыы об = { м: 1 };
        гыы а = [1, об, 3];
        гыы прим = а.найтиИндекс(3);
        гыы реф = а.найтиИндекс(об);
        гыы структ = а.найтиИндекс({ м: 1 });
        "#,
    );
    assert_eq!(interp.get("прим"), Some(Value::Number(2.0)));
    assert_eq!(interp.get("реф"), Some(Value::Number(1.0)));
    assert_eq!(interp.get("структ"), Some(Value::Number(-1.0)));
}

#[test]
fn svz_includes_primitive_and_reference() {
    let interp = run_code(
        r#"
        гыы об = { м: 1 };
        гыы а = [1, об, 3];
        гыы прим = а.включает(3);
        гыы реф = а.включает(об);
        гыы структ = а.включает({ м: 1 });
        "#,
    );
    assert_eq!(interp.get("прим"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("реф"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("структ"), Some(Value::Boolean(false)));
}

#[test]
fn svz_set_dedup_primitives_keeps_structural_objects() {
    let interp = run_code(
        r#"
        гыы прим = захуярить Набор([1, 1, 2, 2, 3]);
        гыы рп = прим.размер;
        гыы о1 = { х: 1 };
        гыы о2 = { х: 1 };
        гыы реф = захуярить Набор([о1, о2, о1]);
        гыы рр = реф.размер;
        "#,
    );
    assert_eq!(interp.get("рп"), Some(Value::Number(3.0)));
    assert_eq!(interp.get("рр"), Some(Value::Number(2.0)));
}

#[test]
fn svz_map_key_reference_identity() {
    let interp = run_code(
        r#"
        гыы кл = { ид: 1 };
        гыы другой = { ид: 1 };
        гыы м = захуярить Карта();
        м.поставить(кл, "значение");
        гыы наш = м.взять(кл);
        гыы чужой = м.взять(другой);
        "#,
    );
    assert_eq!(interp.get("наш"), Some(Value::String("значение".into())));
    assert_eq!(interp.get("чужой"), Some(Value::Undefined));
}

#[test]
fn negative_cycle_display_object_no_panic() {
    let interp = run_code(
        r#"
        гыы о = { };
        о.сам = о;
        гыы текст = строка(о);
        "#,
    );
    let text = interp.get("текст").unwrap();
    let s = match text {
        Value::String(s) => s,
        other => panic!("ожидалась строка, получено {other:?}"),
    };
    assert!(s.contains("[Циклично]"), "ожидалось [Циклично] в выводе, получено: {s}");
}

#[test]
fn negative_cycle_display_array_no_panic() {
    let interp = run_code(
        r#"
        гыы а = [1];
        а.втолкнуть(а);
        гыы текст = строка(а);
        "#,
    );
    let text = interp.get("текст").unwrap();
    let s = match text {
        Value::String(s) => s,
        other => panic!("ожидалась строка, получено {other:?}"),
    };
    assert!(s.contains("[Циклично]"), "ожидалось [Циклично] в выводе, получено: {s}");
}

#[test]
fn negative_cycle_json_stringify_errors_not_panics() {
    let err = run_code_err(
        r#"
        гыы о = { };
        о.сам = о;
        Жсон.вСтроку(о);
        "#,
    );
    assert!(err.message.contains("Циклическая"), "ожидалась ошибка о цикле, получено: {}", err.message);
}

#[test]
fn snapshot_map_callback_mutating_receiver_uses_snapshot() {
    let interp = run_code(
        r#"
        гыы а = [1, 2, 3];
        гыы рез = а.преобразовать((х) => {
            а.втолкнуть(99);
            отвечаю х * 2;
        });
        гыы длр = длина(рез);
        гыы дла = длина(а);
        "#,
    );
    assert_struct_eq(interp.get("рез"), Value::array(vec![Value::Number(2.0), Value::Number(4.0), Value::Number(6.0)]));
    assert_eq!(interp.get("длр"), Some(Value::Number(3.0)));
    assert_eq!(interp.get("дла"), Some(Value::Number(6.0)));
}

#[test]
fn snapshot_sort_comparator_mutating_array_no_panic() {
    let interp = run_code(
        r#"
        гыы а = [3, 1, 2];
        а.сортировать((х, у) => {
            а.втолкнуть(0);
            отвечаю х - у;
        });
        гыы перв = а[0];
        "#,
    );
    assert_eq!(interp.get("перв"), Some(Value::Number(1.0)));
}

#[test]
fn shallow_spread_array_new_outer_shared_inner() {
    let interp = run_code(
        r#"
        гыы вн = { ц: 1 };
        гыы а = [вн];
        гыы б = [...а];
        гыы разные = (б == а);
        гыы тотЖеВнутр = (б[0] == а[0]);
        а.втолкнуть(2);
        гыы длб = длина(б);
        вн.ц = 5;
        гыы видно = б[0].ц;
        "#,
    );
    assert_eq!(interp.get("разные"), Some(Value::Boolean(false)));
    assert_eq!(interp.get("тотЖеВнутр"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("длб"), Some(Value::Number(1.0)));
    assert_eq!(interp.get("видно"), Some(Value::Number(5.0)));
}

#[test]
fn shallow_spread_object_new_outer_shared_inner() {
    let interp = run_code(
        r#"
        гыы вн = { ц: 1 };
        гыы о = { поле: вн };
        гыы коп = { ...о };
        гыы разные = (коп == о);
        гыы тотЖе = (коп.поле == о.поле);
        о.новое = 7;
        гыы естьНовое = ("новое" из коп);
        вн.ц = 9;
        гыы видно = коп.поле.ц;
        "#,
    );
    assert_eq!(interp.get("разные"), Some(Value::Boolean(false)));
    assert_eq!(interp.get("тотЖе"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("естьНовое"), Some(Value::Boolean(false)));
    assert_eq!(interp.get("видно"), Some(Value::Number(9.0)));
}

#[test]
fn closure_captures_container_by_reference() {
    let interp = run_code(
        r#"
        гыы а = [1];
        гыы читалка = () => длина(а);
        а.втолкнуть(2);
        гыы рез = читалка();
        "#,
    );
    assert_eq!(interp.get("рез"), Some(Value::Number(2.0)));
}

#[test]
fn set_at_path_missing_intermediate_errors() {
    let err = run_code_err(
        r#"
        гыы о = { };
        о.нет.глубже = 1;
        "#,
    );
    assert!(!err.message.is_empty());
}

#[test]
fn test_structural_eq_helper_handles_cycles() {
    let interp = run_code(
        r#"
        гыы а = { };
        а.сам = а;
        гыы б = { };
        б.сам = б;
        "#,
    );
    let a = interp.get("а").unwrap();
    let b = interp.get("б").unwrap();
    assert!(structural_eq(&a, &a));
    let _ = structural_eq(&a, &b);
}

#[test]
fn ref_semantics_map_alias_sees_mutation() {
    let interp = run_code(
        r#"
        гыы а = захуярить Карта();
        гыы б = а;
        б.set("к", 1);
        гыы виден = а.get("к");
        "#,
    );
    assert_eq!(interp.get("виден"), Some(Value::Number(1.0)));
}

#[test]
fn ref_semantics_set_alias_sees_mutation() {
    let interp = run_code(
        r#"
        гыы а = захуярить Набор();
        гыы б = а;
        б.add(7);
        гыы виден = а.has(7);
        "#,
    );
    assert_eq!(interp.get("виден"), Some(Value::Boolean(true)));
}

#[test]
fn ref_semantics_map_mutated_inside_function_visible_outside() {
    let interp = run_code(
        r#"
        йопта пополнить(м) {
            м.set("х", 42);
        }
        гыы а = захуярить Карта();
        пополнить(а);
        гыы виден = а.get("х");
        "#,
    );
    assert_eq!(interp.get("виден"), Some(Value::Number(42.0)));
}

#[test]
fn eq_map_identity_true_structural_false() {
    let interp = run_code(
        r#"
        гыы а = захуярить Карта();
        гыы б = а;
        гыы в = захуярить Карта();
        гыы ид = а == б;
        гыы структ = а == в;
        "#,
    );
    assert_eq!(interp.get("ид"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("структ"), Some(Value::Boolean(false)));
}

#[test]
fn eq_set_identity_true_structural_false() {
    let interp = run_code(
        r#"
        гыы а = захуярить Набор();
        гыы б = а;
        гыы в = захуярить Набор();
        гыы ид = а == б;
        гыы структ = а == в;
        "#,
    );
    assert_eq!(interp.get("ид"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("структ"), Some(Value::Boolean(false)));
}

#[test]
fn bound_array_method_extract_and_call() {
    let interp = run_code(
        r#"
        гыы м = [1, 2, 3].map;
        гыы рез = м((х) => х * 2);
        "#,
    );
    assert_struct_eq(interp.get("рез"), Value::array(vec![Value::Number(2.0), Value::Number(4.0), Value::Number(6.0)]));
}

#[test]
fn bound_array_method_extract_russian_alias() {
    let interp = run_code(
        r#"
        гыы м = [1, 2, 3].преобразовать;
        гыы рез = м((х) => х + 1);
        "#,
    );
    assert_struct_eq(interp.get("рез"), Value::array(vec![Value::Number(2.0), Value::Number(3.0), Value::Number(4.0)]));
}

#[test]
fn bound_array_unknown_property_is_undefined() {
    let interp = run_code(
        r#"
        гыы а = [1, 2];
        гыы м = а.несуществующийМетод;
        "#,
    );
    assert_eq!(interp.get("м"), Some(Value::Undefined));
}

#[test]
fn bound_array_length_property_unchanged() {
    let interp = run_code(
        r#"
        гыы а = [1, 2];
        гыы д = а.length;
        гыы д2 = а.длина;
        "#,
    );
    assert_eq!(interp.get("д"), Some(Value::Number(2.0)));
    assert_eq!(interp.get("д2"), Some(Value::Number(2.0)));
}

#[test]
fn bound_method_typeof_matches_function() {
    let interp = run_code(
        r#"
        гыы м = [1].map;
        гыы тм = тип(м);
        йопта ф() {}
        гыы тф = тип(ф);
        "#,
    );
    assert_eq!(interp.get("тм"), Some(Value::String("функция".into())));
    assert_eq!(interp.get("тф"), Some(Value::String("функция".into())));
}

#[test]
fn bound_array_push_mutates_original_via_shared_receiver() {
    let interp = run_code(
        r#"
        гыы а = [1, 2, 3];
        гыы п = а.push;
        п(4);
        гыы д = длина(а);
        гыы посл = а[3];
        "#,
    );
    assert_eq!(interp.get("д"), Some(Value::Number(4.0)));
    assert_eq!(interp.get("посл"), Some(Value::Number(4.0)));
}

#[test]
fn bound_array_index_access_with_string_key_errors() {
    let err = run_code_err(
        r#"
        гыы а = [1, 2];
        гыы м = а["map"];
        "#,
    );
    assert!(err.message.contains("индексировать"), "ожидалась ошибка индексации, получено: {}", err.message);
}
