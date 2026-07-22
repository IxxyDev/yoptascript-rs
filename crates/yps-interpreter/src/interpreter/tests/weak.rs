use super::*;

#[test]
fn weak_map_set_get_has_delete() {
    let i = run_code(
        r#"
        гыы карта = захуярить СлабаяКарта();
        гыы ключ = { а: 1 };
        карта.поставить(ключ, 42);
        гыы значение = карта.взять(ключ);
        гыы есть = карта.имеет(ключ);
        гыы удалён = карта.удалить(ключ);
        гыы после = карта.имеет(ключ);
        гыы мимо = карта.взять(ключ);
        "#,
    );
    assert_eq!(i.get("значение"), Some(Value::Number(42.0)));
    assert_eq!(i.get("есть"), Some(Value::Boolean(true)));
    assert_eq!(i.get("удалён"), Some(Value::Boolean(true)));
    assert_eq!(i.get("после"), Some(Value::Boolean(false)));
    assert_eq!(i.get("мимо"), Some(Value::Undefined));
}

#[test]
fn weak_map_english_aliases_and_pairs_constructor() {
    let i = run_code(
        r#"
        гыы ключ = { б: 2 };
        гыы карта = захуярить СлабаяКарта([[ключ, "пара"]]);
        гыы значение = карта.get(ключ);
        карта.set(ключ, "новая");
        гыы есть = карта.has(ключ);
        гыы удалён = карта.delete(ключ);
        "#,
    );
    assert_eq!(i.get("значение"), Some(Value::String("пара".into())));
    assert_eq!(i.get("есть"), Some(Value::Boolean(true)));
    assert_eq!(i.get("удалён"), Some(Value::Boolean(true)));
}

#[test]
fn weak_map_rejects_primitive_key() {
    let err = run_code_err("гыы карта = захуярить СлабаяКарта(); карта.поставить(5, 1);");
    assert!(err.message.contains("СлабаяКарта"), "{}", err.message);
    assert!(err.message.contains("число"), "{}", err.message);
}

#[test]
fn weak_map_primitive_lookups_are_misses() {
    let i = run_code(
        r#"
        гыы карта = захуярить СлабаяКарта();
        гыы есть = карта.имеет(5);
        гыы значение = карта.взять("строка");
        гыы удалён = карта.удалить(правда);
        "#,
    );
    assert_eq!(i.get("есть"), Some(Value::Boolean(false)));
    assert_eq!(i.get("значение"), Some(Value::Undefined));
    assert_eq!(i.get("удалён"), Some(Value::Boolean(false)));
}

#[test]
fn weak_map_prunes_dead_keys() {
    let mut i = run_code(
        r#"
        гыы карта = захуярить СлабаяКарта();
        йопта временно() {
            гыы ключ = { а: 1 };
            карта.поставить(ключ, 42);
            отвечаю карта.взять(ключ);
        }
        гыы внутри = временно();
        "#,
    );
    assert_eq!(i.get("внутри"), Some(Value::Number(42.0)));
    let Some(Value::WeakMap(store)) = i.get("карта") else {
        panic!("ожидалась слабая карта");
    };
    assert_eq!(store.borrow().len(), 1);
    assert!(store.borrow().values().all(|(key, _)| !key.is_alive()));
    run_more(&mut i, "карта.имеет({});");
    assert_eq!(store.borrow().len(), 0);
}

#[test]
fn weak_set_add_has_delete() {
    let i = run_code(
        r#"
        гыы набор = захуярить СлабыйНабор();
        гыы об = { х: 1 };
        набор.добавить(об);
        гыы есть = набор.имеет(об);
        гыы удалён = набор.удалить(об);
        гыы после = набор.имеет(об);
        гыы мимо = набор.имеет(5);
        "#,
    );
    assert_eq!(i.get("есть"), Some(Value::Boolean(true)));
    assert_eq!(i.get("удалён"), Some(Value::Boolean(true)));
    assert_eq!(i.get("после"), Some(Value::Boolean(false)));
    assert_eq!(i.get("мимо"), Some(Value::Boolean(false)));
}

#[test]
fn weak_set_rejects_primitive_value() {
    let err = run_code_err("гыы набор = захуярить СлабыйНабор(); набор.добавить(\"ы\");");
    assert!(err.message.contains("СлабыйНабор"), "{}", err.message);
}

