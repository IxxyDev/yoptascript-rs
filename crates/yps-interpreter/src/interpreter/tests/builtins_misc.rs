use super::*;

#[test]
fn bigint_literal() {
    let interp = run_code(
        r#"
        гыы a = 123n;
        "#,
    );
    assert_eq!(interp.get("a"), Some(Value::BigInt(123)));
}

#[test]
fn bigint_arithmetic() {
    let interp = run_code(
        r#"
        гыы a = 1000000000000n;
        гыы b = 2n;
        гыы s = a + b;
        гыы d = a - b;
        гыы m = a * b;
        гыы q = a / b;
        гыы r = a % b;
        гыы p = b ** 10n;
        "#,
    );
    assert_eq!(interp.get("s"), Some(Value::BigInt(1_000_000_000_002)));
    assert_eq!(interp.get("d"), Some(Value::BigInt(999_999_999_998)));
    assert_eq!(interp.get("m"), Some(Value::BigInt(2_000_000_000_000)));
    assert_eq!(interp.get("q"), Some(Value::BigInt(500_000_000_000)));
    assert_eq!(interp.get("r"), Some(Value::BigInt(0)));
    assert_eq!(interp.get("p"), Some(Value::BigInt(1024)));
}

#[test]
fn bigint_unary_minus() {
    let interp = run_code(
        r#"
        гыы a = -7n;
        "#,
    );
    assert_eq!(interp.get("a"), Some(Value::BigInt(-7)));
}

