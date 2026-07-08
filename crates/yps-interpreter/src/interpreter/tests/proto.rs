use super::*;

#[test]
fn proto_create_and_lookup() {
    let interp = run_code(
        r#"
        гыы родитель = { привет: "ку" };
        гыы потомок = Кент.создать(родитель);
        гыы рез = потомок.привет;
        "#,
    );
    assert_eq!(interp.get("рез"), Some(Value::String("ку".to_string())));
}

#[test]
fn proto_own_shadows_parent() {
    let interp = run_code(
        r#"
        гыы родитель = { x: 1 };
        гыы потомок = Кент.создать(родитель);
        потомок.x = 2;
        гыы a = потомок.x;
        гыы b = родитель.x;
        "#,
    );
    assert_eq!(interp.get("a"), Some(Value::Number(2.0)));
    assert_eq!(interp.get("b"), Some(Value::Number(1.0)));
}

#[test]
fn proto_get_proto_returns_null_when_none() {
    let interp = run_code(
        r#"
        гыы o = { a: 1 };
        гыы p = Кент.прототип(o);
        "#,
    );
    assert_eq!(interp.get("p"), Some(Value::Null));
}

#[test]
fn proto_set_proto_changes_lookup() {
    let interp = run_code(
        r#"
        гыы a = { ключ: "ааа" };
        гыы b = { ключ: "ббб" };
        гыы o = Кент.создать(a);
        o = Кент.назначитьПрототип(o, b);
        гыы r = o.ключ;
        "#,
    );
    assert_eq!(interp.get("r"), Some(Value::String("ббб".to_string())));
}

#[test]
fn proto_chain_two_levels() {
    let interp = run_code(
        r#"
        гыы дед = { имя: "дед" };
        гыы отец = Кент.создать(дед);
        гыы сын = Кент.создать(отец);
        гыы рез = сын.имя;
        "#,
    );
    assert_eq!(interp.get("рез"), Some(Value::String("дед".to_string())));
}

#[test]
fn class_prototype_has_methods() {
    let interp = run_code(
        r#"
        клёво К {
            привет() { отвечаю "хай"; }
        }
        гыы proto = К.прототип;
        гыы m = proto.привет;
        гыы t = чезажижан m;
        "#,
    );
    assert_eq!(interp.get("t"), Some(Value::String("функция".to_string())));
}

#[test]
fn instance_constructor_returns_class() {
    let interp = run_code(
        r#"
        клёво К {}
        гыы x = захуярить К();
        гыы c = x.конструктор;
        гыы тот = c == К;
        "#,
    );
    assert_eq!(interp.get("тот"), Some(Value::Boolean(true)));
}

#[test]
fn proto_instance_carries_class_proto() {
    let interp = run_code(
        r#"
        клёво К {
            привет() { отвечаю "хай"; }
        }
        гыы x = захуярить К();
        гыы p = Кент.прототип(x);
        гыы t = чезажижан p.привет;
        "#,
    );
    assert_eq!(interp.get("t"), Some(Value::String("функция".to_string())));
}

#[test]
fn proto_constructor_accessor_on_instance() {
    let interp = run_code(
        r#"
        клёво К { }
        гыы x = захуярить К();
        гыы c = x.конструктор;
        гыы тот = c === К;
        "#,
    );
    assert_eq!(interp.get("тот"), Some(Value::Boolean(true)));
}

#[test]
fn proto_instanceof_via_object_create_class_proto() {
    let interp = run_code(
        r#"
        клёво К {
            метод() { отвечаю 1; }
        }
        гыы x = Кент.создать(К.прототип);
        гыы есть = x шкура К;
        "#,
    );
    assert_eq!(interp.get("есть"), Some(Value::Boolean(true)));
}

#[test]
fn proto_method_dispatch_works_after_class_rebinding() {
    let interp = run_code(
        r#"
        клёво К {
            f() { отвечаю 7; }
        }
        гыы x = захуярить К();
        гыы К = 0;
        гыы рез = x.f();
        "#,
    );
    assert_eq!(interp.get("рез"), Some(Value::Number(7.0)));
}

