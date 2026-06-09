use super::*;

#[test]
fn run_repl_expression_returns_value() {
    let mut interp = Interpreter::new();
    let prog = parse_src("1 + 2;");
    let result = interp.run_repl(&prog).unwrap();
    assert_eq!(result, Some(Value::Number(3.0)));
}

#[test]
fn run_repl_preserves_state_between_calls() {
    let mut interp = Interpreter::new();
    let prog1 = parse_src("гыы а = 5;");
    let r1 = interp.run_repl(&prog1).unwrap();
    assert_eq!(r1, None);
    let prog2 = parse_src("а * 2;");
    let r2 = interp.run_repl(&prog2).unwrap();
    assert_eq!(r2, Some(Value::Number(10.0)));
}

#[test]
fn run_repl_continues_after_error() {
    let mut interp = Interpreter::new();
    let prog1 = parse_src("гыы а = 42;");
    interp.run_repl(&prog1).unwrap();
    let prog_err = parse_src("кидай \"ошибка\";");
    let err = interp.run_repl(&prog_err);
    assert!(err.is_err());
    let prog2 = parse_src("а;");
    let result = interp.run_repl(&prog2).unwrap();
    assert_eq!(result, Some(Value::Number(42.0)));
}

#[test]
fn run_repl_last_expr_of_multiple_stmts() {
    let mut interp = Interpreter::new();
    let prog = parse_src("сказать(1); 7;");
    let result = interp.run_repl(&prog).unwrap();
    assert_eq!(result, Some(Value::Number(7.0)));
}

#[test]
fn run_repl_ghost_timer_does_not_fire_after_error() {
    let mut interp = Interpreter::new();
    let prog_init = parse_src("гыы с = 0;");
    interp.run_repl(&prog_init).unwrap();
    let prog_fail = parse_src("чутка(() => { с = 999; }, 10); кидай \"ой\";");
    let err = interp.run_repl(&prog_fail);
    assert!(err.is_err());
    assert!(interp.macrotasks.is_empty());
    let prog_harmless = parse_src("1;");
    interp.run_repl(&prog_harmless).unwrap();
    let prog_check = parse_src("с;");
    let result = interp.run_repl(&prog_check).unwrap();
    assert_eq!(result, Some(Value::Number(0.0)));
}
