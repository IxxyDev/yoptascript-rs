use super::*;

#[test]
fn generator_collects_yielded_values() {
    let i = run_code(
        r#"
        пиздюли диапазон(н) {
            го (гыы и = 0; и < н; и += 1) {
                поебалу и;
            }
        }
        гыы рез = [];
        го (гыы х сашаГрей диапазон(3)) {
            рез.втолкнуть(х);
        }
        "#,
    );
    assert_struct_eq(i.get("рез"), Value::array(vec![Value::Number(0.0), Value::Number(1.0), Value::Number(2.0)]));
}

#[test]
fn generator_for_loop_closures_capture_per_iteration() {
    let i = run_code(
        r#"
        пиздюли ген() {
            гыы f = [];
            го (гыы и = 0; и < 3; и = и + 1) { f.втолкнуть(() => и); }
            поебалу f[0]();
            поебалу f[1]();
            поебалу f[2]();
        }
        гыы рез = [];
        го (гыы х сашаГрей ген()) { рез.втолкнуть(х); }
        "#,
    );
    assert_struct_eq(i.get("рез"), Value::array(vec![Value::Number(0.0), Value::Number(1.0), Value::Number(2.0)]));
}

#[test]
fn generator_for_of_closures_capture_per_iteration() {
    let i = run_code(
        r#"
        пиздюли ген() {
            гыы f = [];
            го (гыы x сашаГрей [10, 20, 30]) { f.втолкнуть(() => x); }
            поебалу f[0]();
            поебалу f[1]();
            поебалу f[2]();
        }
        гыы рез = [];
        го (гыы х сашаГрей ген()) { рез.втолкнуть(х); }
        "#,
    );
    assert_struct_eq(i.get("рез"), Value::array(vec![Value::Number(10.0), Value::Number(20.0), Value::Number(30.0)]));
}

#[test]
fn generator_for_loop_capture_through_nested_block() {
    let i = run_code(
        r#"
        пиздюли ген() {
            гыы f = [];
            го (гыы и = 0; и < 3; и = и + 1) { { f.втолкнуть(() => и); } }
            поебалу f[0]();
            поебалу f[1]();
            поебалу f[2]();
        }
        гыы рез = [];
        го (гыы х сашаГрей ген()) { рез.втолкнуть(х); }
        "#,
    );
    assert_struct_eq(i.get("рез"), Value::array(vec![Value::Number(0.0), Value::Number(1.0), Value::Number(2.0)]));
}

#[test]
fn generator_for_loop_capture_through_try() {
    let i = run_code(
        r#"
        пиздюли ген() {
            гыы f = [];
            го (гыы и = 0; и < 3; и = и + 1) {
                хапнуть { f.втолкнуть(() => и); } тюряжка { }
            }
            поебалу f[0]();
            поебалу f[1]();
            поебалу f[2]();
        }
        гыы рез = [];
        го (гыы х сашаГрей ген()) { рез.втолкнуть(х); }
        "#,
    );
    assert_struct_eq(i.get("рез"), Value::array(vec![Value::Number(0.0), Value::Number(1.0), Value::Number(2.0)]));
}

#[test]
fn generator_yield_without_argument() {
    let i = run_code(
        r#"
        пиздюли пусто() {
            поебалу;
            поебалу;
        }
        гыы рез = [];
        го (гыы х сашаГрей пусто()) {
            рез.втолкнуть(х);
        }
        "#,
    );
    assert_struct_eq(i.get("рез"), Value::array(vec![Value::Undefined, Value::Undefined]));
}

#[test]
fn generator_yield_delegate_flattens_iterable() {
    let i = run_code(
        r#"
        пиздюли вн() {
            поебалу 10;
            поебалу 20;
        }
        пиздюли внеш() {
            поебалу 1;
            поебалуна вн();
            поебалу 2;
        }
        гыы рез = [];
        го (гыы х сашаГрей внеш()) {
            рез.втолкнуть(х);
        }
        "#,
    );
    assert_struct_eq(
        i.get("рез"),
        Value::array(vec![Value::Number(1.0), Value::Number(10.0), Value::Number(20.0), Value::Number(2.0)]),
    );
}

