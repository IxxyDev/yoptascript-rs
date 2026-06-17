use super::*;

#[test]
fn switch_matches_first_case() {
    let interp = run_code(
        r#"
        гыы результат = 0;
        базарпо (1) {
            тема 1: {
                результат = 10;
            }
            тема 2: {
                результат = 20;
            }
        }
        "#,
    );
    assert_eq!(interp.get("результат"), Some(Value::Number(10.0)));
}

#[test]
fn switch_matches_second_case() {
    let interp = run_code(
        r#"
        гыы результат = 0;
        базарпо (2) {
            тема 1: {
                результат = 10;
            }
            тема 2: {
                результат = 20;
            }
        }
        "#,
    );
    assert_eq!(interp.get("результат"), Some(Value::Number(20.0)));
}

#[test]
fn switch_default_when_no_match() {
    let interp = run_code(
        r#"
        гыы результат = 0;
        базарпо (99) {
            тема 1: {
                результат = 10;
            }
            нуичо {
                результат = 42;
            }
        }
        "#,
    );
    assert_eq!(interp.get("результат"), Some(Value::Number(42.0)));
}

#[test]
fn switch_no_match_no_default() {
    let interp = run_code(
        r#"
        гыы результат = 0;
        базарпо (99) {
            тема 1: {
                результат = 10;
            }
        }
        "#,
    );
    assert_eq!(interp.get("результат"), Some(Value::Number(0.0)));
}

#[test]
fn switch_with_string_cases() {
    let interp = run_code(
        r#"
        гыы результат = "";
        базарпо ("привет") {
            тема "пока": {
                результат = "прощание";
            }
            тема "привет": {
                результат = "приветствие";
            }
        }
        "#,
    );
    assert_eq!(interp.get("результат"), Some(Value::String("приветствие".to_string())));
}

#[test]
fn switch_with_variable_expr() {
    let interp = run_code(
        r#"
        гыы х = 3;
        гыы результат = 0;
        базарпо (х) {
            тема 1: {
                результат = 10;
            }
            тема 3: {
                результат = 30;
            }
            нуичо {
                результат = 99;
            }
        }
        "#,
    );
    assert_eq!(interp.get("результат"), Some(Value::Number(30.0)));
}

#[test]
fn switch_no_fallthrough() {
    let interp = run_code(
        r#"
        гыы результат = 0;
        базарпо (1) {
            тема 1: {
                результат = результат + 10;
            }
            тема 2: {
                результат = результат + 20;
            }
        }
        "#,
    );
    assert_eq!(interp.get("результат"), Some(Value::Number(10.0)));
}

#[test]
fn switch_default_only() {
    let interp = run_code(
        r#"
        гыы результат = 0;
        базарпо (1) {
            нуичо {
                результат = 42;
            }
        }
        "#,
    );
    assert_eq!(interp.get("результат"), Some(Value::Number(42.0)));
}

#[test]
fn switch_with_return_in_function() {
    let interp = run_code(
        r#"
        йопта проверка(х) {
            базарпо (х) {
                тема 1: {
                    отвечаю 10;
                }
                тема 2: {
                    отвечаю 20;
                }
                нуичо {
                    отвечаю 0;
                }
            }
        }
        гыы а = проверка(1);
        гыы б = проверка(2);
        гыы в = проверка(99);
        "#,
    );
    assert_eq!(interp.get("а"), Some(Value::Number(10.0)));
    assert_eq!(interp.get("б"), Some(Value::Number(20.0)));
    assert_eq!(interp.get("в"), Some(Value::Number(0.0)));
}

#[test]
fn do_while_executes_at_least_once() {
    let interp = run_code(
        r#"
        гыы счётчик = 0;
        крутани {
            счётчик = счётчик + 1;
        } потрещим (лож);
        "#,
    );
    assert_eq!(interp.get("счётчик"), Some(Value::Number(1.0)));
}

