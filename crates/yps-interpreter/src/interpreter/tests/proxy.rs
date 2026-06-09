use super::*;

#[test]
fn proxy_get_trap() {
    let i = run_code(
        r#"
        участковый п = захуярить Посредник({ а: 1 }, { получить: (ц, к, пр) => "G:" + к });
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
        участковый п = захуярить Посредник({ имя: "Зэ" }, {});
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
        участковый цель = {};
        участковый п = захуярить Посредник(цель, {
            установить: (ц, к, зн, пр) => { захвачено = к + "=" + зн; отвечаю правда; }
        });
        п.поле = 5;
        п["инд"] = 6;

        участковый цель2 = {};
        участковый п2 = захуярить Посредник(цель2, {});
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
        участковый п = захуярить Посредник({}, { есть: (ц, к) => к === "да" });
        гыы а = "да" чоунастут п;
        гыы б = "нет" чоунастут п;

        участковый пд = захуярить Посредник({ ключ: 1 }, {});
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
        участковый п = захуярить Посредник({}, { удалить: (ц, к) => { удалено = к; отвечаю правда; } });
        гыы рез = ёбнуть п.поле;

        участковый цель = { x: 1 };
        участковый пд = захуярить Посредник(цель, {});
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
        участковый сумма = (а, б) => а + б;
        участковый п = захуярить Посредник(сумма, { применить: (ц, этот, аргс) => аргс[0] * аргс[1] });
        гыы с_ловушкой = п(3, 4);
        участковый пд = захуярить Посредник(сумма, {});
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
        участковый п = захуярить Посредник(Зверь, { построить: (ц, аргс) => ({ вид: "особый" }) });
        участковый о = захуярить п();
        гыы с_ловушкой = о.вид;
        участковый пд = захуярить Посредник(Зверь, {});
        участковый о2 = захуярить пд();
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
        участковый по = захуярить Посредник({}, {});
        участковый пф = захуярить Посредник(() => 1, {});
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
        участковый п = захуярить Посредник(5, {});
        "#,
    );
    assert!(err.message.contains("Посредник"), "ожидалась ошибка о цели Посредника, получено: {}", err.message);
}

#[test]
fn proxy_rejects_non_object_handler() {
    let err = run_code_err(
        r#"
        участковый п = захуярить Посредник({}, 5);
        "#,
    );
    assert!(err.message.contains("обработчик"), "ожидалась ошибка об обработчике, получено: {}", err.message);
}

#[test]
fn proxy_collection_method_forwards() {
    let i = run_code(
        r#"
        участковый п = захуярить Посредник([1, 2, 3], {});
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
        участковый пм = захуярить Посредник([1, 2, 3], {});
        гыы масс = [...пм];
        гыы дл = длина(масс);

        участковый по = захуярить Посредник({ а: 1, б: 2 }, {});
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
        участковый ц = {};
        участковый о = {};
        участковый п1 = захуярить Посредник(ц, о);
        участковый п2 = захуярить Посредник(ц, о);
        гыы разные = п1 === п2;
        участковый п3 = п1;
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
        участковый п = захуярить Посредник({}, { установить: (ц, к, зн, пр) => лож });
        п.x = 1;
        "#,
    );
    assert!(err.message.contains("установить"), "ожидалась ошибка отвергнутой записи, получено: {}", err.message);
}

#[test]
fn proxy_array_default_set_in_bounds() {
    let i = run_code(
        r#"
        участковый ц = [1, 2, 3];
        участковый п = захуярить Посредник(ц, {});
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
        участковый ц = [1, 2];
        участковый п = захуярить Посредник(ц, {});
        п[5] = 99;
        "#,
    );
    assert!(err.message.contains("вне диапазона"), "ожидалась ошибка о границах массива, получено: {}", err.message);
}