#[test]
fn bigint_compare() {
    let interp = run_code(
        r#"
        гыы a = 5n < 7n;
        гыы b = 5n == 5n;
        гыы c = 5n > 7n;
        "#,
    );
    assert_eq!(interp.get("a"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("b"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("c"), Some(Value::Boolean(false)));
}

#[test]
fn bigint_typeof() {
    let interp = run_code(
        r#"
        гыы t = чезажижан 9n;
        "#,
    );
    assert_eq!(interp.get("t"), Some(Value::String("бигцелое".to_string())));
}

#[test]
fn bigint_mixed_with_number_errors() {
    let err = run_code_err("гыы x = 1n + 2;");
    assert!(err.message.contains("Нельзя смешивать"));
}

#[test]
fn bigint_div_by_zero_errors() {
    let err = run_code_err("гыы x = 5n / 0n;");
    assert!(err.message.contains("ноль"));
}

#[test]
fn bigint_constructor_from_string() {
    let interp = run_code(
        r#"
        гыы a = БигЦелое("999");
        "#,
    );
    assert_eq!(interp.get("a"), Some(Value::BigInt(999)));
}

#[test]
fn bigint_constructor_from_number() {
    let interp = run_code(
        r#"
        гыы a = БигЦелое(42);
        "#,
    );
    assert_eq!(interp.get("a"), Some(Value::BigInt(42)));
}

#[test]
fn bigint_constructor_rejects_fractional() {
    let err = run_code_err(r#"гыы x = БигЦелое(1.5);"#);
    assert!(err.message.contains("целое"));
}

#[test]
fn date_construct_and_instance_method_gate() {
    let interp = run_code(
        r#"
        гыы д = захуярить Дата(0);
        гыы год = д.год();
        гыы исо = д.вИСО();
        "#,
    );
    assert_eq!(interp.get("год"), Some(Value::Number(1970.0)));
    assert_eq!(interp.get("исо"), Some(Value::String("1970-01-01T00:00:00.000Z".to_string())));
}

#[test]
fn date_getters_utc() {
    let interp = run_code(
        r#"
        гыы д = захуярить Дата("2000-02-29T12:30:45.500Z");
        гыы год = д.год();
        гыы месяц = д.месяц();
        гыы день = д.день();
        гыы часы = д.часы();
        гыы минуты = д.минуты();
        гыы секунды = д.секунды();
        гыы мс = д.миллисекунды();
        "#,
    );
    assert_eq!(interp.get("год"), Some(Value::Number(2000.0)));
    assert_eq!(interp.get("месяц"), Some(Value::Number(1.0)));
    assert_eq!(interp.get("день"), Some(Value::Number(29.0)));
    assert_eq!(interp.get("часы"), Some(Value::Number(12.0)));
    assert_eq!(interp.get("минуты"), Some(Value::Number(30.0)));
    assert_eq!(interp.get("секунды"), Some(Value::Number(45.0)));
    assert_eq!(interp.get("мс"), Some(Value::Number(500.0)));
}

#[test]
fn date_weekday_epoch() {
    let interp = run_code(
        r#"
        гыы д = захуярить Дата(0);
        гыы дн = д.деньНедели();
        "#,
    );
    assert_eq!(interp.get("дн"), Some(Value::Number(4.0)));
}

#[test]
fn date_before_1970() {
    let interp = run_code(
        r#"
        гыы д = захуярить Дата(-86400000);
        гыы исо = д.вИСО();
        "#,
    );
    assert_eq!(interp.get("исо"), Some(Value::String("1969-12-31T00:00:00.000Z".to_string())));
}

#[test]
fn date_invalid_returns_nan_and_invalid_string() {
    let interp = run_code(
        r#"
        гыы д = захуярить Дата("не дата");
        гыы исо = д.вИСО();
        гыы год = д.год();
        гыы плохо = год !== год;
        "#,
    );
    assert_eq!(interp.get("исо"), Some(Value::String("Invalid Date".to_string())));
    assert_eq!(interp.get("плохо"), Some(Value::Boolean(true)));
}

#[test]
fn date_identity_equality() {
    let interp = run_code(
        r#"
        гыы а = захуярить Дата(5);
        гыы б = захуярить Дата(5);
        гыы разные = а !== б;
        гыы сам = а === а;
        "#,
    );
    assert_eq!(interp.get("разные"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("сам"), Some(Value::Boolean(true)));
}

#[test]
fn date_map_keys_do_not_collapse() {
    let interp = run_code(
        r#"
        гыы м = захуярить Карта();
        м.поставить(захуярить Дата(5), "а");
        м.поставить(захуярить Дата(5), "б");
        гыы размер = м.размер;
        "#,
    );
    assert_eq!(interp.get("размер"), Some(Value::Number(2.0)));
}

#[test]
fn date_typeof_is_object() {
    let interp = run_code(
        r#"
        гыы д = захуярить Дата(0);
        гыы оп = чезажижан д;
        гыы имя = тип(д);
        "#,
    );
    assert_eq!(interp.get("оп"), Some(Value::String("объект".to_string())));
    assert_eq!(interp.get("имя"), Some(Value::String("дата".to_string())));
}

#[test]
fn date_now_is_number() {
    let interp = run_code(
        r#"
        гыы т = тип(Дата.сейчас());
        "#,
    );
    assert_eq!(interp.get("т"), Some(Value::String("число".to_string())));
}

#[test]
fn date_json_serializes_to_iso() {
    let interp = run_code(
        r#"
        гыы д = захуярить Дата(0);
        гыы с = Жсон.вСтроку(д);
        "#,
    );
    assert_eq!(interp.get("с"), Some(Value::String("\"1970-01-01T00:00:00.000Z\"".to_string())));
}

#[test]
fn date_json_invalid_to_null() {
    let interp = run_code(
        r#"
        гыы д = захуярить Дата("не дата");
        гыы с = Жсон.вСтроку(д);
        "#,
    );
    assert_eq!(interp.get("с"), Some(Value::String("null".to_string())));
}

#[test]
fn date_setters_mutate_in_place_and_return_new_time() {
    let interp = run_code(
        r#"
        гыы д = захуярить Дата(0);
        гыы врем = д.поставитьГод(2020, 5, 15);
        гыы совпадает = врем === д.времяМс();
        гыы исо = д.вИСО();
        "#,
    );
    assert_eq!(interp.get("совпадает"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("исо"), Some(Value::String("2020-06-15T00:00:00.000Z".to_string())));
}

#[test]
fn date_set_month_rollover() {
    let interp = run_code(
        r#"
        гыы д = захуярить Дата(Дата.разобрать("2020-01-01T00:00:00Z"));
        д.поставитьМесяц(13);
        гыы исо = д.вИСО();
        "#,
    );
    assert_eq!(interp.get("исо"), Some(Value::String("2021-02-01T00:00:00.000Z".to_string())));
}

#[test]
fn date_set_hours_and_minutes_rollover() {
    let interp = run_code(
        r#"
        гыы д = захуярить Дата(Дата.разобрать("2020-01-01T10:30:15.500Z"));
        д.поставитьЧасы(25);
        гыы исо = д.вИСО();
        "#,
    );
    assert_eq!(interp.get("исо"), Some(Value::String("2020-01-02T01:30:15.500Z".to_string())));
}

#[test]
fn date_utc_getters_and_setters_match_local() {
    let interp = run_code(
        r#"
        гыы д = захуярить Дата(Дата.разобрать("2020-06-15T10:20:30.400Z"));
        гыы годРавны = д.год() === д.годUTC();
        гыы смещение = д.смещениеЧасовогоПояса();
        д.поставитьМесяцUTC(0);
        гыы исо = д.вИСО();
        "#,
    );
    assert_eq!(interp.get("годРавны"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("смещение"), Some(Value::Number(0.0)));
    assert_eq!(interp.get("исо"), Some(Value::String("2020-01-15T10:20:30.400Z".to_string())));
}

#[test]
fn date_set_time_and_get_time() {
    let interp = run_code(
        r#"
        гыы д = захуярить Дата(0);
        гыы врем = д.поставитьВремя(86400000);
        "#,
    );
    assert_eq!(interp.get("врем"), Some(Value::Number(86_400_000.0)));
}

#[test]
fn date_parse_iso_variants() {
    let interp = run_code(
        r#"
        гыы а = Дата.разобрать("2026-07-19");
        гыы б = Дата.разобрать("2026-07-19T12:00:00Z");
        гыы в = Дата.разобрать("2026-07-19T12:00:00+02:00");
        гыы г = Дата.разобрать("не дата");
        гыы гПлохо = г !== г;
        "#,
    );
    assert_eq!(interp.get("а"), Some(Value::Number(1_784_419_200_000.0)));
    assert_eq!(interp.get("б"), Some(Value::Number(1_784_462_400_000.0)));
    assert_eq!(interp.get("в"), Some(Value::Number(1_784_455_200_000.0)));
    assert_eq!(interp.get("гПлохо"), Some(Value::Boolean(true)));
}

#[test]
fn date_set_full_year_on_invalid_date() {
    let interp = run_code(
        r#"
        гыы д = захуярить Дата("не дата");
        д.поставитьГод(2020);
        гыы исо = д.вИСО();
        "#,
    );
    assert_eq!(interp.get("исо"), Some(Value::String("2020-01-01T00:00:00.000Z".to_string())));
}

#[test]
fn reflect_get_existing_property() {
    let interp = run_code(
        r#"
        гыы о = {а: 42};
        гыы р = Отражение.получить(о, "а");
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::Number(42.0)));
}

#[test]
fn reflect_get_missing_property_returns_undefined() {
    let interp = run_code(
        r#"
        гыы о = {а: 1};
        гыы р = Отражение.получить(о, "нет");
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::Undefined));
}

#[test]
fn reflect_has_true() {
    let interp = run_code(
        r#"
        гыы о = {ключ: "значение"};
        гыы р = Отражение.есть(о, "ключ");
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::Boolean(true)));
}

#[test]
fn reflect_has_false() {
    let interp = run_code(
        r#"
        гыы о = {ключ: "значение"};
        гыы р = Отражение.есть(о, "нет");
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::Boolean(false)));
}

#[test]
fn reflect_get_prototype_of_object_without_proto() {
    let interp = run_code(
        r#"
        гыы р = Отражение.прототипОт({});
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::Null));
}

#[test]
fn reflect_get_prototype_of_instance_returns_class() {
    let interp = run_code(
        r#"
        клёво Точка {
            Точка(х) {
                тырыпыры.х = х;
            }
        }
        гыы т = захуярить Точка(5);
        гыы прото = Отражение.прототипОт(т);
        гыы совпадает = прото == Точка;
        "#,
    );
    assert_eq!(interp.get("совпадает"), Some(Value::Boolean(true)));
}

#[test]
fn reflect_own_keys_returns_array_without_internals() {
    let interp = run_code(
        r#"
        гыы о = {а: 1, б: 2};
        гыы ключи = Отражение.собственныеКлючи(о);
        гыы длн = длина(ключи);
        "#,
    );
    assert_eq!(interp.get("длн"), Some(Value::Number(2.0)));
}

#[test]
fn reflect_own_keys_excludes_private() {
    let interp = run_code(
        r#"
        клёво Бокс {
            #секрет = 1;
            Бокс() {
                тырыпыры.открытое = 2;
            }
        }
        гыы б = захуярить Бокс();
        гыы ключи = Отражение.собственныеКлючи(б);
        гыы длн = длина(ключи);
        гыы первый = ключи[0];
        "#,
    );
    assert_eq!(interp.get("длн"), Some(Value::Number(1.0)));
    assert_eq!(interp.get("первый"), Some(Value::String("открытое".to_string())));
}

#[test]
fn reflect_apply_calls_function() {
    let interp = run_code(
        r#"
        йопта сложить(а, б) {
            отвечаю а + б;
        }
        гыы р = Отражение.применить(сложить, ноль, [3, 4]);
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::Number(7.0)));
}

