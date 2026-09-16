use super::{Value, assert_struct_eq, run_code, run_code_err, run_more, run_script};
use crate::Interpreter;

#[test]
fn global_function_recursion() {
    let interp = run_code(
        "йопта фиб(н) { вилкойвглаз (н < 2) { отвечаю н; } отвечаю фиб(н - 1) + фиб(н - 2); }\nгыы вывод = фиб(10);",
    );
    assert_eq!(interp.get("вывод"), Some(Value::Number(55.0)));
}

#[test]
fn local_shadows_global() {
    let interp = run_code("гыы значение = 1;\nйопта фн() { гыы значение = 2; отвечаю значение; }\nгыы вывод = фн();");
    assert_eq!(interp.get("вывод"), Some(Value::Number(2.0)));
}

#[test]
fn parameter_shadows_global() {
    let interp = run_code("гыы значение = 1;\nйопта фн(значение) { отвечаю значение; }\nгыы вывод = фн(42);");
    assert_eq!(interp.get("вывод"), Some(Value::Number(42.0)));
}

#[test]
fn closure_captures_loop_variable() {
    let interp = run_code(
        "гыы функции = [];\nго (гыы и = 0; и < 3; и++) { втолкнуть(функции, йопта() { отвечаю и; }); }\nгыы вывод = функции[0]() + функции[1]() + функции[2]();",
    );
    assert_eq!(interp.get("вывод"), Some(Value::Number(3.0)));
}

#[test]
fn generator_reads_global_across_suspend() {
    let interp = run_code(
        "гыы шаг = 10;\nпиздюли ген() { поебалу шаг; поебалу шаг; }\nгыы рез = [];\nго (гыы х сашаГрей ген()) { рез.втолкнуть(х); шаг = 20; }",
    );
    assert_struct_eq(interp.get("рез"), Value::array(vec![Value::Number(10.0), Value::Number(20.0)]));
}

#[test]
fn hoisted_function_called_from_nested_scope() {
    let interp =
        run_code("йопта внешняя() { отвечаю помощник(); йопта помощник() { отвечаю 7; } }\nгыы вывод = внешняя();");
    assert_eq!(interp.get("вывод"), Some(Value::Number(7.0)));
}

#[test]
fn reassigned_global_is_read_live_inside_function() {
    let interp =
        run_code("гыы счётчик = 1;\nйопта читать() { отвечаю счётчик; }\nсчётчик = 99;\nгыы вывод = читать();");
    assert_eq!(interp.get("вывод"), Some(Value::Number(99.0)));
}

#[test]
fn destructuring_locals_do_not_leak_to_global_read() {
    let interp =
        run_code("гыы ключ = 100;\nйопта фн() { гыы { ключ } = { ключ: 5 }; отвечаю ключ; }\nгыы вывод = фн();");
    assert_eq!(interp.get("вывод"), Some(Value::Number(5.0)));
}

#[test]
fn block_local_read_before_declaration_is_tdz_error() {
    let err = run_code_err(
        "гыы значение = 1;\nйопта фн() { { гыы промежуточное = значение; гыы значение = 2; отвечаю промежуточное; } }\nгыы вывод = фн();",
    );
    assert!(err.message.contains("до её инициализации"), "got: {}", err.message);
    assert!(err.message.contains("значение"), "got: {}", err.message);
}

#[test]
fn function_body_read_before_declaration_is_tdz_error() {
    let err = run_code_err("гыы значение = 1;\nйопта фн() { отвечаю значение; гыы значение = 2; }\nфн();");
    assert!(err.message.contains("до её инициализации"), "got: {}", err.message);
}

#[test]
fn arrow_body_read_before_declaration_is_tdz_error() {
    let err = run_code_err("гыы значение = 1;\nгыы фн = () => { отвечаю значение; гыы значение = 2; };\nфн();");
    assert!(err.message.contains("до её инициализации"), "got: {}", err.message);
}

#[test]
fn async_body_read_before_declaration_is_tdz_error() {
    let err = run_code_err("гыы значение = 1;\nассо йопта фн() { отвечаю значение; гыы значение = 2; }\nфн();");
    assert!(err.message.contains("до её инициализации"), "got: {}", err.message);
}