#[test]
fn do_while_loops_while_true() {
    let interp = run_code(
        r#"
        гыы счётчик = 0;
        крутани {
            счётчик = счётчик + 1;
        } потрещим (счётчик < 5);
        "#,
    );
    assert_eq!(interp.get("счётчик"), Some(Value::Number(5.0)));
}

#[test]
fn do_while_break() {
    let interp = run_code(
        r#"
        гыы счётчик = 0;
        крутани {
            счётчик = счётчик + 1;
            вилкойвглаз (счётчик == 3) {
                харэ;
            }
        } потрещим (счётчик < 10);
        "#,
    );
    assert_eq!(interp.get("счётчик"), Some(Value::Number(3.0)));
}

#[test]
fn do_while_continue() {
    let interp = run_code(
        r#"
        гыы счётчик = 0;
        гыы сумма = 0;
        крутани {
            счётчик = счётчик + 1;
            вилкойвглаз (счётчик == 3) {
                двигай;
            }
            сумма = сумма + счётчик;
        } потрещим (счётчик < 5);
        "#,
    );
    assert_eq!(interp.get("счётчик"), Some(Value::Number(5.0)));
    assert_eq!(interp.get("сумма"), Some(Value::Number(12.0)));
}

#[test]
fn do_while_with_return() {
    let interp = run_code(
        r#"
        йопта сумма() {
            гыы с = 0;
            гыы и = 0;
            крутани {
                и = и + 1;
                с = с + и;
                вилкойвглаз (и == 3) {
                    отвечаю с;
                }
            } потрещим (правда);
        }
        гыы результат = сумма();
        "#,
    );
    assert_eq!(interp.get("результат"), Some(Value::Number(6.0)));
}

#[test]
fn for_in_array() {
    let interp = run_code(
        r#"
        гыы сумма = 0;
        гыы арр = [1, 2, 3, 4];
        го (х из арр) {
            сумма = сумма + х;
        }
        "#,
    );
    assert_eq!(interp.get("сумма"), Some(Value::Number(10.0)));
}

#[test]
fn for_in_empty_array() {
    let interp = run_code(
        r#"
        гыы сумма = 0;
        го (х из []) {
            сумма = сумма + 1;
        }
        "#,
    );
    assert_eq!(interp.get("сумма"), Some(Value::Number(0.0)));
}

#[test]
fn for_in_object_keys() {
    let interp = run_code(
        r#"
        гыы счётчик = 0;
        гыы чел = { имя: "Вася", возраст: 25 };
        го (к из чел) {
            счётчик = счётчик + 1;
        }
        "#,
    );
    assert_eq!(interp.get("счётчик"), Some(Value::Number(2.0)));
}

#[test]
fn for_in_break() {
    let interp = run_code(
        r#"
        гыы сумма = 0;
        го (х из [10, 20, 30, 40]) {
            сумма = сумма + х;
            вилкойвглаз (х == 20) {
                харэ;
            }
        }
        "#,
    );
    assert_eq!(interp.get("сумма"), Some(Value::Number(30.0)));
}

#[test]
fn for_in_continue() {
    let interp = run_code(
        r#"
        гыы сумма = 0;
        го (х из [1, 2, 3, 4, 5]) {
            вилкойвглаз (х == 3) {
                двигай;
            }
            сумма = сумма + х;
        }
        "#,
    );
    assert_eq!(interp.get("сумма"), Some(Value::Number(12.0)));
}

#[test]
fn for_in_with_return() {
    let interp = run_code(
        r#"
        йопта найти(арр) {
            го (х из арр) {
                вилкойвглаз (х > 3) {
                    отвечаю х;
                }
            }
            отвечаю 0;
        }
        гыы результат = найти([1, 2, 5, 4]);
        "#,
    );
    assert_eq!(interp.get("результат"), Some(Value::Number(5.0)));
}

#[test]
fn for_in_non_iterable_fails() {
    let err = run_code_err(
        r#"
        го (х из 42) {
            гыы а = 1;
        }
        "#,
    );
    assert!(err.message.contains("итерировать"));
}