#[test]
fn generator_iterable_in_for_of() {
    let i = run_code(
        r#"
        пиздюли тройка() {
            поебалу 1;
            поебалу 2;
            поебалу 3;
        }
        гыы сумма = 0;
        го (гыы х сашаГрей тройка()) {
            сумма += х;
        }
        "#,
    );
    assert_eq!(i.get("сумма"), Some(Value::Number(6.0)));
}

#[test]
fn generator_early_return_stops_collection() {
    let i = run_code(
        r#"
        пиздюли стоп() {
            поебалу 1;
            отвечаю;
            поебалу 2;
        }
        гыы рез = [];
        го (гыы х сашаГрей стоп()) {
            рез.втолкнуть(х);
        }
        "#,
    );
    assert_struct_eq(i.get("рез"), Value::array(vec![Value::Number(1.0)]));
}

#[test]
fn generator_is_lazy_infinite_take() {
    let i = run_code(
        r#"
        пиздюли натуральные() {
            гыы н = 0;
            потрещим (правда) {
                поебалу н;
                н = н + 1;
            }
        }
        гыы рез = натуральные().взять(4).вМассив();
        "#,
    );
    assert_struct_eq(
        i.get("рез"),
        Value::array(vec![Value::Number(0.0), Value::Number(1.0), Value::Number(2.0), Value::Number(3.0)]),
    );
}

#[test]
fn generator_closure_counter_preserves_state() {
    let i = run_code(
        r#"
        пиздюли счёт() {
            гыы н = 10;
            потрещим (правда) {
                поебалу н;
                н = н + 1;
            }
        }
        гыы ит = счёт();
        гыы а = ит.следующий().значение;
        гыы б = ит.следующий().значение;
        гыы в = ит.следующий().значение;
        "#,
    );
    assert_eq!(i.get("а"), Some(Value::Number(10.0)));
    assert_eq!(i.get("б"), Some(Value::Number(11.0)));
    assert_eq!(i.get("в"), Some(Value::Number(12.0)));
}

#[test]
fn generator_yield_in_if_branch() {
    let i = run_code(
        r#"
        пиздюли только_чёт() {
            го (гыы и = 0; и < 5; и += 1) {
                вилкойвглаз (и % 2 === 0) {
                    поебалу и;
                }
            }
        }
        гыы рез = [];
        го (гыы х сашаГрей только_чёт()) { рез.втолкнуть(х); }
        "#,
    );
    assert_struct_eq(i.get("рез"), Value::array(vec![Value::Number(0.0), Value::Number(2.0), Value::Number(4.0)]));
}

#[test]
fn generator_yield_value_via_next_protocol() {
    let i = run_code(
        r#"
        пиздюли пара() {
            поебалу "а";
            поебалу "б";
        }
        гыы ит = пара();
        гыы р1 = ит.следующий();
        гыы р2 = ит.следующий();
        гыы р3 = ит.следующий();
        "#,
    );
    let r1 = i.get("р1").unwrap();
    let r2 = i.get("р2").unwrap();
    let r3 = i.get("р3").unwrap();
    if let Value::Object(m) = r1 {
        let m = m.borrow();
        assert_eq!(m.get("значение"), Some(&Value::String("а".into())));
        assert_eq!(m.get("готово"), Some(&Value::Boolean(false)));
    } else {
        panic!("ожидался объект, получено {r1:?}");
    }
    if let Value::Object(m) = r2 {
        let m = m.borrow();
        assert_eq!(m.get("значение"), Some(&Value::String("б".into())));
        assert_eq!(m.get("готово"), Some(&Value::Boolean(false)));
    } else {
        panic!();
    }
    if let Value::Object(m) = r3 {
        let m = m.borrow();
        assert_eq!(m.get("готово"), Some(&Value::Boolean(true)));
    } else {
        panic!();
    }
}