#[test]
fn generator_body_read_before_declaration_is_tdz_error() {
    let err = run_code_err(
        "гыы значение = 1;\nпиздюли ген() { поебалу значение; гыы значение = 2; }\nго (гыы _ сашаГрей ген()) {}",
    );
    assert!(err.message.contains("до её инициализации"), "got: {}", err.message);
}

#[test]
fn method_body_read_before_declaration_is_tdz_error() {
    let err = run_code_err(
        "гыы значение = 1;\nклево К { метод() { отвечаю значение; гыы значение = 2; } }\nзахуярить К().метод();",
    );
    assert!(err.message.contains("до её инициализации"), "got: {}", err.message);
}

#[test]
fn nested_block_after_await_is_tdz_error() {
    let err = run_code_err(
        r#"
        ассо йопта главная() {
            сидетьНахуй 1;
            { сказать(у); гыы у = 2; }
        }
        главная();
        "#,
    );
    assert!(err.message.contains("до её инициализации"), "got: {}", err.message);
    assert!(err.message.contains('у'), "got: {}", err.message);
}

#[test]
fn nested_block_in_generator_is_tdz_error() {
    let err = run_code_err(
        r#"
        пиздюли ген() {
            { сказать(у); гыы у = 2; }
            поебалу 1;
        }
        го (гыы _ сашаГрей ген()) {}
        "#,
    );
    assert!(err.message.contains("до её инициализации"), "got: {}", err.message);
}

#[test]
fn loop_body_after_await_is_tdz_error() {
    let err = run_code_err(
        r#"
        ассо йопта главная() {
            сидетьНахуй 1;
            го (гыы и = 0; и < 1; и += 1) { сказать(з); гыы з = 1; }
        }
        главная();
        "#,
    );
    assert!(err.message.contains("до её инициализации"), "got: {}", err.message);
}

#[test]
fn catch_block_read_before_declaration_is_tdz_error() {
    let err = run_code_err(
        r#"
        йопта фн() {
            хапнуть { кидай "бум"; } гоп (е) { сказать(з); гыы з = 1; }
        }
        фн();
        "#,
    );
    assert!(err.message.contains("до её инициализации"), "got: {}", err.message);
}

#[test]
fn block_local_read_after_declaration_shadows_outer() {
    let interp =
        run_code("гыы значение = 1;\nйопта фн() { { гыы значение = 2; отвечаю значение; } }\nгыы вывод = фн();");
    assert_eq!(interp.get("вывод"), Some(Value::Number(2.0)));
}

#[test]
fn outer_read_in_block_without_local_declaration_works() {
    let interp = run_code("гыы значение = 1;\nйопта фн() { { отвечаю значение; } }\nгыы вывод = фн();");
    assert_eq!(interp.get("вывод"), Some(Value::Number(1.0)));
}

#[test]
fn typeof_block_local_before_declaration_is_tdz_error() {
    let err = run_code_err("{ гыы вид = чезажижан значение; гыы значение = 2; }");
    assert!(err.message.contains("до её инициализации"), "got: {}", err.message);
}

#[test]
fn local_shadows_builtin_name() {
    let interp = run_code("йопта фн() { гыы длина = 42; отвечаю длина; }\nгыы вывод = фн();");
    assert_eq!(interp.get("вывод"), Some(Value::Number(42.0)));
}

#[test]
fn repl_function_reads_global_from_earlier_input() {
    let mut interp = Interpreter::new();
    run_more(&mut interp, "гыы общий = 5;");
    run_more(&mut interp, "йопта читать() { отвечаю общий; }");
    let out = run_more(&mut interp, "читать();");
    assert_eq!(out, Some(Value::Number(5.0)));
}

#[test]
fn repl_redeclared_global_updates_function_view() {
    let mut interp = Interpreter::new();
    run_more(&mut interp, "гыы общий = 1;");
    run_more(&mut interp, "йопта читать() { отвечаю общий; }");
    run_more(&mut interp, "общий = 2;");
    let out = run_more(&mut interp, "читать();");
    assert_eq!(out, Some(Value::Number(2.0)));
}

