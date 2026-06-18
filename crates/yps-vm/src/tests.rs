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
    assert_eq!(run("участковый ПИ = 3.14; сказать(ПИ);"), "3.14\n");
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
    let msg = run_err("участковый х = 1; х = 2;");
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
fn console_family() {
    assert_eq!(run(r#"сказать.инфо("инфо"); сказать.отладка("дбг");"#), "инфо\nдбг\n");
}

#[test]
fn division_by_zero_is_error() {
    let msg = run_err("сказать(1 / 0);");
    assert!(msg.contains("ноль"), "сообщение: {msg}");
}

#[test]
fn relational_on_non_numbers_errors_like_interpreter() {
    assert!(run_to_string(&parse(r#"сказать("a" < "b");"#)).is_err());
    assert!(run_to_string(&parse(r#"сказать(1 < "2");"#)).is_err());
    let msg = run_err(r#"сказать("a" < "b");"#);
    assert!(msg.contains("Сравнение требует числа"), "сообщение: {msg}");
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
fn unsupported_features_are_compile_errors() {
    assert!(compile_program(&parse("кидай 1;")).is_err());
    assert!(compile_program(&parse("хапнуть { } гоп (е) { }")).is_err());
    assert!(compile_program(&parse("клёво К { }")).is_err());
}

#[test]
fn disassembler_runs() {
    let proto = compile_program(&parse("сказать(1 + 2);")).unwrap();
    let text = crate::disassemble(&proto);
    assert!(text.contains("proto"));
}
