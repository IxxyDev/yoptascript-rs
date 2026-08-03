use super::{Value, assert_struct_eq, run_code, run_code_err, run_more};
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