#[test]
fn let_redeclares_const_builtin_and_is_assignable() {
    let i = run_code("гыы строка = \"привет\"; строка = \"мир\";");
    assert_eq!(i.get("строка"), Some(Value::String("мир".into())));
}

#[test]
fn inner_let_shadows_outer_const_and_is_assignable() {
    let i = run_code(
        r#"
        ясенХуй к = 1;
        гыы рез = 0;
        йопта ф() {
            гыы к = 2;
            к = 3;
            отвечаю к;
        }
        рез = ф();
        "#,
    );
    assert_eq!(i.get("рез"), Some(Value::Number(3.0)));
    assert_eq!(i.get("к"), Some(Value::Number(1.0)));
}

#[test]
fn outer_const_still_protected_after_inner_shadow() {
    let err = run_code_err(
        r#"
        ясенХуй к = 1;
        йопта ф() {
            гыы к = 2;
            к = 3;
        }
        ф();
        к = 5;
        "#,
    );
    assert!(err.message.contains("Нельзя изменить константу"), "неожиданное сообщение: {}", err.message);
}

#[test]
fn member_write_through_shadowing_root_is_allowed() {
    let i = run_code(
        r#"
        ясенХуй о = { х: 1 };
        гыы рез = 0;
        йопта ф() {
            гыы о = { х: 2 };
            о.х = 3;
            отвечаю о.х;
        }
        рез = ф();
        "#,
    );
    assert_eq!(i.get("рез"), Some(Value::Number(3.0)));
}

#[test]
fn const_object_property_write_is_allowed() {
    let i = run_code("ясенХуй о = { х: 1 }; о.х = 2; гыы рез = о.х;");
    assert_eq!(i.get("рез"), Some(Value::Number(2.0)));
}

#[test]
fn const_array_index_write_is_allowed() {
    let i = run_code("ясенХуй а = [1, 2]; а[0] = 5; а[1] += 40; гыы рез = а[0] + а[1];");
    assert_eq!(i.get("рез"), Some(Value::Number(47.0)));
}

#[test]
fn mutating_method_on_const_receiver_is_allowed() {
    let i = run_code("ясенХуй а = [1]; а.push(2); гыы рез = а.length;");
    assert_eq!(i.get("рез"), Some(Value::Number(2.0)));
}

#[test]
fn const_rebinding_still_rejected() {
    let err = run_code_err("ясенХуй к = 1; к = 2;");
    assert!(err.message.contains("Нельзя изменить константу"), "неожиданное сообщение: {}", err.message);
}

#[test]
fn slot_read_falls_through_to_outer_scope_before_declaration() {
    let i = run_code("гыы х = 1; гыы рез = 0; го (гыы и = 0; и < 1; и++) { рез = х; }");
    assert_eq!(i.get("рез"), Some(Value::Number(1.0)));
}

#[test]
fn block_tdz_still_reported() {
    let err = run_code_err("гыы х = 1; { сказать(х); гыы х = 2; }");
    assert!(err.message.contains("до её инициализации"), "неожиданное сообщение: {}", err.message);
}

#[test]
fn function_body_tdz_still_reported() {
    let err = run_code_err("йопта ф() { отвечаю х; гыы х = 1; } ф();");
    assert!(err.message.contains("до её инициализации"), "неожиданное сообщение: {}", err.message);
}

#[test]
fn const_in_nested_scope_still_rejected() {
    let err = run_code_err("йопта ф() { ясенХуй к = 1; к = 2; } ф();");
    assert!(err.message.contains("Нельзя изменить константу"), "неожиданное сообщение: {}", err.message);
}

#[test]
fn global_const_assigned_from_function_still_rejected() {
    let err = run_code_err("ясенХуй к = 1; йопта ф() { к = 2; } ф();");
    assert!(err.message.contains("Нельзя изменить константу"), "неожиданное сообщение: {}", err.message);
}