#[test]
fn reflect_construct_creates_instance() {
    let interp = run_code(
        r#"
        клёво Точка {
            Точка(х, у) {
                тырыпыры.х = х;
                тырыпыры.у = у;
            }
        }
        гыы т = Отражение.построить(Точка, [10, 20]);
        гыы рх = т.х;
        "#,
    );
    assert_eq!(interp.get("рх"), Some(Value::Number(10.0)));
}

#[test]
fn reflect_set_on_plain_object() {
    let interp = run_code(
        r#"
        гыы о = {};
        гыы ок = Отражение.установить(о, "а", 5);
        гыы знач = о.а;
        "#,
    );
    assert_eq!(interp.get("ок"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("знач"), Some(Value::Number(5.0)));
}

#[test]
fn reflect_set_routes_through_proxy_trap() {
    let interp = run_code(
        r#"
        гыы захвачено = "";
        ясенХуй п = захуярить Посредник({}, {
            установить: (ц, к, зн, пр) => { захвачено = к + "=" + зн; отвечаю правда; }
        });
        Отражение.установить(п, "поле", 9);
        "#,
    );
    assert_eq!(interp.get("захвачено"), Some(Value::String("поле=9".to_string())));
}

#[test]
fn reflect_delete_on_plain_object_and_proxy() {
    let interp = run_code(
        r#"
        гыы о = { а: 1 };
        гыы ок = Отражение.удалить(о, "а");
        гыы после = тип(о.а);
        гыы удалено = "";
        ясенХуй п = захуярить Посредник({}, { удалить: (ц, к) => { удалено = к; отвечаю правда; } });
        Отражение.удалить(п, "x");
        "#,
    );
    assert_eq!(interp.get("ок"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("после"), Some(Value::String("неопределено".to_string())));
    assert_eq!(interp.get("удалено"), Some(Value::String("x".to_string())));
}

#[test]
fn reflect_set_prototype_of_plain_and_proxy() {
    let interp = run_code(
        r#"
        клёво К {}
        гыы экз = захуярить К();
        гыы о = {};
        гыы ок = Отражение.назначитьПрототип(о, экз);
        гыы есть = о шкура К;
        гыы захвачено = ноль;
        ясенХуй п = захуярить Посредник({}, {
            назначитьПрототип: (ц, прт) => { захвачено = прт; отвечаю правда; }
        });
        Отражение.назначитьПрототип(п, экз);
        "#,
    );
    assert_eq!(interp.get("ок"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("есть"), Some(Value::Boolean(true)));
    assert!(matches!(interp.get("захвачено"), Some(Value::Object(_))));
}

#[test]
fn reflect_define_and_describe_property_plain() {
    let interp = run_code(
        r#"
        гыы о = {};
        гыы ок = Отражение.определитьСвойство(о, "п", { значение: 42 });
        гыы деск = Отражение.описатьСвойство(о, "п").значение;
        "#,
    );
    assert_eq!(interp.get("ок"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("деск"), Some(Value::Number(42.0)));
}

#[test]
fn reflect_define_property_routes_through_proxy_trap() {
    let interp = run_code(
        r#"
        гыы определено = "";
        ясенХуй п = захуярить Посредник({}, {
            определитьСвойство: (ц, к, деск) => { определено = к; отвечаю правда; }
        });
        гыы ок = Отражение.определитьСвойство(п, "поле", { значение: 1 });
        "#,
    );
    assert_eq!(interp.get("ок"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("определено"), Some(Value::String("поле".to_string())));
}

#[test]
fn reflect_extensibility_plain_and_proxy() {
    let interp = run_code(
        r#"
        гыы о = {};
        гыы до = Отражение.расширяем(о);
        Отражение.запретитьРасширение(о);
        гыы после = Отражение.расширяем(о);
        гыы запрещено = 0;
        ясенХуй п = захуярить Посредник({}, {
            расширяем: (ц) => лож,
            запретитьРасширение: (ц) => { запрещено = запрещено + 1; отвечаю правда; }
        });
        гыы прокси_расш = Отражение.расширяем(п);
        Отражение.запретитьРасширение(п);
        "#,
    );
    assert_eq!(interp.get("до"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("после"), Some(Value::Boolean(false)));
    assert_eq!(interp.get("прокси_расш"), Some(Value::Boolean(false)));
    assert_eq!(interp.get("запрещено"), Some(Value::Number(1.0)));
}

#[test]
fn reflect_forwarding_handler_uses_target_defaults() {
    let interp = run_code(
        r#"
        ясенХуй п = захуярить Посредник({ x: 1 }, {
            получить: (ц, к, пр) => Отражение.получить(ц, к),
            установить: (ц, к, зн, пр) => Отражение.установить(ц, к, зн),
            есть: (ц, к) => Отражение.есть(ц, к),
            собственныеКлючи: (ц) => Отражение.собственныеКлючи(ц)
        });
        п.y = 2;
        гыы сумма = п.x + п.y;
        гыы естьX = "x" чоунастут п;
        гыы ключи = Отражение.собственныеКлючи(п).склеить(",");
        "#,
    );
    assert_eq!(interp.get("сумма"), Some(Value::Number(3.0)));
    assert_eq!(interp.get("естьX"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("ключи"), Some(Value::String("x,y".to_string())));
}
