use std::cell::RefCell;
use std::rc::Rc;

use yps_lexer::{Lexer, SourceFile};
use yps_parser::Parser;
use yps_parser::ast::Program;

use crate::{Vm, VmError, compile_program, run_to_string, run_to_string_with_limit};

fn parse(src: &str) -> Program {
    let source = SourceFile::new("<тест>".to_string(), src.to_string());
    let lexer = Lexer::new(&source);
    let (tokens, ld) = lexer.tokenize();
    assert!(ld.is_empty(), "лексические ошибки: {ld:?}");
    let parser = Parser::new(&tokens, &source);
    let (program, pd) = parser.parse_program();
    assert!(pd.is_empty(), "ошибки разбора: {pd:?}");
    program
}

fn run(src: &str) -> String {
    run_to_string(&parse(src)).expect("выполнение VM")
}

fn run_err(src: &str) -> String {
    match run_to_string(&parse(src)) {
        Ok(out) => panic!("ожидалась ошибка, получен вывод: {out:?}"),
        Err(e) => e.to_string(),
    }
}

fn run_interp(src: &str) -> String {
    let program = parse(src);
    let sink = yps_interpreter::BufferSink::new();
    let mut interp = yps_interpreter::Interpreter::new();
    interp.set_output_sink(Box::new(sink.clone()));
    interp.run(&program).expect("выполнение интерпретатора");
    sink.take()
}

fn run_interp_err(src: &str) -> String {
    let program = parse(src);
    let sink = yps_interpreter::BufferSink::new();
    let mut interp = yps_interpreter::Interpreter::new();
    interp.set_output_sink(Box::new(sink.clone()));
    match interp.run(&program) {
        Ok(()) => panic!("ожидалась ошибка интерпретатора, получен вывод: {:?}", sink.take()),
        Err(e) => e.message,
    }
}

#[test]
fn arithmetic_and_precedence() {
    assert_eq!(run("сказать(1 + 2 * 3);"), "7\n");
    assert_eq!(run("сказать((1 + 2) * 3);"), "9\n");
    assert_eq!(run("сказать(2 ** 10);"), "1024\n");
    assert_eq!(run("сказать(7 % 3);"), "1\n");
    assert_eq!(run("сказать(10 / 4);"), "2.5\n");
}

#[test]
fn step_limit_stops_infinite_loops() {
    let err = run_to_string_with_limit(&parse("го(;;){}"), 100_000).unwrap_err();
    assert!(err.to_string().contains("превышен лимит шагов"), "неожиданная ошибка: {err}");
    let err = run_to_string_with_limit(&parse("потрещим (правда) {}"), 100_000).unwrap_err();
    assert!(err.to_string().contains("превышен лимит шагов"), "неожиданная ошибка: {err}");
    let ok =
        run_to_string_with_limit(&parse("гыы с = 0; го (гыы и = 0; и < 100; и += 1) { с += и; } сказать(с);"), 100_000)
            .unwrap();
    assert_eq!(ok, "4950\n");
}

#[test]
fn bitwise_not_coercion_parity() {
    for src in [
        "сказать(~\";\");",
        "сказать(~\"12\");",
        "сказать(~правда);",
        "сказать(~ноль);",
        "сказать(~неибу);",
        "сказать(~5n);",
        "сказать(~-1n);",
    ] {
        assert_eq!(run(src), run_interp(src), "расхождение на {src:?}");
    }
    assert_eq!(run("сказать(~\";\");"), "-1\n");
    assert_eq!(run("сказать(~5n);"), "-6n\n");
}

