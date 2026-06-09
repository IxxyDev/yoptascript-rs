use super::*;

#[test]
fn alias_true_trulio() {
    let interp = run_code("гыы р = трулио;");
    assert_eq!(interp.get("р"), Some(Value::Boolean(true)));
}

#[test]
fn alias_true_chotko() {
    let interp = run_code("гыы р = чотко;");
    assert_eq!(interp.get("р"), Some(Value::Boolean(true)));
}

#[test]
fn alias_false_netrulio() {
    let interp = run_code("гыы р = нетрулио;");
    assert_eq!(interp.get("р"), Some(Value::Boolean(false)));
}

#[test]
fn alias_false_pizdish() {
    let interp = run_code("гыы р = пиздишь;");
    assert_eq!(interp.get("р"), Some(Value::Boolean(false)));
}

#[test]
fn alias_null_nullio() {
    let interp = run_code("гыы р = нуллио;");
    assert_eq!(interp.get("р"), Some(Value::Null));
}

#[test]
fn alias_null_porozhnyak() {
    let interp = run_code("гыы р = порожняк;");
    assert_eq!(interp.get("р"), Some(Value::Null));
}

#[test]
fn alias_undefined_neibu() {
    let interp = run_code("гыы р = неибу;");
    assert_eq!(interp.get("р"), Some(Value::Undefined));
}

#[test]
fn alias_throw_pnh() {
    let interp = run_code(
        r#"
        гыы р = 0;
        хапнуть {
            пнх 42;
        } гоп (е) {
            р = е;
        }
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::Number(42.0)));
}

#[test]
fn alias_switch_estcho() {
    let interp = run_code(
        r#"
        гыы р = 0;
        естьчо (1) {
            лещ 1: {
                р = 10;
            }
            аеслинайду 2: {
                р = 20;
            }
            пахану {
                р = 99;
            }
        }
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::Number(10.0)));
}

#[test]
fn alias_do_while_krch() {
    let interp = run_code(
        r#"
        гыы р = 0;
        крч {
            р = р + 1;
        } потрещим (р < 3);
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::Number(3.0)));
}

#[test]
#[allow(clippy::approx_constant)]
fn alias_const_yasen_huy_capital() {
    let interp = run_code(
        r#"
        ЯсенХуй ПИ = 3.14;
        гыы р = ПИ;
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::Number(3.14)));
}

#[test]
fn dict_alias_chounastoot_for_in() {
    let interp = run_code(
        r#"
        гыы об = { а: 1, б: 2 };
        гыы ключи = [];
        го (гыы к чоунастут об) {
            ключи.push(к);
        }
        гыы длина = ключи.length;
        "#,
    );
    assert_eq!(interp.get("длина"), Some(Value::Number(2.0)));
}

#[test]
fn dict_alias_nan_global() {
    let interp = run_code(
        r#"
        гыы н = нихуя;
        гыы это_нан = Хуйня.нихуя(н);
        "#,
    );
    assert_eq!(interp.get("это_нан"), Some(Value::Boolean(true)));
}

#[test]
fn dict_alias_nan_is_const() {
    let err = run_code_err(
        r#"
        нихуя = 1;
        "#,
    );
    assert!(err.message.contains("константу") || err.message.contains("const"));
}

#[test]
fn dict_modifier_private_field_blocks_outside_access() {
    let err = run_code_err(
        r#"
        клёво К {
            Кошелёк() {}
            мой бабки = 500;
        }
        гыы к = захуярить К();
        гыы х = к.#бабки;
        "#,
    );
    assert!(err.message.contains("приватному полю"));
}

#[test]
fn dict_modifier_private_field_accessible_inside_method() {
    let i = run_code(
        r#"
        клёво К {
            мой значение = 42;
            читать() { отвечаю тырыпыры.#значение; }
        }
        гыы о = захуярить К();
        гыы р = о.читать();
        "#,
    );
    assert_eq!(i.get("р"), Some(Value::Number(42.0)));
}

#[test]
fn dict_modifier_public_protected_parse_as_public() {
    let i = run_code(
        r#"
        клёво К {
            ебанное публ = 1;
            подкрыша прот = 2;
            ебанное взять() { отвечаю тырыпыры.публ + тырыпыры.прот; }
        }
        гыы о = захуярить К();
        гыы а = о.публ;
        гыы б = о.прот;
        гыы в = о.взять();
        "#,
    );
    assert_eq!(i.get("а"), Some(Value::Number(1.0)));
    assert_eq!(i.get("б"), Some(Value::Number(2.0)));
    assert_eq!(i.get("в"), Some(Value::Number(3.0)));
}

#[test]
fn dict_debugger_is_noop() {
    let interp = run_code(
        r#"
        гыы а = 1;
        логопед;
        гыы б = 2;
        "#,
    );
    assert_eq!(interp.get("а"), Some(Value::Number(1.0)));
    assert_eq!(interp.get("б"), Some(Value::Number(2.0)));
}