#[test]
fn generator_break_exits_inner_loop() {
    let i = run_code(
        r#"
        пиздюли до_пяти() {
            гыы и = 0;
            потрещим (правда) {
                вилкойвглаз (и >= 5) { харэ; }
                поебалу и;
                и = и + 1;
            }
            поебалу 99;
        }
        гыы рез = [];
        го (гыы х сашаГрей до_пяти()) { рез.втолкнуть(х); }
        "#,
    );
    assert_struct_eq(
        i.get("рез"),
        Value::array(vec![
            Value::Number(0.0),
            Value::Number(1.0),
            Value::Number(2.0),
            Value::Number(3.0),
            Value::Number(4.0),
            Value::Number(99.0),
        ]),
    );
}

#[test]
fn generator_continue_in_while() {
    let i = run_code(
        r#"
        пиздюли без_трёх() {
            гыы и = 0;
            потрещим (и < 5) {
                и = и + 1;
                вилкойвглаз (и === 3) { двигай; }
                поебалу и;
            }
        }
        гыы рез = [];
        го (гыы х сашаГрей без_трёх()) { рез.втолкнуть(х); }
        "#,
    );
    assert_struct_eq(
        i.get("рез"),
        Value::array(vec![Value::Number(1.0), Value::Number(2.0), Value::Number(4.0), Value::Number(5.0)]),
    );
}

#[test]
fn generator_try_catch_yields_in_catch() {
    let i = run_code(
        r#"
        пиздюли спотыкается() {
            хапнуть {
                поебалу 1;
                кидай "бум";
                поебалу 999;
            } гоп (е) {
                поебалу е;
                поебалу 2;
            }
        }
        гыы рез = [];
        го (гыы х сашаГрей спотыкается()) { рез.втолкнуть(х); }
        "#,
    );
    assert_struct_eq(
        i.get("рез"),
        Value::array(vec![Value::Number(1.0), Value::String("бум".into()), Value::Number(2.0)]),
    );
}

#[test]
fn generator_iter_helpers_compose() {
    let i = run_code(
        r#"
        пиздюли диап() {
            го (гыы и = 0; и < 10; и += 1) { поебалу и; }
        }
        гыы рез = диап()
            .filter((х) => х % 2 === 0)
            .map((х) => х * х)
            .взять(3)
            .вМассив();
        "#,
    );
    assert_struct_eq(i.get("рез"), Value::array(vec![Value::Number(0.0), Value::Number(4.0), Value::Number(16.0)]));
}

#[test]
fn generator_yield_in_subexpression_errors() {
    let err = run_code_err(
        r#"
        пиздюли плохо() {
            гыы х = (поебалу 1) + 2;
        }
        плохо().следующий();
        "#,
    );
    assert!(err.message.contains("поебалу"));
}

#[test]
fn yield_outside_generator_errors() {
    let err = run_code_err(
        r#"
        йопта обыч() { поебалу 1; }
        обыч();
        "#,
    );
    assert!(err.message.contains("пиздюли"));
}

#[test]
fn generator_return_method_basic() {
    let i = run_code(
        r#"
        пиздюли ген() {
            поебалу 1;
            поебалу 2;
            поебалу 3;
        }
        гыы г = ген();
        гыы а = г.следующий().значение;
        гыы р = г.вернуть(42);
        гыы знач = р.значение;
        гыы готово = р.готово;
        гыы посл = г.следующий().готово;
        "#,
    );
    assert_eq!(i.get("а"), Some(Value::Number(1.0)));
    assert_eq!(i.get("знач"), Some(Value::Number(42.0)));
    assert_eq!(i.get("готово"), Some(Value::Boolean(true)));
    assert_eq!(i.get("посл"), Some(Value::Boolean(true)));
}

#[test]
fn generator_return_runs_finally_side_effect() {
    let i = run_code(
        r#"
        гыы счёт = 0;
        пиздюли ген() {
            хапнуть {
                поебалу 1;
                поебалу 2;
            } тюряжка {
                счёт = счёт + 1;
            }
        }
        гыы г = ген();
        г.следующий();
        гыы р = г.вернуть(7);
        гыы знач = р.значение;
        гыы готово = р.готово;
        "#,
    );
    assert_eq!(i.get("счёт"), Some(Value::Number(1.0)));
    assert_eq!(i.get("знач"), Some(Value::Number(7.0)));
    assert_eq!(i.get("готово"), Some(Value::Boolean(true)));
}

