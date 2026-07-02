use yps_lexer::{Lexer, SourceFile};
use yps_parser::Parser;
use yps_parser::ast::Program;

use crate::{compile_program, run_to_string};

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

#[test]
fn arithmetic_and_precedence() {
    assert_eq!(run("сказать(1 + 2 * 3);"), "7\n");
    assert_eq!(run("сказать((1 + 2) * 3);"), "9\n");
    assert_eq!(run("сказать(2 ** 10);"), "1024\n");
    assert_eq!(run("сказать(7 % 3);"), "1\n");
    assert_eq!(run("сказать(10 / 4);"), "2.5\n");
}

#[test]
fn number_formatting_matches_interpreter() {
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
fn unsupported_features_are_compile_errors() {
    assert!(compile_program(&parse("гыы о = {}; о.п++;")).is_err());
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
    let proto = compile_program(&parse("сказать(1 + 2);")).unwrap();
    let text = crate::disassemble(&proto);
    assert!(text.contains("proto"));
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
    assert!(compile_program(&parse("гыы м = спиздить(\"./x\");")).is_ok());
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
