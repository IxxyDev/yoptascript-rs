use super::*;

#[test]
fn proxy_get_trap() {
    let i = run_code(
        r#"
        ясенХуй п = захуярить Посредник({ а: 1 }, { получить: (ц, к, пр) => "G:" + к });
        гыы поле = п.x;
        гыы инд = п["y"];
        "#,
    );
    assert_eq!(i.get("поле"), Some(Value::String("G:x".to_string())));
    assert_eq!(i.get("инд"), Some(Value::String("G:y".to_string())));
}

#[test]
fn proxy_get_default_forwards() {
    let i = run_code(
        r#"
        ясенХуй п = захуярить Посредник({ имя: "Зэ" }, {});
        гыы р = п.имя;
        "#,
    );
    assert_eq!(i.get("р"), Some(Value::String("Зэ".to_string())));
}

#[test]
fn proxy_set_trap_and_default() {
    let i = run_code(
        r#"
        гыы захвачено = "";
        ясенХуй цель = {};
        ясенХуй п = захуярить Посредник(цель, {
            установить: (ц, к, зн, пр) => { захвачено = к + "=" + зн; отвечаю правда; }
        });
        п.поле = 5;
        п["инд"] = 6;

        ясенХуй цель2 = {};
        ясенХуй п2 = захуярить Посредник(цель2, {});
        п2.k = 9;
        гыы из_цели = цель2.k;
        "#,
    );
    assert_eq!(i.get("захвачено"), Some(Value::String("инд=6".to_string())));
    assert_eq!(i.get("из_цели"), Some(Value::Number(9.0)));
}

#[test]
fn proxy_has_trap_and_default() {
    let i = run_code(
        r#"
        ясенХуй п = захуярить Посредник({}, { есть: (ц, к) => к === "да" });
        гыы а = "да" чоунастут п;
        гыы б = "нет" чоунастут п;

        ясенХуй пд = захуярить Посредник({ ключ: 1 }, {});
        гыы в = "ключ" чоунастут пд;
        гыы г = "нету" чоунастут пд;
        "#,
    );
    assert_eq!(i.get("а"), Some(Value::Boolean(true)));
    assert_eq!(i.get("б"), Some(Value::Boolean(false)));
    assert_eq!(i.get("в"), Some(Value::Boolean(true)));
    assert_eq!(i.get("г"), Some(Value::Boolean(false)));
}

#[test]
fn proxy_delete_trap_and_default() {
    let i = run_code(
        r#"
        гыы удалено = "";
        ясенХуй п = захуярить Посредник({}, { удалить: (ц, к) => { удалено = к; отвечаю правда; } });
        гыы рез = ёбнуть п.поле;

        ясенХуй цель = { x: 1 };
        ясенХуй пд = захуярить Посредник(цель, {});
        ёбнуть пд.x;
        гыы тип_после = тип(цель.x);
        "#,
    );
    assert_eq!(i.get("удалено"), Some(Value::String("поле".to_string())));
    assert_eq!(i.get("рез"), Some(Value::Boolean(true)));
    assert_eq!(i.get("тип_после"), Some(Value::String("неопределено".to_string())));
}

#[test]
fn proxy_apply_trap_and_default() {
    let i = run_code(
        r#"
        ясенХуй сумма = (а, б) => а + б;
        ясенХуй п = захуярить Посредник(сумма, { применить: (ц, этот, аргс) => аргс[0] * аргс[1] });
        гыы с_ловушкой = п(3, 4);
        ясенХуй пд = захуярить Посредник(сумма, {});
        гыы без_ловушки = пд(3, 4);
        "#,
    );
    assert_eq!(i.get("с_ловушкой"), Some(Value::Number(12.0)));
    assert_eq!(i.get("без_ловушки"), Some(Value::Number(7.0)));
}

#[test]
fn proxy_construct_trap_and_default() {
    let i = run_code(
        r#"
        клёво Зверь { Зверь() { тырыпыры.вид = "обычный"; } }
        ясенХуй п = захуярить Посредник(Зверь, { построить: (ц, аргс) => ({ вид: "особый" }) });
        ясенХуй о = захуярить п();
        гыы с_ловушкой = о.вид;
        ясенХуй пд = захуярить Посредник(Зверь, {});
        ясенХуй о2 = захуярить пд();
        гыы без_ловушки = о2.вид;
        "#,
    );
    assert_eq!(i.get("с_ловушкой"), Some(Value::String("особый".to_string())));
    assert_eq!(i.get("без_ловушки"), Some(Value::String("обычный".to_string())));
}

#[test]
fn proxy_typeof_transparent() {
    let i = run_code(
        r#"
        ясенХуй по = захуярить Посредник({}, {});
        ясенХуй пф = захуярить Посредник(() => 1, {});
        гыы то = тип(по);
        гыы тф = тип(пф);
        "#,
    );
    assert_eq!(i.get("то"), Some(Value::String("объект".to_string())));
    assert_eq!(i.get("тф"), Some(Value::String("функция".to_string())));
}

