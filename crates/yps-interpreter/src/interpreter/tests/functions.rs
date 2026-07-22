use super::*;

#[test]
fn arrow_function_expr_body() {
    let interp = run_code(
        r#"
        гыы двойное = (х) => х * 2;
        гыы р = двойное(5);
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::Number(10.0)));
}

#[test]
fn arrow_function_block_body() {
    let interp = run_code(
        r#"
        гыы сумма = (а, б) => {
            отвечаю а + б;
        };
        гыы р = сумма(3, 4);
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::Number(7.0)));
}

#[test]
fn arrow_function_no_params() {
    let interp = run_code(
        r#"
        гыы привет = () => "здарова";
        гыы р = привет();
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::String("здарова".into())));
}

#[test]
fn arrow_function_single_param_no_parens() {
    let interp = run_code(
        r#"
        гыы квадрат = х => х * х;
        гыы р = квадрат(6);
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::Number(36.0)));
}

#[test]
fn arrow_function_as_callback() {
    let interp = run_code(
        r#"
        йопта применить(ф, знач) {
            отвечаю ф(знач);
        }
        гыы р = применить((х) => х + 10, 5);
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::Number(15.0)));
}

#[test]
fn arrow_function_iife() {
    let interp = run_code(
        r#"
        гыы р = ((а, б) => а * б)(3, 7);
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::Number(21.0)));
}

#[test]
fn closure_captures_variable() {
    let interp = run_code(
        r#"
        йопта создать() {
            гыы н = 0;
            отвечаю () => {
                н = н + 1;
                отвечаю н;
            };
        }
        гыы инкр = создать();
        гыы а = инкр();
        гыы б = инкр();
        гыы в = инкр();
        "#,
    );
    assert_eq!(interp.get("а"), Some(Value::Number(1.0)));
    assert_eq!(interp.get("б"), Some(Value::Number(2.0)));
    assert_eq!(interp.get("в"), Some(Value::Number(3.0)));
}

#[test]
fn closure_independent_instances() {
    let interp = run_code(
        r#"
        йопта счётчик() {
            гыы н = 0;
            отвечаю () => {
                н = н + 1;
                отвечаю н;
            };
        }
        гыы а = счётчик();
        гыы б = счётчик();
        а();
        а();
        б();
        гыы ра = а();
        гыы рб = б();
        "#,
    );
    assert_eq!(interp.get("ра"), Some(Value::Number(3.0)));
    assert_eq!(interp.get("рб"), Some(Value::Number(2.0)));
}

#[test]
fn closure_captures_outer_scope() {
    let interp = run_code(
        r#"
        гыы х = 10;
        йопта создать() {
            отвечаю () => х;
        }
        гыы получить = создать();
        гыы р = получить();
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::Number(10.0)));
}

#[test]
fn closure_sees_mutations_of_shared_variable() {
    let interp = run_code(
        r#"
        йопта создать() {
            гыы н = 0;
            гыы инкр = () => {
                н = н + 1;
            };
            гыы получить = () => н;
            отвечаю [инкр, получить];
        }
        гыы пара = создать();
        гыы инкр = пара[0];
        гыы получить = пара[1];
        инкр();
        инкр();
        гыы р = получить();
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::Number(2.0)));
}

#[test]
fn closure_in_loop() {
    let interp = run_code(
        r#"
        гыы функции = [];
        го (гыы и = 0; и < 3; и++) {
            гыы текущий = и;
            функции = втолкнуть(функции, () => текущий);
        }
        гыы а = функции[0]();
        гыы б = функции[1]();
        гыы в = функции[2]();
        "#,
    );
    assert_eq!(interp.get("а"), Some(Value::Number(0.0)));
    assert_eq!(interp.get("б"), Some(Value::Number(1.0)));
    assert_eq!(interp.get("в"), Some(Value::Number(2.0)));
}

#[test]
fn nested_closure() {
    let interp = run_code(
        r#"
        йопта внешняя(х) {
            отвечаю (у) => {
                отвечаю () => х + у;
            };
        }
        гыы ф = внешняя(10)(20);
        гыы р = ф();
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::Number(30.0)));
}

#[test]
fn default_param_used_when_no_arg() {
    let interp = run_code(
        r#"
        йопта приветствие(имя = "мир") {
            отвечаю имя;
        }
        гыы р = приветствие();
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::String("мир".into())));
}

#[test]
fn default_param_overridden_by_arg() {
    let interp = run_code(
        r#"
        йопта приветствие(имя = "мир") {
            отвечаю имя;
        }
        гыы р = приветствие("братан");
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::String("братан".into())));
}