#[test]
fn generator_return_with_yielding_finally() {
    let i = run_code(
        r#"
        пиздюли ген() {
            хапнуть {
                поебалу 1;
                поебалу 2;
            } тюряжка {
                поебалу 100;
            }
        }
        гыы г = ген();
        г.следующий();
        гыы р1 = г.вернуть(7);
        гыы зн1 = р1.значение;
        гыы гт1 = р1.готово;
        гыы р2 = г.следующий();
        гыы зн2 = р2.значение;
        гыы гт2 = р2.готово;
        "#,
    );
    assert_eq!(i.get("зн1"), Some(Value::Number(100.0)));
    assert_eq!(i.get("гт1"), Some(Value::Boolean(false)));
    assert_eq!(i.get("зн2"), Some(Value::Number(7.0)));
    assert_eq!(i.get("гт2"), Some(Value::Boolean(true)));
}

#[test]
fn generator_throw_caught_by_inner_try() {
    let i = run_code(
        r#"
        пиздюли ген() {
            хапнуть {
                поебалу 1;
            } гоп (е) {
                поебалу е;
            }
        }
        гыы г = ген();
        г.следующий();
        гыы р = г.кинуть("упс");
        гыы зн = р.значение;
        гыы гт = р.готово;
        "#,
    );
    assert_eq!(i.get("зн"), Some(Value::String("упс".into())));
    assert_eq!(i.get("гт"), Some(Value::Boolean(false)));
}

#[test]
fn generator_throw_uncaught_propagates() {
    let err = run_code_err(
        r#"
        пиздюли ген() {
            поебалу 1;
        }
        гыы г = ген();
        г.следующий();
        г.кинуть("бах");
        "#,
    );
    assert_eq!(err.thrown, Some(Box::new(Value::String("бах".into()))));
}

#[test]
fn generator_return_on_completed() {
    let i = run_code(
        r#"
        пиздюли ген() {
            поебалу 1;
        }
        гыы г = ген();
        г.следующий();
        г.следующий();
        гыы р = г.вернуть(99);
        гыы зн = р.значение;
        гыы гт = р.готово;
        "#,
    );
    assert_eq!(i.get("зн"), Some(Value::Number(99.0)));
    assert_eq!(i.get("гт"), Some(Value::Boolean(true)));
}

#[test]
fn generator_throw_on_completed() {
    let err = run_code_err(
        r#"
        пиздюли ген() {
            поебалу 1;
        }
        гыы г = ген();
        г.следующий();
        г.следующий();
        г.кинуть("после конца");
        "#,
    );
    assert_eq!(err.thrown, Some(Box::new(Value::String("после конца".into()))));
}

#[test]
fn generator_return_before_first_next() {
    let i = run_code(
        r#"
        пиздюли ген() {
            поебалу 1;
            поебалу 2;
        }
        гыы г = ген();
        гыы р = г.вернуть(11);
        гыы зн = р.значение;
        гыы гт = р.готово;
        гыы посл = г.следующий().готово;
        "#,
    );
    assert_eq!(i.get("зн"), Some(Value::Number(11.0)));
    assert_eq!(i.get("гт"), Some(Value::Boolean(true)));
    assert_eq!(i.get("посл"), Some(Value::Boolean(true)));
}

#[test]
fn generator_yield_delegate_non_iterable_errors() {
    let err = run_code_err(
        r#"
        пиздюли ген() {
            поебалуна 42;
        }
        гыы г = ген();
        г.следующий();
        "#,
    );
    assert!(err.message.contains("итерировать") || err.message.contains("итер"));
}