#[test]
fn proxy_rejects_non_object_target() {
    let err = run_code_err(
        r#"
        ясенХуй п = захуярить Посредник(5, {});
        "#,
    );
    assert!(err.message.contains("Посредник"), "ожидалась ошибка о цели Посредника, получено: {}", err.message);
}

#[test]
fn proxy_rejects_non_object_handler() {
    let err = run_code_err(
        r#"
        ясенХуй п = захуярить Посредник({}, 5);
        "#,
    );
    assert!(err.message.contains("обработчик"), "ожидалась ошибка об обработчике, получено: {}", err.message);
}

#[test]
fn proxy_collection_method_forwards() {
    let i = run_code(
        r#"
        ясенХуй п = захуярить Посредник([1, 2, 3], {});
        п.добавить(4);
        гыы дл = п.длина;
        гыы перв = п[0];
        "#,
    );
    assert_eq!(i.get("дл"), Some(Value::Number(4.0)));
    assert_eq!(i.get("перв"), Some(Value::Number(1.0)));
}

#[test]
fn proxy_spread_forwards_to_target() {
    let i = run_code(
        r#"
        ясенХуй пм = захуярить Посредник([1, 2, 3], {});
        гыы масс = [...пм];
        гыы дл = длина(масс);

        ясенХуй по = захуярить Посредник({ а: 1, б: 2 }, {});
        гыы об = { ...по, в: 3 };
        гыы ва = об.а;
        гыы вв = об.в;
        "#,
    );
    assert_eq!(i.get("дл"), Some(Value::Number(3.0)));
    assert_eq!(i.get("ва"), Some(Value::Number(1.0)));
    assert_eq!(i.get("вв"), Some(Value::Number(3.0)));
}

#[test]
fn proxy_identity_is_distinct() {
    let i = run_code(
        r#"
        ясенХуй ц = {};
        ясенХуй о = {};
        ясенХуй п1 = захуярить Посредник(ц, о);
        ясенХуй п2 = захуярить Посредник(ц, о);
        гыы разные = п1 === п2;
        ясенХуй п3 = п1;
        гыы одинаковые = п1 === п3;
        "#,
    );
    assert_eq!(i.get("разные"), Some(Value::Boolean(false)));
    assert_eq!(i.get("одинаковые"), Some(Value::Boolean(true)));
}

#[test]
fn proxy_set_trap_rejection_throws() {
    let err = run_code_err(
        r#"
        ясенХуй п = захуярить Посредник({}, { установить: (ц, к, зн, пр) => лож });
        п.x = 1;
        "#,
    );
    assert!(err.message.contains("установить"), "ожидалась ошибка отвергнутой записи, получено: {}", err.message);
}

#[test]
fn proxy_array_default_set_in_bounds() {
    let i = run_code(
        r#"
        ясенХуй ц = [1, 2, 3];
        ясенХуй п = захуярить Посредник(ц, {});
        п[1] = 99;
        гыы знач = ц[1];
        "#,
    );
    assert_eq!(i.get("знач"), Some(Value::Number(99.0)));
}

#[test]
fn proxy_array_default_set_out_of_bounds_errors() {
    let err = run_code_err(
        r#"
        ясенХуй ц = [1, 2];
        ясенХуй п = захуярить Посредник(ц, {});
        п[5] = 99;
        "#,
    );
    assert!(err.message.contains("вне диапазона"), "ожидалась ошибка о границах массива, получено: {}", err.message);
}

#[test]
fn proxy_own_keys_trap_drives_object_keys() {
    let i = run_code(
        r#"
        гыы вызвано = 0;
        ясенХуй п = захуярить Посредник({ а: 1, б: 2 }, {
            собственныеКлючи: (ц) => { вызвано = вызвано + 1; отвечаю ["x", "y", "z"]; }
        });
        гыы ключи = Кент.ключи(п).склеить(",");
        "#,
    );
    assert_eq!(i.get("ключи"), Some(Value::String("x,y,z".to_string())));
    assert_eq!(i.get("вызвано"), Some(Value::Number(1.0)));
}

#[test]
fn proxy_own_keys_absent_falls_back_to_target() {
    let i = run_code(
        r#"
        ясенХуй п = захуярить Посредник({ а: 1, б: 2 }, {});
        гыы ключи = Кент.ключи(п).склеить(",");
        "#,
    );
    assert_eq!(i.get("ключи"), Some(Value::String("а,б".to_string())));
}

