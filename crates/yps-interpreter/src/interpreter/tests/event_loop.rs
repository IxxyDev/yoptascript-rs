use super::*;

#[test]
fn чутка_fires_in_deadline_order() {
    let interp = run_code(
        r#"
        гыы лог = [];
        чутка(() => { лог = втолкнуть(лог, "A"); }, 30);
        чутка(() => { лог = втолкнуть(лог, "B"); }, 5);
        "#,
    );
    assert_struct_eq(interp.get("лог"), Value::array(vec![Value::String("B".into()), Value::String("A".into())]));
}

#[test]
#[allow(non_snake_case)]
fn отменаЧутки_prevents_callback() {
    let interp = run_code(
        r#"
        гыы лог = [];
        гыы ид = чутка(() => { лог = втолкнуть(лог, "X"); }, 5);
        отменаЧутки(ид);
        "#,
    );
    assert_struct_eq(interp.get("лог"), Value::array(vec![]));
}

#[test]
fn интервал_fires_multiple_times_then_cancels() {
    let interp = run_code(
        r#"
        гыы счёт = 0;
        гыы ид = ноль;
        ид = интервал(() => {
            счёт = счёт + 1;
            вилкойвглаз (счёт >= 3) {
                отменаИнтервала(ид);
            }
        }, 1);
        "#,
    );
    assert_eq!(interp.get("счёт"), Some(Value::Number(3.0)));
}

#[test]
fn сразу_runs_before_macrotask() {
    let interp = run_code(
        r#"
        гыы лог = [];
        чутка(() => { лог = втолкнуть(лог, "макро"); }, 0);
        сразу(() => { лог = втолкнуть(лог, "микро"); });
        "#,
    );
    assert_struct_eq(
        interp.get("лог"),
        Value::array(vec![Value::String("микро".into()), Value::String("макро".into())]),
    );
}

#[test]
#[allow(non_snake_case)]
fn наСледующемТике_has_priority_over_сразу() {
    let interp = run_code(
        r#"
        гыы лог = [];
        чутка(() => {
            сразу(() => { лог = втолкнуть(лог, "обычная"); });
            наСледующемТике(() => { лог = втолкнуть(лог, "приоритет"); });
        }, 0);
        "#,
    );
    assert_struct_eq(
        interp.get("лог"),
        Value::array(vec![Value::String("приоритет".into()), Value::String("обычная".into())]),
    );
}

#[test]
fn await_parks_on_pending_promise_resolved_by_chutka() {
    let interp = run_code(
        r#"
        ассо йопта получить() {
            отвечаю захуярить СловоПацана((решить, _) => {
                чутка(() => решить(42), 5);
            });
        }
        ассо йопта главное() {
            гыы х = сидетьНахуй получить();
            отвечаю х;
        }
        гыы p = главное();
        гыы итог = ноль;
        p.потом((v) => { итог = v; });
        "#,
    );
    assert_eq!(interp.get("итог"), Some(Value::Number(42.0)));
}

#[test]
fn top_level_await_drives_loop() {
    let interp = run_code(
        r#"
        гыы р = захуярить СловоПацана((решить, _) => {
            чутка(() => решить("готово"), 5);
        });
        гыы значение = сидетьНахуй р;
        "#,
    );
    assert_eq!(interp.get("значение"), Some(Value::String("готово".into())));
}

#[test]
fn await_chain_across_delays() {
    let interp = run_code(
        r#"
        йопта задержанный(значение, мс) {
            отвечаю захуярить СловоПацана((решить, _) => {
                чутка(() => решить(значение), мс);
            });
        }
        ассо йопта суммаЧерезЗадержки() {
            гыы а = сидетьНахуй задержанный(10, 5);
            гыы б = сидетьНахуй задержанный(20, 5);
            гыы в = сидетьНахуй задержанный(30, 5);
            отвечаю а + б + в;
        }
        гыы итог = ноль;
        суммаЧерезЗадержки().потом((v) => { итог = v; });
        "#,
    );
    assert_eq!(interp.get("итог"), Some(Value::Number(60.0)));
}