#[test]
fn generator_next_arg_two_way_communication() {
    let i = run_code(
        r#"
        пиздюли двусторонний() {
            гыы а = поебалу 1;
            гыы б = поебалу 2;
            отвечаю а + б;
        }
        гыы г = двусторонний();
        гыы р1 = г.следующий("игнор").значение;
        гыы р2 = г.следующий(10).значение;
        гыы р3 = г.следующий(20);
        гыы итог = р3.значение;
        гыы гт = р3.готово;
        "#,
    );
    assert_eq!(i.get("р1"), Some(Value::Number(1.0)));
    assert_eq!(i.get("р2"), Some(Value::Number(2.0)));
    assert_eq!(i.get("итог"), Some(Value::Number(30.0)));
    assert_eq!(i.get("гт"), Some(Value::Boolean(true)));
}

#[test]
fn generator_return_in_finally_overrides_gen_return() {
    let i = run_code(
        r#"
        пиздюли ген() {
            хапнуть {
                поебалу 1;
            } тюряжка {
                отвечаю "из-финалли";
            }
        }
        гыы г = ген();
        гыы первый = г.следующий().значение;
        гыы р = г.вернуть(99);
        гыы зн = р.значение;
        гыы гт = р.готово;
        гыы посл = г.следующий().готово;
        "#,
    );
    assert_eq!(i.get("первый"), Some(Value::Number(1.0)));
    assert_eq!(i.get("зн"), Some(Value::String("из-финалли".into())));
    assert_eq!(i.get("гт"), Some(Value::Boolean(true)));
    assert_eq!(i.get("посл"), Some(Value::Boolean(true)));
}

#[test]
fn generator_closed_when_for_of_breaks_early() {
    let i = run_code(
        r#"
        гыы лог = [];
        пиздюли счёт() {
            хапнуть {
                гыы и = 0;
                потрещим (правда) {
                    поебалу и;
                    и = и + 1;
                }
            } тюряжка {
                лог.втолкнуть("закрыт");
            }
        }
        гыы сумма = 0;
        го (гыы х сашаГрей счёт()) {
            вилкойвглаз (х >= 3) { харэ; }
            сумма = сумма + х;
        }
        "#,
    );
    assert_eq!(i.get("сумма"), Some(Value::Number(3.0)));
    assert_struct_eq(i.get("лог"), Value::array(vec![Value::String("закрыт".into())]));
}

#[test]
fn generator_yield_delegate_forwards_sent_values() {
    let i = run_code(
        r#"
        гыы лог = [];
        пиздюли вн() {
            гыы а = поебалу "i1";
            лог.втолкнуть(а);
            гыы б = поебалу "i2";
            лог.втолкнуть(б);
        }
        пиздюли внеш() {
            поебалуна вн();
        }
        гыы ит = внеш();
        ит.следующий();
        ит.следующий("S1");
        ит.следующий("S2");
        "#,
    );
    assert_struct_eq(i.get("лог"), Value::array(vec![Value::String("S1".into()), Value::String("S2".into())]));
}

#[test]
fn generator_yield_delegate_return_runs_inner_finally() {
    let i = run_code(
        r#"
        гыы лог = [];
        пиздюли вн() {
            хапнуть {
                поебалу "i1";
                поебалу "i2";
            } тюряжка {
                лог.втолкнуть("вн-финалли");
            }
        }
        пиздюли внеш() {
            поебалуна вн();
            поебалу "после";
        }
        гыы ит = внеш();
        ит.следующий();
        гыы р = ит.вернуть("СТОП");
        гыы зн = р.значение;
        гыы гт = р.готово;
        "#,
    );
    assert_struct_eq(i.get("лог"), Value::array(vec![Value::String("вн-финалли".into())]));
    assert_eq!(i.get("зн"), Some(Value::String("СТОП".into())));
    assert_eq!(i.get("гт"), Some(Value::Boolean(true)));
}

#[test]
fn generator_yield_delegate_throw_recovers_in_inner_catch() {
    let i = run_code(
        r#"
        гыы лог = [];
        пиздюли вн() {
            хапнуть {
                поебалу "i1";
                поебалу "i2";
            } гоп (e) {
                лог.втолкнуть(e);
                поебалу "восстановлен";
            }
        }
        пиздюли внеш() {
            поебалуна вн();
            поебалу "после";
        }
        гыы ит = внеш();
        ит.следующий();
        гыы р = ит.кинуть("БАХ");
        гыы зн = р.значение;
        гыы гт = р.готово;
        гыы дальше = ит.следующий().значение;
        "#,
    );
    assert_struct_eq(i.get("лог"), Value::array(vec![Value::String("БАХ".into())]));
    assert_eq!(i.get("зн"), Some(Value::String("восстановлен".into())));
    assert_eq!(i.get("гт"), Some(Value::Boolean(false)));
    assert_eq!(i.get("дальше"), Some(Value::String("после".into())));
}

