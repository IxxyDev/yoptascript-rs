use super::*;

const LEAKY_SRC: &str = r#"
йопта внешняя() {
    йопта внутренняя(н) { отвечаю н; }
    отвечаю внутренняя(1);
}
гыы н = 0;
потрещим (н < 300) { внешняя(); н = н + 1; }
"#;

#[test]
fn collect_cycles_frees_unreachable_closure_frames() {
    let mut i = run_code(LEAKY_SRC);
    let before = i.live_frames();
    assert!(before >= 300, "ожидалась утечка кадров, live_frames = {before}");
    let cleared = i.collect_cycles();
    assert!(cleared >= 300, "ожидалось >= 300 очищенных кадров, получено {cleared}");
    let after = i.live_frames();
    assert!(after < 50, "после сборки осталось {after} кадров");
}

#[test]
fn closure_survives_cycle_collection() {
    let mut i = run_code(
        r#"
        йопта счетчик() {
            гыы х = 0;
            йопта инк() { х = х + 1; отвечаю х; }
            отвечаю инк;
        }
        гыы ф = счетчик();
        ф();
        "#,
    );
    i.collect_cycles();
    let v = run_more(&mut i, "ф();");
    assert_eq!(v, Some(Value::Number(2.0)));
}

#[test]
fn generator_survives_cycle_collection() {
    let mut i = run_code(
        r#"
        йопта делать() {
            гыы база = 10;
            пиздюли г() { поебалу база + 1; поебалу база + 2; }
            отвечаю г();
        }
        гыы ит = делать();
        ит.следующий();
        "#,
    );
    i.collect_cycles();
    let v = run_more(&mut i, "ит.следующий().значение;");
    assert_eq!(v, Some(Value::Number(12.0)));
}

#[test]
fn iterator_map_callback_survives_cycle_collection() {
    let mut i = run_code(
        r#"
        йопта строить() {
            гыы м = 5;
            отвечаю Итератор.от([1, 2]).преобразовать((х) => х * м);
        }
        гыы ит = строить();
        "#,
    );
    i.collect_cycles();
    let v = run_more(&mut i, "ит.вМассив();");
    match v {
        Some(Value::Array(arr)) => {
            assert_eq!(arr.borrow().0.clone(), vec![Value::Number(5.0), Value::Number(10.0)]);
        }
        other => panic!("ожидался массив, получено {other:?}"),
    }
}

#[test]
fn promise_handler_survives_cycle_collection() {
    let mut i = run_code(
        r#"
        гыы рез = 0;
        гыы сохрРеш = ноль;
        ясенХуй п = захуярить СловоПацана((реш, отв) => { сохрРеш = реш; });
        йопта подготовить() {
            гыы добавка = 1;
            п.потом((з) => { рез = з + добавка; });
        }
        подготовить();
        "#,
    );
    i.collect_cycles();
    run_more(&mut i, "сохрРеш(41);");
    assert_eq!(i.get("рез"), Some(Value::Number(42.0)));
}

#[test]
fn class_method_env_survives_cycle_collection() {
    let mut i = run_code(
        r#"
        гыы К = ноль;
        вилкойвглаз (правда) {
            гыы секрет = 7;
            клёво Класс { дай() { отвечаю секрет; } }
            К = Класс;
        }
        гыы к = захуярить К();
        "#,
    );
    i.collect_cycles();
    let v = run_more(&mut i, "к.дай();");
    assert_eq!(v, Some(Value::Number(7.0)));
}

#[test]
fn proxy_trap_env_survives_cycle_collection() {
    let mut i = run_code(
        r#"
        йопта сделать() {
            гыы добавка = 100;
            отвечаю захуярить Посредник({ х: 1 }, { получить: (ц, к, пр) => ц[к] + добавка });
        }
        гыы п = сделать();
        "#,
    );
    i.collect_cycles();
    let v = run_more(&mut i, "п.х;");
    assert_eq!(v, Some(Value::Number(101.0)));
}

#[test]
fn collect_cycles_noop_with_pending_tasks() {
    let mut i = run_code(LEAKY_SRC);
    i.microtasks.push_back(Box::new(|_, _| Ok(())));
    assert_eq!(i.collect_cycles(), 0, "сборка при непустых очередях должна быть no-op");
    i.microtasks.clear();
    assert!(i.collect_cycles() >= 300, "после опустошения очередей сборка должна сработать");
}

#[test]
fn repl_auto_collects_cycles_between_inputs() {
    let mut i = Interpreter::new();
    run_more(&mut i, LEAKY_SRC);
    let live = i.live_frames();
    assert!(live < 100, "REPL не собрал циклы между вводами: live_frames = {live}");
}

#[test]
fn repl_collect_keeps_returned_closure_alive() {
    let mut i = Interpreter::new();
    let src = format!(
        "{LEAKY_SRC}
йопта дай() {{ гыы х = 42; йопта внутр() {{ отвечаю х; }} отвечаю внутр; }}
дай();"
    );
    let v = run_more(&mut i, &src).expect("ожидалось значение");
    let r = i.call_function(v, vec![], yps_lexer::Span { start: 0, end: 0 }).expect("вызов замыкания после сборки");
    assert_eq!(r, Value::Number(42.0));
}