#[test]
fn weak_set_prunes_dead_entries() {
    let mut i = run_code(
        r#"
        гыы набор = захуярить СлабыйНабор();
        йопта временно() {
            гыы об = {};
            набор.добавить(об);
            отвечаю набор.имеет(об);
        }
        гыы внутри = временно();
        "#,
    );
    assert_eq!(i.get("внутри"), Some(Value::Boolean(true)));
    let Some(Value::WeakSet(store)) = i.get("набор") else {
        panic!("ожидался слабый набор");
    };
    assert!(store.borrow().values().all(|key| !key.is_alive()));
    run_more(&mut i, "набор.имеет({});");
    assert_eq!(store.borrow().len(), 0);
}

#[test]
fn weak_ref_deref_alive_and_dead() {
    let i = run_code(
        r#"
        гыы сс = ноль;
        йопта временно() {
            гыы объект = { а: 5 };
            сс = захуярить СлабаяСсылка(объект);
            отвечаю сс.разыменовать().а;
        }
        гыы пока_жив = временно();
        гыы после = сс.разыменовать();
        "#,
    );
    assert_eq!(i.get("пока_жив"), Some(Value::Number(5.0)));
    assert_eq!(i.get("после"), Some(Value::Undefined));
}

#[test]
fn weak_ref_keeps_alive_target_reachable() {
    let i = run_code(
        r#"
        гыы объект = { а: 9 };
        гыы сс = захуярить СлабаяСсылка(объект);
        гыы тот_же = сс.deref();
        гыы совпало = тот_же === объект;
        "#,
    );
    assert_eq!(i.get("совпало"), Some(Value::Boolean(true)));
}

#[test]
fn weak_ref_rejects_primitive_target() {
    let err = run_code_err("захуярить СлабаяСсылка(5);");
    assert!(err.message.contains("СлабаяСсылка"), "{}", err.message);
}

#[test]
fn weak_globals_typeof_is_object() {
    let i = run_code(
        r#"
        гыы карта = захуярить СлабаяКарта();
        гыы тк = чезажижан карта;
        гыы сс = захуярить СлабаяСсылка({});
        гыы тс = чезажижан сс;
        "#,
    );
    assert_eq!(i.get("тк"), Some(Value::String("объект".into())));
    assert_eq!(i.get("тс"), Some(Value::String("объект".into())));
}

#[test]
fn finalization_registry_fires_after_target_dies() {
    let i = run_code(
        r#"
        гыы лог = [];
        йопта запиши(что) { втолкнуть(лог, что); }
        гыы реестр = захуярить РеестрФинализации(запиши);
        йопта временно() {
            гыы объект = {};
            реестр.зарегистрировать(объект, "первый");
        }
        временно();
        "#,
    );
    let Some(Value::Array(log)) = i.get("лог") else {
        panic!("ожидался массив");
    };
    let log = log.borrow();
    assert_eq!(log.len(), 1);
    assert_eq!(log[0], Value::String("первый".into()));
}

#[test]
fn finalization_registry_does_not_fire_for_alive_target() {
    let i = run_code(
        r#"
        гыы лог = [];
        йопта запиши(что) { втолкнуть(лог, что); }
        гыы реестр = захуярить РеестрФинализации(запиши);
        гыы живой = {};
        реестр.зарегистрировать(живой, "жив");
        "#,
    );
    let Some(Value::Array(log)) = i.get("лог") else {
        panic!("ожидался массив");
    };
    assert_eq!(log.borrow().len(), 0);
}

#[test]
fn finalization_registry_unregister_cancels_callback() {
    let i = run_code(
        r#"
        гыы лог = [];
        йопта запиши(что) { втолкнуть(лог, что); }
        гыы реестр = захуярить РеестрФинализации(запиши);
        гыы токен = { т: 1 };
        йопта временно() {
            гыы объект = {};
            реестр.зарегистрировать(объект, "помеченный", токен);
        }
        временно();
        гыы снято = реестр.снять(токен);
        "#,
    );
    assert_eq!(i.get("снято"), Some(Value::Boolean(true)));
    let Some(Value::Array(log)) = i.get("лог") else {
        panic!("ожидался массив");
    };
    assert_eq!(log.borrow().len(), 0);
}

#[test]
fn finalization_registry_rejects_non_callable() {
    let err = run_code_err("захуярить РеестрФинализации(5);");
    assert!(err.message.contains("РеестрФинализации"), "{}", err.message);
}

#[test]
fn finalization_registry_register_rejects_primitive_target() {
    let err = run_code_err(
        r#"
        йопта запиши(что) {}
        гыы реестр = захуярить РеестрФинализации(запиши);
        реестр.зарегистрировать(5, "х");
        "#,
    );
    assert!(err.message.contains("зарегистрировать"), "{}", err.message);
}