#[test]
fn generator_throw_in_finally_overrides_gen_return() {
    let err = run_code_err(
        r#"
        пиздюли ген() {
            хапнуть {
                поебалу 1;
            } тюряжка {
                кидай "из-финалли";
            }
        }
        гыы г = ген();
        г.следующий();
        г.вернуть(99);
        "#,
    );
    assert_eq!(err.thrown, Some(Box::new(Value::String("из-финалли".into()))));
}

#[test]
fn generator_yield_delegate_return_value_captured_in_decl() {
    let i = run_code(
        r#"
        пиздюли вн() {
            поебалу 1;
            отвечаю "итог";
        }
        пиздюли внеш() {
            гыы р = поебалуна вн();
            поебалу р;
        }
        гыы ит = внеш();
        гыы первое = ит.следующий().значение;
        гыы второе = ит.следующий().значение;
        "#,
    );
    assert_eq!(i.get("первое"), Some(Value::Number(1.0)));
    assert_eq!(i.get("второе"), Some(Value::String("итог".into())));
}

#[test]
fn generator_yield_delegate_return_value_captured_with_const() {
    let i = run_code(
        r#"
        пиздюли вн() {
            поебалу 1;
            отвечаю 7;
        }
        пиздюли внеш() {
            ясенХуй р = поебалуна вн();
            поебалу р;
        }
        гыы ит = внеш();
        ит.следующий();
        гыы захвачено = ит.следующий().значение;
        "#,
    );
    assert_eq!(i.get("захвачено"), Some(Value::Number(7.0)));
}

#[test]
fn async_generator_for_await_collects_values() {
    let i = run_code(
        r#"
        гыы лог = [];
        ассо пиздюли ген() {
            поебалу 1;
            поебалу 2;
            поебалу 3;
        }
        ассо йопта потр() {
            го сидетьНахуй (гыы х сашаГрей ген()) {
                лог.push(х);
            }
        }
        потр();
        "#,
    );
    match i.get("лог") {
        Some(Value::Array(a)) => {
            let b = a.borrow();
            assert_eq!(b.0, vec![Value::Number(1.0), Value::Number(2.0), Value::Number(3.0)]);
        }
        other => panic!("Ожидался массив, получено {other:?}"),
    }
}

#[test]
fn async_generator_await_and_yield_interleave() {
    let i = run_code(
        r#"
        гыы лог = [];
        ассо йопта удвоить(н) { отвечаю н * 2; }
        ассо пиздюли ген() {
            гыы а = сидетьНахуй удвоить(1);
            поебалу а;
            гыы б = сидетьНахуй удвоить(а);
            поебалу б;
        }
        ассо йопта потр() {
            го сидетьНахуй (гыы х сашаГрей ген()) {
                лог.push(х);
            }
        }
        потр();
        "#,
    );
    match i.get("лог") {
        Some(Value::Array(a)) => {
            assert_eq!(a.borrow().0, vec![Value::Number(2.0), Value::Number(4.0)]);
        }
        other => panic!("Ожидался массив, получено {other:?}"),
    }
}

#[test]
fn async_generator_next_returns_promise() {
    let i = run_code(
        r#"
        ассо пиздюли ген() { поебалу 1; }
        гыы ит = ген();
        гыы т = тип(ит.следующий());
        "#,
    );
    assert_eq!(i.get("т"), Some(Value::String("обещание".into())));
}

#[test]
fn for_await_over_sync_iterable_of_promises() {
    let i = run_code(
        r#"
        гыы сумма = 0;
        ассо йопта потр() {
            гыы обещания = [СловоПацана.решить(1), СловоПацана.решить(2), СловоПацана.решить(3)];
            го сидетьНахуй (гыы х сашаГрей обещания) {
                сумма += х;
            }
        }
        потр();
        "#,
    );
    assert_eq!(i.get("сумма"), Some(Value::Number(6.0)));
}