#[test]
fn await_rejected_promise_caught_by_try_catch() {
    let interp = run_code(
        r#"
        ассо йопта плохо() {
            гыы p = захуярить СловоПацана((_, отвергнуть) => {
                чутка(() => отвергнуть("боль"), 5);
            });
            сидетьНахуй p;
        }
        ассо йопта главное() {
            гыы пойман = ноль;
            хапнуть {
                сидетьНахуй плохо();
            } гоп (e) {
                пойман = e;
            }
            отвечаю пойман;
        }
        гыы итог = ноль;
        главное().потом((v) => { итог = v; });
        "#,
    );
    assert_eq!(interp.get("итог"), Some(Value::String("боль".into())));
}

#[test]
fn timer_callback_throw_does_not_kill_loop() {
    let interp = run_code(
        r#"
        гыы лог = [];
        чутка(() => { кидай "плохой коллбэк"; }, 5);
        чутка(() => { лог = втолкнуть(лог, "выжил"); }, 10);
        "#,
    );
    assert_struct_eq(interp.get("лог"), Value::array(vec![Value::String("выжил".into())]));
}

#[test]
fn interval_continues_after_throwing_tick() {
    let interp = run_code(
        r#"
        гыы счёт = 0;
        гыы ид = ноль;
        ид = интервал(() => {
            счёт = счёт + 1;
            вилкойвглаз (счёт == 1) {
                кидай "сбой";
            }
            вилкойвглаз (счёт >= 3) {
                отменаИнтервала(ид);
            }
        }, 1);
        "#,
    );
    assert_eq!(interp.get("счёт"), Some(Value::Number(3.0)));
}

#[test]
fn await_on_non_promise_returns_value() {
    let interp = run_code(
        r#"
        ассо йопта f() {
            отвечаю сидетьНахуй 42;
        }
        гыы итог = ноль;
        f().потом((v) => { итог = v; });
        "#,
    );
    assert_eq!(interp.get("итог"), Some(Value::Number(42.0)));
}

#[test]
fn async_fn_returns_pending_immediately() {
    let interp = run_code(
        r#"
        гыы маркер = "before";
        ассо йопта работа() {
            маркер = "inside";
            отвечаю 1;
        }
        гыы пара = [работа(), маркер];
        "#,
    );
    let pair = interp.get("пара").unwrap();
    let Value::Array(items) = pair else { panic!("expected array") };
    let items = items.borrow();
    assert!(matches!(items[0], Value::Promise { .. }), "first element must be a Promise");
    assert_eq!(items[1], Value::String("before".into()));
    assert_eq!(interp.get("маркер"), Some(Value::String("inside".into())));
}

#[test]
fn async_fn_returning_pending_promise_adopts() {
    let interp = run_code(
        r#"
        ассо йопта внутри() {
            отвечаю захуярить СловоПацана((решить, _) => {
                чутка(() => решить(99), 5);
            });
        }
        ассо йопта снаружи() {
            отвечаю внутри();
        }
        гыы итог = ноль;
        снаружи().потом((v) => { итог = v; });
        "#,
    );
    assert_eq!(interp.get("итог"), Some(Value::Number(99.0)));
}