#[test]
fn proxy_own_keys_drives_for_in_and_spread() {
    let i = run_code(
        r#"
        ясенХуй п = захуярить Посредник({ а: 1 }, {
            собственныеКлючи: (ц) => ["k1", "k2"],
            получить: (ц, к) => "V:" + к
        });
        гыы собрано = "";
        го (ясенХуй к из п) { собрано = собрано + к; }
        ясенХуй об = { ...п };
        гыы значK1 = об.k1;
        "#,
    );
    assert_eq!(i.get("собрано"), Some(Value::String("k1k2".to_string())));
    assert_eq!(i.get("значK1"), Some(Value::String("V:k1".to_string())));
}

#[test]
fn proxy_get_prototype_of_trap_drives_object_prototype() {
    let i = run_code(
        r#"
        ясенХуй прото = { помечен: правда };
        ясенХуй п = захуярить Посредник({}, { прототипОт: (ц) => прото });
        гыы результат = Кент.прототип(п).помечен;
        "#,
    );
    assert_eq!(i.get("результат"), Some(Value::Boolean(true)));
}

#[test]
fn proxy_get_prototype_of_drives_instanceof() {
    let i = run_code(
        r#"
        клёво Зверь {}
        ясенХуй экз = захуярить Зверь();
        ясенХуй п = захуярить Посредник({}, { прототипОт: (ц) => экз });
        гыы с_ловушкой = п шкура Зверь;
        ясенХуй пд = захуярить Посредник(экз, {});
        гыы без_ловушки = пд шкура Зверь;
        "#,
    );
    assert_eq!(i.get("с_ловушкой"), Some(Value::Boolean(true)));
    assert_eq!(i.get("без_ловушки"), Some(Value::Boolean(true)));
}

#[test]
fn proxy_set_prototype_of_trap_fires() {
    let i = run_code(
        r#"
        гыы захвачено = ноль;
        ясенХуй прото = { тег: "новый" };
        ясенХуй п = захуярить Посредник({}, {
            назначитьПрототип: (ц, прт) => { захвачено = прт.тег; отвечаю правда; }
        });
        Кент.назначитьПрототип(п, прото);
        "#,
    );
    assert_eq!(i.get("захвачено"), Some(Value::String("новый".to_string())));
}

#[test]
fn proxy_set_prototype_of_absent_falls_back_to_target() {
    let i = run_code(
        r#"
        ясенХуй цель = {};
        ясенХуй прото = { тег: "цельный" };
        ясенХуй п = захуярить Посредник(цель, {});
        Кент.назначитьПрототип(п, прото);
        гыы результат = Кент.прототип(цель).тег;
        "#,
    );
    assert_eq!(i.get("результат"), Some(Value::String("цельный".to_string())));
}

#[test]
fn proxy_define_property_trap_and_descriptor_trap() {
    let i = run_code(
        r#"
        гыы определено = "";
        ясенХуй п = захуярить Посредник({}, {
            определитьСвойство: (ц, к, деск) => { определено = к; отвечаю правда; },
            описатьСвойство: (ц, к) => ({ значение: "из-ловушки" })
        });
        Кент.определитьСвойство(п, "поле", { значение: 1 });
        гыы деск = Кент.описатьСвойство(п, "поле").значение;
        "#,
    );
    assert_eq!(i.get("определено"), Some(Value::String("поле".to_string())));
    assert_eq!(i.get("деск"), Some(Value::String("из-ловушки".to_string())));
}

#[test]
fn proxy_define_property_absent_falls_back_to_target() {
    let i = run_code(
        r#"
        ясенХуй цель = {};
        ясенХуй п = захуярить Посредник(цель, {});
        Кент.определитьСвойство(п, "поле", { значение: 7 });
        гыы прямо = цель.поле;
        гыы деск = Кент.описатьСвойство(п, "поле").значение;
        "#,
    );
    assert_eq!(i.get("прямо"), Some(Value::Number(7.0)));
    assert_eq!(i.get("деск"), Some(Value::Number(7.0)));
}

#[test]
fn proxy_is_extensible_and_prevent_extensions_traps() {
    let i = run_code(
        r#"
        гыы запрещено = 0;
        ясенХуй п = захуярить Посредник({}, {
            расширяем: (ц) => лож,
            запретитьРасширение: (ц) => { запрещено = запрещено + 1; отвечаю правда; }
        });
        гыы расш = Кент.расширяем(п);
        Кент.запретитьРасширение(п);
        "#,
    );
    assert_eq!(i.get("расш"), Some(Value::Boolean(false)));
    assert_eq!(i.get("запрещено"), Some(Value::Number(1.0)));
}

#[test]
fn proxy_extensibility_absent_falls_back_to_target() {
    let i = run_code(
        r#"
        ясенХуй цель = {};
        ясенХуй п = захуярить Посредник(цель, {});
        гыы до = Кент.расширяем(п);
        Кент.запретитьРасширение(п);
        гыы после = Кент.расширяем(цель);
        "#,
    );
    assert_eq!(i.get("до"), Some(Value::Boolean(true)));
    assert_eq!(i.get("после"), Some(Value::Boolean(false)));
}