#[test]
fn number_formatting_matches_interpreter() {
    for src in ["сказать(-0);", r#"сказать(число("абв"));"#, "сказать(0.5);", "сказать(9007199254740992);"]
    {
        assert_eq!(run(src), run_interp(src), "исходник: {src}");
    }
    assert_eq!(run("сказать(-0);"), "-0\n");
    assert_eq!(run(r#"сказать(число("абв"));"#), "NaN\n");
    assert_eq!(run("сказать(0.5);"), "0.5\n");
    assert_eq!(run("сказать(9007199254740992);"), "9007199254740992\n");
}

#[test]
fn string_concat_and_template() {
    assert_eq!(run(r#"сказать("a" + "b" + 1);"#), "ab1\n");
    assert_eq!(run(r#"гыы х = 5; сказать(`знач=${х}`);"#), "знач=5\n");
}

#[test]
fn comparison_and_equality() {
    assert_eq!(run("сказать(1 < 2, 2 <= 2, 3 > 4, 1 == 1, 1 === 1, 1 != 2);"), "true true false true true true\n");
    assert_eq!(run(r#"сказать(1 == "1", 1 === "1");"#), "true false\n");
}

#[test]
fn logical_short_circuit_no_side_effect() {
    let src = r#"
        гыы счёт = 0;
        йопта тик() { счёт += 1; отвечаю правда; }
        лож && тик();
        правда || тик();
        сказать(счёт);
    "#;
    assert_eq!(run(src), "0\n");
}

#[test]
fn nullish_and_ternary() {
    assert_eq!(run("сказать(ноль ?? 7);"), "7\n");
    assert_eq!(run("сказать(0 ?? 7);"), "0\n");
    assert_eq!(run("сказать(5 > 3 ? \"да\" : \"нет\");"), "да\n");
}

#[test]
fn variables_const_and_shadowing() {
    assert_eq!(run("гыы х = 1; х = х + 4; сказать(х);"), "5\n");
    assert_eq!(run("ясенХуй ПИ = 3.14; сказать(ПИ);"), "3.14\n");
    let src = r#"
        гыы х = 1;
        {
            гыы х = 2;
            сказать(х);
        }
        сказать(х);
    "#;
    assert_eq!(run(src), "2\n1\n");
}

#[test]
fn const_mutation_is_runtime_error() {
    let msg = run_err("ясенХуй х = 1; х = 2;");
    assert!(msg.contains("константу"), "сообщение: {msg}");
}

#[test]
fn compound_assignment_and_postfix() {
    let src = r#"
        гыы х = 10;
        х += 5; сказать(х);
        х -= 3; сказать(х);
        х *= 2; сказать(х);
        х /= 4; сказать(х);
        гыы и = 0;
        и++;
        сказать(и);
    "#;
    assert_eq!(run(src), "15\n12\n24\n6\n1\n");
}

#[test]
fn if_else_chain() {
    let src = r#"
        йопта знак(н) {
            вилкойвглаз (н > 0) { отвечаю "плюс"; }
            иливжопураз вилкойвглаз (н < 0) { отвечаю "минус"; }
            отвечаю "ноль";
        }
        сказать(знак(5), знак(-2), знак(0));
    "#;
    assert_eq!(run(src), "плюс минус ноль\n");
}

#[test]
fn while_loop() {
    let src = r#"
        гыы и = 0;
        гыы сумма = 0;
        потрещим (и < 5) {
            сумма += и;
            и++;
        }
        сказать(сумма);
    "#;
    assert_eq!(run(src), "10\n");
}

#[test]
fn for_loop_with_break_continue() {
    let src = r#"
        гыы сумма = 0;
        го (гыы и = 0; и < 10; и++) {
            вилкойвглаз (и == 5) { харэ; }
            вилкойвглаз (и % 2 == 0) { двигай; }
            сумма += и;
        }
        сказать(сумма);
    "#;
    assert_eq!(run(src), "4\n");
}

#[test]
fn nested_loops() {
    let src = r#"
        гыы счёт = 0;
        го (гыы и = 0; и < 3; и++) {
            го (гыы к = 0; к < 3; к++) {
                счёт++;
            }
        }
        сказать(счёт);
    "#;
    assert_eq!(run(src), "9\n");
}

#[test]
fn recursion_factorial() {
    let src = r#"
        йопта факт(н) {
            вилкойвглаз (н <= 1) { отвечаю 1; }
            отвечаю н * факт(н - 1);
        }
        сказать(факт(5));
    "#;
    assert_eq!(run(src), "120\n");
}

#[test]
fn closures_capture_counter() {
    let src = r#"
        йопта счётчик() {
            гыы н = 0;
            отвечаю () => { н += 1; отвечаю н; };
        }
        гыы с = счётчик();
        сказать(с(), с(), с());
    "#;
    assert_eq!(run(src), "1 2 3\n");
}

#[test]
fn default_and_rest_params() {
    assert_eq!(run("йопта ф(а, б = 10) { отвечаю а + б; } сказать(ф(5), ф(5, 1));"), "15 6\n");
    let src = r#"
        йопта собрать(первый, ...остальные) {
            отвечаю первый + длина(остальные);
        }
        сказать(собрать(1, 2, 3, 4));
    "#;
    assert_eq!(run(src), "4\n");
}

#[test]
fn arrays_objects_indexing() {
    assert_eq!(run("гыы а = [1, 2, 3]; сказать(а[0], а[2]);"), "1 3\n");
    assert_eq!(run("гыы а = [1, 2, 3]; а[1] = 99; сказать(а);"), "[1, 99, 3]\n");
    assert_eq!(run(r#"гыы о = { имя: "Йопта", лет: 3 }; сказать(о.имя, о["лет"]);"#), "Йопта 3\n");
    assert_eq!(run(r#"гыы о = {}; о.поле = 7; сказать(о.поле);"#), "7\n");
}

#[test]
fn nested_member_index_assignment() {
    let src = r#"
        гыы данные = [ { счёт: 0 }, { счёт: 0 } ];
        данные[0].счёт = 42;
        сказать(данные[0].счёт, данные[1].счёт);
    "#;
    assert_eq!(run(src), "42 0\n");
}

#[test]
fn computed_object_keys() {
    let src = r#"
        гыы ключ = "динам";
        гыы о = { [ключ]: 1 };
        сказать(о.динам);
    "#;
    assert_eq!(run(src), "1\n");
}

#[test]
fn builtins_core() {
    assert_eq!(run(r#"сказать(длина("привет"), длина([1, 2, 3]));"#), "6 3\n");
    assert_eq!(
        run(r#"сказать(тип(1), тип("с"), тип(правда), тип([]), тип({}));"#),
        "число строка булево массив объект\n"
    );
    assert_eq!(run(r#"сказать(число("42"), строка(42));"#), "42 42\n");
    assert_eq!(run("гыы а = [1]; втолкнуть(а, 2); сказать(а);"), "[1, 2]\n");
}

#[test]
fn array_method_call_push() {
    assert_eq!(run("гыы а = []; а.втолкнуть(1); а.втолкнуть(2); сказать(а);"), "[1, 2]\n");
}

#[test]
fn array_method_push_returns_new_length() {
    assert_eq!(run("гыы а = []; сказать(а.втолкнуть(5));"), "1\n");
    assert_eq!(run("гыы а = [1, 2]; сказать(а.втолкнуть(3));"), "3\n");
}

#[test]
fn array_method_push_is_variadic() {
    assert_eq!(run("гыы б = []; б.втолкнуть(1, 2); сказать(б);"), "[1, 2]\n");
}

#[test]
fn array_method_push_zero_args_is_noop() {
    assert_eq!(run("гыы в = [3]; сказать(в.втолкнуть()); сказать(в);"), "1\n[3]\n");
}

#[test]
fn array_method_push_aliases() {
    assert_eq!(run("гыы а = []; а.push(1); а.добавить(2); а.втолкнуть(3); сказать(а);"), "[1, 2, 3]\n");
}

#[test]
fn per_iteration_binding_nested_outer_capture() {
    assert_eq!(
        run(
            "гыы f = [ноль, ноль, ноль, ноль]; гыы k = 0; го (гыы и = 0; и < 2; и = и + 1) { го (гыы j = 0; j < 2; j = j + 1) { f[k] = () => и; k = k + 1; } } сказать(f[0](), f[1](), f[2](), f[3]());"
        ),
        "0 0 1 1\n"
    );
}

#[test]
fn per_iteration_binding_with_continue() {
    assert_eq!(
        run(
            "гыы f = []; го (гыы и = 0; и < 5; и = и + 1) { вилкойвглаз (и % 2 == 0) { двигай; } f.втолкнуть(() => и); } сказать(f[0](), f[1]());"
        ),
        "1 3\n"
    );
}

#[test]
fn per_iteration_binding_for() {
    assert_eq!(
        run(
            "гыы f = [ноль, ноль, ноль]; го (гыы и = 0; и < 3; и = и + 1) { f[и] = () => и; } сказать(f[0](), f[1](), f[2]());"
        ),
        "0 1 2\n"
    );
}

#[test]
fn per_iteration_binding_for_of() {
    assert_eq!(
        run(
            "гыы f = [ноль, ноль, ноль]; гыы k = 0; го (гыы x сашаГрей [10, 20, 30]) { f[k] = () => x; k = k + 1; } сказать(f[0](), f[1](), f[2]());"
        ),
        "10 20 30\n"
    );
}

#[test]
fn per_iteration_binding_for_in() {
    assert_eq!(
        run(
            "гыы o = { а: 1, б: 2 }; гыы f = [ноль, ноль]; гыы k = 0; го (гыы ключ из o) { f[k] = () => ключ; k = k + 1; } сказать(f[0](), f[1]());"
        ),
        "а б\n"
    );
}

#[test]
fn console_family() {
    assert_eq!(run(r#"сказать.инфо("инфо"); сказать.отладка("дбг");"#), "инфо\nдбг\n");
}

#[test]
fn number_division_by_zero_matches_interpreter() {
    for src in ["сказать(1 / 0);", "сказать(-1 / 0);", "сказать(0 / 0);", "сказать(5 % 0);"]
    {
        assert_eq!(run(src), run_interp(src), "исходник: {src}");
    }
    assert_eq!(run("сказать(1 / 0);"), "Infinity\n");
    assert_eq!(run("сказать(-1 / 0);"), "-Infinity\n");
    assert_eq!(run("сказать(0 / 0);"), "NaN\n");
    assert_eq!(run("сказать(5 % 0);"), "NaN\n");
}

#[test]
fn bigint_division_by_zero_is_error() {
    let msg = run_err("сказать(1n / 0n);");
    assert!(msg.contains("ноль"), "сообщение: {msg}");
    let msg = run_err("сказать(1n % 0n);");
    assert!(msg.contains("ноль"), "сообщение: {msg}");
}

#[test]
fn relational_coerces_like_interpreter() {
    for src in [
        r#"сказать("a" < "b");"#,
        r#"сказать("10" < "9");"#,
        r#"сказать("5" > 3);"#,
        "сказать(правда < 2);",
        "сказать(ноль < 1);",
        r#"сказать(2 > "10");"#,
    ] {
        assert_eq!(run(src), run_interp(src), "исходник: {src}");
    }
    assert_eq!(run(r#"сказать("a" < "b");"#), "true\n");
    assert_eq!(run(r#"сказать("10" < "9");"#), "true\n");
    assert_eq!(run(r#"сказать("5" > 3);"#), "true\n");
    assert_eq!(run("сказать(правда < 2);"), "true\n");
    assert_eq!(run("сказать(ноль < 1);"), "true\n");
    assert_eq!(run(r#"сказать(2 > "10");"#), "false\n");
}

#[test]
fn out_of_range_array_set_errors_no_growth() {
    let msg = run_err("гыы а = [1]; а[1000000] = 1;");
    assert!(msg.contains("вне диапазона"), "сообщение: {msg}");
}

#[test]
fn function_declarations_are_hoisted() {
    assert_eq!(run("сказать(ф()); йопта ф() { отвечаю 42; }"), "42\n");
    let src = r#"
        сказать(чёт(4), нечёт(4));
        йопта чёт(н) {
            вилкойвглаз (н == 0) { отвечаю правда; }
            отвечаю нечёт(н - 1);
        }
        йопта нечёт(н) {
            вилкойвглаз (н == 0) { отвечаю лож; }
            отвечаю чёт(н - 1);
        }
    "#;
    assert_eq!(run(src), "true false\n");
}

#[test]
fn hoisted_nested_function_captures_enclosing_local() {
    let src = r#"
        йопта внешняя() {
            гыы префикс = "p:";
            йопта метка(с) { отвечаю префикс + с; }
            отвечаю метка("x");
        }
        сказать(внешняя());
    "#;
    assert_eq!(run(src), "p:x\n");
}

#[test]
fn nested_function_call_before_decl() {
    let src = r#"
        йопта внешняя() {
            отвечаю метка("x");
            йопта метка(с) { отвечаю "m:" + с; }
        }
        сказать(внешняя());
    "#;
    assert_eq!(run(src), "m:x\n");
}

#[test]
fn spread_in_array_literal() {
    assert_eq!(run("гыы а = [2, 3]; сказать([1, ...а, 4]);"), "[1, 2, 3, 4]\n");
    assert_eq!(run(r#"сказать([..."аб", "в"]);"#), "[а, б, в]\n");
}

#[test]
fn spread_in_call_args() {
    let src = r#"
        йопта сумма(а, б, в) { отвечаю а + б + в; }
        сказать(сумма(...[1, 2, 3]));
        сказать(сумма(1, ...[2, 3]));
    "#;
    assert_eq!(run(src), "6\n6\n");
}

#[test]
fn spread_in_object_literal() {
    assert_eq!(run("гыы о = { а: 1 }; гыы н = { ...о, б: 2 }; сказать(н.а, н.б);"), "1 2\n");
    assert_eq!(run("гыы о = { а: 1 }; сказать({ ...о, а: 9 }.а);"), "9\n");
}

#[test]
fn object_getter_and_setter() {
    let src = r#"
        гыы хран = 5;
        гыы о = { get х() { отвечаю хран; }, set х(в) { хран = в; } };
        сказать(о.х);
        о.х = 11;
        сказать(о.х, хран);
    "#;
    assert_eq!(run(src), "5\n11 11\n");
}

#[test]
fn array_destructuring_decl() {
    assert_eq!(run("гыы [а, б] = [1, 2]; сказать(а, б);"), "1 2\n");
    assert_eq!(run("гыы [п, ...ост] = [1, 2, 3]; сказать(п, ост);"), "1 [2, 3]\n");
    assert_eq!(run("гыы [, в] = [1, 2]; сказать(в);"), "2\n");
}

#[test]
fn object_destructuring_decl() {
    assert_eq!(run("гыы { х, у } = { х: 1, у: 2 }; сказать(х, у);"), "1 2\n");
    assert_eq!(run("гыы { х: к } = { х: 7 }; сказать(к);"), "7\n");
    assert_eq!(run("гыы { а, ...пр } = { а: 1, б: 2 }; сказать(а, пр);"), "1 {б: 2}\n");
    assert_eq!(run("гыы { п = 9 } = {}; сказать(п);"), "9\n");
}

#[test]
fn nested_destructuring_decl() {
    assert_eq!(run("гыы [[а, б]] = [[1, 2]]; сказать(а, б);"), "1 2\n");
    assert_eq!(run("гыы { к: { л } } = { к: { л: 3 } }; сказать(л);"), "3\n");
}

#[test]
fn param_destructuring() {
    assert_eq!(run("йопта ф({ х, у }) { отвечаю х + у; } сказать(ф({ х: 3, у: 4 }));"), "7\n");
    assert_eq!(run("йопта ф([а, б]) { отвечаю а * б; } сказать(ф([5, 6]));"), "30\n");
    assert_eq!(run("йопта ф({ а, б = 10 }) { отвечаю а + б; } сказать(ф({ а: 1 }));"), "11\n");
}

#[test]
fn delete_operator() {
    assert_eq!(run("гыы о = { а: 1, б: 2 }; ёбнуть о.а; сказать(о);"), "{б: 2}\n");
    assert_eq!(run(r#"гыы о = { а: 1 }; гыы к = "а"; ёбнуть о[к]; сказать(о);"#), "{}\n");
    assert_eq!(run("гыы а = [1, 2, 3]; ёбнуть а[1]; сказать(а);"), "[1, undefined, 3]\n");
}

#[test]
fn compound_assign_member_and_index() {
    assert_eq!(run("гыы о = { с: 10 }; о.с += 5; сказать(о.с);"), "15\n");
    assert_eq!(run("гыы а = [1, 2]; а[0] *= 10; сказать(а);"), "[10, 2]\n");
    assert_eq!(run("гыы а = [4]; гыы и = 0; а[и] -= 1; сказать(а);"), "[3]\n");
}

#[test]
fn labeled_loops_break_continue() {
    let src = r#"
        гыы сумма = 0;
        внеш: го (гыы и = 0; и < 3; и++) {
            го (гыы к = 0; к < 3; к++) {
                вилкойвглаз (к == 2) { двигай внеш; }
                сумма += 1;
            }
        }
        сказать(сумма);
    "#;
    assert_eq!(run(src), "6\n");
    let src2 = r#"
        гыы с = 0;
        м: потрещим (правда) {
            го (гыы и = 0; и < 100; и++) {
                вилкойвглаз (и == 4) { харэ м; }
                с++;
            }
        }
        сказать(с);
    "#;
    assert_eq!(run(src2), "4\n");
}

#[test]
fn class_basic_methods_and_fields() {
    let src = r#"
        клёво Точка {
            х = 1;
            Точка(х) { тырыпыры.х = х; }
            дабл() { отвечаю тырыпыры.х * 2; }
        }
        гыы т = захуярить Точка(21);
        сказать(т.х);
        сказать(т.дабл());
        сказать(т шкура Точка);
    "#;
    assert_eq!(run(src), "21\n42\ntrue\n");
}

#[test]
fn class_inheritance_super_chain() {
    let src = r#"
        клёво A { A() { тырыпыры.имя = "A"; } зов() { отвечаю "A"; } }
        клёво B батя A { B() { яга(); } зов() { отвечаю яга.зов() + "B"; } }
        клёво C батя B { C() { яга(); } зов() { отвечаю яга.зов() + "C"; } }
        гыы c = захуярить C();
        сказать(c.имя);
        сказать(c.зов());
    "#;
    assert_eq!(run(src), "A\nABC\n");
}

#[test]
fn class_static_field_mutation() {
    let src = r#"
        клёво S {
            попонятия н = 0;
            попонятия инкр() { тырыпыры.н = тырыпыры.н + 1; отвечаю тырыпыры.н; }
        }
        сказать(S.инкр());
        сказать(S.инкр());
        сказать(S.н);
    "#;
    assert_eq!(run(src), "1\n2\n2\n");
}

#[test]
fn class_static_block_interleaves_with_fields() {
    let src = r#"
        клёво К {
            попонятия а = 1;
            попонятия {
                тырыпыры.б = тырыпыры.а + 1;
            }
            попонятия в = 2;
            попонятия {
                тырыпыры.г = тырыпыры.в + тырыпыры.б;
            }
        }
        сказать(К.а);
        сказать(К.б);
        сказать(К.в);
        сказать(К.г);
    "#;
    assert_eq!(run(src), "1\n2\n2\n4\n");
}

#[test]
fn class_static_block_can_call_static_method() {
    let src = r#"
        клёво К {
            попонятия сумма = 5;
            попонятия удвой() { отвечаю тырыпыры.сумма * 2; }
            попонятия {
                тырыпыры.итог = тырыпыры.удвой();
            }
        }
        сказать(К.итог);
    "#;
    assert_eq!(run(src), "10\n");
}

#[test]
fn class_static_block_subclass_this_is_subclass() {
    let src = r#"
        клёво Баз {
            попонятия сумма = 5;
        }
        клёво Расш батя Баз {
            попонятия {
                тырыпыры.метка = "расш:" + тырыпыры.сумма;
            }
        }
        сказать(Расш.метка);
    "#;
    assert_eq!(run(src), "расш:5\n");
}

#[test]
fn class_getters_setters() {
    let src = r#"
        клёво T {
            T() { тырыпыры._в = 5; }
            get в() { отвечаю тырыпыры._в; }
            set в(х) { тырыпыры._в = х * 2; }
        }
        гыы т = захуярить T();
        сказать(т.в);
        т.в = 10;
        сказать(т.в);
    "#;
    assert_eq!(run(src), "5\n20\n");
}

#[test]
fn tagged_template_basic() {
    let src = r#"
        йопта тег(строки, ...вал) {
            отвечаю строки[0] + ":" + вал[0] + ":" + строки[1];
        }
        сказать(тег`a${42}b`);
    "#;
    assert_eq!(run(src), "a:42:b\n");
}

#[test]
fn bigint_literals_and_arithmetic() {
    assert_eq!(run("сказать(100000000000000000000n + 1n);"), "100000000000000000001n\n");
    assert_eq!(run("сказать(2n ** 64n);"), "18446744073709551616n\n");
    assert_eq!(run("сказать(10n / 3n);"), "3n\n");
    assert_eq!(run("сказать(-5n);"), "-5n\n");
    assert_eq!(run("сказать(тип(1n));"), "бигцелое\n");
    assert_eq!(run("сказать(5n === 5n);"), "true\n");
    assert_eq!(run("сказать(5n == 5);"), "true\n");
    assert_eq!(run("сказать(5n === 5);"), "false\n");
    assert_eq!(run("сказать(БигЦелое(\"42\"));"), "42n\n");
}

#[test]
fn regex_test_exec_tostring() {
    assert_eq!(run("сказать(/\\d+/.проверить(\"abc123\"));"), "true\n");
    assert_eq!(run("сказать(/\\d+/.test(\"abc\"));"), "false\n");
    assert_eq!(run("сказать(/(\\d)-(\\d)/.exec(\"1-2\")[2]);"), "2\n");
    assert_eq!(run("сказать(/abc/i.вСтроку());"), "/abc/i\n");
    assert_eq!(run("сказать(/abc/gi.флаги);"), "gi\n");
    assert_eq!(run("сказать(/foo(?=bar)/.проверить(\"foobar\"));"), "true\n");
    assert_eq!(run("сказать(тип(/x/));"), "регэксп\n");
}

#[test]
fn match_all_iterator_is_iterable() {
    let src = r#"
        го (м сашаГрей "foo=1, bar=2".найтиВсе(/(\w+)=(\d+)/g)) {
            сказать(м["1"] + "=" + м["2"]);
        }
    "#;
    let out = run(src);
    assert_eq!(out, "foo=1\nbar=2\n");
    assert_eq!(run_interp(src), out);
}

#[test]
fn match_all_iterator_spreads_and_destructures() {
    let src = r#"
        гыы все = [..."x=7, y=8".найтиВсе(/(\w+)=(\d+)/g)];
        сказать(все.длина);
        сказать(все[0]["0"]);
        сказать(все[1]["2"]);
        гыы [первый, второй] = [..."a=1, b=2".найтиВсе(/(\w+)=(\d+)/g)];
        сказать(первый["1"] + второй["1"]);
    "#;
    let out = run(src);
    assert_eq!(out, "2\nx=7\n8\nab\n");
    assert_eq!(run_interp(src), out);
}

#[test]
fn dynamic_regexp_constructor() {
    assert_eq!(run(r#"сказать(RegExp("\\d+", "g").флаги);"#), "g\n");
    assert_eq!(run(r#"сказать("год 2026 век 21".совпадает(RegExp("\\d+", "g")));"#), "[2026, 21]\n");
    assert_eq!(run(r#"сказать(RegExp(/ab/i).вСтроку());"#), "/ab/i\n");
    assert_eq!(run(r#"сказать(RegExp(/ab/i, "g").вСтроку());"#), "/ab/g\n");
    assert!(run_err(r#"RegExp(42);"#).contains("RegExp ожидает строку или regex"));
}

#[test]
fn regexp_last_index_is_assignable() {
    let src = r#"
        гыы р = /\d/g;
        р.последнийИндекс = 2;
        сказать(р.найти("a1b2")["0"]);
        сказать(р.последнийИндекс);
    "#;
    let out = run(src);
    assert_eq!(out, "2\n4\n");
    assert_eq!(run_interp(src), out);
}

#[test]
fn destructuring_assignment_expressions() {
    let src = r#"
        гыы а = 0; гыы б = 0; гыы остаток = [];
        [а, б] = [1, 2];
        сказать(а + " " + б);
        [а, ...остаток] = [10, 20, 30];
        сказать(а);
        сказать(остаток);
        гыы п = 0;
        [п = 9] = [];
        сказать(п);
        [п = 9] = [4];
        сказать(п);
        гыы об = {};
        [об.поле] = [77];
        сказать(об.поле);
        гыы масс = [0, 0];
        [масс[1], масс[0]] = ["a", "b"];
        сказать(масс);
    "#;
    let out = run(src);
    assert_eq!(out, "1 2\n10\n[20, 30]\n9\n4\n77\n[b, a]\n");
    assert_eq!(run_interp(src), out);
}

#[test]
fn destructuring_assignment_objects_and_nesting() {
    let src = r#"
        гыы х = 0; гыы у = 0;
        ({х, у} = {х: 5, у: 6});
        сказать(х + " " + у);
        гыы пр = 0; гыы ост = {};
        ({пр: пр, ...ост} = {пр: 1, ещё: 2, третий: 3});
        сказать(пр);
        сказать(ост);
        гыы вложА = 0; гыы вложБ = 0;
        [[вложА], {к: вложБ}] = [[8], {к: 9}];
        сказать(вложА + " " + вложБ);
        гыы клч = "динам"; гыы знач = 0;
        ({[клч]: знач} = {динам: 42});
        сказать(знач);
        гыы а = 0; гыы б = 0;
        сказать([а, б] = [7, 8]);
        (а) = 100;
        сказать(а);
    "#;
    let out = run(src);
    assert_eq!(out, "5 6\n1\n{ещё: 2, третий: 3}\n8 9\n42\n[7, 8]\n100\n");
    assert_eq!(run_interp(src), out);
}

#[test]
fn destructuring_assignment_inside_expressions_and_scopes() {
    let src = r#"
        йопта ф() {
            гыы л = 1;
            гыы а = 0; гыы б = 0; гыы ост = {};
            гыы к1 = "п1"; гыы к2 = "п2";
            сказать(л + 1);
            ({[к1]: а, [к2]: б, ...ост} = {п1: 1, п2: 2, п3: 3, п4: 4});
            сказать(а + " " + б);
            сказать(ост);
            гыы масс = [0, 0];
            сказать(1 + ([масс[0], масс[1]] = [3, 4]).длина);
            сказать(масс);
            гыы сч = 0;
            йопта поб() { сч += 1; отвечаю "к" + сч; }
            гыы з1 = 0; гыы ост2 = {};
            ({[поб()]: з1, ...ост2} = {к1: 11, к2: 22});
            сказать(з1);
            сказать(ост2);
            сказать(сч);
        }
        ф();
    "#;
    let out = run(src);
    assert_eq!(out, "2\n1 2\n{п3: 3, п4: 4}\n3\n[3, 4]\n11\n{к2: 22}\n1\n");
    assert_eq!(run_interp(src), out);
}

#[test]
fn grouped_assignment_targets_match_interpreter() {
    for (src, expected) in [
        ("гыы х = [1]; (х[0]) += 1; сказать(х);", "[2]\n"),
        ("гыы о = {а: 1}; (о.а) *= 5; сказать(о.а);", "5\n"),
        ("гыы х = 0; (х) ||= 3; сказать(х);", "3\n"),
        ("гыы а = 0; гыы б = 0; [(а), (б)] = [1, 2]; сказать(а, б);", "1 2\n"),
        ("гыы а = 0; ({к: (а)} = {к: 9}); сказать(а);", "9\n"),
    ] {
        assert_same_as_interpreter(src, expected);
    }
}

#[test]
fn try_catch_finally_paths() {
    let src = r#"
        хапнуть {
            сказать("до");
            кидай "бах";
            сказать("после");
        } гоп (е) {
            сказать("поймал " + е);
        } тюряжка {
            сказать("финал");
        }
        сказать("конец");
    "#;
    assert_eq!(run(src), "до\nпоймал бах\nфинал\nконец\n");
}

#[test]
fn finally_runs_on_return() {
    let src = r#"
        йопта ф() {
            хапнуть {
                отвечаю "из_трая";
            } тюряжка {
                сказать("финал");
            }
        }
        сказать(ф());
    "#;
    assert_eq!(run(src), "финал\nиз_трая\n");
}

#[test]
fn finally_locals_on_the_throw_path_do_not_alias_the_pending_exception() {
    let src = r#"
        хапнуть {
            хапнуть {
                кидай "внеш";
            } тюряжка {
                гыы х = 5;
                хапнуть { кидай "вн"; } гоп (е) { сказать(х, е); }
            }
        } гоп (е) {
            сказать("поймал", е);
        }
    "#;
    assert_eq!(run(src), "5 вн\nпоймал внеш\n");
    assert_eq!(run(src), run_interp(src));
}

#[test]
fn generator_step_result_has_only_the_russian_keys() {
    let src = r#"
        пиздюли г() { хапнуть { поебалу 1; } гоп (е) {} }
        гыы р = г().next();
        сказать(р, р.value);
    "#;
    assert_eq!(run(src), "{значение: 1, готово: false} undefined\n");
    assert_eq!(run(src), run_interp(src));
}

#[test]
fn switch_no_fallthrough() {
    let src = r#"
        йопта тест(х) {
            базарпо (х) {
                тема 1: { сказать("один"); }
                тема 2: { сказать("два"); }
                нуичо { сказать("другое"); }
            }
        }
        тест(1); тест(2); тест(9);
    "#;
    assert_eq!(run(src), "один\nдва\nдругое\n");
}

#[test]
fn for_in_keys_and_for_of_values() {
    assert_eq!(run("гыы об = { а: 1, б: 2 }; го (гыы к чоунастут об) { сказать(к); }"), "а\nб\n");
    assert_eq!(run("го (гыы зн сашаГрей [10, 20]) { сказать(зн); }"), "10\n20\n");
    assert_eq!(run(r#"го (гыы с сашаГрей "аб") { сказать(с); }"#), "а\nб\n");
}

#[test]
fn optional_chaining_short_circuits() {
    assert_eq!(run("гыы н = ноль; сказать(н?.поле);"), "undefined\n");
    assert_eq!(run(r#"гыы о = { имя: "Х" }; сказать(о?.имя);"#), "Х\n");
    assert_eq!(run("сказать(ноль?.[0]);"), "undefined\n");
    assert_eq!(run("сказать(ноль?.());"), "undefined\n");
}

#[test]
fn throw_caught_across_call_frames() {
    let src = r#"
        йопта кидальщик() { кидай "ой"; }
        хапнуть {
            кидальщик();
        } гоп (е) {
            сказать("поймал " + е);
        }
    "#;
    assert_eq!(run(src), "поймал ой\n");
}

#[test]
fn disassembler_runs() {
    let proto = compile_program(&parse("гыы а = 1; гыы б = 2; сказать(а + б);")).unwrap();
    let text = crate::disassemble(&proto);
    assert!(text.contains("== proto"), "заголовок диза: {text}");
    assert!(text.contains("Constant(0)"), "константа 1: {text}");
    assert!(text.contains("Constant(2)"), "константа 2: {text}");
    assert!(text.contains("Add"), "сложение: {text}");
    assert!(text.contains("Call(1)"), "вызов: {text}");
}

#[test]
fn binop_in_object_and_array() {
    assert_eq!(run("сказать(\"а\" из { а: 1 });"), "true\n");
    assert_eq!(run("сказать(\"б\" из { а: 1 });"), "false\n");
    assert_eq!(run("сказать(0 из [10, 20]);"), "true\n");
    assert_eq!(run("сказать(2 из [10, 20]);"), "false\n");
}

#[test]
fn logical_assign_operators() {
    assert_eq!(run("гыы а = ноль; а ??= 5; сказать(а);"), "5\n");
    assert_eq!(run("гыы а = 3; а ??= 5; сказать(а);"), "3\n");
    assert_eq!(run("гыы б = 1; б &&= 9; сказать(б);"), "9\n");
    assert_eq!(run("гыы в = 0; в ||= 7; сказать(в);"), "7\n");
}

#[test]
fn logical_assign_member_and_index() {
    assert_eq!(run("гыы о = { х: ноль }; о.х ??= 8; сказать(о.х);"), "8\n");
    assert_eq!(run("гыы м = [ноль]; м[0] ??= 4; сказать(м[0]);"), "4\n");
}

#[test]
fn super_call_with_spread() {
    let src = r#"
        клёво A { A(а, б) { тырыпыры.с = а + б; } }
        клёво B батя A { B(...аргс) { яга(...аргс); } }
        сказать(захуярить B(3, 4).с);
    "#;
    assert_eq!(run(src), "7\n");
}

#[test]
fn super_invoke_with_spread() {
    let src = r#"
        клёво A { соедини(а, б) { отвечаю а + б; } }
        клёво B батя A { вызов(...аргс) { отвечаю яга.соедини(...аргс); } }
        сказать(захуярить B().вызов(5, 6));
    "#;
    assert_eq!(run(src), "11\n");
}

#[test]
fn class_decorator_applied() {
    let src = r#"
        йопта тег(класс, контекст) {
            сказать(контекст.вид + ":" + контекст.имя);
            отвечаю класс;
        }
        @тег
        клёво К { показать() { отвечаю 1; } }
        сказать(захуярить К().показать());
    "#;
    assert_eq!(run(src), "класс:К\n1\n");
}

#[test]
fn method_decorator_wraps() {
    let src = r#"
        йопта дважды(метод, контекст) {
            отвечаю (а) => { отвечаю метод(а) * 2; };
        }
        клёво К { @дважды м(х) { отвечаю х + 1; } }
        сказать(захуярить К().м(10));
    "#;
    assert_eq!(run(src), "22\n");
}

#[test]
fn field_decorator_transforms() {
    let src = r#"
        йопта плюс10(_, контекст) {
            отвечаю (нач) => { отвечаю нач + 10; };
        }
        клёво К { @плюс10 поле = 5; }
        сказать(захуярить К().поле);
    "#;
    assert_eq!(run(src), "15\n");
}

#[test]
fn using_disposes_lifo() {
    let src = r#"
        гыы лог = "";
        {
            юзай а = { расход: () => { лог = лог + "а"; } };
            юзай б = { расход: () => { лог = лог + "б"; } };
        }
        сказать(лог);
    "#;
    assert_eq!(run(src), "ба\n");
}

#[test]
fn using_requires_dispose_method() {
    let err = run_err("{ юзай р = { данные: 1 }; }");
    assert!(err.contains("расход"), "ошибка: {err}");
}

#[test]
fn async_using_disposes_on_scope_exit() {
    let src = r#"
        гыы лог = "";
        {
            юзай сидетьНахуй р = { асинхРасход: () => { лог = лог + "р"; } };
        }
        сказать(лог);
    "#;
    assert_eq!(run(src), "р\n");
}

#[test]
fn async_using_lifo_mixed_with_sync() {
    let src = r#"
        гыы лог = "";
        {
            юзай а = { расход: () => { лог = лог + "а"; } };
            юзай сидетьНахуй б = { асинхРасход: () => { лог = лог + "б"; } };
            юзай в = { расход: () => { лог = лог + "в"; } };
        }
        сказать(лог);
    "#;
    assert_eq!(run(src), "вба\n");
}

#[test]
fn async_using_falls_back_to_sync_dispose() {
    let src = r#"
        гыы лог = "";
        {
            юзай сидетьНахуй р = { расход: () => { лог = лог + "sync"; } };
        }
        сказать(лог);
    "#;
    assert_eq!(run(src), "sync\n");
}

#[test]
fn async_using_awaits_promise_before_scope_exit() {
    let src = r#"
        гыы лог = "";
        {
            юзай сидетьНахуй р = {
                асинхРасход: ассо йопта() {
                    лог = лог + "начало;";
                    сидетьНахуй СловоПацана.решить(0);
                    лог = лог + "конец;";
                }
            };
            лог = лог + "тело;";
        }
        лог = лог + "после;";
        сказать(лог);
    "#;
    assert_eq!(run(src), "тело;начало;конец;после;\n");
}

#[test]
fn async_using_rejection_is_catchable() {
    let src = r#"
        гыы пойман = "";
        хапнуть {
            юзай сидетьНахуй р = { асинхРасход: ассо йопта() { кидай "бабах"; } };
        } гоп (е) {
            пойман = е;
        }
        сказать(пойман);
    "#;
    assert_eq!(run(src), "бабах\n");
}

#[test]
fn async_using_requires_dispose_method() {
    let err = run_err("{ юзай сидетьНахуй р = { данные: 1 }; }");
    assert!(err.contains("асинхРасход"), "ошибка: {err}");
    assert!(err.contains("расход"), "ошибка: {err}");
}

#[test]
fn async_function_returns_promise_and_await_unwraps() {
    let src = r#"
ассо йопта получить() { отвечаю 42; }
ассо йопта главная() {
    гыы значение = сидетьНахуй получить();
    сказать("значение:", значение);
}
главная();
"#;
    assert_eq!(run(src), "значение: 42\n");
}

#[test]
fn await_reject_is_catchable() {
    let src = r#"
ассо йопта плохо() { кидай "облом"; }
ассо йопта главная() {
    хапнуть {
        сидетьНахуй плохо();
    } гоп (e) {
        сказать("поймано:", e);
    }
}
главная();
"#;
    assert_eq!(run(src), "поймано: облом\n");
}

#[test]
fn promise_all_resolves_in_order() {
    let src = r#"
ассо йопта главная() {
    гыы р = сидетьНахуй СловоПацана.всех([СловоПацана.решить(1), 2, СловоПацана.решить(3)]);
    сказать(р);
}
главная();
"#;
    assert_eq!(run(src), "[1, 2, 3]\n");
}

#[test]
fn microtask_ordering_matches() {
    let prog = r#"
сказать("старт");
СловоПацана.решить("микро").потом((v) => сказать(v));
сказать("конец");
"#;
    assert_eq!(run(prog), "старт\nконец\nмикро\n");
}

#[test]
fn dynamic_import_compiles() {
    let proto = compile_program(&parse("гыы м = спиздить(\"./x\");")).expect("компиляция");
    let text = crate::disassemble(&proto);
    assert!(text.contains("DynamicImport"), "диз: {text}");
}

#[test]
fn generator_basic_yield_and_done() {
    let src = r#"
        пиздюли счёт() { поебалу 1; поебалу 2; поебалу 3; }
        ясенХуй г = счёт();
        ясенХуй а = г.следующий();
        сказать(а.значение + " " + а.готово);
        ясенХуй б = г.следующий();
        сказать(б.значение + " " + б.готово);
        ясенХуй в = г.следующий();
        сказать(в.значение + " " + в.готово);
        ясенХуй д = г.следующий();
        сказать(д.значение + " " + д.готово);
    "#;
    assert_eq!(run(src), "1 false\n2 false\n3 false\nundefined true\n");
}

#[test]
fn generator_two_way_send() {
    let src = r#"
        пиздюли д() {
            гыы а = поебалу 1;
            гыы б = поебалу 2;
            отвечаю а + б;
        }
        ясенХуй г = д();
        сказать(г.следующий().значение);
        сказать(г.следующий(10).значение);
        сказать(г.следующий(20).значение);
    "#;
    assert_eq!(run(src), "1\n2\n30\n");
}

#[test]
fn generator_for_of_and_spread() {
    let src = r#"
        пиздюли диап(н) { го (гыы и = 0; и < н; и += 1) { поебалу и; } }
        гыы с = 0;
        го (гыы х сашаГрей диап(5)) { с = с + х; }
        сказать(с);
        сказать([...диап(3)]);
    "#;
    assert_eq!(run(src), "10\n[0, 1, 2]\n");
}

#[test]
fn generator_delegate_captures_return() {
    let src = r#"
        пиздюли внутр() { поебалу "а"; поебалу "б"; отвечаю "Р"; }
        пиздюли внеш() {
            гыы з = поебалуна внутр();
            поебалу з;
        }
        гыы вых = "";
        го (гыы т сашаГрей внеш()) { вых = вых + т; }
        сказать(вых);
    "#;
    assert_eq!(run(src), "абР\n");
}

#[test]
fn generator_return_runs_finally_once() {
    let src = r#"
        гыы лог = "";
        пиздюли г() {
            хапнуть { поебалу 1; поебалу 2; } тюряжка { лог = лог + "ф;"; }
        }
        ясенХуй ит = г();
        ит.следующий();
        ясенХуй р = ит.вернуть(7);
        сказать(р.значение + " " + р.готово + " " + лог);
    "#;
    assert_eq!(run(src), "7 true ф;\n");
}

#[test]
fn generator_throw_caught_inside() {
    let src = r#"
        пиздюли г() {
            хапнуть { поебалу "а"; поебалу "б"; }
            гоп (е) { сказать("поймал " + е); поебалу "восст"; }
        }
        ясенХуй ит = г();
        сказать(ит.следующий().значение);
        сказать(ит.кинуть("X").значение);
    "#;
    assert_eq!(run(src), "а\nпоймал X\nвосст\n");
}

#[test]
fn generator_for_of_break_closes() {
    let src = r#"
        пиздюли беск() {
            го (гыы и = 0; правда; и += 1) {
                хапнуть { поебалу и; } тюряжка { сказать("закрыт " + и); }
            }
        }
        го (гыы х сашаГрей беск()) {
            сказать("итер " + х);
            вилкойвглаз (х >= 1) { харэ; }
        }
    "#;
    assert_eq!(run(src), "итер 0\nзакрыт 0\nитер 1\nзакрыт 1\n");
}

#[test]
fn yield_outside_generator_is_compile_error() {
    assert!(compile_program(&parse("йопта об() { поебалу 1; } об();")).is_err());
}

#[test]
fn empty_array_is_truthy() {
    assert_eq!(run(r#"вилкойвглаз ([]) { сказать("правда"); } иливжопураз { сказать("лож"); }"#), "правда\n");
}

#[test]
fn object_preserves_insertion_order_after_reassignment() {
    assert_eq!(
        run(r#"гыы о = { б: 1, а: 2, в: 3 }; о.б = 9; сказать(Жсон.вСтроку(о));"#),
        "{\"б\":9,\"а\":2,\"в\":3}\n"
    );
}

#[test]
fn global_redefinition_takes_latest_value() {
    assert_eq!(run("гыы х = 1; гыы х = 2; сказать(х);"), "2\n");
}

#[test]
fn nan_global_alias_matches_interpreter() {
    for (src, expected) in [
        ("сказать(нихуя);", "NaN\n"),
        ("сказать(Хуйня.нихуя(нихуя));", "true\n"),
        ("сказать(нихуя == нихуя, нихуя + 1, тип(нихуя));", "false NaN число\n"),
    ] {
        assert_same_as_interpreter(src, expected);
    }
    assert!(run_err("нихуя = 1;").contains("нихуя"));
}

#[test]
fn is_error_builtin_matches_interpreter() {
    for (src, expected) in [
        ("сказать(тип(этоКосяк));", "функция\n"),
        (
            "сказать(этоКосяк(Косяк(\"x\")), этоКосяк(1), этоКосяк({}), этоКосяк(ноль), этоКосяк(этоКосяк));",
            "true false false false false\n",
        ),
        ("хапнуть { кидай Косяк(\"бум\"); } гоп (е) { сказать(этоКосяк(е)); }", "true\n"),
        ("хапнуть { кидай 1; } гоп (е) { сказать(этоКосяк(е)); }", "false\n"),
        ("гыы ф = этоКосяк; сказать(ф(Косяк(\"y\")));", "true\n"),
        ("+этоКосяк;", ""),
    ] {
        assert_same_as_interpreter(src, expected);
    }
    assert!(run_err("этоКосяк();").contains("этоКосяк"));
}

#[test]
fn regexp_crosses_the_stdlib_bridge_both_ways() {
    for (src, expected) in [
        ("сказать(Жсон.вСтроку(/a/g), Жсон.вСтроку({р: /a/}));", "{} {\"р\":{}}\n"),
        ("сказать(Кент.значения({р: /x/i}), Кент.значения({р: /x/i})[0].flags);", "[/x/i] i\n"),
        ("гыы р = /b/g; сказать(Кент.ключи({р}), р.exec(\"abab\").index, р.lastIndex);", "[р] 1 2\n"),
        (
            "гыы р = /b/g; гыы к = Кент.значения({р})[0]; к.exec(\"abab\"); сказать(р.lastIndex, к.lastIndex, к.test(\"b\"));",
            "2 2 true\n",
        ),
        ("гыы п = Посредник({}, { get: (ц, к) => /a/g }); сказать(п.x.test(\"a\"), п.x.source);", "true a\n"),
        ("гыы п = Посредник({ р: /q/ }, {}); сказать(Кент.ключи(п), п.р.flags);", "[р] \n"),
        (
            "гыы п = Посредник({}, { set: (ц, к, з) => { сказать(тип(з), з.source); отвечаю правда; } }); п.р = /zz/;",
            "регэксп zz\n",
        ),
    ] {
        assert_same_as_interpreter(src, expected);
    }
    assert!(run_err("Матан.макс(/a/, 1);").contains("регэксп"));
}

#[test]
fn object_pattern_on_non_object_matches_interpreter() {
    for (src, expected) in [
        ("гыы {длина, х, у = 7} = \"abc\"; сказать(длина, х, у);", "3 undefined 7\n"),
        ("гыы {длина: д} = [1, 2]; сказать(д);", "2\n"),
        ("гыы п = Посредник({}, { get: (ц, к) => к + \"!\" }); гыы {а, б} = п; сказать(а, б);", "а! б!\n"),
        ("йопта ф({длина: л}) { отвечаю л; } сказать(ф(\"abcd\"));", "4\n"),
        ("гыы д = 0; ({длина: д} = \"xy\"); сказать(д);", "2\n"),
    ] {
        assert_same_as_interpreter(src, expected);
    }
    for (src, vm_err, interp_err) in [
        ("гыы {х} = 42;", "нельзя читать свойство 'х' у типа 'число'", "Нельзя получить свойство у типа 'число'"),
        ("гыы {х} = ноль;", "нельзя читать свойство 'х' у типа 'нулл'", "Нельзя получить свойство у типа 'нулл'"),
        ("гыы {длина, ...р} = \"abc\";", "Невозможно деструктурировать", "Невозможно деструктурировать"),
    ] {
        assert!(run_err(src).contains(vm_err), "исходник: {src}");
        assert!(run_interp_err(src).contains(interp_err), "исходник: {src}");
    }
}

#[test]
fn stdin_builtins_resolve_as_functions() {
    for (src, expected) in [
        ("сказать(тип(прочестьСтроку), тип(прочестьВсё));", "функция функция\n"),
        ("сказать(прочестьСтроку, прочестьВсё);", "[встроенная прочестьСтроку] [встроенная прочестьВсё]\n"),
    ] {
        assert_same_as_interpreter(src, expected);
    }
}

#[test]
fn recursive_getter_overflows_gracefully_not_crash() {
    let err = run_err(
        r#"
        клёво К { get значение() { отвечаю тырыпыры.значение; } }
        гыы к = захуярить К();
        сказать(к.значение);
        "#,
    );
    assert!(err.contains("переполнение стека вызовов"), "ожидалась ловимая ошибка глубины, получено: {err}");
}

#[test]
fn bridge_identity_cache_stays_bounded() {
    run(r#"
        го (гыы и = 0; и < 8192; и++) {
            Жсон.вСтроку([и]);
        }
        "#);
    let len = crate::bridge::identity_cache_len();
    assert!(len <= 2 * crate::bridge::IDENTITY_CACHE_PRUNE_THRESHOLD, "кэш идентичности разросся неограниченно: {len}");
}

#[test]
fn bridge_identity_cache_threshold_grows_for_many_live_objects() {
    run(r#"
        гыы хранилище = [];
        го (гыы и = 0; и < 4000; и++) {
            гыы обж = [и];
            хранилище.втолкнуть(обж);
            Жсон.вСтроку(обж);
        }
        "#);
    let prune_at = crate::bridge::identity_cache_prune_at();
    assert!(
        prune_at > crate::bridge::IDENTITY_CACHE_PRUNE_THRESHOLD,
        "порог не вырос при большом числе живых объектов: {prune_at}"
    );
}

const USER_ITER_OBJ: &str = r#"
гыы об = { значения: [1, 2, 3] };
об[Симбол.итератор] = йопта() {
    гыы источник = об.значения;
    гыы индекс = 0;
    отвечаю {
        следующий: йопта() {
            вилкойвглаз (индекс < длина(источник)) {
                гыы зн = источник[индекс];
                индекс = индекс + 1;
                отвечаю { значение: зн, готово: лож };
            } иливжопураз {
                отвечаю { значение: ноль, готово: правда };
            }
        }
    };
};
"#;

#[test]
fn spread_array_literal_honors_user_symbol_iterator() {
    let src = format!("{USER_ITER_OBJ}\nсказать([...об]);");
    assert_eq!(run(&src), "[1, 2, 3]\n");
}

#[test]
fn spread_array_literal_mixes_user_iterator_with_elements() {
    let src = format!("{USER_ITER_OBJ}\nсказать([0, ...об, 99]);");
    assert_eq!(run(&src), "[0, 1, 2, 3, 99]\n");
}

#[test]
fn spread_call_args_honors_user_symbol_iterator() {
    let src = format!("{USER_ITER_OBJ}\nйопта ф(а, б, в) {{ отвечаю [а, б, в]; }}\nсказать(ф(...об));");
    assert_eq!(run(&src), "[1, 2, 3]\n");
}

#[test]
fn for_of_honors_user_symbol_iterator() {
    let src = format!("{USER_ITER_OBJ}\nгыы с = 0;\nго (гыы х сашаГрей об) {{ с = с + х; }}\nсказать(с);");
    assert_eq!(run(&src), "6\n");
}

#[test]
fn spread_user_iterator_immediately_done_yields_empty_array() {
    let src = r#"
        гыы об = {};
        об[Симбол.итератор] = йопта() {
            отвечаю { следующий: йопта() { отвечаю { значение: ноль, готово: правда }; } };
        };
        сказать([...об]);
    "#;
    assert_eq!(run(src), "[]\n");
}

#[test]
fn spread_user_iterator_next_throws_is_catchable() {
    let src = r#"
        гыы об = {};
        об[Симбол.итератор] = йопта() {
            отвечаю { следующий: йопта() { кидай "бум"; } };
        };
        гыы поймали = ноль;
        хапнуть {
            гыы рез = [...об];
        } гоп (е) {
            поймали = е;
        }
        сказать(поймали);
    "#;
    assert_eq!(run(src), "бум\n");
}

#[test]
fn spread_object_without_iterator_keeps_error() {
    assert_eq!(run_err("гыы об = { а: 1 }; гыы рез = [...об];"), "Нельзя развернуть тип 'объект' в массив");
}

#[test]
fn destructure_array_honors_user_symbol_iterator() {
    let src = format!("{USER_ITER_OBJ}\nгыы [а, б] = об;\nсказать(а);\nсказать(б);");
    assert_eq!(run(&src), "1\n2\n");
}

#[test]
fn destructure_array_rest_honors_user_symbol_iterator() {
    let src = format!("{USER_ITER_OBJ}\nгыы [а, ...ост] = об;\nсказать(а);\nсказать(ост);");
    assert_eq!(run(&src), "1\n[2, 3]\n");
}

#[test]
fn destructure_array_param_honors_user_symbol_iterator() {
    let src = format!("{USER_ITER_OBJ}\nйопта ф([а, б]) {{ отвечаю а + б; }}\nсказать(ф(об));");
    assert_eq!(run(&src), "3\n");
}

#[test]
fn destructure_nested_array_honors_user_symbol_iterator() {
    let src = r#"
        гыы об = { значения: [1, 2, 3] };
        об[Симбол.итератор] = йопта() {
            гыы источник = об.значения;
            гыы индекс = 0;
            отвечаю {
                следующий: йопта() {
                    вилкойвглаз (индекс < длина(источник)) {
                        гыы зн = источник[индекс];
                        индекс = индекс + 1;
                        отвечаю { значение: зн, готово: лож };
                    } иливжопураз {
                        отвечаю { значение: ноль, готово: правда };
                    }
                }
            };
        };
        гыы внешний = { значения: [об, 99] };
        внешний[Симбол.итератор] = йопта() {
            гыы источник = внешний.значения;
            гыы индекс = 0;
            отвечаю {
                следующий: йопта() {
                    вилкойвглаз (индекс < длина(источник)) {
                        гыы зн = источник[индекс];
                        индекс = индекс + 1;
                        отвечаю { значение: зн, готово: лож };
                    } иливжопураз {
                        отвечаю { значение: ноль, готово: правда };
                    }
                }
            };
        };
        гыы [[х], у] = внешний;
        сказать(х);
        сказать(у);
    "#;
    assert_eq!(run(src), "1\n99\n");
}

#[test]
fn destructure_array_rest_non_iterable_object_keeps_error() {
    let src = "гыы об = { а: 1 }; гыы [а, ...ост] = об;";
    assert_eq!(run_err(src), "Невозможно деструктурировать объект как массив");
}

#[test]
fn destructure_array_user_iterator_next_throws_is_catchable() {
    let src = r#"
        гыы об = {};
        об[Симбол.итератор] = йопта() {
            отвечаю { следующий: йопта() { кидай "бум"; } };
        };
        гыы поймали = ноль;
        хапнуть {
            гыы [а, б] = об;
        } гоп (е) {
            поймали = е;
        }
        сказать(поймали);
    "#;
    assert_eq!(run(src), "бум\n");
}

#[test]
fn string_instance_methods_dispatch() {
    assert_eq!(run(r#"сказать("привет".символВ(0));"#), "п\n");
    assert_eq!(run(r#"сказать("Hello".вВерхнийРегистр());"#), "HELLO\n");
    assert_eq!(run(r#"сказать("a,b,c".разбить(",").склеить("|"));"#), "a|b|c\n");
    assert_eq!(run(r#"сказать("  край  ".обрезать());"#), "край\n");
    assert_eq!(run(r#"сказать("ха".повторить(3));"#), "хахаха\n");
}

#[test]
fn string_length_property_is_utf16() {
    assert_eq!(run(r#"сказать("привет".длина);"#), "6\n");
    assert_eq!(run(r#"сказать("привет".length);"#), "6\n");
    assert_eq!(run(r#"сказать("👍".длина);"#), "2\n");
}

#[test]
fn array_length_property() {
    assert_eq!(run("сказать([1,2,3].длина);"), "3\n");
    assert_eq!(run("сказать([1,2,3].length);"), "3\n");
}

#[test]
fn array_callback_methods_dispatch() {
    assert_eq!(run("сказать([1,2,3].преобразовать(х => х * 2).склеить(\",\"));"), "2,4,6\n");
    assert_eq!(run("сказать([1,2,3,4].отфильтровать(х => х % 2 === 0).склеить(\",\"));"), "2,4\n");
    assert_eq!(run("сказать([1,2,3].свернуть((а, б) => а + б, 0));"), "6\n");
}

#[test]
fn array_mutation_shares_native_backing() {
    let src = r#"
        гыы а = [1, 2, 3];
        гыы б = а;
        б.втолкнуть(4);
        сказать(а.склеить(","));
    "#;
    assert_eq!(run(src), "1,2,3,4\n");
}

#[test]
fn array_sort_in_place_mutates_original() {
    let src = r#"
        гыы а = [3, 1, 2];
        а.сортировать((x, y) => x - y);
        сказать(а.склеить(","));
    "#;
    assert_eq!(run(src), "1,2,3\n");
}

#[test]
fn splice_returns_removed_and_mutates() {
    let src = r#"
        гыы а = [1, 2, 3, 4, 5];
        сказать(а.вырезать(1, 2).склеить(","));
        сказать(а.склеить(","));
    "#;
    assert_eq!(run(src), "2,3\n1,4,5\n");
}

#[test]
fn string_bound_method_extraction_is_callable() {
    let src = r#"
        гыы ф = "abc".вВерхнийРегистр;
        сказать(тип(ф));
        сказать(ф());
    "#;
    assert_eq!(run(src), "функция\nABC\n");
}

#[test]
fn array_bound_method_extraction_is_callable() {
    let src = r#"
        гыы ф = [1, 2, 3].склеить;
        сказать(ф("-"));
    "#;
    assert_eq!(run(src), "1-2-3\n");
}

#[test]
fn array_iterator_manual_next() {
    let src = r#"
        гыы ит = [7, 8].значения();
        сказать(ит.следующий().значение);
        сказать(ит.следующий().значение);
        сказать(ит.следующий().готово);
    "#;
    assert_eq!(run(src), "7\n8\ntrue\n");
}

#[test]
fn array_iterator_spread_and_type() {
    assert_eq!(run("сказать([...[10, 20].ключи()].склеить(\",\"));"), "0,1\n");
    assert_eq!(run("сказать(тип([1, 2].записи()));"), "итератор\n");
    assert_eq!(run("сказать([...[\"a\", \"b\"].записи()][1].склеить(\",\"));"), "1,b\n");
}

#[test]
fn string_regex_methods_with_literal() {
    assert_eq!(run(r#"сказать("hello".заменить(/l/g, "L"));"#), "heLLo\n");
    assert_eq!(run(r#"сказать("a1b2".заменить(/\d/g, ц => "[" + ц + "]"));"#), "a[1]b[2]\n");
    assert_eq!(run(r#"сказать("a1b2c3".совпадает(/\d/g).склеить(","));"#), "1,2,3\n");
    assert_eq!(run(r#"сказать("a,b;c".разбить(/[,;]/).склеить("|"));"#), "a|b|c\n");
}

#[test]
fn string_method_argument_error_is_catchable() {
    let src = r#"
        хапнуть { "x".символВ(); } гоп (е) { сказать(е.message); }
    "#;
    assert_eq!(run(src), "'charAt' ожидает минимум 1 аргумент(ов), получено 0\n");
}

#[test]
fn unknown_string_property_is_undefined() {
    assert_eq!(run(r#"сказать("abc".нетуТакого);"#), "undefined\n");
}

#[test]
fn unknown_array_property_is_undefined() {
    assert_eq!(run("сказать([1,2].нетуТакого);"), "undefined\n");
}

#[test]
fn async_generator_for_await_collects() {
    let src = r#"
        ассо пиздюли ген() {
            поебалу 1;
            поебалу 2;
            поебалу 3;
        }
        ассо йопта потр() {
            го сидетьНахуй (гыы х сашаГрей ген()) {
                сказать(х);
            }
        }
        потр();
    "#;
    assert_eq!(run(src), "1\n2\n3\n");
}

#[test]
fn async_generator_await_yield_interleave() {
    let src = r#"
        ассо йопта удвоить(н) { отвечаю н * 2; }
        ассо пиздюли ген() {
            гыы а = сидетьНахуй удвоить(1);
            поебалу а;
            гыы б = сидетьНахуй удвоить(а);
            поебалу б;
        }
        ассо йопта потр() {
            го сидетьНахуй (гыы х сашаГрей ген()) { сказать(х); }
        }
        потр();
    "#;
    assert_eq!(run(src), "2\n4\n");
}

#[test]
fn async_generator_next_returns_promise() {
    let src = r#"
        ассо пиздюли ген() { поебалу 1; }
        гыы ит = ген();
        сказать(тип(ит.следующий()));
    "#;
    assert_eq!(run(src), "обещание\n");
}

#[test]
fn async_generator_for_await_sync_promises() {
    let src = r#"
        ассо йопта потр() {
            гыы п = [СловоПацана.решить(1), СловоПацана.решить(2), СловоПацана.решить(3)];
            гыы сумма = 0;
            го сидетьНахуй (гыы х сашаГрей п) { сумма += х; }
            сказать(сумма);
        }
        потр();
    "#;
    assert_eq!(run(src), "6\n");
}

#[test]
fn async_generator_break_runs_finally() {
    let src = r#"
        ассо пиздюли ген() {
            хапнуть {
                поебалу 1;
                поебалу 2;
                поебалу 3;
            } тюряжка {
                сказать("финал");
            }
        }
        ассо йопта потр() {
            го сидетьНахуй (гыы х сашаГрей ген()) {
                сказать(х);
                вилкойвглаз (х === 2) { харэ; }
            }
        }
        потр();
    "#;
    assert_eq!(run(src), "1\n2\nфинал\n");
}

#[test]
fn async_generator_error_propagates_to_catch() {
    let src = r#"
        ассо пиздюли ген() {
            поебалу 1;
            кидай "бабах";
        }
        ассо йопта потр() {
            хапнуть {
                го сидетьНахуй (гыы х сашаГрей ген()) { сказать(х); }
            } гоп (е) {
                сказать("поймал " + е);
            }
        }
        потр();
    "#;
    assert_eq!(run(src), "1\nпоймал бабах\n");
}

#[test]
fn async_generator_yield_delegate_sync_iterable() {
    let src = r#"
        ассо пиздюли ген() {
            поебалуна [1, 2];
            поебалу 3;
        }
        ассо йопта потр() {
            го сидетьНахуй (гыы х сашаГрей ген()) { сказать(х); }
        }
        потр();
    "#;
    assert_eq!(run(src), "1\n2\n3\n");
}

#[test]
fn async_generator_function_expression() {
    let src = r#"
        гыы ген = ассо пиздюли() {
            поебалу 10;
            поебалу 20;
        };
        ассо йопта потр() {
            го сидетьНахуй (гыы х сашаГрей ген()) { сказать(х); }
        }
        потр();
    "#;
    assert_eq!(run(src), "10\n20\n");
}

#[test]
fn for_await_over_user_async_iterator_vm() {
    let src = r#"
        гыы объект = {
            [Симбол.асинхИтератор]: йопта() {
                гыы и = 0;
                отвечаю {
                    следующий: ассо йопта() {
                        вилкойвглаз (и < 3) { и += 1; отвечаю { значение: и, готово: лож }; }
                        отвечаю { значение: ноль, готово: правда };
                    }
                };
            }
        };
        ассо йопта потр() {
            го сидетьНахуй (гыы х сашаГрей объект) { сказать(х); }
        }
        потр();
    "#;
    assert_eq!(run(src), "1\n2\n3\n");
}

#[test]
fn async_generator_sync_spread_rejected_vm() {
    let src = r#"
        ассо пиздюли ген() { поебалу 1; }
        гыы а = [...ген()];
    "#;
    assert!(run_err(src).contains("синхронно"));
}

#[test]
fn async_generator_sync_for_of_rejected_vm() {
    let src = r#"
        ассо пиздюли ген() { поебалу 1; }
        го (гыы х сашаГрей ген()) { сказать(х); }
    "#;
    assert!(run_err(src).contains("синхронно"));
}

fn run_vm(src: &str) -> Vm {
    let mut vm = Vm::with_writer(Box::new(std::io::sink()));
    let proto = compile_program(&parse(src)).expect("компиляция");
    vm.run(proto).expect("выполнение VM");
    vm
}

#[test]
fn gc_breaks_object_cycles() {
    let src = r#"
        йопта делать() {
            гыы а = {};
            гыы б = {};
            а.друг = б;
            б.друг = а;
        }
        го (гыы и = 0; и < 200; и = и + 1) { делать(); }
    "#;
    let mut vm = run_vm(src);
    let before = vm.live_objects();
    let cleared = vm.collect_cycles();
    let after = vm.live_objects();
    assert!(cleared > 0, "циклы объектов должны разрываться, очищено {cleared}");
    assert!(after < before, "живых объектов должно стать меньше: было {before}, стало {after}");
}

#[test]
fn gc_breaks_closure_upvalue_cycles() {
    let src = r#"
        йопта делать() {
            гыы сам = {};
            сам.ф = () => сам;
        }
        го (гыы и = 0; и < 200; и = и + 1) { делать(); }
    "#;
    let mut vm = run_vm(src);
    let before = vm.live_objects();
    let cleared = vm.collect_cycles();
    assert!(cleared > 0, "циклы замыканий должны разрываться, очищено {cleared}");
    assert!(vm.live_objects() < before, "живых объектов должно стать меньше после сборки");
}

#[test]
fn gc_second_pass_finds_nothing_new() {
    let src = r#"
        йопта делать() {
            гыы а = {};
            гыы б = {};
            а.друг = б;
            б.друг = а;
        }
        го (гыы и = 0; и < 200; и = и + 1) { делать(); }
    "#;
    let mut vm = run_vm(src);
    vm.collect_cycles();
    assert_eq!(vm.collect_cycles(), 0, "повторная сборка не должна ничего чистить");
}

#[test]
fn gc_keeps_live_objects_bounded() {
    let src = r#"
        йопта мусор() {
            гыы а = {};
            гыы б = {};
            а.друг = б;
            б.друг = а;
        }
        го (гыы и = 0; и < 60000; и = и + 1) { мусор(); }
    "#;
    let vm = run_vm(src);
    let live = vm.live_objects();
    assert!(live < 20000, "живых объектов слишком много без ограничения: {live}");
}

#[test]
fn gc_preserves_reachable_data() {
    let src = r#"
        гыы живой = { имя: "тест", числа: [1, 2, 3], вложенный: { глубина: 42 } };
        йопта мусор() {
            гыы а = {};
            гыы б = {};
            а.друг = б;
            б.друг = а;
        }
        го (гыы и = 0; и < 60000; и = и + 1) { мусор(); }
        сказать(живой.имя);
        сказать(живой.числа[0] + живой.числа[1] + живой.числа[2]);
        сказать(живой.вложенный.глубина);
    "#;
    assert_eq!(run(src), "тест\n6\n42\n");
}

#[test]
fn gc_preserves_reachable_data_explicit_collect() {
    let src = r#"
        гыы живой = { значение: 7, список: [1, 2, 3] };
        живой.список.втолкнуть(живой);
    "#;
    let mut vm = run_vm(src);
    let cleared = vm.collect_cycles();
    assert_eq!(cleared, 0, "достижимый цикл не должен собираться, очищено {cleared}");
}

#[test]
fn gc_preserves_suspended_generator_captures() {
    let src = r#"
        пиздюли ген() {
            гыы секрет = [11, 22, 33];
            поебалу секрет[0];
            поебалу секрет[1];
            поебалу секрет[2];
        }
        ясенХуй г = ген();
        сказать(г.следующий().значение);
        йопта мусор() {
            гыы а = {};
            гыы б = {};
            а.друг = б;
            б.друг = а;
        }
        го (гыы и = 0; и < 60000; и = и + 1) { мусор(); }
        сказать(г.следующий().значение);
        сказать(г.следующий().значение);
    "#;
    assert_eq!(run(src), "11\n22\n33\n");
}

struct SharedWriter(std::rc::Rc<std::cell::RefCell<Vec<u8>>>);

impl std::io::Write for SharedWriter {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.0.borrow_mut().extend_from_slice(b);
        Ok(b.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn gc_manual_collect_preserves_async_pending_roots() {
    let buf = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let mut vm = Vm::with_writer(Box::new(SharedWriter(std::rc::Rc::clone(&buf))));

    let setup = r#"
        ясенХуй п = СловоПацана.сРешалками();
        йопта мусор() {
            гыы а = {};
            гыы б = {};
            а.друг = б;
            б.друг = а;
        }
        йопта настройка() {
            гыы секрет = { значение: 777 };
            п.обещание.потом((з) => { сказать(секрет.значение, з); });
        }
        настройка();
        го (гыы и = 0; и < 200; и = и + 1) { мусор(); }
    "#;
    vm.run(compile_program(&parse(setup)).expect("компиляция")).expect("выполнение VM");

    vm.collect_cycles();

    vm.run(compile_program(&parse("п.решить(777);")).expect("компиляция")).expect("выполнение VM");
    drop(vm);

    let out = String::from_utf8_lossy(&buf.borrow()).into_owned();
    assert_eq!(out, "777 777\n");
}

#[test]
fn const_fold_float_edge_cases_match_runtime() {
    assert_eq!(run("сказать(-0.0);"), run("гыы н = -0.0; сказать(н);"));
    assert_eq!(run("сказать(0.1 + 0.2);"), run("гыы а = 0.1; гыы б = 0.2; сказать(а + б);"));
}

#[test]
fn const_fold_nan_comparisons_are_false() {
    assert_eq!(
        run("сказать((0 / 0) < 1, (0 / 0) > 1, (0 / 0) <= 1, (0 / 0) >= 1, (0 / 0) == (0 / 0));"),
        "false false false false false\n"
    );
}

#[test]
fn const_fold_does_not_touch_cross_type_add() {
    assert_eq!(run(r#"сказать("а" + 1);"#), run(r#"гыы с = "а"; сказать(с + 1);"#));
    assert_eq!(run(r#"сказать(1 + "а");"#), run(r#"гыы н = 1; сказать(н + "а");"#));
}

#[test]
fn const_fold_does_not_touch_partial_short_circuit() {
    assert_eq!(run("гыы х = 5; сказать(правда && х);"), "5\n");
    assert_eq!(run("гыы х = 5; сказать(лож || х);"), "5\n");
    assert_eq!(run("сказать(лож && пропавшая);"), "false\n");
}

#[test]
fn const_fold_bigint_left_to_runtime() {
    assert_eq!(run("сказать(3n + 4n);"), run("гыы а = 3n; сказать(а + 4n);"));
    let msg = run_err("сказать(1n + 1);");
    assert!(msg.contains("смешивать"), "сообщение: {msg}");
}

#[test]
fn const_fold_nested_tree_matches_runtime() {
    assert_eq!(
        run("сказать((1 + 2 * 3 - 4 / 2) ** 2);"),
        run("гыы а = 1; гыы б = 2; гыы в = 3; гыы г = 4; сказать((а + б * в - г / б) ** 2);")
    );
}

#[test]
fn ic_overwrite_keeps_warm_read_valid() {
    let src = r#"
гыы о = { х: 1 };
гыы с = 0;
го (гыы и = 0; и < 5; и = и + 1) { с = с + о.х; }
о.х = 100;
го (гыы и = 0; и < 3; и = и + 1) { с = с + о.х; }
сказать(с);
"#;
    assert_eq!(run(src), "305\n");
}

#[test]
fn ic_new_key_invalidates_read() {
    let src = r#"
гыы о = { х: 1 };
го (гыы и = 0; и < 4; и = и + 1) { сказать(о.х); }
о.ы = 2;
сказать(о.х, о.ы);
"#;
    assert_eq!(run(src), "1\n1\n1\n1\n1 2\n");
}

#[test]
fn ic_removed_key_invalidates_read() {
    let src = r#"
гыы о = { х: 1, ы: 2 };
го (гыы и = 0; и < 3; и = и + 1) { сказать(о.ы); }
ёбнуть о.х;
сказать(о.ы);
"#;
    assert_eq!(run(src), "2\n2\n2\n2\n");
}

#[test]
fn ic_alternating_identities_stay_correct() {
    let src = r#"
йопта прочти(о) { отвечаю о.з; }
гыы а = { з: "А" };
гыы б = { з: "Б" };
го (гыы и = 0; и < 4; и = и + 1) {
    вилкойвглаз (и % 2 == 0) { сказать(прочти(а)); } иливжопураз { сказать(прочти(б)); }
}
"#;
    assert_eq!(run(src), "А\nБ\nА\nБ\n");
}

#[test]
fn ic_frozen_write_blocked_through_warm_site() {
    let src = r#"
йопта пиши(о) { о.n = о.n + 1; отвечаю о.n; }
гыы о = { n: 0 };
го (гыы и = 0; и < 3; и = и + 1) { пиши(о); }
Кент.заморозить(о);
пиши(о);
сказать(о.n);
"#;
    assert_eq!(run(src), "3\n");
}

#[test]
fn ic_getter_side_effect_not_cached() {
    let src = r#"
гыы т = 0;
гыы о = { get к() { т = т + 1; отвечаю т; } };
го (гыы и = 0; и < 3; и = и + 1) { сказать(о.к); }
"#;
    assert_eq!(run(src), "1\n2\n3\n");
}

#[test]
fn ic_class_field_warm_read_and_write() {
    let src = r#"
клёво Я { в = 0; Я(в) { тырыпыры.в = в; } }
гыы а = захуярить Я(3);
гыы с = 0;
го (гыы и = 0; и < 4; и = и + 1) { с = с + а.в; }
а.в = 50;
сказать(с, а.в);
"#;
    assert_eq!(run(src), "12 50\n");
}

#[test]
fn ic_set_alternating_identities() {
    let src = r#"
йопта пиши(о, в) { о.поле = в; отвечаю о.поле; }
гыы а = { поле: 0 };
гыы б = { поле: 0 };
сказать(пиши(а, 11), пиши(б, 22), пиши(а, 33));
"#;
    assert_eq!(run(src), "11 22 33\n");
}

#[test]
fn ic_invoke_alternates_between_base_and_subclass() {
    let src = r#"
клёво А { к() { отвечаю "а"; } }
клёво Б батя А { к() { отвечаю "б"; } }
йопта зови(о) { отвечаю о.к(); }
гыы а = захуярить А();
гыы б = захуярить Б();
сказать(зови(а), зови(б), зови(а), зови(б));
"#;
    assert_eq!(run(src), "а б а б\n");
}

#[test]
fn ic_invoke_own_property_shadows_method() {
    let src = r#"
клёво В { к() { отвечаю "метод"; } }
йопта зови(о) { отвечаю о.к(); }
гыы в = захуярить В();
гыы р = зови(в) + зови(в);
в.к = () => "поле";
сказать(р + зови(в) + зови(в));
"#;
    assert_eq!(run(src), "методметодполеполе\n");
}

#[test]
fn ic_invoke_same_site_two_unrelated_classes() {
    let src = r#"
клёво Г { к() { отвечаю 1; } }
клёво Д { к() { отвечаю 2; } }
йопта зови(о) { отвечаю о.к(); }
сказать(зови(захуярить Г()), зови(захуярить Д()), зови(захуярить Г()));
"#;
    assert_eq!(run(src), "1 2 1\n");
}

#[test]
fn ic_invoke_preserves_this_and_super() {
    let src = r#"
клёво Е {
  Е(н) { тырыпыры.н = н; }
  имя() { отвечаю "Е" + тырыпыры.н; }
  полное() { отвечаю тырыпыры.имя() + "!"; }
}
клёво Ж батя Е {
  Ж(н) { яга(н); }
  имя() { отвечаю яга.имя() + "ж"; }
}
йопта зови(о) { отвечаю о.полное(); }
гыы е = захуярить Е(1);
гыы ж = захуярить Ж(2);
сказать(зови(е), зови(ж), зови(е), зови(ж));
"#;
    assert_eq!(run(src), "Е1! Е2ж! Е1! Е2ж!\n");
}

fn ops(src: &str) -> Vec<crate::chunk::Op> {
    compile_program(&parse(src)).expect("компиляция").chunk.code.clone()
}

fn nested_ops(src: &str, name: &str) -> Vec<crate::chunk::Op> {
    let proto = compile_program(&parse(src)).expect("компиляция");
    proto
        .chunk
        .constants
        .iter()
        .find_map(|c| match c {
            crate::chunk::Constant::Proto(p) if p.name == name => Some(p.chunk.code.clone()),
            _ => None,
        })
        .expect("вложенный прототип")
}

#[test]
fn peephole_folds_not_into_inverted_jump() {
    let code = ops("гыы а = 1; вилкойвглаз (чобля а) { сказать(1); }");
    assert!(!code.contains(&crate::chunk::Op::Not), "{code:?}");
    assert!(code.iter().any(|op| matches!(op, crate::chunk::Op::JumpIfTrue(_))), "{code:?}");
    assert_eq!(run("гыы а = 0; вилкойвглаз (чобля а) { сказать(\"да\"); }"), "да\n");
    assert_eq!(run("гыы а = 1; потрещим (чобля а) { сказать(\"нет\"); }"), "");
}

#[test]
fn peephole_drops_constant_if_branches() {
    let taken = ops("вилкойвглаз (правда) { сказать(1); } иливжопураз { сказать(2); }");
    assert!(!taken.iter().any(|op| matches!(op, crate::chunk::Op::JumpIfFalse(_))), "{taken:?}");
    assert_eq!(taken, ops("{ сказать(1); }"));

    let skipped = ops("вилкойвглаз (лож) { сказать(1); } иливжопураз { сказать(2); }");
    assert_eq!(skipped, ops("{ сказать(2); }"));

    assert_eq!(ops("вилкойвглаз (лож) { сказать(1); }"), ops(""));
    assert_eq!(run("вилкойвглаз (1 хуёвей 2) { сказать(\"да\"); } иливжопураз { сказать(\"нет\"); }"), "да\n");
}

#[test]
fn peephole_drops_constant_while_condition() {
    let code = ops("потрещим (правда) { харэ; }");
    assert!(!code.iter().any(|op| matches!(op, crate::chunk::Op::JumpIfFalse(_))), "{code:?}");
    assert!(!code.contains(&crate::chunk::Op::True), "{code:?}");
    assert_eq!(run("гыы и = 0; потрещим (правда) { и = и + 1; вилкойвглаз (и пизже 3) { харэ; } } сказать(и);"), "4\n");
    assert_eq!(
        run(
            "гыы с = 0; гыы и = 0; потрещим (правда) { и = и + 1; вилкойвглаз (и пизже 5) { харэ; } двигай; с = 99; } сказать(и, с);"
        ),
        "6 0\n"
    );
}

#[test]
fn peephole_drops_dead_push_pop_pairs() {
    let code = nested_ops("йопта ф() { гыы а = 1; а; }", "ф");
    assert!(!code.iter().any(|op| matches!(op, crate::chunk::Op::GetLocal(_))), "{code:?}");
    let code = ops("1; правда; ноль; неибу;");
    assert!(!code.contains(&crate::chunk::Op::Pop), "{code:?}");
    assert_eq!(code, ops(""));
    assert_eq!(run("йопта ф() { гыы а = 1; а; отвечаю а; } сказать(ф());"), "1\n");
}

#[test]
fn ic_invoke_survives_a_collection_between_calls() {
    let src = r#"
клёво Щ {
  Щ(н) { тырыпыры.н = н; }
  дай() { отвечаю тырыпыры.н; }
}
йопта зови(о) { отвечаю о.дай(); }
йопта мусор() {
    гыы а = {};
    гыы б = {};
    а.друг = б;
    б.друг = а;
}
гыы сумма = 0;
го (гыы к = 0; к < 3; к = к + 1) { сумма = сумма + зови(захуярить Щ(к)); }
го (гыы и = 0; и < 60000; и = и + 1) { мусор(); }
сумма = сумма + зови(захуярить Щ(10));
сказать(сумма);
"#;
    assert_eq!(run(src), "13\n");
    let mut vm = run_vm(src);
    let live = vm.live_objects();
    assert!(live < 20000, "сборка не сработала, живых объектов: {live}");
    vm.collect_cycles();
}

#[test]
fn ic_invoke_depth_overflow_is_catchable() {
    let src = r#"
клёво Р {
  дай() { отвечаю 7; }
  глубже(н) { отвечаю тырыпыры.глубже(н + 1); }
}
гыы р = захуярить Р();
хапнуть { р.глубже(0); } гоп(е) { сказать("поймал"); }
сказать(р.дай());
"#;
    assert_eq!(run(src), "поймал\n7\n");
}

#[test]
fn peephole_barrier_keeps_inverted_jump_target() {
    let code = nested_ops("йопта ф(а) { вилкойвглаз (чобля а) { отвечаю 1; } иливжопураз { отвечаю 2; } }", "ф");
    assert!(!code.contains(&crate::chunk::Op::Not), "{code:?}");
    let (at, target) = code
        .iter()
        .enumerate()
        .find_map(|(i, op)| match op {
            crate::chunk::Op::JumpIfTrue(t) => Some((i, *t)),
            _ => None,
        })
        .expect("инвертированный переход");
    assert!(target > at && target < code.len(), "{code:?}");
    assert!(matches!(code[target - 1], crate::chunk::Op::Jump(_)), "{code:?}");
    assert_eq!(
        run("йопта ф(а) { вилкойвглаз (чобля а) { отвечаю 1; } иливжопураз { отвечаю 2; } } сказать(ф(0), ф(1));"),
        "1 2\n"
    );
}

#[test]
fn peephole_barrier_keeps_ternary_statement_pop() {
    let code = ops("гыы а = 1; а ? 10 : 20;");
    let pop_at = code.iter().position(|op| *op == crate::chunk::Op::Pop).expect("Pop после тернарника");
    assert!(matches!(code[pop_at - 1], crate::chunk::Op::Constant(_)), "{code:?}");
    assert_eq!(run("гыы а = 1; а ? 10 : 20; сказать(а);"), "1\n");
}

#[test]
fn peephole_barrier_keeps_continue_target() {
    let code = nested_ops("йопта ф() { гыы и = 0; потрещим (и хуёвей 5) { и = и + 1; 7; двигай; } отвечаю и; }", "ф");
    let targets: Vec<usize> = code
        .iter()
        .filter_map(|op| match op {
            crate::chunk::Op::Jump(t) => Some(*t),
            _ => None,
        })
        .collect();
    assert_eq!(targets.len(), 2, "{code:?}");
    assert!(targets.iter().all(|t| matches!(code[*t], crate::chunk::Op::GetLocal(_))), "{code:?}");
    assert_eq!(
        run("йопта ф() { гыы и = 0; потрещим (и хуёвей 5) { и = и + 1; 7; двигай; } отвечаю и; } сказать(ф());"),
        "5\n"
    );
}

#[test]
fn peephole_labeled_break_leaves_constant_true_loop() {
    let src = r#"
гыы с = 0;
внешний: потрещим (правда) {
    потрещим (правда) {
        с = с + 1;
        вилкойвглаз (с пизже 3) { харэ внешний; }
        харэ;
    }
    с = с + 10;
}
сказать(с);
"#;
    assert_eq!(run(src), "12\n");
    assert_eq!(run(src), run_interp(src));
}

struct ModuleDir(std::path::PathBuf);

impl ModuleDir {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("yps_vm_modules_{tag}_{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("создать каталог модулей");
        Self(dir)
    }

    fn write(&self, name: &str, code: &str) {
        std::fs::write(self.0.join(name), code).expect("записать модуль");
    }
}

impl Drop for ModuleDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn run_registered(dir: &ModuleDir, main: &str) -> (Rc<RefCell<yps_lexer::Sources>>, Result<(), VmError>) {
    let sources = Rc::new(RefCell::new(yps_lexer::Sources::default()));
    let source = sources.borrow_mut().add("main.yopta".to_string(), main.to_string());
    let (tokens, lex_diags) = Lexer::new(&source).tokenize();
    assert!(lex_diags.is_empty(), "{lex_diags:?}");
    let (program, parse_diags) = Parser::new(&tokens, &source).parse_program();
    assert!(parse_diags.is_empty(), "{parse_diags:?}");
    let proto = compile_program(&program).expect("компиляция");
    let mut vm = Vm::with_writer(Box::new(std::io::sink()));
    vm.set_base_path(dir.0.clone());
    vm.set_sources(Rc::clone(&sources));
    let result = vm.run(proto).map(|_| ());
    (sources, result)
}

#[test]
fn error_inside_a_module_function_resolves_to_the_module_source() {
    let dir = ModuleDir::new("fn");
    dir.write("m.yopta", "\nпредъява йопта упасть() {\n  гыы о = ноль;\n  отвечаю о.поле;\n}\n");

    let (sources, result) = run_registered(&dir, "спиздить { упасть } из \"m\";\nупасть();\n");

    let err = result.expect_err("обращение к полю ноль должно упасть");
    let sources = sources.borrow();
    let owner = sources.lookup(err.span.start).expect("спан ошибки должен принадлежать исходнику");
    assert!(owner.name.ends_with("m.yopta"), "ошибка приписана {}", owner.name);
    assert_eq!(owner.position(err.span.start), (4, 11));
}

#[test]
fn nested_module_inherits_the_source_registry() {
    let dir = ModuleDir::new("nested");
    dir.write("inner.yopta", "предъява йопта упасть() {\n  гыы о = ноль;\n  отвечаю о.поле;\n}\n");
    dir.write("outer.yopta", "спиздить { упасть } из \"inner\";\nпредъява гыы итог = упасть();\n");

    let (sources, result) = run_registered(&dir, "спиздить { итог } из \"outer\";\n");

    let err = result.expect_err("вложенный модуль должен упасть");
    let sources = sources.borrow();
    let owner = sources.lookup(err.span.start).expect("спан ошибки должен принадлежать исходнику");
    assert!(owner.name.ends_with("inner.yopta"), "ошибка приписана {}: {}", owner.name, err.message);
    assert_eq!(owner.position(err.span.start), (3, 11));
}

#[test]
fn exported_function_reads_and_writes_its_own_module_globals() {
    let dir = ModuleDir::new("globals");
    dir.write("inner.yopta", "предъява йопта дай() { отвечаю 1; }\n");
    dir.write(
        "outer.yopta",
        "спиздить { дай } из \"inner\";\nгыы счёт = 10;\nпредъява йопта шаг() { счёт = счёт + дай(); отвечаю счёт; }\n",
    );

    let (_, result) = run_registered(
        &dir,
        "спиздить { шаг } из \"outer\";\nгыы счёт = 100;\nшаг();\nвилкойвглаз (шаг() !== 12 || счёт !== 100) { кидай \"мимо\"; }\n",
    );

    result.expect("функция модуля должна видеть глобалы своего модуля");
}

fn vm_error(src: &str) -> VmError {
    let proto = compile_program(&parse(src)).expect("компиляция");
    let mut vm = Vm::with_writer(Box::new(std::io::sink()));
    vm.run(proto).expect_err("ожидалась ошибка выполнения")
}

fn vm_error_position(src: &str) -> (usize, usize) {
    SourceFile::new("<тест>".to_string(), src.to_string()).position(vm_error(src).span.start)
}

#[test]
fn uncaught_top_level_throw_points_at_the_throw_statement() {
    assert_eq!(vm_error_position("гыы а = 1;\n\n  кидай \"бум\";\n"), (3, 3));
}

#[test]
fn uncaught_throw_inside_a_function_points_at_the_throw_statement() {
    assert_eq!(vm_error_position("йопта ф() {\n  кидай \"бум\";\n}\n\nф();\n"), (2, 3));
}

#[test]
fn uncaught_throw_inside_a_module_function_resolves_to_the_module_source() {
    let dir = ModuleDir::new("throw");
    dir.write("m.yopta", "\nпредъява йопта упасть() {\n  кидай \"из модуля\";\n}\n");

    let (sources, result) = run_registered(&dir, "спиздить { упасть } из \"m\";\nупасть();\n");

    let err = result.expect_err("исключение должно дойти до верха");
    let sources = sources.borrow();
    let owner = sources.lookup(err.span.start).expect("спан ошибки должен принадлежать исходнику");
    assert!(owner.name.ends_with("m.yopta"), "ошибка приписана {}", owner.name);
    assert_eq!(owner.position(err.span.start), (3, 3));
}

#[test]
fn a_syntax_error_in_a_module_is_rendered_as_a_located_diagnostic() {
    let dir = ModuleDir::new("syntax");
    dir.write("bad.yopta", "предъява гыы х = 1;\nгыы у = ;\n");

    let (_sources, result) = run_registered(&dir, "спиздить { х } из \"bad\";\n");

    let err = result.expect_err("модуль с ошибкой разбора");
    assert!(err.message.contains("bad.yopta:2:9: Ошибка: "), "{}", err.message);
}

#[test]
fn a_lexer_error_in_a_module_is_rendered_as_a_located_diagnostic() {
    let dir = ModuleDir::new("lex");
    dir.write("bad.yopta", "предъява гыы х = 1;\nгыы у = §;\n");

    let (_sources, result) = run_registered(&dir, "спиздить { х } из \"bad\";\n");

    let err = result.expect_err("модуль с ошибкой лексера");
    assert!(err.message.contains("bad.yopta:2:9: Ошибка: Неизвестный символ"), "{}", err.message);
}

fn interp_error_position(src: &str) -> (usize, usize) {
    let program = parse(src);
    let mut interp = yps_interpreter::Interpreter::new();
    interp.set_output_sink(Box::new(yps_interpreter::BufferSink::new()));
    let err = interp.run(&program).expect_err("ожидалась ошибка интерпретатора");
    SourceFile::new("<тест>".to_string(), src.to_string()).position(err.span.start)
}

fn assert_both_backends_report(src: &str, expected: (usize, usize)) {
    assert_eq!(interp_error_position(src), expected, "интерпретатор");
    assert_eq!(vm_error_position(src), expected, "VM");
}

#[test]
fn throw_position_survives_a_finally_block_at_top_level() {
    assert_both_backends_report("хапнуть {\n  кидай \"а\";\n} тюряжка {\n  гыы б = 1;\n}\n", (2, 3));
}

#[test]
fn throw_position_survives_a_finally_block_inside_a_function() {
    let src = "йопта ф() {\n  хапнуть {\n    кидай \"а\";\n  } тюряжка {\n    гыы б = 1;\n  }\n}\nф();\n";

    assert_both_backends_report(src, (3, 5));
}

#[test]
fn throw_position_survives_nested_finally_blocks() {
    let src =
        "хапнуть {\n  хапнуть {\n    кидай \"а\";\n  } тюряжка {\n    гыы б = 1;\n  }\n} тюряжка {\n  гыы в = 2;\n}\n";

    assert_both_backends_report(src, (3, 5));
}

#[test]
fn a_throw_from_a_finally_block_replaces_the_pending_position() {
    let src = "хапнуть {\n  кидай \"а\";\n} тюряжка {\n  кидай \"б\";\n}\n";

    assert_both_backends_report(src, (4, 3));
}

#[test]
fn throw_position_survives_a_callback_called_by_a_bridged_builtin() {
    let src = "гыы к = захуярить Карта();\nк.set(1, 2);\nк.forEach(() => {\n  кидай \"бум\";\n});\n";

    assert_both_backends_report(src, (4, 3));
}

#[test]
fn error_position_survives_a_callback_called_by_a_bridged_builtin() {
    let src = "гыы к = захуярить Карта();\nк.set(1, 2);\nк.forEach(() => {\n  гыы о = ноль;\n  о.поле;\n});\n";

    assert_both_backends_report(src, (5, 3));
}

#[test]
fn a_caught_and_rethrown_value_reports_the_rethrow_statement() {
    let src = "хапнуть {\n  кидай \"а\";\n} гоп (е) {\n  кидай е;\n}\n";

    assert_both_backends_report(src, (4, 3));
}

#[test]
fn closure_reads_local_of_suspended_async_function() {
    let src = r#"
        ассо йопта д() {
            гыы к = 5;
            чутка(() => сказать(к), 0);
            сидетьНахуй подождать(5);
        }
        д();
    "#;
    assert_eq!(run(src), "5\n");
    assert_eq!(run(src), run_interp(src));
}

#[test]
fn closure_writes_local_of_suspended_async_function() {
    let src = r#"
        ассо йопта д() {
            гыы к = 1;
            чутка(() => { к = 2; }, 0);
            сидетьНахуй подождать(5);
            сказать(к);
            к = 3;
            сказать(к);
        }
        д();
    "#;
    assert_eq!(run(src), "2\n3\n");
    assert_eq!(run(src), run_interp(src));
}

#[test]
fn closure_shares_local_with_suspended_generator() {
    let src = r#"
        пиздюли г() {
            гыы к = 1;
            поебалу (н) => { к = н; отвечаю к; };
            сказать(к);
            к = 3;
            поебалу 0;
            сказать(к);
        }
        гыы и = г();
        гыы ф = и.следующий().значение;
        сказать(ф(2));
        и.следующий();
        сказать(ф(4));
        и.следующий();
    "#;
    assert_eq!(run(src), "2\n2\n4\n4\n");
    assert_eq!(run(src), run_interp(src));
}

#[test]
fn generator_calls_closure_over_a_local_of_its_caller() {
    let src = r#"
        пиздюли г(прочесть) {
            гыы мусор = 0;
            поебалу прочесть() + мусор;
        }
        йопта ф() {
            гыы н = 7;
            ясенХуй прочесть = () => н;
            отвечаю г(прочесть).следующий().значение;
        }
        сказать(ф());
    "#;
    assert_eq!(run(src), "7\n");
    assert_eq!(run(src), run_interp(src));
}

#[test]
fn console_time_reports_elapsed_for_a_started_label() {
    let out = run("сказать.время(\"м\"); сказать.времяСтоп(\"м\");");
    assert!(out.starts_with("м: ") && out.ends_with(" мс\n"), "вывод: {out:?}");
}

#[test]
fn console_time_stop_without_start_is_an_error() {
    let src = "сказать.времяСтоп(\"нет такой\");";
    assert!(run_err(src).contains("не запущен"));
    assert!(run_interp_err(src).contains("не запущен"));
}

#[test]
fn console_table_matches_interpreter() {
    let src = "сказать.таблица([{ а: 1, б: \"два\" }, { а: 3 }]);";
    assert_eq!(run(src), run_interp(src));
}

fn assert_same_as_interpreter(src: &str, expected: &str) {
    assert_eq!(run(src), expected, "VM, исходник: {src}");
    assert_eq!(run_interp(src), expected, "интерпретатор, исходник: {src}");
}

#[test]
fn abort_unsubscribe_is_callable() {
    assert_same_as_interpreter(
        r#"
        гыы к = захуярить КонтроллёрОтмены();
        гыы отписка = к.сигнал.подписатьсяНаОтмену(() => сказать("нет"));
        отписка();
        к.отменить("стоп");
        сказать("конец");
        "#,
        "конец\n",
    );
}

#[test]
fn abort_listener_on_aborted_signal_runs_as_microtask() {
    assert_same_as_interpreter(
        r#"
        гыы к = захуярить КонтроллёрОтмены();
        к.отменить("стоп");
        к.сигнал.подписатьсяНаОтмену(() => сказать("слушатель"));
        сказать("синхронно");
        "#,
        "синхронно\nслушатель\n",
    );
}

#[test]
fn abort_listener_survives_cycle_collection() {
    let src = r#"
        гыы к = захуярить КонтроллёрОтмены();
        йопта подписать() {
            гыы а = { имя: "жив" };
            а.я = а;
            к.сигнал.подписатьсяНаОтмену(() => сказать(а.я.имя));
        }
        йопта мусор() {
            гыы а = {};
            гыы б = {};
            а.д = б;
            б.д = а;
        }
        подписать();
        го (гыы и = 0; и < 60000; и = и + 1) { мусор(); }
        к.отменить("стоп");
    "#;
    assert_eq!(run(src), "жив\n");
}

#[test]
fn abort_cancels_timers_scheduled_with_a_signal() {
    assert_same_as_interpreter(
        r#"
        гыы к = захуярить КонтроллёрОтмены();
        чутка(() => сказать("чутка"), 1, { сигнал: к.сигнал });
        интервал(() => сказать("интервал"), 1, { сигнал: к.сигнал });
        к.отменить("стоп");
        сказать(чутка(() => сказать("поздно"), 0, { сигнал: к.сигнал }));
        сказать(интервал(() => сказать("поздно"), 0, { сигнал: к.сигнал }));
        "#,
        "0\n0\n",
    );
}

#[test]
fn timer_options_are_validated() {
    for src in [
        "чутка(() => 1, 0, 5);",
        "интервал(() => 1, 0, { сигнал: 5 });",
        "подождать(0, 5);",
        "подождать(0, { сигнал: 5 });",
    ] {
        assert_eq!(run_err(src).split(": ").last(), run_interp_err(src).split(": ").last(), "{src}");
    }
}

#[test]
fn abort_rejects_pending_wait_with_the_signal_reason() {
    assert_same_as_interpreter(
        r#"
        гыы к = захуярить КонтроллёрОтмены();
        подождать(50, { сигнал: к.сигнал }).ловить((е) => сказать("идущий:", е));
        к.отменить("стоп");
        подождать(50, { сигнал: к.сигнал }).ловить((е) => сказать("поздний:", е));
        "#,
        "идущий: стоп\nпоздний: стоп\n",
    );
}

#[test]
fn timeout_signal_aborts_with_a_timeout_error() {
    assert_same_as_interpreter(
        r#"
        гыы с = СигналОтмены.отВремени(1);
        сказать(с.отменён);
        с.обещание.ловить((е) => сказать(е.name, е.message, с.отменён));
        "#,
        "false\nОшибкаОтмены Тайм-аут true\n",
    );
}

#[test]
fn timeout_signal_validates_its_delay() {
    let src = "СигналОтмены.отВремени(\"нет\");";
    assert!(run_err(src).contains("ожидает миллисекунды числом"));
    assert!(run_interp_err(src).contains("ожидает миллисекунды числом"));
}

#[test]
fn signal_promise_is_cached_and_settled_for_an_aborted_signal() {
    assert_same_as_interpreter(
        r#"
        гыы к = захуярить КонтроллёрОтмены();
        к.отменить("стоп");
        гыы с = к.сигнал;
        сказать(с.обещание === с.обещание);
        с.обещание.ловить((е) => сказать("обещание:", е));
        СловоПацана.отПодождать(с).ловить((е) => сказать("отПодождать:", е));
        "#,
        "true\nобещание: стоп\nотПодождать: стоп\n",
    );
}

#[test]
fn from_wait_rejects_a_non_signal() {
    let src = "СловоПацана.отПодождать(42);";
    assert!(run_err(src).contains("ожидает сигнал отмены"));
    assert!(run_interp_err(src).contains("ожидает сигнал отмены"));
}

#[test]
fn coroutine_awaiting_signal_promise_survives_cycle_collection() {
    let src = r#"
        гыы к = захуярить КонтроллёрОтмены();
        ассо йопта ждать() {
            гыы а = { имя: "жив" };
            а.я = а;
            хапнуть {
                сидетьНахуй к.сигнал.обещание;
            } гоп (е) {
                сказать(а.я.имя, е);
            }
        }
        йопта мусор() {
            гыы а = {};
            гыы б = {};
            а.д = б;
            б.д = а;
        }
        ждать();
        го (гыы и = 0; и < 60000; и = и + 1) { мусор(); }
        к.отменить("стоп");
    "#;
    assert_eq!(run(src), "жив стоп\n");
}

#[test]
fn abort_listener_stops_at_its_throw() {
    let src = r#"
        гыы к = захуярить КонтроллёрОтмены();
        к.сигнал.подписатьсяНаОтмену(() => { кидай "б"; сказать("нет"); });
        к.отменить();
        сказать("конец");
    "#;
    assert_same_as_interpreter(src, "конец\n");
}

#[test]
fn timer_callback_stops_at_its_throw() {
    let src = r#"
        чутка(() => { кидай "б"; сказать("нет"); }, 0);
        чутка(() => сказать("конец"), 1);
    "#;
    assert_same_as_interpreter(src, "конец\n");
}

#[test]
fn caught_callback_error_leaves_the_caller_running() {
    let src = r#"
        йопта ф() {
            гыы местная = 7;
            хапнуть {
                Отражение.применить(() => { гыы в = 1; кидай в; }, ноль, []);
            } гоп (е) {
                отвечаю местная + е;
            }
        }
        сказать(ф());
    "#;
    assert_same_as_interpreter(src, "8\n");
}

#[test]
fn closure_keeps_local_after_generator_return_runs_finally() {
    let src = r#"
        гыы ф = ноль;
        пиздюли г() {
            гыы х = 41;
            ф = () => х + 1;
            хапнуть { поебалу 1; } тюряжка { }
        }
        гыы и = г();
        и.следующий();
        и.вернуть(5);
        сказать(ф());
    "#;
    assert_same_as_interpreter(src, "42\n");
}

#[test]
fn abort_listener_may_be_any_callable() {
    assert_same_as_interpreter(
        r#"
        гыы к = захуярить КонтроллёрОтмены();
        гыы отменён = захуярить КонтроллёрОтмены();
        отменён.отменить();
        гыы до = СловоПацана.сРешалками();
        гыы после = СловоПацана.сРешалками();
        к.сигнал.подписатьсяНаОтмену(до.решить);
        отменён.сигнал.подписатьсяНаОтмену(после.решить);
        до.обещание.потом(() => сказать("до"));
        после.обещание.потом(() => сказать("после"));
        к.отменить();
        "#,
        "до\nпосле\n",
    );
}

#[test]
fn abort_listener_may_be_async() {
    assert_same_as_interpreter(
        r#"
        гыы к = захуярить КонтроллёрОтмены();
        к.сигнал.подписатьсяНаОтмену(ассо () => {
            сидетьНахуй подождать(0);
            сказать("после ожидания");
        });
        к.отменить();
        сказать("синхронно");
        "#,
        "синхронно\nпосле ожидания\n",
    );
}

#[test]
fn abort_listener_unsubscribe_is_honoured() {
    assert_same_as_interpreter(
        r#"
        гыы к = захуярить КонтроллёрОтмены();
        гыы отписка = к.сигнал.подписатьсяНаОтмену(() => сказать("отписан"));
        к.сигнал.подписатьсяНаОтмену(() => сказать("остался"));
        отписка();
        к.отменить();
        "#,
        "остался\n",
    );
}

#[test]
fn abort_listener_must_be_callable() {
    let src = "захуярить КонтроллёрОтмены().сигнал.подписатьсяНаОтмену(5);";
    assert!(run_err(src).contains("ожидает функцию"));
    assert!(run_interp_err(src).contains("ожидает функцию"));
}

#[test]
fn symbol_keys_stay_distinct_and_hidden() {
    assert_same_as_interpreter(
        r#"
        гыы а = Симбол("x");
        гыы б = Симбол("x");
        гыы о = {};
        о[а] = 1;
        о[б] = 2;
        сказать(о[а], о[б], Кент.ключи(о).длина);
        сказать(а чоунастут о, о);
        ёбнуть о[а];
        сказать(о[а], о[б]);
        гыы л = { [б]() { отвечаю "метод"; } };
        сказать(л[б]());
        "#,
        "1 2 0\ntrue {}\nundefined 2\nметод\n",
    );
}

#[test]
fn well_known_symbols_are_symbols() {
    assert_same_as_interpreter(
        r#"
        ясенХуй и = Симбол.итератор;
        сказать(тип(и), и === Симбол.итератор, и === Симбол.вПримитив);
        гыы о = {};
        о[и] = () => ({ следующий: () => ({ значение: 7, готово: правда }) });
        сказать([...о].длина, Кент.ключи(о).длина);
        "#,
        "символ true false\n0 0\n",
    );
}

#[test]
fn spread_copies_symbol_keys() {
    assert_same_as_interpreter(
        r#"
        гыы с = Симбол("x");
        гыы о = { в: 2 };
        о[с] = 1;
        гыы копия = { ...о };
        гыы { в, ...остаток } = о;
        сказать(копия[с], остаток[с], в, Кент.ключи(копия).длина);
        "#,
        "1 1 2 1\n",
    );
}

#[test]
fn loose_equality_uses_user_coercion_hooks() {
    assert_same_as_interpreter(
        r#"
        гыы об = { вЧисло: йопта() { отвечаю 42; } };
        сказать(об == 42, 42 == об, об != 42, об == об, об == ноль);
        гыы стр = { вСтроку: йопта() { отвечаю "а"; } };
        сказать(стр == "а", стр == {});
        гыы один = { вЧисло: () => 1 };
        сказать(один == правда);
        "#,
        "true true false true false\ntrue false\ntrue\n",
    );
}

#[test]
fn promise_chain_survives_cycle_collection_inside_a_reaction() {
    let src = r#"
        йопта ф() {
            гыы л = { а: [1, 2, 3] };
            л.сам = л;
            СловоПацана.решить(1)
                .потом(() => {
                    го (гыы и = 0; и < 60000; и++) {
                        гыы о = {};
                        о.с = о;
                    }
                })
                .потом(() => сказать("л", л.а));
        }
        ф();
    "#;
    assert_same_as_interpreter(src, "л [1, 2, 3]\n");
}

#[test]
fn timer_keeps_its_captures_through_cycle_collection_in_the_callback() {
    let src = r#"
        йопта ф() {
            гыы л = { а: 1 };
            л.сам = л;
            чутка(() => {
                го (гыы и = 0; и < 60000; и++) {
                    гыы о = {};
                    о.с = о;
                }
                сказать(л.сам.а);
            }, 0);
        }
        ф();
    "#;
    assert_same_as_interpreter(src, "1\n");
}

#[test]
fn entering_a_running_generator_is_an_error() {
    let src = r#"
        гыы и = ноль;
        пиздюли г() {
            поебалу 1;
            и.следующий();
            поебалу 2;
        }
        и = г();
        сказать(и.следующий().значение);
        хапнуть {
            и.следующий();
        } гоп (е) {
            сказать(е.message);
        }
        сказать(и.следующий());
    "#;
    assert_same_as_interpreter(src, "1\nИтератор уже выполняется\n{значение: undefined, готово: true}\n");
}

#[test]
fn generator_is_finished_after_a_runtime_error() {
    let src = r#"
        пиздюли г() {
            поебалу 1;
            неведомая();
            поебалу 2;
        }
        гыы и = г();
        сказать(и.следующий().значение);
        хапнуть { и.следующий(); } гоп (е) { сказать("поймал"); }
        сказать(и.следующий());
    "#;
    assert_same_as_interpreter(src, "1\nпоймал\n{значение: undefined, готово: true}\n");
}

#[test]
fn stdlib_mutations_reach_the_vm_object() {
    assert_same_as_interpreter(
        r#"
        гыы т = {};
        Кент.назначить(т, { а: 1 });
        сказать(т.а);
        гыы мас = [1];
        гыы о = { мас };
        Кент.назначить(о, { б: 2 });
        сказать(о.мас === мас, о.б);
        "#,
        "1\ntrue 2\n",
    );
}

#[test]
fn stdlib_respects_vm_object_state() {
    assert_same_as_interpreter(
        r#"
        гыы о = { х: 1 };
        Кент.заморозить(о);
        Кент.назначить(о, { х: 2, у: 3 });
        сказать(о.х, о.у, Кент.заморожен(о));
        гыы п = { х: 1 };
        Отражение.запретитьРасширение(п);
        п.у = 2;
        сказать(п.у, Кент.расширяем(п));
        "#,
        "1 undefined true\nundefined false\n",
    );
}

#[test]
fn cyclic_values_cross_the_bridge() {
    assert_same_as_interpreter(
        r#"
        гыы а = { имя: "а" };
        а.я = а;
        сказать(Кент.ключи(а));
        гыы м = [1];
        м[0] = м;
        гыы к = захуярить Карта();
        к.set("м", м);
        к.set("а", а);
        сказать(к.get("а") === а, к.get("а").я === а, к.get("м")[0] === м);
        "#,
        "[имя, я]\ntrue true true\n",
    );
}

#[test]
fn collections_keep_the_identity_of_vm_values() {
    assert_same_as_interpreter(
        r#"
        гыы о = { х: 1 };
        йопта ф() { отвечаю 7; }
        гыы к = захуярить Карта();
        к.set("о", о);
        к.set("ф", ф);
        сказать(к.get("о") === о, к.get("ф") === ф, к.get("ф")());
        о.х = 2;
        сказать(к.get("о").х);
        гыы в = к.get("о");
        в.х = 3;
        сказать(о.х);
        гыы н = захуярить Набор();
        н.add(о);
        сказать(н.has(о), н.has({ х: 3 }));
        "#,
        "true true 7\n2\n3\ntrue false\n",
    );
}

#[test]
fn proxy_shares_its_target_with_the_vm() {
    assert_same_as_interpreter(
        r#"
        гыы ц = { а: 1 };
        гыы п = Посредник(ц, {});
        п.б = 2;
        сказать(ц.б);
        ц.в = 3;
        сказать(п.в, "в" чоунастут п);
        ёбнуть п.а;
        сказать(ц.а, "а" чоунастут п);
        гыы ключ = "б";
        ёбнуть п[ключ];
        сказать(ц.б);
        "#,
        "2\n3 true\nundefined false\nundefined\n",
    );
}

#[test]
fn proxy_trap_sees_the_vm_target() {
    assert_same_as_interpreter(
        r#"
        гыы ц = { а: 1 };
        гыы п = Посредник(ц, {
            установить: (м, к, з) => {
                сказать(м === ц);
                м[к] = з * 2;
                отвечаю правда;
            },
            удалить: (м, к) => {
                сказать("удаляю", к);
                отвечаю ёбнуть м[к];
            }
        });
        п.б = 2;
        сказать(ц.б, п.б);
        ёбнуть п.б;
        сказать(ц.б);
        "#,
        "true\n4 4\nудаляю б\nundefined\n",
    );
}

#[test]
fn abort_reason_keeps_its_identity() {
    assert_same_as_interpreter(
        r#"
        гыы р = { код: 1 };
        гыы к = захуярить КонтроллёрОтмены();
        к.отменить(р);
        сказать(к.сигнал.причина === р);
        хапнуть {
            к.сигнал.выкинутьЕслиОтменён();
        } гоп (е) {
            сказать(е === р);
        }
        к.сигнал.обещание.ловить((е) => сказать(е === р));
        "#,
        "true\ntrue\ntrue\n",
    );
}

#[test]
fn functions_stay_the_same_after_crossing_the_bridge() {
    assert_same_as_interpreter(
        r#"
        йопта ф() { отвечаю 7; }
        гыы о = { ф };
        Кент.назначить(о, { г: ф });
        сказать(о.ф === ф, о.г === ф, о.г());
        "#,
        "true true 7\n",
    );
}

#[test]
fn accessors_defined_through_the_stdlib_work_on_the_vm() {
    assert_same_as_interpreter(
        r#"
        гыы о = {};
        Кент.определитьСвойство(о, "х", { получить: () => 7 });
        сказать(о.х);
        сказать(тип(Кент.описатьСвойство(о, "х").получить));
        "#,
        "7\nфункция\n",
    );
}

#[test]
fn callback_kept_by_the_stdlib_survives_cycle_collection() {
    let src = r#"
        гыы к = захуярить Карта();
        йопта положить() {
            гыы л = { а: 1 };
            л.сам = л;
            к.set("ф", () => л.сам.а);
        }
        положить();
        СловоПацана.решить(1).потом(() => {
            го (гыы и = 0; и < 60000; и++) {
                гыы о = {};
                о.с = о;
            }
            сказать(к.get("ф")());
        });
    "#;
    assert_same_as_interpreter(src, "1\n");
}

#[test]
fn deeply_nested_value_crosses_the_bridge() {
    let src = r#"
        гыы о = {};
        го (гыы и = 0; и < 100000; и++) { о = { в: о }; }
        сказать(Кент.ключи(о));
    "#;
    assert_eq!(run(src), "[в]\n");
}

#[test]
fn dropping_deep_structures_does_not_overflow_the_stack() {
    let src = r#"
        гыы о = {};
        гыы м = [];
        го (гыы и = 0; и < 200000; и++) {
            о = { в: о };
            м = [м];
        }
        о = ноль;
        м = ноль;
        сказать("конец");
    "#;
    assert_eq!(run(src), "конец\n");
}

#[test]
fn objects_inherit_along_the_prototype_chain() {
    assert_same_as_interpreter(
        r#"
        гыы дед = { д: 1, скажи() { отвечаю тырыпыры.имя; } };
        гыы отец = Кент.создать(дед);
        отец.о = 2;
        гыы сын = Кент.создать(отец);
        сын.имя = "сын";
        сказать(сын.д, сын.о, сын.нет, сын.скажи());
        сказать(Кент.прототип(сын) === отец, Отражение.прототипОт(отец) === дед);
        сказать(Кент.ключи(сын), Кент.имеетСвоё(сын, "д"));
        дед.д = 10;
        сказать(сын.д);
        Кент.назначитьПрототип(сын, { д: 99 });
        сказать(сын.д, сын.о);
        Кент.назначитьПрототип(сын, ноль);
        сказать(сын.д, Кент.прототип(сын));
        "#,
        "1 2 undefined сын\ntrue true\n[имя] false\n10\n99 undefined\nundefined null\n",
    );
}

#[test]
fn class_prototype_object_behaves_like_the_class() {
    assert_same_as_interpreter(
        r#"
        клёво База {
            База(имя) { тырыпыры.имя = имя; }
            кто() { отвечаю "база " + тырыпыры.имя; }
        }
        клёво Сын батя База {
            кто() { отвечаю "сын, " + яга.кто(); }
        }
        сказать(Сын.prototype === Сын.прототип, Сын.prototype.конструктор === Сын);
        сказать(тип(Сын.prototype.кто), тип(Сын.prototype.нет));
        гыы о = Кент.создать(Сын.prototype);
        о.имя = "о";
        сказать(о.кто());
        сказать(о шкура Сын, о шкура База, {} шкура База);
        сказать(о.конструктор === Сын, Кент.прототип(о) === Сын.prototype);
        гыы ц = захуярить Сын("ц");
        сказать(Кент.прототип(ц) === Сын.prototype, ц.кто());
        гыы к = Кент.создать(Сын);
        к.имя = "к";
        сказать(к.кто(), к шкура База);
        "#,
        "true true\nфункция неопределено\nсын, база о\ntrue true false\ntrue true\ntrue сын, база ц\nсын, база к true\n",
    );
}

#[test]
fn inherited_accessors_use_the_receiver() {
    assert_same_as_interpreter(
        r#"
        гыы п = { get г() { отвечаю тырыпыры.имя; } };
        гыы о = Кент.создать(п);
        о.имя = "о";
        п.имя = "п";
        сказать(о.г, п.г);
        клёво К {
            get ч() { отвечаю тырыпыры.х; }
        }
        гыы и = Кент.создать(К.prototype);
        и.х = 5;
        сказать(и.ч);
        "#,
        "о п\n5\n",
    );
}

#[test]
fn in_operator_walks_the_prototype_chain() {
    assert_same_as_interpreter(
        r#"
        гыы п = { а: 1, get г() { отвечаю 2; } };
        гыы о = Кент.создать(п);
        сказать("а" чоунастут о, "г" чоунастут о, "нет" чоунастут о);
        клёво К { м() {} get ч() { отвечаю 1; } }
        клёво Н батя К {}
        гыы н = захуярить Н();
        сказать("м" чоунастут н, "ч" чоунастут н, "нет" чоунастут н);
        гыы пр = Посредник(о, {});
        сказать("а" чоунастут пр);
        "#,
        "true true false\ntrue true false\ntrue\n",
    );
}

#[test]
fn iterator_helpers_drive_a_generator_lazily() {
    assert_same_as_interpreter(
        r#"
        пиздюли г() {
            гыы и = 0;
            потрещим (правда) {
                сказать("выдаю", и);
                поебалу и;
                и = и + 1;
            }
        }
        сказать(Итератор.от(г()).преобразовать((н) => н * 10).взять(2).вМассив());
        "#,
        "выдаю 0\nвыдаю 1\n[0, 10]\n",
    );
}

#[test]
fn generator_held_by_an_iterator_survives_cycle_collection() {
    let src = r#"
        пиздюли г() {
            гыы л = { а: 1 };
            л.сам = л;
            поебалу 0;
            поебалу л.сам.а;
        }
        гыы ит = Итератор.от(г());
        ит.следующий();
        СловоПацана.решить(1).потом(() => {
            го (гыы и = 0; и < 60000; и++) {
                гыы о = {};
                о.с = о;
            }
            сказать(ит.следующий().значение);
        });
    "#;
    assert_same_as_interpreter(src, "1\n");
}

#[test]
fn iterator_from_accepts_the_iteration_protocol() {
    assert_same_as_interpreter(
        r#"
        гыы счёт = 0;
        гыы шаги = {
            следующий() {
                счёт = счёт + 1;
                отвечаю { значение: счёт, готово: счёт > 3 };
            }
        };
        сказать(Итератор.от(шаги).преобразовать((х) => х * 2).вМассив());
        гыы набор = {};
        набор[Симбол.итератор] = () => {
            гыы н = 0;
            отвечаю { следующий: () => { н = н + 1; отвечаю { значение: н, готово: н > 2 }; } };
        };
        сказать(Итератор.от(набор).вМассив());
        "#,
        "[2, 4, 6]\n[1, 2]\n",
    );
}

#[test]
fn stdlib_write_back_keeps_newer_vm_state() {
    assert_same_as_interpreter(
        r#"
        гыы а = [1, 2, 3];
        Кент.группировать(а, (х) => {
            вилкойвглаз (а.длина < 5) { а.втолкнуть(9); }
            отвечаю "г";
        });
        сказать(а.длина);
        гыы б = [1, 2, 3, 4];
        Кент.группировать(б, (х) => {
            вилкойвглаз (б.длина > 2) { б.вытолкнуть(); }
            отвечаю "г";
        });
        сказать(б.длина);
        гыы в = [1, 2];
        Кент.группировать(в, (х) => {
            Отражение.установить(в, 0, 7);
            в[0] = 8;
            отвечаю "г";
        });
        сказать(в[0]);
        гыы г = { х: -0 };
        Кент.назначить(г, { х: 0 });
        сказать(Кент.есть(г.х, 0));
        гыы д = { х: 0 };
        Кент.назначить(д, { х: -0 });
        сказать(Кент.есть(д.х, -0));
        "#,
        "5\n2\n8\ntrue\ntrue\n",
    );
}

#[test]
fn value_held_only_by_a_collection_stays_the_same_value() {
    assert_same_as_interpreter(
        r#"
        клёво К {
            К(х) { тырыпыры.х = х; }
            м() { отвечаю тырыпыры.х; }
        }
        гыы к = захуярить Карта();
        к.set("а", []);
        к.get("а").push(1);
        к.get("а").push(2);
        сказать(к.get("а"));
        к.set("о", { счёт: 0 });
        гыы о = к.get("о");
        о.счёт = 5;
        сказать(к.get("о").счёт);
        к.set("э", захуярить К(3));
        сказать(к.get("э").м(), к.get("э") шкура К);
        "#,
        "[1, 2]\n5\n3 true\n",
    );
}

#[test]
fn cyclic_value_held_by_a_collection_survives_cycle_collection() {
    let src = r#"
        клёво К { м() { отвечаю тырыпыры.х; } }
        гыы к = захуярить Карта();
        йопта положить() {
            гыы э = захуярить К();
            э.х = 2;
            э.я = э;
            к.set("э", э);
        }
        положить();
        СловоПацана.решить(1).потом(() => {
            го (гыы и = 0; и < 60000; и++) {
                гыы о = {};
                о.с = о;
            }
            сказать(к.get("э").м());
        });
    "#;
    assert_same_as_interpreter(src, "2\n");
}

#[test]
fn stdlib_sees_current_proxy_target() {
    assert_same_as_interpreter(
        r#"
        гыы ц = { а: 1 };
        гыы п = Посредник(ц, {});
        ц.б = 2;
        сказать(Кент.ключи(п));
        сказать(Жсон.вСтроку(п));
        Отражение.установить(п, "в", 3);
        сказать(ц.в);
        гыы об = { п };
        ц.г = 4;
        сказать(Жсон.вСтроку(об));
        гыы ар = [1];
        гыы пм = Посредник(ар, {});
        ар.push(2);
        сказать(Отражение.собственныеКлючи(пм).длина);
        "#,
        "[а, б]\n{\"а\":1,\"б\":2}\n3\n{\"п\":{\"а\":1,\"б\":2,\"в\":3,\"г\":4}}\n3\n",
    );
}

#[test]
fn for_of_pulls_a_user_iterator_lazily_and_closes_it_on_break() {
    assert_same_as_interpreter(
        r#"
        гыы счёт = 0;
        гыы ит = {};
        ит[Симбол.итератор] = () => ({
            следующий: () => {
                счёт = счёт + 1;
                отвечаю { значение: счёт, готово: лож };
            },
            вернуть: () => {
                сказать("закрыт");
                отвечаю {};
            }
        });
        го (ясенХуй х сашаГрей ит) {
            сказать("тело", х);
            вилкойвглаз (х === 2) { харэ; }
        }
        сказать(счёт);
        гыы конечный = {};
        конечный[Симбол.итератор] = () => {
            гыы н = 0;
            отвечаю {
                следующий: () => { н = н + 1; отвечаю { значение: н, готово: н > 2 }; },
                вернуть: () => { сказать("лишний вызов"); отвечаю {}; }
            };
        };
        го (ясенХуй х сашаГрей конечный) { сказать(х); }
        "#,
        "тело 1\nтело 2\nзакрыт\n2\n1\n2\n",
    );
}

#[test]
fn for_await_pulls_a_user_async_iterator_lazily() {
    assert_same_as_interpreter(
        r#"
        гыы счёт = 0;
        гыы ит = {};
        ит[Симбол.асинхИтератор] = () => ({
            следующий: () => {
                счёт = счёт + 1;
                отвечаю СловоПацана.решить({ значение: счёт, готово: лож });
            }
        });
        ассо йопта главная() {
            го сидетьНахуй (ясенХуй х сашаГрей ит) { харэ; }
            сказать(счёт);
        }
        главная();
        "#,
        "1\n",
    );
}

#[test]
fn for_of_over_an_infinite_iterator_helper_stops_at_break() {
    assert_same_as_interpreter(
        r#"
        гыы б = Итератор.от({ следующий() { отвечаю { значение: 1, готово: лож }; } });
        го (гыы х сашаГрей б.взять(100000000)) {
            сказать("тело", х);
            харэ;
        }
        сказать("вышли");
        "#,
        "тело 1\nвышли\n",
    );
}

#[test]
fn stdlib_calls_user_methods_with_this() {
    assert_same_as_interpreter(
        r#"
        гыы и = Итератор.от({
            н: 0,
            следующий() {
                тырыпыры.н = тырыпыры.н + 1;
                отвечаю { значение: тырыпыры.н, готово: тырыпыры.н > 2 };
            }
        });
        сказать(и.вМассив());
        гыы п = Посредник({ а: 1 }, { к: 7, получить(ц, кл) { отвечаю тырыпыры.к; } });
        сказать(п.а);
        гыы о = { х: 2, м() { отвечаю тырыпыры.х; } };
        сказать(Отражение.применить(о.м, { х: 3 }, []));
        "#,
        "[1, 2]\n7\n3\n",
    );
}

#[test]
fn iterator_helpers_close_their_source() {
    assert_same_as_interpreter(
        r#"
        гыы и = Итератор.от({
            следующий() { отвечаю { значение: 1, готово: лож }; },
            вернуть() { сказать("закрыт"); отвечаю {}; }
        });
        сказать(и.взять(1).вМассив());
        пиздюли г() {
            хапнуть { поебалу 1; поебалу 2; } тюряжка { сказать("финал"); }
        }
        сказать(Итератор.от(г()).взять(1).вМассив());
        гыы н = Кент.создать({ следующий() { отвечаю { готово: правда }; } });
        сказать(Итератор.от(н).вМассив());
        "#,
        "закрыт\n[1]\nфинал\n[1]\n[]\n",
    );
}

#[test]
fn loose_equality_converts_arrays_and_symbols() {
    assert_same_as_interpreter(
        r#"
        сказать([1] == 1, [1, 2] == "1,2", [] == "", [] == ноль);
        гыы с = Симбол("s");
        гыы у = {};
        у[Симбол.вПримитив] = йопта() { отвечаю с; };
        сказать(у == с);
        "#,
        "true true true false\ntrue\n",
    );
}

#[test]
fn cyclic_array_converts_to_a_string() {
    assert_same_as_interpreter(
        r#"
        гыы а = [1];
        а.push(а);
        сказать("" + а, а == "x");
        гыы б = [1, []];
        б[1].push(б);
        сказать("" + б);
        "#,
        "1, false\n1,\n",
    );
}

#[test]
fn deep_values_convert_and_drop_without_overflow() {
    let src = r#"
        гыы д = [];
        го (гыы и = 0; и < 200000; и++) { д = [д]; }
        сказать(д == "x", ("" + д).длина);
        гыы ф = ноль;
        го (гыы и = 0; и < 200000; и++) {
            участковый п = ф;
            ф = () => п;
        }
        ф = ноль;
        сказать("конец");
    "#;
    assert_same_as_interpreter(src, "false 0\nконец\n");
}

#[test]
fn index_access_sees_the_chain_and_accessors() {
    assert_same_as_interpreter(
        r#"
        гыы с = Симбол("x");
        гыы п = { х: 1 };
        п[с] = 2;
        гыы о = Кент.создать(п);
        сказать(о["х"], о[с]);
        клёво К { get г() { отвечаю 5; } м() { отвечаю 6; } set з(в) { тырыпыры._з = в; } }
        гыы э = захуярить К();
        сказать(э["г"], тип(э["м"]));
        э["з"] = 9;
        сказать(э._з);
        гыы д = { get г() { отвечаю 7; } };
        сказать(д["г"]);
        "#,
        "1 2\n5 функция\n9\n7\n",
    );
}

#[test]
fn inherited_setters_run_and_frozen_prototypes_block_writes() {
    assert_same_as_interpreter(
        r#"
        гыы лог = [];
        гыы п = { set х(в) { лог.push(в); тырыпыры.видел = в; } };
        гыы о = Кент.создать(п);
        о.х = 1;
        сказать(лог, Кент.ключи(о), о.видел);
        гыы к = "х";
        о[к] = 2;
        сказать(лог);
        клёво К { set с(в) { тырыпыры._с = в; } get с() { отвечаю тырыпыры._с; } }
        гыы д = Кент.создать(К.prototype);
        д.с = 4;
        сказать(д.с, Кент.ключи(д));
        гыы н = Кент.создать(Кент.заморозить({ ф: 1 }));
        н.ф = 2;
        сказать(н.ф, Кент.ключи(н));
        гыы в = Кент.создать({ ф: 1 });
        в.ф = 2;
        сказать(в.ф, Кент.ключи(в));
        "#,
        "[1] [видел] 1\n[1, 2]\n4 [_с]\n1 []\n2 [ф]\n",
    );
}

#[test]
fn any_expression_can_be_the_base_of_an_assignment() {
    assert_same_as_interpreter(
        r#"
        гыы о = { х: 1 };
        гыы м = [1];
        гыы счёт = 0;
        йопта дай() { счёт = счёт + 1; отвечаю о; }
        йопта мас() { отвечаю м; }
        дай().х = 5;
        мас()[0] = 6;
        сказать(о.х, м[0], счёт);
        дай().вложен = { а: 1 };
        дай().вложен.а = 2;
        сказать(о.вложен.а, счёт);
        "#,
        "5 6 1\n2 3\n",
    );
}

#[test]
fn prototype_cycles_are_rejected() {
    assert_same_as_interpreter(
        r#"
        гыы а = {};
        гыы б = Кент.создать(а);
        хапнуть {
            Кент.назначитьПрототип(а, б);
            сказать("принято");
        } гоп (е) {
            сказать("отвергнут");
        }
        сказать(Отражение.назначитьПрототип(а, б), а.нет, "нет" чоунастут а);
        сказать(Отражение.назначитьПрототип(а, а), Кент.прототип(а));
        "#,
        "отвергнут\nfalse undefined false\nfalse null\n",
    );
}

#[test]
fn long_prototype_chains_work() {
    assert_same_as_interpreter(
        r#"
        гыы о = { х: 1 };
        го (гыы и = 0; и < 1000; и++) { о = Кент.создать(о); }
        сказать(о.х, "х" чоунастут о);
        "#,
        "1 true\n",
    );
}

#[test]
fn class_prototype_chain_ends_at_null() {
    assert_same_as_interpreter(
        r#"
        клёво Р { р() {} }
        клёво К батя Р { м() {} }
        сказать(Кент.прототип(К.prototype) === Р.prototype);
        сказать(Кент.прототип(Р.prototype));
        сказать(Отражение.прототипОт(К.prototype) === Р.prototype);
        гыы п = захуярить К();
        гыы н = 0;
        потрещим (п) { п = Кент.прототип(п); н = н + 1; }
        сказать(н);
        "#,
        "true\nnull\ntrue\n3\n",
    );
}

#[test]
fn instance_prototype_shadows_its_class_method() {
    assert_same_as_interpreter(
        r#"
        клёво К { м() { отвечаю "класс"; } }
        гыы э = захуярить К();
        э.м = йопта() { отвечаю "своё"; };
        гыы о = Кент.создать(э);
        сказать(о.м());
        "#,
        "своё\n",
    );
}

#[test]
fn writes_to_a_class_prototype_reach_instances() {
    assert_same_as_interpreter(
        r#"
        клёво Р { м() { отвечаю "родитель"; } к() { отвечаю "к"; } }
        клёво К батя Р { }
        гыы э = захуярить К();
        сказать(э.м());
        Р.prototype.м = йопта() { отвечаю "патч"; };
        Р.prototype.н = йопта() { отвечаю "новый"; };
        сказать(э.м(), э.н(), э.к(), захуярить К().м());
        К.prototype.м = йопта() { отвечаю "свой " + тырыпыры.х; };
        э.х = 1;
        сказать(э.м(), захуярить Р().м());
        сказать(К.prototype.к === Р.prototype.к);
        "#,
        "родитель\nпатч новый к патч\nсвой 1 патч\ntrue\n",
    );
}

#[test]
fn class_methods_keep_their_identity() {
    assert_same_as_interpreter(
        r#"
        клёво К { м() { отвечаю 1; } }
        гыы э = захуярить К();
        сказать(э.м === э.м, э.м === К.prototype.м, э.м === захуярить К().м);
        ёбнуть К.prototype.м;
        сказать(тип(э.м));
        "#,
        "true true true\nнеопределено\n",
    );
}

#[test]
fn reflection_treats_symbols_as_keys() {
    assert_same_as_interpreter(
        r#"
        гыы с = Симбол("x");
        гыы о = { а: 2 };
        о[с] = 1;
        сказать(Кент.имеетСвоё(о, с), Отражение.есть(о, с), Отражение.получить(о, с));
        сказать(Отражение.собственныеКлючи(о).длина, Отражение.собственныеКлючи(о)[1] === с);
        Отражение.установить(о, с, 5);
        сказать(о[с]);
        сказать(Кент.описатьСвойство(о, с).значение);
        Кент.определитьСвойство(о, с, { значение: 6 });
        сказать(о[с]);
        Отражение.удалить(о, с);
        сказать(о[с], с чоунастут о);
        сказать(Отражение.собственныеКлючи({ [Симбол.итератор]: 1 })[0] === Симбол.итератор);
        го (гыы к из { [с]: 1, б: 2 }) { сказать("ключ", к); }
        "#,
        "true true 1\n2 true\n5\n5\n6\nundefined false\ntrue\nключ б\n",
    );
}

#[test]
fn json_stringify_supports_to_json_replacer_and_indent() {
    assert_same_as_interpreter(
        r#"
        сказать(Жсон.вСтроку({ х: 1, вЖсон() { отвечаю тырыпыры.х + 1; } }));
        сказать(Жсон.вСтроку({ а: { вЖсон(к) { отвечаю "ключ " + к; } } }));
        сказать(Жсон.вСтроку({ а: 1, б: "с" }, (к, з) => {
            отвечаю тип(з) === "число" ? з * 2 : з;
        }));
        сказать(Жсон.вСтроку({ а: 1, б: 2 }, (к, з) => к === "а" ? неибу : з));
        сказать(Жсон.вСтроку([1, неибу, 2], (к, з) => к === "0" ? неибу : з));
        сказать(Жсон.вСтроку({ а: 1, б: 2, в: { а: 3, г: 4 } }, ["а", "в"]));
        сказать(Жсон.вСтроку({ а: [1, {}], б: [] }, ноль, 2));
        сказать(Жсон.вСтроку([1], ноль, "--"));
        сказать(тип(Жсон.вСтроку(неибу)));
        гыы о = { х: 1 };
        Жсон.вСтроку(о, (к, в) => {
            вилкойвглаз (к === "") { Кент.назначить(о, { х: 3 }); о.х = 4; }
            отвечаю в;
        });
        сказать(о.х);
        "#,
        concat!(
            "2\n",
            "{\"а\":\"ключ а\"}\n",
            "{\"а\":2,\"б\":\"с\"}\n",
            "{\"б\":2}\n",
            "[null,null,2]\n",
            "{\"а\":1,\"в\":{\"а\":3}}\n",
            "{\n  \"а\": [\n    1,\n    {}\n  ],\n  \"б\": []\n}\n",
            "[\n--1\n]\n",
            "неопределено\n",
            "4\n"
        ),
    );
}

#[test]
fn stdlib_sees_class_methods_of_instances() {
    assert_same_as_interpreter(
        r#"
        клёво Р { следующий() { отвечаю { значение: 1, готово: правда }; } }
        клёво Н батя Р { вЖсон() { отвечаю "н"; } скажи() { отвечаю тырыпыры.х; } }
        гыы э = захуярить Н();
        э.х = 3;
        сказать(Итератор.от(э).вМассив(), Жсон.вСтроку(э), Кент.ключи(э));
        гыы п = Посредник(э, {});
        сказать(п.скажи());
        "#,
        "[] \"н\" [х]\n3\n",
    );
}

#[test]
fn bound_method_crosses_the_bridge() {
    assert_same_as_interpreter(
        r#"
        гыы к = захуярить Карта();
        к.set("б", [1, 2].push);
        сказать(тип(к.get("б")));
        "#,
        "функция\n",
    );
}