#[test]
fn for_in_string_array() {
    let interp = run_code(
        r#"
        гыы результат = "";
        го (с из ["а", "б", "в"]) {
            результат = результат + с;
        }
        "#,
    );
    assert_eq!(interp.get("результат"), Some(Value::String("абв".to_string())));
}

#[test]
fn for_of_array() {
    let i = run_code(
        r#"
        гыы сумма = 0;
        гыы массив = [1, 2, 3, 4, 5];
        го (элем сашаГрей массив) {
            сумма = сумма + элем;
        }
        "#,
    );
    assert_eq!(i.get("сумма"), Some(Value::Number(15.0)));
}

#[test]
fn for_of_string() {
    let i = run_code(
        r#"
        гыы рез = "";
        го (ч сашаГрей "abc") {
            рез = рез + ч + "-";
        }
        "#,
    );
    assert_eq!(i.get("рез"), Some(Value::String("a-b-c-".to_string())));
}

#[test]
fn for_of_with_break() {
    let i = run_code(
        r#"
        гыы рез = 0;
        го (э сашаГрей [1, 2, 3, 4, 5]) {
            вилкойвглаз (э === 4) {
                харэ;
            }
            рез = рез + э;
        }
        "#,
    );
    assert_eq!(i.get("рез"), Some(Value::Number(6.0)));
}

#[test]
fn for_of_with_continue() {
    let i = run_code(
        r#"
        гыы рез = 0;
        го (э сашаГрей [1, 2, 3, 4, 5]) {
            вилкойвглаз (э === 3) {
                двигай;
            }
            рез = рез + э;
        }
        "#,
    );
    assert_eq!(i.get("рез"), Some(Value::Number(12.0)));
}

#[test]
fn break_to_unknown_label_errors() {
    let err = run_code_err(
        r#"
        го (гыы и = 0; и < 3; и += 1) {
            харэ чужая;
        }
        "#,
    );
    assert!(
        err.message.contains("Метка 'чужая' не найдена"),
        "ожидалась ошибка о ненайденной метке, got: {}",
        err.message
    );
}

#[test]
fn hoisting_call_before_declaration() {
    let interp = run_code(
        r#"
        гыы результат = удвоить(21);
        йопта удвоить(х) { отвечаю х * 2; }
        "#,
    );
    assert_eq!(interp.get("результат"), Some(Value::Number(42.0)));
}

#[test]
fn hoisting_inside_function_body() {
    let interp = run_code(
        r#"
        йопта внешняя() {
            отвечаю помощник(10);
            йопта помощник(х) { отвечаю х + 1; }
        }
        гыы результат = внешняя();
        "#,
    );
    assert_eq!(interp.get("результат"), Some(Value::Number(11.0)));
}

#[test]
fn hoisting_mutual_recursion_before_decls() {
    let interp = run_code(
        r#"
        гыы результат = чёт(4);
        йопта чёт(н) { вилкойвглаз (н == 0) { отвечаю правда; } отвечаю нечёт(н - 1); }
        йопта нечёт(н) { вилкойвглаз (н == 0) { отвечаю лож; } отвечаю чёт(н - 1); }
        "#,
    );
    assert_eq!(interp.get("результат"), Some(Value::Boolean(true)));
}

#[test]
fn hoisting_in_block_scope() {
    let interp = run_code(
        r#"
        гыы снаружи = 0;
        {
            снаружи = тройка();
            йопта тройка() { отвечаю 3; }
        }
        "#,
    );
    assert_eq!(interp.get("снаружи"), Some(Value::Number(3.0)));
}

#[test]
fn labeled_break_exits_outer_loop() {
    let interp = run_code(
        r#"
        гыы счёт = 0;
        внешний: го (гыы и = 0; и < 5; и += 1) {
            го (гыы ж = 0; ж < 5; ж += 1) {
                счёт += 1;
                вилкойвглаз (ж == 1) {
                    харэ внешний;
                }
            }
        }
        "#,
    );
    assert_eq!(interp.get("счёт"), Some(Value::Number(2.0)));
}