#[test]
fn for_await_over_user_async_iterator() {
    let i = run_code(
        r#"
        гыы лог = [];
        гыы объект = {
            [Симбол.асинхИтератор]: йопта() {
                гыы и = 0;
                отвечаю {
                    следующий: ассо йопта() {
                        вилкойвглаз (и < 3) {
                            и += 1;
                            отвечаю { значение: и, готово: лож };
                        }
                        отвечаю { значение: ноль, готово: правда };
                    }
                };
            }
        };
        ассо йопта потр() {
            го сидетьНахуй (гыы х сашаГрей объект) {
                лог.push(х);
            }
        }
        потр();
        "#,
    );
    match i.get("лог") {
        Some(Value::Array(a)) => {
            assert_eq!(a.borrow().0, vec![Value::Number(1.0), Value::Number(2.0), Value::Number(3.0)]);
        }
        other => panic!("Ожидался массив, получено {other:?}"),
    }
}

#[test]
fn async_generator_error_propagates_to_consumer_catch() {
    let i = run_code(
        r#"
        гыы поймано = "";
        ассо пиздюли ген() {
            поебалу 1;
            кидай "бабах";
        }
        ассо йопта потр() {
            хапнуть {
                го сидетьНахуй (гыы х сашаГрей ген()) {}
            } гоп (е) {
                поймано = е;
            }
        }
        потр();
        "#,
    );
    assert_eq!(i.get("поймано"), Some(Value::String("бабах".into())));
}

#[test]
fn async_generator_break_runs_finally() {
    let i = run_code(
        r#"
        гыы лог = [];
        ассо пиздюли ген() {
            хапнуть {
                поебалу 1;
                поебалу 2;
                поебалу 3;
            } тюряжка {
                лог.push("финал");
            }
        }
        ассо йопта потр() {
            го сидетьНахуй (гыы х сашаГрей ген()) {
                лог.push(х);
                вилкойвглаз (х === 2) { харэ; }
            }
        }
        потр();
        "#,
    );
    match i.get("лог") {
        Some(Value::Array(a)) => {
            assert_eq!(a.borrow().0, vec![Value::Number(1.0), Value::Number(2.0), Value::String("финал".into())]);
        }
        other => panic!("Ожидался массив, получено {other:?}"),
    }
}

#[test]
fn async_generator_yield_delegate_sync_iterable() {
    let i = run_code(
        r#"
        гыы лог = [];
        ассо пиздюли ген() {
            поебалуна [1, 2];
            поебалу 3;
        }
        ассо йопта потр() {
            го сидетьНахуй (гыы х сашаГрей ген()) {
                лог.push(х);
            }
        }
        потр();
        "#,
    );
    match i.get("лог") {
        Some(Value::Array(a)) => {
            assert_eq!(a.borrow().0, vec![Value::Number(1.0), Value::Number(2.0), Value::Number(3.0)]);
        }
        other => panic!("Ожидался массив, получено {other:?}"),
    }
}

#[test]
fn async_generator_function_expression() {
    let i = run_code(
        r#"
        гыы лог = [];
        гыы ген = ассо пиздюли() {
            поебалу 10;
            поебалу 20;
        };
        ассо йопта потр() {
            го сидетьНахуй (гыы х сашаГрей ген()) {
                лог.push(х);
            }
        }
        потр();
        "#,
    );
    match i.get("лог") {
        Some(Value::Array(a)) => {
            assert_eq!(a.borrow().0, vec![Value::Number(10.0), Value::Number(20.0)]);
        }
        other => panic!("Ожидался массив, получено {other:?}"),
    }
}

#[test]
fn async_generator_sync_iteration_rejected() {
    let err = run_code_err(
        r#"
        ассо пиздюли ген() { поебалу 1; }
        гыы а = [...ген()];
        "#,
    );
    assert!(err.message.contains("синхронно"), "неожиданное сообщение: {}", err.message);
}