#[test]
fn default_param_multiple() {
    let interp = run_code(
        r#"
        йопта сумма(а, б = 10, в = 20) {
            отвечаю а + б + в;
        }
        гыы р1 = сумма(1);
        гыы р2 = сумма(1, 2);
        гыы р3 = сумма(1, 2, 3);
        "#,
    );
    assert_eq!(interp.get("р1"), Some(Value::Number(31.0)));
    assert_eq!(interp.get("р2"), Some(Value::Number(23.0)));
    assert_eq!(interp.get("р3"), Some(Value::Number(6.0)));
}

#[test]
fn default_param_expression() {
    let interp = run_code(
        r#"
        йопта фн(а = 2 + 3) {
            отвечаю а;
        }
        гыы р = фн();
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::Number(5.0)));
}

#[test]
fn default_param_arrow_function() {
    let interp = run_code(
        r#"
        гыы фн = (а = 42) => { отвечаю а; };
        гыы р = фн();
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::Number(42.0)));
}

#[test]
fn rest_param_collects_extra_args() {
    let interp = run_code(
        r#"
        йопта фн(а, ...остальное) {
            отвечаю остальное;
        }
        гыы р = фн(1, 2, 3, 4);
        "#,
    );
    assert_struct_eq(interp.get("р"), Value::array(vec![Value::Number(2.0), Value::Number(3.0), Value::Number(4.0)]));
}

#[test]
fn rest_param_empty_when_no_extra_args() {
    let interp = run_code(
        r#"
        йопта фн(а, ...остальное) {
            отвечаю остальное;
        }
        гыы р = фн(1);
        "#,
    );
    assert_struct_eq(interp.get("р"), Value::array(vec![]));
}

#[test]
fn rest_param_only() {
    let interp = run_code(
        r#"
        йопта фн(...все) {
            отвечаю все;
        }
        гыы р = фн(1, 2, 3);
        "#,
    );
    assert_struct_eq(interp.get("р"), Value::array(vec![Value::Number(1.0), Value::Number(2.0), Value::Number(3.0)]));
}

#[test]
fn rest_param_arrow_function() {
    let interp = run_code(
        r#"
        гыы фн = (...арг) => { отвечаю арг; };
        гыы р = фн(10, 20);
        "#,
    );
    assert_struct_eq(interp.get("р"), Value::array(vec![Value::Number(10.0), Value::Number(20.0)]));
}

#[test]
fn default_and_rest_params_combined() {
    let interp = run_code(
        r#"
        йопта фн(а, б = 99, ...ост) {
            отвечаю а + б;
        }
        гыы р1 = фн(1);
        гыы р2 = фн(1, 2);
        гыы р3 = фн(1, 2, 3, 4);
        "#,
    );
    assert_eq!(interp.get("р1"), Some(Value::Number(100.0)));
    assert_eq!(interp.get("р2"), Some(Value::Number(3.0)));
    assert_eq!(interp.get("р3"), Some(Value::Number(3.0)));
}

#[test]
fn too_few_args_without_defaults_error() {
    let err = run_code_err(
        r#"
        йопта фн(а, б) {
            отвечаю а + б;
        }
        фн(1);
        "#,
    );
    assert!(err.message.contains("минимум 2"));
}

#[test]
fn extra_args_ignored_like_js() {
    let interp = run_code(
        r#"
        йопта фн(а) {
            отвечаю а;
        }
        гыы р = фн(1, 2, 3);
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::Number(1.0)));
}

#[test]
fn named_function_expr_supports_recursion() {
    let interp = run_code(
        r#"
        гыы ф = йопта фиб(н) { вилкойвглаз (н < 2) { отвечаю н; } отвечаю фиб(н - 1) + фиб(н - 2); };
        гыы рез = ф(10);
        "#,
    );
    assert_eq!(interp.get("рез").unwrap(), Value::Number(55.0));
    assert!(interp.get("фиб").is_none());
}

#[test]
fn named_function_expr_display_uses_name() {
    let interp = run_code("гыы ф = йопта фиб(н) { отвечаю н; };");
    let func = interp.get("ф").unwrap();
    assert_eq!(func.to_string(), "[функция фиб]");
}

#[test]
fn anon_function_expr_still_works() {
    let interp = run_code(
        r#"
        гыы ф = йопта(х) { отвечаю х * 2; };
        гыы рез = ф(21);
        "#,
    );
    assert_eq!(interp.get("рез").unwrap(), Value::Number(42.0));
    assert_eq!(interp.get("ф").unwrap().to_string(), "[анонимная функция]");
}