#[test]
fn labeled_continue_continues_outer_loop() {
    let interp = run_code(
        r#"
        гыы счёт = 0;
        внешний: го (гыы и = 0; и < 3; и += 1) {
            го (гыы ж = 0; ж < 5; ж += 1) {
                счёт += 1;
                двигай внешний;
            }
        }
        "#,
    );
    assert_eq!(interp.get("счёт"), Some(Value::Number(3.0)));
}

#[test]
fn labeled_break_from_block() {
    let interp = run_code(
        r#"
        гыы счёт = 0;
        блок: {
            счёт += 1;
            харэ блок;
            счёт += 100;
        }
        "#,
    );
    assert_eq!(interp.get("счёт"), Some(Value::Number(1.0)));
}

#[test]
fn unlabeled_break_still_breaks_inner_only() {
    let interp = run_code(
        r#"
        гыы счёт = 0;
        го (гыы и = 0; и < 3; и += 1) {
            го (гыы ж = 0; ж < 5; ж += 1) {
                вилкойвглаз (ж == 1) { харэ; }
                счёт += 1;
            }
        }
        "#,
    );
    assert_eq!(interp.get("счёт"), Some(Value::Number(3.0)));
}

#[test]
fn unlabeled_break_in_generator_still_works() {
    let i = run_code(
        r#"
        пиздюли ген() {
            го (гыы к = 0; к < 5; к += 1) {
                вилкойвглаз (к == 2) { харэ; }
                поебалу к;
            }
            поебалу 99;
        }
        гыы рез = [];
        го (гыы х сашаГрей ген()) {
            рез.втолкнуть(х);
        }
        "#,
    );
    assert_struct_eq(i.get("рез"), Value::array(vec![Value::Number(0.0), Value::Number(1.0), Value::Number(99.0)]));
}

#[test]
fn labeled_break_in_generator_errors() {
    let err = run_code_err(
        r#"
        пиздюли ген() {
            метка: потрещим (правда) {
                харэ метка;
            }
        }
        гыы г = ген();
        г.следующий();
        "#,
    );
    assert!(
        err.message.contains("Маркированный 'харэ'"),
        "ожидалась ошибка о маркированном харэ в генераторе, got: {}",
        err.message
    );
}

#[test]
fn for_loop_closures_capture_per_iteration() {
    let interp = run_code(
        r#"
        гыы фс = [];
        го (гыы и = 0; и < 3; и += 1) { фс.втолкнуть(() => и); }
        гыы р = [фс[0](), фс[1](), фс[2]()];
        "#,
    );
    assert_struct_eq(interp.get("р"), Value::array(vec![Value::Number(0.0), Value::Number(1.0), Value::Number(2.0)]));
}

#[test]
fn for_of_closures_capture_per_iteration() {
    let interp = run_code(
        r#"
        гыы гс = [];
        го (гыы х сашаГрей [10, 20, 30]) { гс.втолкнуть(() => х); }
        гыы р = [гс[0](), гс[1](), гс[2]()];
        "#,
    );
    assert_struct_eq(
        interp.get("р"),
        Value::array(vec![Value::Number(10.0), Value::Number(20.0), Value::Number(30.0)]),
    );
}

#[test]
fn for_loop_continue_capture_skips_iteration() {
    let interp = run_code(
        r#"
        гыы фс = [];
        го (гыы и = 0; и < 4; и += 1) { вилкойвглаз (и === 1) { двигай; } фс.втолкнуть(() => и); }
        гыы р = [фс[0](), фс[1](), фс[2]()];
        "#,
    );
    assert_struct_eq(interp.get("р"), Value::array(vec![Value::Number(0.0), Value::Number(2.0), Value::Number(3.0)]));
}

#[test]
fn for_loop_sum_still_accumulates() {
    let interp = run_code(
        r#"
        гыы с = 0;
        го (гыы и = 0; и < 5; и += 1) { с += и; }
        "#,
    );
    assert_eq!(interp.get("с"), Some(Value::Number(10.0)));
}