#[test]
fn catch_parameter_shadows_outer_binding() {
    let i = run_code("гыы е = 1; гыы рез = 0; хапнуть { кидай 42; } гоп (е) { рез = е; } рез += е;");
    assert_eq!(i.get("рез"), Some(Value::Number(43.0)));
}

#[test]
fn named_function_expression_sees_its_own_name() {
    let i = run_code(
        "гыы ф = йопта сам(н) { вилкойвглаз (н < 1) { отвечаю 0; } отвечаю н + сам(н - 1); }; гыы рез = ф(3);",
    );
    assert_eq!(i.get("рез"), Some(Value::Number(6.0)));
}

#[test]
fn per_iteration_bindings_are_distinct_in_for_of() {
    let i = run_code(
        "гыы фн = []; го (гыы х сашаГрей [1, 2, 3]) { фн.push(() => х); } гыы рез = фн[0]() * 100 + фн[1]() * 10 + фн[2]();",
    );
    assert_eq!(i.get("рез"), Some(Value::Number(123.0)));
}

#[test]
fn deeply_nested_closure_reads_enclosing_locals() {
    let i = run_code(
        "йопта внеш() { гыы а = 1; отвечаю () => { гыы б = 2; отвечаю () => а + б; }; } гыы рез = внеш()()();",
    );
    assert_eq!(i.get("рез"), Some(Value::Number(3.0)));
}

#[test]
fn inner_block_shadowing_does_not_leak_outward() {
    let i = run_code("гыы х = 1; { гыы х = 2; } гыы рез = х;");
    assert_eq!(i.get("рез"), Some(Value::Number(1.0)));
}

#[test]
fn second_script_run_keeps_earlier_root_bindings() {
    let mut interp = Interpreter::new();
    run_script(&mut interp, "гыы первая = 7;");
    run_script(&mut interp, "гыы вторая = первая + 1;");
    assert_eq!(interp.get("первая"), Some(Value::Number(7.0)));
    assert_eq!(interp.get("вторая"), Some(Value::Number(8.0)));
}

#[test]
fn shadowing_a_builtin_at_root_wins() {
    let i = run_code("гыы длина = 5; гыы рез = длина;");
    assert_eq!(i.get("рез"), Some(Value::Number(5.0)));
}

#[test]
fn paren_arrow_body_does_not_alias_another_scope_layout() {
    let i = run_code("йопта ч(к){отвечаю к;};;;;\nясенХуй г = (а, б) => а + б;\nгыы р1 = ч(5);\nгыы р2 = г(1, 2);");
    assert_eq!(i.get("р1"), Some(Value::Number(5.0)));
    assert_eq!(i.get("р2"), Some(Value::Number(3.0)));
}

#[test]
fn proxy_trap_params_resolve_next_to_a_paren_arrow_target() {
    let i = run_code(
        "ясенХуй защ = захуярить Посредник({ баланс: 100 }, { получить: (цель, ключ) => { отвечаю цель[ключ]; } });\nясенХуй лог = захуярить Посредник((а, б) => а + б, { применить: (цель, этот, арг) => цель(арг[0], арг[1]) });\nгыы р1 = защ.баланс;\nгыы р2 = лог(7, 8);",
    );
    assert_eq!(i.get("р1"), Some(Value::Number(100.0)));
    assert_eq!(i.get("р2"), Some(Value::Number(15.0)));
}

#[test]
fn more_than_sixty_four_root_bindings_fall_back_to_names() {
    let mut src = String::new();
    for i in 0..70 {
        src.push_str(&format!("гыы в{i} = {i};\n"));
    }
    src.push_str("гыы рез = в0 + в69;");
    let i = run_code(&src);
    assert_eq!(i.get("рез"), Some(Value::Number(69.0)));
}

#[test]
fn more_than_sixty_four_locals_fall_back_to_names() {
    let mut body = String::new();
    for i in 0..70 {
        body.push_str(&format!("гыы л{i} = {i}; "));
    }
    let src = format!("йопта фн() {{ {body}отвечаю л0 + л69; }}\nгыы рез = фн();");
    let i = run_code(&src);
    assert_eq!(i.get("рез"), Some(Value::Number(69.0)));
}