#[test]
fn proto_has_own_filters_internal_keys() {
    let interp = run_code(
        r#"
        клёво К {
            поле = 1;
        }
        гыы x = захуярить К();
        гыы a = Кент.имеетСвоё(x, "поле");
        гыы b = Кент.имеетСвоё(x, "__class__");
        гыы c = Кент.имеетСвоё(x, "__proto__");
        "#,
    );
    assert_eq!(interp.get("a"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("b"), Some(Value::Boolean(false)));
    assert_eq!(interp.get("c"), Some(Value::Boolean(false)));
}

#[test]
fn proto_instanceof_through_object_create_chain() {
    let interp = run_code(
        r#"
        клёво Животное { }
        клёво Собака батя Животное { }
        гыы o = захуярить Собака();
        гыы p = Кент.создать(o);
        гыы есть_собака = p шкура Собака;
        гыы есть_животное = p шкура Животное;
        "#,
    );
    assert_eq!(interp.get("есть_собака"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("есть_животное"), Some(Value::Boolean(true)));
}

#[test]
fn proto_set_proto_on_instance_changes_dispatch() {
    let interp = run_code(
        r#"
        клёво К {
            f() { отвечаю "из_К"; }
        }
        гыы x = захуярить К();
        x = Кент.назначитьПрототип(x, { f: () => "из_прото" });
        гыы рез = x.f();
        "#,
    );
    assert_eq!(interp.get("рез"), Some(Value::String("из_прото".to_string())));
}

#[test]
fn proto_set_proto_to_null_breaks_dispatch() {
    let err = run_code_err(
        r#"
        клёво К {
            f() { отвечаю 1; }
        }
        гыы x = захуярить К();
        x = Кент.назначитьПрототип(x, ноль);
        x.f();
        "#,
    );
    assert!(err.message.contains("функц") || err.message.contains("undefined") || err.message.contains("определ"));
}

#[test]
fn proto_keys_does_not_expose_internals() {
    let interp = run_code(
        r#"
        клёво К {
            поле = 5;
        }
        гыы x = захуярить К();
        гыы ks = Кент.ключи(x);
        гыы дл = ks.длина;
        "#,
    );
    assert_eq!(interp.get("дл"), Some(Value::Number(1.0)));
}

#[test]
fn define_property_data_form_readable() {
    let interp = run_code(
        r#"
        гыы o = {};
        Кент.определитьСвойство(o, "x", { значение: 42 });
        гыы v = o.x;
        "#,
    );
    assert_eq!(interp.get("v"), Some(Value::Number(42.0)));
}

#[test]
fn define_property_accessor_getter_invoked() {
    let interp = run_code(
        r#"
        гыы cnt = 0;
        гыы o = {};
        Кент.определитьСвойство(o, "y", {
            получить: () => { cnt = cnt + 1; }
        });
        гыы a = o.y;
        гыы b = o.y;
        "#,
    );
    assert_eq!(interp.get("cnt"), Some(Value::Number(2.0)));
}

#[test]
fn define_property_accessor_setter_invoked() {
    let interp = run_code(
        r#"
        гыы captured = 0;
        гыы o = {};
        Кент.определитьСвойство(o, "z", {
            установить: (v) => { captured = v; }
        });
        o.z = 99;
        "#,
    );
    assert_eq!(interp.get("captured"), Some(Value::Number(99.0)));
}

#[test]
fn define_property_accessor_replaces_data_property() {
    let interp = run_code(
        r#"
        гыы o = { p: 1 };
        Кент.определитьСвойство(o, "p", { получить: () => 2 });
        гыы v = o.p;
        гыы has_data = Кент.имеетСвоё(o, "p");
        "#,
    );
    assert_eq!(interp.get("v"), Some(Value::Number(2.0)));
    assert_eq!(interp.get("has_data"), Some(Value::Boolean(true)));
}

#[test]
fn has_own_true_for_getter_only_accessor() {
    let interp = run_code(
        r#"
        гыы o = {};
        Кент.определитьСвойство(o, "p", { получить: () => 2 });
        гыы has = Кент.имеетСвоё(o, "p");
        "#,
    );
    assert_eq!(interp.get("has"), Some(Value::Boolean(true)));
}

#[test]
fn has_own_true_for_setter_only_accessor() {
    let interp = run_code(
        r#"
        гыы o = {};
        Кент.определитьСвойство(o, "p", { установить: (v) => {} });
        гыы has = Кент.имеетСвоё(o, "p");
        "#,
    );
    assert_eq!(interp.get("has"), Some(Value::Boolean(true)));
}

#[test]
fn has_own_true_for_getter_and_setter_accessor() {
    let interp = run_code(
        r#"
        гыы o = {};
        Кент.определитьСвойство(o, "p", { получить: () => 2, установить: (v) => {} });
        гыы has = Кент.имеетСвоё(o, "p");
        "#,
    );
    assert_eq!(interp.get("has"), Some(Value::Boolean(true)));
}

#[test]
fn has_own_true_for_plain_property() {
    let interp = run_code(
        r#"
        гыы o = { p: 1 };
        гыы has = Кент.имеетСвоё(o, "p");
        "#,
    );
    assert_eq!(interp.get("has"), Some(Value::Boolean(true)));
}

#[test]
fn has_own_false_for_missing_property() {
    let interp = run_code(
        r#"
        гыы o = { p: 1 };
        гыы has = Кент.имеетСвоё(o, "отсутствует");
        "#,
    );
    assert_eq!(interp.get("has"), Some(Value::Boolean(false)));
}

#[test]
fn has_own_false_for_internal_getter_key_query() {
    let interp = run_code(
        r#"
        гыы o = {};
        Кент.определитьСвойство(o, "p", { получить: () => 2 });
        гыы has = Кент.имеетСвоё(o, "__get_p__");
        "#,
    );
    assert_eq!(interp.get("has"), Some(Value::Boolean(false)));
}

#[test]
fn keys_still_omit_accessor_only_properties() {
    let interp = run_code(
        r#"
        гыы o = { a: 1 };
        Кент.определитьСвойство(o, "p", { получить: () => 2 });
        гыы ks = Кент.ключи(o);
        гыы n = длина(ks);
        "#,
    );
    assert_eq!(interp.get("n"), Some(Value::Number(1.0)));
}

#[test]
fn define_property_data_replaces_accessor_property() {
    let interp = run_code(
        r#"
        гыы o = {};
        Кент.определитьСвойство(o, "p", { получить: () => 2 });
        Кент.определитьСвойство(o, "p", { значение: 7 });
        гыы v = o.p;
        "#,
    );
    assert_eq!(interp.get("v"), Some(Value::Number(7.0)));
}

#[test]
fn define_property_value_and_accessor_conflict_errors() {
    let err = run_code_err(
        r#"
        гыы o = {};
        Кент.определитьСвойство(o, "p", { значение: 1, получить: () => 2 });
        "#,
    );
    assert!(err.message.contains("значение") && err.message.contains("получить"));
}

#[test]
fn define_property_no_descriptor_fields_is_undefined() {
    let interp = run_code(
        r#"
        гыы o = {};
        Кент.определитьСвойство(o, "p", {});
        гыы v = o.p;
        "#,
    );
    assert_eq!(interp.get("v"), Some(Value::Undefined));
}

#[test]
fn get_own_property_descriptor_data_form() {
    let interp = run_code(
        r#"
        гыы o = { p: 5 };
        гыы d = Кент.описатьСвойство(o, "p");
        гыы v = d.значение;
        "#,
    );
    assert_eq!(interp.get("v"), Some(Value::Number(5.0)));
}

#[test]
fn get_own_property_descriptor_accessor_form() {
    let interp = run_code(
        r#"
        гыы o = {};
        Кент.определитьСвойство(o, "p", { получить: () => 1, установить: (v) => {} });
        гыы d = Кент.описатьСвойство(o, "p");
        гыы g = d.получить;
        гыы s = d.установить;
        "#,
    );
    let g = interp.get("g").unwrap();
    let s = interp.get("s").unwrap();
    assert!(g.is_callable());
    assert!(s.is_callable());
}

#[test]
fn get_own_property_descriptor_missing_is_undefined() {
    let interp = run_code(
        r#"
        гыы o = {};
        гыы d = Кент.описатьСвойство(o, "нет");
        "#,
    );
    assert_eq!(interp.get("d"), Some(Value::Undefined));
}

#[test]
fn get_own_property_descriptor_hides_internal_keys() {
    let interp = run_code(
        r#"
        клёво К {
            поле = 5;
        }
        гыы x = захуярить К();
        гыы d = Кент.описатьСвойство(x, "__class__");
        "#,
    );
    assert_eq!(interp.get("d"), Some(Value::Undefined));
}

#[test]
fn define_property_on_frozen_object_is_noop() {
    let interp = run_code(
        r#"
        гыы o = { p: 1 };
        Кент.заморозить(o);
        Кент.определитьСвойство(o, "p", { значение: 2 });
        гыы v = o.p;
        "#,
    );
    assert_eq!(interp.get("v"), Some(Value::Number(1.0)));
}