#[test]
fn non_callable_to_chutka_errors_with_call_site() {
    let err = run_code_err(r#"чутка(42, 10);"#);
    assert!(err.message.contains("'чутка'"), "got: {}", err.message);
    assert!(err.message.contains("функцию"), "got: {}", err.message);
}

#[test]
fn non_callable_to_srazu_errors() {
    let err = run_code_err(r#"сразу("не функция");"#);
    assert!(err.message.contains("'сразу'"), "got: {}", err.message);
}

#[test]
fn await_depth_limit_caught() {
    let interp = run_code(
        r#"
        ассо йопта глубина(н) {
            вилкойвглаз (н <= 0) {
                отвечаю 0;
            }
            гыы p = захуярить СловоПацана((решить, _) => {
                чутка(() => решить(н), 0);
            });
            гыы значение = сидетьНахуй p;
            гыы дальше = сидетьНахуй глубина(н - 1);
            отвечаю значение + дальше;
        }
        гыы пойман = ноль;
        ассо йопта запуск() {
            хапнуть {
                сидетьНахуй глубина(30);
            } гоп (e) {
                пойман = e.message;
            }
        }
        запуск();
        "#,
    );
    let caught = interp.get("пойман").unwrap();
    if let Value::String(s) = caught {
        assert!(s.contains("глубин"), "expected depth-limit message, got: {s}");
    } else {
        panic!("expected error string, got {caught:?}");
    }
}

#[test]
fn nested_intervals_independent_cancellation() {
    let interp = run_code(
        r#"
        гыы лог = [];
        гыы внешнийИд = ноль;
        гыы внутреннийИд = ноль;
        гыы внешнийСчёт = 0;
        внешнийИд = интервал(() => {
            внешнийСчёт = внешнийСчёт + 1;
            лог = втолкнуть(лог, "внешний");
            вилкойвглаз (внешнийСчёт == 1) {
                гыы внутреннийСчёт = 0;
                внутреннийИд = интервал(() => {
                    внутреннийСчёт = внутреннийСчёт + 1;
                    лог = втолкнуть(лог, "внутренний");
                    вилкойвглаз (внутреннийСчёт >= 2) {
                        отменаИнтервала(внутреннийИд);
                    }
                }, 1);
            }
            вилкойвглаз (внешнийСчёт >= 3) {
                отменаИнтервала(внешнийИд);
            }
        }, 5);
        "#,
    );
    let log = interp.get("лог").unwrap();
    let Value::Array(items) = log else { panic!("expected array") };
    let items = items.borrow();
    let labels: Vec<&str> = items.iter().map(|v| if let Value::String(s) = v { s.as_str() } else { "?" }).collect();
    assert_eq!(labels.iter().filter(|l| **l == "внешний").count(), 3);
    assert_eq!(labels.iter().filter(|l| **l == "внутренний").count(), 2);
}

#[test]
fn interval_cancelled_mid_callback_fires_exactly_once() {
    let interp = run_code(
        r#"
        гыы счёт = 0;
        гыы ид = ноль;
        ид = интервал(() => {
            счёт = счёт + 1;
            отменаИнтервала(ид);
        }, 1);
        "#,
    );
    assert_eq!(interp.get("счёт"), Some(Value::Number(1.0)));
}

#[test]
fn rejected_promise_then_catch_path() {
    let interp = run_code(
        r#"
        гыы пойман = ноль;
        захуярить СловоПацана((_, отвергнуть) => {
            чутка(() => отвергнуть("ошибка"), 5);
        }).ловить((e) => { пойман = e; });
        "#,
    );
    assert_eq!(interp.get("пойман"), Some(Value::String("ошибка".into())));
}

#[test]
fn promise_all_resolves_after_async_delays() {
    let interp = run_code(
        r#"
        йопта задержка(значение, мс) {
            отвечаю захуярить СловоПацана((решить, _) => {
                чутка(() => решить(значение), мс);
            });
        }
        гыы итог = ноль;
        СловоПацана.всех([задержка(1, 10), задержка(2, 5), задержка(3, 15)])
            .потом((v) => { итог = v; });
        "#,
    );
    assert_struct_eq(
        interp.get("итог"),
        Value::array(vec![Value::Number(1.0), Value::Number(2.0), Value::Number(3.0)]),
    );
}

#[test]
fn promise_all_rejects_on_first_failure() {
    let interp = run_code(
        r#"
        гыы пойман = ноль;
        СловоПацана.всех([
            захуярить СловоПацана((решить, _) => { чутка(() => решить(1), 20); }),
            захуярить СловоПацана((_, отвергнуть) => { чутка(() => отвергнуть("плохо"), 5); })
        ]).ловить((e) => { пойман = e; });
        "#,
    );
    assert_eq!(interp.get("пойман"), Some(Value::String("плохо".into())));
}

#[test]
fn promise_race_takes_first_settled() {
    let interp = run_code(
        r#"
        гыы итог = ноль;
        СловоПацана.гонка([
            захуярить СловоПацана((решить, _) => { чутка(() => решить("медленно"), 20); }),
            захуярить СловоПацана((решить, _) => { чутка(() => решить("быстро"), 5); })
        ]).потом((v) => { итог = v; });
        "#,
    );
    assert_eq!(interp.get("итог"), Some(Value::String("быстро".into())));
}

#[test]
fn promise_any_skips_rejections_returns_first_success() {
    let interp = run_code(
        r#"
        гыы итог = ноль;
        СловоПацана.любой([
            захуярить СловоПацана((_, отвергнуть) => { чутка(() => отвергнуть("a"), 5); }),
            захуярить СловоПацана((решить, _) => { чутка(() => решить("ок"), 10); }),
            захуярить СловоПацана((_, отвергнуть) => { чутка(() => отвергнуть("b"), 15); })
        ]).потом((v) => { итог = v; });
        "#,
    );
    assert_eq!(interp.get("итог"), Some(Value::String("ок".into())));
}

#[test]
fn promise_any_all_rejected_emits_aggregate_error() {
    let interp = run_code(
        r#"
        гыы имя = ноль;
        гыы ошибки = ноль;
        СловоПацана.любой([
            захуярить СловоПацана((_, отвергнуть) => { чутка(() => отвергнуть("a"), 5); }),
            захуярить СловоПацана((_, отвергнуть) => { чутка(() => отвергнуть("b"), 10); })
        ]).ловить((e) => { имя = e.name; ошибки = e.errors; });
        "#,
    );
    assert_eq!(interp.get("имя"), Some(Value::String("ВсёОбосралось".into())));
    assert_struct_eq(interp.get("ошибки"), Value::array(vec![Value::String("a".into()), Value::String("b".into())]));
}

#[test]
fn promise_all_settled_collects_all_outcomes() {
    let interp = run_code(
        r#"
        гыы статусы = ноль;
        гыы первое = ноль;
        гыы причина = ноль;
        гыы третье = ноль;
        СловоПацана.всехУстаканить([
            захуярить СловоПацана((решить, _) => { чутка(() => решить(1), 5); }),
            захуярить СловоПацана((_, отвергнуть) => { чутка(() => отвергнуть("плохо"), 10); }),
            захуярить СловоПацана((решить, _) => { чутка(() => решить(3), 15); })
        ]).потом((v) => {
            статусы = [v[0].статус, v[1].статус, v[2].статус];
            первое = v[0].значение;
            причина = v[1].причина;
            третье = v[2].значение;
        });
        "#,
    );
    assert_struct_eq(
        interp.get("статусы"),
        Value::array(vec![
            Value::String("выполнено".into()),
            Value::String("отклонено".into()),
            Value::String("выполнено".into()),
        ]),
    );
    assert_eq!(interp.get("первое"), Some(Value::Number(1.0)));
    assert_eq!(interp.get("причина"), Some(Value::String("плохо".into())));
    assert_eq!(interp.get("третье"), Some(Value::Number(3.0)));
}

#[test]
fn promise_all_empty_array_resolves_to_empty() {
    let interp = run_code(
        r#"
        гыы итог = ноль;
        СловоПацана.всех([]).потом((v) => { итог = v; });
        "#,
    );
    assert_struct_eq(interp.get("итог"), Value::array(Vec::new()));
}

#[test]
fn promise_any_empty_array_rejects_with_aggregate() {
    let interp = run_code(
        r#"
        гыы имя = ноль;
        СловоПацана.любой([]).ловить((e) => { имя = e.name; });
        "#,
    );
    assert_eq!(interp.get("имя"), Some(Value::String("ВсёОбосралось".into())));
}

#[test]
fn promise_race_with_rejection_first_propagates() {
    let interp = run_code(
        r#"
        гыы пойман = ноль;
        СловоПацана.гонка([
            захуярить СловоПацана((решить, _) => { чутка(() => решить("поздно"), 20); }),
            захуярить СловоПацана((_, отвергнуть) => { чутка(() => отвергнуть("рано"), 5); })
        ]).ловить((e) => { пойман = e; });
        "#,
    );
    assert_eq!(interp.get("пойман"), Some(Value::String("рано".into())));
}

#[test]
fn test_await_podozhdat_blocks_min_50ms() {
    let start = std::time::Instant::now();
    let interp = run_code(
        r#"
        гыы готово = лож;
        ассо йопта главное() {
            сидетьНахуй подождать(50);
            готово = правда;
        }
        главное();
        "#,
    );
    let elapsed = start.elapsed();
    assert_eq!(interp.get("готово"), Some(Value::Boolean(true)));
    assert!(elapsed.as_millis() >= 40, "ожидалось >=40мс, было {}мс", elapsed.as_millis());
    assert!(elapsed.as_millis() < 500, "ожидалось <500мс, было {}мс", elapsed.as_millis());
}

#[test]
fn test_await_podozhdat_aborts_via_signal() {
    let interp = run_code(
        r#"
        гыы пойман = ноль;
        ассо йопта главное() {
            гыы к = захуярить КонтроллёрОтмены();
            чутка(() => к.отменить({ name: "ОшибкаОтмены", message: "сигнал" }), 5);
            хапнуть {
                сидетьНахуй подождать(500, { сигнал: к.сигнал });
            } гоп (e) {
                пойман = e.message;
            }
        }
        главное();
        "#,
    );
    assert_eq!(interp.get("пойман"), Some(Value::String("сигнал".into())));
}

#[test]
fn test_sochereit_runs_before_macrotask() {
    let interp = run_code(
        r#"
        гыы лог = [];
        чутка(() => { лог = втолкнуть(лог, "макро"); }, 0);
        сОчередить(() => { лог = втолкнуть(лог, "микро"); });
        "#,
    );
    assert_struct_eq(
        interp.get("лог"),
        Value::array(vec![Value::String("микро".into()), Value::String("макро".into())]),
    );
}

#[test]
fn test_promise_race_picks_shortest_timer() {
    let interp = run_code(
        r#"
        гыы итог = ноль;
        ассо йопта главное() {
            итог = сидетьНахуй СловоПацана.гонка([подождать(50), подождать(5), подождать(100)]);
        }
        главное();
        "#,
    );
    assert_eq!(interp.get("итог"), Some(Value::Undefined));
}

#[test]
fn test_abort_signal_ot_vremeni_rejects() {
    let interp = run_code(
        r#"
        гыы имя = ноль;
        гыы сообщ = ноль;
        ассо йопта главное() {
            гыы с = СигналОтмены.отВремени(10);
            хапнуть {
                сидетьНахуй с.обещание;
            } гоп (e) {
                имя = e.name;
                сообщ = e.message;
            }
        }
        главное();
        "#,
    );
    assert_eq!(interp.get("имя"), Some(Value::String("ОшибкаОтмены".into())));
    assert_eq!(interp.get("сообщ"), Some(Value::String("Тайм-аут".into())));
}

#[test]
fn test_signal_promise_cached_no_listener_leak() {
    let interp = run_code(
        r#"
        гыы к = захуярить КонтроллёрОтмены();
        гыы сиг = к.сигнал;
        гыы и = 0;
        потрещим (и < 100) {
            гыы _ = сиг.обещание;
            и = и + 1;
        }
        "#,
    );
    let sig = interp.get("сиг").expect("сиг должен быть определён");
    let state = match sig {
        Value::AbortSignal { state } => state,
        other => panic!("ожидался AbortSignal, получено {other:?}"),
    };
    let count = state.borrow().listeners.len();
    assert!(count <= 2, "ожидалось <=2 слушателей, было {count}");
}