#[test]
fn sixty_fifth_binding_does_not_alias_const_flags() {
    let mut src = String::new();
    for i in 0..64 {
        src.push_str(&format!("гыы в{i} = {i};\n"));
    }
    src.push_str("ясенХуй к64 = 999;\nв0 = 42;\nгыы рез = в0;");
    let i = run_code(&src);
    assert_eq!(i.get("рез"), Some(Value::Number(42.0)));
}

#[test]
fn exactly_sixty_four_bindings_still_read_back() {
    let mut src = String::new();
    for i in 0..64 {
        src.push_str(&format!("гыы в{i} = {i};\n"));
    }
    src.push_str("гыы рез = в0 + в63;");
    let i = run_code(&src);
    assert_eq!(i.get("рез"), Some(Value::Number(63.0)));
}

#[test]
fn error_out_of_classic_for_head_does_not_leak_a_frame() {
    let i = run_code(
        "гыы х = \"ИКС\";\nхапнуть { гыы у = \"ИГРЕК\"; го (гыы и = 0; и < 2; и = и + 1) { боом(); } } гоп (е) { }\nгыы рез = х;\nйопта ф() { отвечаю х; }\nгыы рез2 = ф();",
    );
    assert_eq!(i.get("рез"), Some(Value::String("ИКС".into())));
    assert_eq!(i.get("рез2"), Some(Value::String("ИКС".into())));
}

#[test]
fn error_in_for_update_does_not_leak_a_frame() {
    let i = run_code(
        "гыы х = \"ИКС\";\nхапнуть { гыы у = \"ИГРЕК\"; го (гыы и = 0; и < 2; боом()) { } } гоп (е) { }\nгыы рез = х;",
    );
    assert_eq!(i.get("рез"), Some(Value::String("ИКС".into())));
}

#[test]
fn error_out_of_for_in_does_not_leak_a_frame() {
    let i = run_code(
        "гыы х = \"ИКС\";\nхапнуть { гыы у = \"ИГРЕК\"; го (гыы к из { а: 1, б: 2 }) { боом(); } } гоп (е) { }\nгыы рез = х;",
    );
    assert_eq!(i.get("рез"), Some(Value::String("ИКС".into())));
}

#[test]
fn error_out_of_for_of_array_does_not_leak_a_frame() {
    let i = run_code(
        "гыы х = \"ИКС\";\nхапнуть { гыы у = \"ИГРЕК\"; го (гыы э сашаГрей [7, 8]) { боом(); } } гоп (е) { }\nгыы рез = х;",
    );
    assert_eq!(i.get("рез"), Some(Value::String("ИКС".into())));
}

#[test]
fn throwing_user_iterator_does_not_leak_a_frame() {
    let i = run_code(
        "гыы х = \"ИКС\";\nхапнуть { гыы у = \"ИГРЕК\"; ясенХуй ит = { [Симбол.итератор]: () => ({ следующий: () => { кидай \"бум\"; } }) }; го (гыы э сашаГрей ит) { } } гоп (е) { }\nгыы рез = х;",
    );
    assert_eq!(i.get("рез"), Some(Value::String("ИКС".into())));
}

#[test]
fn error_out_of_builtin_iterator_does_not_leak_a_frame() {
    let i = run_code(
        "гыы х = \"ИКС\";\nхапнуть { гыы у = \"ИГРЕК\"; го (гыы э сашаГрей Итератор.от([1, 2])) { боом(); } } гоп (е) { }\nгыы рез = х;",
    );
    assert_eq!(i.get("рез"), Some(Value::String("ИКС".into())));
}

#[test]
fn failed_parameter_binding_restores_the_caller_environment() {
    let i = run_code(
        "гыы х = \"ИКС\";\nхапнуть { гыы у = \"ИГРЕК\"; йопта ф({ а }) { отвечаю а; } ф(ноль); } гоп (е) { }\nгыы рез = х;",
    );
    assert_eq!(i.get("рез"), Some(Value::String("ИКС".into())));
}
