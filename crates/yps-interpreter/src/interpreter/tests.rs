use super::*;
use yps_lexer::{Lexer, SourceFile};
use yps_parser::Parser;

fn run_code(src: &str) -> Interpreter {
    let source = SourceFile::new("test".to_string(), src.to_string());
    let (tokens, lex_diags) = Lexer::new(&source).tokenize();
    assert!(lex_diags.is_empty(), "Ошибки лексера: {lex_diags:?}");
    let (program, parse_diags) = Parser::new(&tokens, &source).parse_program();
    assert!(parse_diags.is_empty(), "Ошибки парсера: {parse_diags:?}");
    let mut interp = Interpreter::new();
    interp.run(&program).expect("Ошибка интерпретатора");
    interp
}

fn run_code_err(src: &str) -> RuntimeError {
    let source = SourceFile::new("test".to_string(), src.to_string());
    let (tokens, _) = Lexer::new(&source).tokenize();
    let (program, _) = Parser::new(&tokens, &source).parse_program();
    let mut interp = Interpreter::new();
    interp.run(&program).unwrap_err()
}

#[test]
fn assign_array_index() {
    let interp = run_code(
        r#"
        гыы арр = [1, 2, 3];
        арр[0] = 10;
        гыы результат = арр[0];
        "#,
    );
    assert_eq!(interp.get("результат"), Some(Value::Number(10.0)));
}

#[test]
fn assign_array_index_middle() {
    let interp = run_code(
        r#"
        гыы арр = [1, 2, 3];
        арр[1] = 42;
        гыы результат = арр[1];
        "#,
    );
    assert_eq!(interp.get("результат"), Some(Value::Number(42.0)));
}

#[test]
fn assign_array_index_preserves_other_elements() {
    let interp = run_code(
        r#"
        гыы арр = [10, 20, 30];
        арр[1] = 99;
        гыы а = арр[0];
        гыы б = арр[2];
        "#,
    );
    assert_eq!(interp.get("а"), Some(Value::Number(10.0)));
    assert_eq!(interp.get("б"), Some(Value::Number(30.0)));
}

#[test]
fn assign_array_index_out_of_bounds() {
    let err = run_code_err(
        r#"
        гыы арр = [1, 2];
        арр[5] = 10;
        "#,
    );
    assert!(err.message.contains("вне диапазона") || err.message.contains("Индекс"));
}

#[test]
fn assign_object_member() {
    let interp = run_code(
        r#"
        гыы чел = { имя: "Вася", возраст: 25 };
        чел.имя = "Петя";
        гыы результат = чел.имя;
        "#,
    );
    assert_eq!(interp.get("результат"), Some(Value::String("Петя".to_string())));
}

#[test]
fn assign_object_member_new_property() {
    let interp = run_code(
        r#"
        гыы чел = { имя: "Вася" };
        чел.возраст = 30;
        гыы результат = чел.возраст;
        "#,
    );
    assert_eq!(interp.get("результат"), Some(Value::Number(30.0)));
}

#[test]
fn assign_object_bracket_notation() {
    let interp = run_code(
        r#"
        гыы чел = { имя: "Вася" };
        чел["имя"] = "Коля";
        гыы результат = чел.имя;
        "#,
    );
    assert_eq!(interp.get("результат"), Some(Value::String("Коля".to_string())));
}

#[test]
fn assign_member_on_non_object_fails() {
    let err = run_code_err(
        r#"
        гыы х = 5;
        х.поле = 10;
        "#,
    );
    assert!(err.message.contains("свойство") || err.message.contains("объект"));
}

#[test]
fn compound_assign_array_index() {
    let interp = run_code(
        r#"
        гыы арр = [10, 20, 30];
        арр[0] += 5;
        гыы результат = арр[0];
        "#,
    );
    assert_eq!(interp.get("результат"), Some(Value::Number(15.0)));
}

#[test]
fn compound_assign_object_member() {
    let interp = run_code(
        r#"
        гыы чел = { баланс: 100 };
        чел.баланс -= 30;
        гыы результат = чел.баланс;
        "#,
    );
    assert_eq!(interp.get("результат"), Some(Value::Number(70.0)));
}

#[test]
fn assign_nested_array() {
    let interp = run_code(
        r#"
        гыы матрица = [[1, 2], [3, 4]];
        матрица[0][1] = 99;
        гыы результат = матрица[0][1];
        "#,
    );
    assert_eq!(interp.get("результат"), Some(Value::Number(99.0)));
}

#[test]
fn assign_nested_object() {
    let interp = run_code(
        r#"
        гыы данные = { внутри: { значение: 1 } };
        данные.внутри.значение = 42;
        гыы результат = данные.внутри.значение;
        "#,
    );
    assert_eq!(interp.get("результат"), Some(Value::Number(42.0)));
}

#[test]
fn assign_object_in_array() {
    let interp = run_code(
        r#"
        гыы список = [{ имя: "А" }, { имя: "Б" }];
        список[0].имя = "В";
        гыы результат = список[0].имя;
        "#,
    );
    assert_eq!(interp.get("результат"), Some(Value::String("В".to_string())));
}

#[test]
fn assign_array_in_object() {
    let interp = run_code(
        r#"
        гыы данные = { список: [1, 2, 3] };
        данные.список[2] = 99;
        гыы результат = данные.список[2];
        "#,
    );
    assert_eq!(interp.get("результат"), Some(Value::Number(99.0)));
}

#[test]
fn try_catch_catches_runtime_error() {
    let interp = run_code(
        r#"
        гыы результат = 0;
        хапнуть {
            гыы х = 1 / 0;
        } гоп (е) {
            результат = 1;
        }
        "#,
    );
    assert_eq!(interp.get("результат"), Some(Value::Number(1.0)));
}

#[test]
fn try_catch_catches_throw() {
    let interp = run_code(
        r#"
        гыы результат = "";
        хапнуть {
            кидай "ошибка";
        } гоп (е) {
            результат = е;
        }
        "#,
    );
    assert_eq!(interp.get("результат"), Some(Value::String("ошибка".to_string())));
}

#[test]
fn try_catch_throw_number() {
    let interp = run_code(
        r#"
        гыы результат = 0;
        хапнуть {
            кидай 42;
        } гоп (е) {
            результат = е;
        }
        "#,
    );
    assert_eq!(interp.get("результат"), Some(Value::Number(42.0)));
}

#[test]
fn try_catch_no_error_skips_catch() {
    let interp = run_code(
        r#"
        гыы результат = 1;
        хапнуть {
            результат = 2;
        } гоп (е) {
            результат = 3;
        }
        "#,
    );
    assert_eq!(interp.get("результат"), Some(Value::Number(2.0)));
}

#[test]
fn try_finally_runs_always() {
    let interp = run_code(
        r#"
        гыы результат = 0;
        хапнуть {
            результат = 1;
        } тюряжка {
            результат = результат + 10;
        }
        "#,
    );
    assert_eq!(interp.get("результат"), Some(Value::Number(11.0)));
}

#[test]
fn try_catch_finally_on_error() {
    let interp = run_code(
        r#"
        гыы шаг1 = 0;
        гыы шаг2 = 0;
        хапнуть {
            кидай "бум";
        } гоп (е) {
            шаг1 = 1;
        } тюряжка {
            шаг2 = 1;
        }
        "#,
    );
    assert_eq!(interp.get("шаг1"), Some(Value::Number(1.0)));
    assert_eq!(interp.get("шаг2"), Some(Value::Number(1.0)));
}

#[test]
fn try_catch_finally_no_error() {
    let interp = run_code(
        r#"
        гыы шаг1 = 0;
        гыы шаг2 = 0;
        хапнуть {
            шаг1 = 1;
        } гоп (е) {
            шаг1 = 99;
        } тюряжка {
            шаг2 = 1;
        }
        "#,
    );
    assert_eq!(interp.get("шаг1"), Some(Value::Number(1.0)));
    assert_eq!(interp.get("шаг2"), Some(Value::Number(1.0)));
}

#[test]
fn uncaught_throw_is_error() {
    let err = run_code_err(
        r#"
        кидай "паника";
        "#,
    );
    assert!(err.message.contains("Необработанное исключение"));
}

#[test]
fn try_catch_without_param() {
    let interp = run_code(
        r#"
        гыы результат = 0;
        хапнуть {
            кидай "бум";
        } гоп {
            результат = 1;
        }
        "#,
    );
    assert_eq!(interp.get("результат"), Some(Value::Number(1.0)));
}

#[test]
fn try_catch_runtime_error_message() {
    let interp = run_code(
        r#"
        гыы результат = "";
        хапнуть {
            гыы х = неизвестная;
        } гоп (е) {
            результат = е.message;
        }
        "#,
    );
    let val = interp.get("результат").unwrap();
    if let Value::String(s) = val {
        assert!(s.contains("не определена"));
    } else {
        panic!("Expected string error message");
    }
}

#[test]
fn nested_try_catch() {
    let interp = run_code(
        r#"
        гыы результат = "";
        хапнуть {
            хапнуть {
                кидай "внутри";
            } гоп (е) {
                кидай "снаружи";
            }
        } гоп (е) {
            результат = е;
        }
        "#,
    );
    assert_eq!(interp.get("результат"), Some(Value::String("снаружи".to_string())));
}

#[test]
fn try_catch_with_alias_keywords() {
    let interp = run_code(
        r#"
        гыы результат = 0;
        побратски {
            кидай 1;
        } аченетак (е) {
            результат = е;
        }
        "#,
    );
    assert_eq!(interp.get("результат"), Some(Value::Number(1.0)));
}

#[test]
fn finally_runs_after_throw_without_catch() {
    let interp = run_code(
        r#"
        гыы результат = 0;
        хапнуть {
            хапнуть {
                кидай "бум";
            } тюряжка {
                результат = 1;
            }
        } гоп (е) {
            результат = результат + 10;
        }
        "#,
    );
    assert_eq!(interp.get("результат"), Some(Value::Number(11.0)));
}

#[test]
fn finally_runtime_error_preserves_original_via_cause() {
    let err = run_code_err(
        r#"
        хапнуть {
            гыы а = неизвестнаяОригинальная;
        } тюряжка {
            гыы б = неизвестнаяВФинале;
        }
        "#,
    );
    assert!(
        err.message.contains("неизвестнаяВФинале"),
        "ожидается сообщение от финального исключения, получено: {}",
        err.message
    );
    let cause = err.cause.as_deref().expect("ожидается cause с оригинальной ошибкой");
    assert!(
        cause.message.contains("неизвестнаяОригинальная"),
        "cause должен содержать оригинал, получено: {}",
        cause.message
    );
}

#[test]
fn finally_runtime_error_alone_has_no_cause() {
    let err = run_code_err(
        r#"
        хапнуть {
            гыы а = 1;
        } тюряжка {
            гыы б = неизвестная;
        }
        "#,
    );
    assert!(err.message.contains("неизвестная"));
    assert!(err.cause.is_none(), "при отсутствии оригинальной ошибки cause должен быть None");
}

#[test]
fn finally_error_display_shows_cause_chain() {
    let err = run_code_err(
        r#"
        хапнуть {
            гыы а = первая;
        } тюряжка {
            гыы б = вторая;
        }
        "#,
    );
    let s = format!("{err}");
    assert!(s.contains("вторая"), "отображение должно включать финальную ошибку: {s}");
    assert!(s.contains("первая"), "отображение должно включать оригинал: {s}");
    assert!(s.contains("причина"), "отображение должно явно метить причину: {s}");
}

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
fn destructure_array_basic() {
    let interp = run_code(
        r#"
        гыы [а, б, в] = [1, 2, 3];
        "#,
    );
    assert_eq!(interp.get("а"), Some(Value::Number(1.0)));
    assert_eq!(interp.get("б"), Some(Value::Number(2.0)));
    assert_eq!(interp.get("в"), Some(Value::Number(3.0)));
}

#[test]
fn destructure_array_fewer_elements() {
    let interp = run_code(
        r#"
        гыы [а, б] = [1];
        "#,
    );
    assert_eq!(interp.get("а"), Some(Value::Number(1.0)));
    assert_eq!(interp.get("б"), Some(Value::Undefined));
}

#[test]
fn destructure_array_skip_elements() {
    let interp = run_code(
        r#"
        гыы [, , в] = [1, 2, 3];
        "#,
    );
    assert_eq!(interp.get("в"), Some(Value::Number(3.0)));
}

#[test]
fn destructure_array_rest() {
    let interp = run_code(
        r#"
        гыы [а, ...остаток] = [1, 2, 3, 4];
        гыы длинна = длина(остаток);
        гыы б = остаток[0];
        гыы в = остаток[1];
        гыы г = остаток[2];
        "#,
    );
    assert_eq!(interp.get("а"), Some(Value::Number(1.0)));
    assert_eq!(interp.get("длинна"), Some(Value::Number(3.0)));
    assert_eq!(interp.get("б"), Some(Value::Number(2.0)));
    assert_eq!(interp.get("в"), Some(Value::Number(3.0)));
    assert_eq!(interp.get("г"), Some(Value::Number(4.0)));
}

#[test]
fn destructure_array_rest_empty() {
    let interp = run_code(
        r#"
        гыы [а, ...остаток] = [1];
        гыы длинна = длина(остаток);
        "#,
    );
    assert_eq!(interp.get("а"), Some(Value::Number(1.0)));
    assert_eq!(interp.get("длинна"), Some(Value::Number(0.0)));
}

#[test]
fn destructure_array_non_array_fails() {
    let err = run_code_err(
        r#"
        гыы [а, б] = 42;
        "#,
    );
    assert!(err.message.contains("деструктурировать"));
}

#[test]
fn destructure_object_shorthand() {
    let interp = run_code(
        r#"
        гыы {х, у} = { х: 10, у: 20 };
        "#,
    );
    assert_eq!(interp.get("х"), Some(Value::Number(10.0)));
    assert_eq!(interp.get("у"), Some(Value::Number(20.0)));
}

#[test]
fn destructure_object_rename() {
    let interp = run_code(
        r#"
        гыы {х: а, у: б} = { х: 10, у: 20 };
        "#,
    );
    assert_eq!(interp.get("а"), Some(Value::Number(10.0)));
    assert_eq!(interp.get("б"), Some(Value::Number(20.0)));
}

#[test]
fn destructure_object_missing_key() {
    let interp = run_code(
        r#"
        гыы {х, з} = { х: 10, у: 20 };
        "#,
    );
    assert_eq!(interp.get("х"), Some(Value::Number(10.0)));
    assert_eq!(interp.get("з"), Some(Value::Undefined));
}

#[test]
fn destructure_object_rest() {
    let interp = run_code(
        r#"
        гыы {х, ...остаток} = { х: 1, у: 2, з: 3 };
        "#,
    );
    assert_eq!(interp.get("х"), Some(Value::Number(1.0)));
    let rest = interp.get("остаток").unwrap();
    if let Value::Object(map) = rest {
        assert_eq!(map.get("у"), Some(&Value::Number(2.0)));
        assert_eq!(map.get("з"), Some(&Value::Number(3.0)));
        assert_eq!(map.len(), 2);
    } else {
        panic!("Ожидался объект");
    }
}

#[test]
fn destructure_object_non_object_fails() {
    let err = run_code_err(
        r#"
        гыы {х} = 42;
        "#,
    );
    assert!(err.message.contains("деструктурировать"));
}

#[test]
fn destructure_nested_array_in_array() {
    let interp = run_code(
        r#"
        гыы [а, [б, в]] = [1, [2, 3]];
        "#,
    );
    assert_eq!(interp.get("а"), Some(Value::Number(1.0)));
    assert_eq!(interp.get("б"), Some(Value::Number(2.0)));
    assert_eq!(interp.get("в"), Some(Value::Number(3.0)));
}

#[test]
fn destructure_object_in_array() {
    let interp = run_code(
        r#"
        гыы [а, {б}] = [1, { б: 2 }];
        "#,
    );
    assert_eq!(interp.get("а"), Some(Value::Number(1.0)));
    assert_eq!(interp.get("б"), Some(Value::Number(2.0)));
}

#[test]
fn destructure_array_in_object() {
    let interp = run_code(
        r#"
        гыы {данные: [а, б]} = { данные: [10, 20] };
        "#,
    );
    assert_eq!(interp.get("а"), Some(Value::Number(10.0)));
    assert_eq!(interp.get("б"), Some(Value::Number(20.0)));
}

#[test]
fn destructure_const_array() {
    let err = run_code_err(
        r#"
        участковый [а, б] = [1, 2];
        а = 10;
        "#,
    );
    assert!(err.message.contains("константу") || err.message.contains("const"));
}

#[test]
fn destructure_const_object() {
    let err = run_code_err(
        r#"
        участковый {х, у} = { х: 1, у: 2 };
        х = 10;
        "#,
    );
    assert!(err.message.contains("константу") || err.message.contains("const"));
}

#[test]
fn string_escape_newline() {
    let interp = run_code(
        r#"
        гыы с = "привет\nмир";
        "#,
    );
    assert_eq!(interp.get("с"), Some(Value::String("привет\nмир".to_string())));
}

#[test]
fn string_escape_tab() {
    let interp = run_code(
        r#"
        гыы с = "а\tб";
        "#,
    );
    assert_eq!(interp.get("с"), Some(Value::String("а\tб".to_string())));
}

#[test]
fn string_escape_backslash() {
    let interp = run_code(
        r#"
        гыы с = "путь\\файл";
        "#,
    );
    assert_eq!(interp.get("с"), Some(Value::String("путь\\файл".to_string())));
}

#[test]
fn string_escape_quote() {
    let interp = run_code(
        r#"
        гыы с = "он сказал \"да\"";
        "#,
    );
    assert_eq!(interp.get("с"), Some(Value::String("он сказал \"да\"".to_string())));
}

#[test]
fn string_escape_combined() {
    let interp = run_code(
        r#"
        гыы с = "строка1\nстрока2\tтаб";
        "#,
    );
    assert_eq!(interp.get("с"), Some(Value::String("строка1\nстрока2\tтаб".to_string())));
}

#[test]
fn ternary_true_branch() {
    let interp = run_code(
        r#"
        гыы р = правда ? 10 : 20;
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::Number(10.0)));
}

#[test]
fn ternary_false_branch() {
    let interp = run_code(
        r#"
        гыы р = лож ? 10 : 20;
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::Number(20.0)));
}

#[test]
fn ternary_with_expression_condition() {
    let interp = run_code(
        r#"
        гыы x = 7;
        гыы р = x > 5 ? "да" : "нет";
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::String("да".to_string())));
}

#[test]
fn ternary_nested() {
    let interp = run_code(
        r#"
        гыы x = 3;
        гыы р = x > 10 ? "большое" : x > 5 ? "среднее" : "маленькое";
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::String("маленькое".to_string())));
}

#[test]
fn ternary_with_function_call() {
    let interp = run_code(
        r#"
        гыы arr = [1, 2, 3];
        гыы р = длина(arr) > 0 ? arr[0] : ноль;
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::Number(1.0)));
}

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
fn template_no_substitution() {
    let interp = run_code("гыы р = `привет мир`;");
    assert_eq!(interp.get("р"), Some(Value::String("привет мир".to_string())));
}

#[test]
fn template_empty() {
    let interp = run_code("гыы р = ``;");
    assert_eq!(interp.get("р"), Some(Value::String(String::new())));
}

#[test]
fn template_single_interpolation() {
    let interp = run_code(
        r#"
        гыы имя = "Вася";
        гыы р = `привет, ${имя}!`;
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::String("привет, Вася!".to_string())));
}

#[test]
fn template_multiple_interpolations() {
    let interp = run_code(
        r#"
        гыы а = 1;
        гыы б = 2;
        гыы р = `${а} + ${б} = ${а + б}`;
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::String("1 + 2 = 3".to_string())));
}

#[test]
fn template_expression_interpolation() {
    let interp = run_code("гыы р = `результат: ${2 + 3 * 4}`;");
    assert_eq!(interp.get("р"), Some(Value::String("результат: 14".to_string())));
}

#[test]
fn template_with_escape() {
    let interp = run_code("гыы р = `строка1\\nстрока2`;");
    assert_eq!(interp.get("р"), Some(Value::String("строка1\nстрока2".to_string())));
}

#[test]
fn template_multiline() {
    let interp = run_code("гыы р = `строка1\nстрока2`;");
    assert_eq!(interp.get("р"), Some(Value::String("строка1\nстрока2".to_string())));
}

#[test]
fn template_nested() {
    let interp = run_code(
        r#"
        гыы х = 5;
        гыы р = `внешний ${`внутренний ${х}`}`;
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::String("внешний внутренний 5".to_string())));
}

#[test]
fn template_with_object_in_braces() {
    let interp = run_code(
        r#"
        гыы а = [1, 2, 3];
        гыы р = `длина: ${длина(а)}`;
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::String("длина: 3".to_string())));
}

#[test]
fn template_only_interpolation() {
    let interp = run_code(
        r#"
        гыы х = 42;
        гыы р = `${х}`;
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::String("42".to_string())));
}

#[test]
fn template_escaped_dollar() {
    let interp = run_code("гыы р = `цена: \\${100}`;");
    assert_eq!(interp.get("р"), Some(Value::String("цена: ${100}".to_string())));
}

#[test]
fn template_ternary_inside() {
    let interp = run_code(
        r#"
        гыы х = 10;
        гыы р = `число ${х > 5 ? "большое" : "маленькое"}`;
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::String("число большое".to_string())));
}

#[test]
fn function_without_return_gives_undefined() {
    let interp = run_code(
        r#"
        йопта ф() {}
        гыы р = ф();
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::Undefined));
}

#[test]
fn return_without_value_gives_undefined() {
    let interp = run_code(
        r#"
        йопта ф() { отвечаю; }
        гыы р = ф();
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::Undefined));
}

#[test]
fn missing_object_property_gives_undefined() {
    let interp = run_code(
        r#"
        гыы о = { а: 1 };
        гыы р = о.б;
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::Undefined));
}

#[test]
fn array_index_out_of_bounds_gives_undefined() {
    let interp = run_code(
        r#"
        гыы м = [1, 2, 3];
        гыы р = м[10];
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::Undefined));
}

#[test]
fn typeof_undefined() {
    let interp = run_code(
        r#"
        йопта ф() {}
        гыы р = тип(ф());
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::String("неопределено".to_string())));
}

#[test]
fn null_not_equal_undefined() {
    let interp = run_code(
        r#"
        йопта ф() {}
        гыы р = ф() == ноль;
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::Boolean(false)));
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
fn exponent_basic() {
    let interp = run_code("гыы р = 2 ** 3;");
    assert_eq!(interp.get("р"), Some(Value::Number(8.0)));
}

#[test]
fn exponent_right_associative() {
    let interp = run_code("гыы р = 2 ** 3 ** 2;");
    assert_eq!(interp.get("р"), Some(Value::Number(512.0)));
}

#[test]
fn exponent_with_multiply() {
    let interp = run_code("гыы р = 3 * 2 ** 3;");
    assert_eq!(interp.get("р"), Some(Value::Number(24.0)));
}

#[test]
fn exponent_assign() {
    let interp = run_code(
        r#"
        гыы р = 2;
        р **= 10;
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::Number(1024.0)));
}

#[test]
fn nullish_coalescing_null() {
    let interp = run_code("гыы р = ноль ?? 42;");
    assert_eq!(interp.get("р"), Some(Value::Number(42.0)));
}

#[test]
fn nullish_coalescing_undefined() {
    let interp = run_code("гыы р = неибу ?? 42;");
    assert_eq!(interp.get("р"), Some(Value::Number(42.0)));
}

#[test]
fn nullish_coalescing_non_null() {
    let interp = run_code("гыы р = 0 ?? 42;");
    assert_eq!(interp.get("р"), Some(Value::Number(0.0)));
}

#[test]
fn nullish_coalescing_false_is_not_nullish() {
    let interp = run_code("гыы р = лож ?? 42;");
    assert_eq!(interp.get("р"), Some(Value::Boolean(false)));
}

#[test]
fn nullish_coalescing_chain() {
    let interp = run_code("гыы р = ноль ?? неибу ?? 7;");
    assert_eq!(interp.get("р"), Some(Value::Number(7.0)));
}

#[test]
fn nullish_assign_null() {
    let interp = run_code(
        r#"
        гыы р = ноль;
        р ??= 99;
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::Number(99.0)));
}

#[test]
fn nullish_assign_non_null() {
    let interp = run_code(
        r#"
        гыы р = 5;
        р ??= 99;
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::Number(5.0)));
}

#[test]
fn alias_true_trulio() {
    let interp = run_code("гыы р = трулио;");
    assert_eq!(interp.get("р"), Some(Value::Boolean(true)));
}

#[test]
fn alias_true_chotko() {
    let interp = run_code("гыы р = чотко;");
    assert_eq!(interp.get("р"), Some(Value::Boolean(true)));
}

#[test]
fn alias_false_netrulio() {
    let interp = run_code("гыы р = нетрулио;");
    assert_eq!(interp.get("р"), Some(Value::Boolean(false)));
}

#[test]
fn alias_false_pizdish() {
    let interp = run_code("гыы р = пиздишь;");
    assert_eq!(interp.get("р"), Some(Value::Boolean(false)));
}

#[test]
fn alias_null_nullio() {
    let interp = run_code("гыы р = нуллио;");
    assert_eq!(interp.get("р"), Some(Value::Null));
}

#[test]
fn alias_null_porozhnyak() {
    let interp = run_code("гыы р = порожняк;");
    assert_eq!(interp.get("р"), Some(Value::Null));
}

#[test]
fn alias_undefined_neibu() {
    let interp = run_code("гыы р = неибу;");
    assert_eq!(interp.get("р"), Some(Value::Undefined));
}

#[test]
fn alias_throw_pnh() {
    let interp = run_code(
        r#"
        гыы р = 0;
        хапнуть {
            пнх 42;
        } гоп (е) {
            р = е;
        }
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::Number(42.0)));
}

#[test]
fn alias_switch_estcho() {
    let interp = run_code(
        r#"
        гыы р = 0;
        естьчо (1) {
            лещ 1: {
                р = 10;
            }
            аеслинайду 2: {
                р = 20;
            }
            пахану {
                р = 99;
            }
        }
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::Number(10.0)));
}

#[test]
fn alias_do_while_krch() {
    let interp = run_code(
        r#"
        гыы р = 0;
        крч {
            р = р + 1;
        } потрещим (р < 3);
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::Number(3.0)));
}

#[test]
#[allow(clippy::approx_constant)]
fn alias_const_yasen_huy_capital() {
    let interp = run_code(
        r#"
        ЯсенХуй ПИ = 3.14;
        гыы р = ПИ;
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::Number(3.14)));
}

#[test]
fn optional_chain_member_on_object() {
    let interp = run_code(
        r#"
        гыы чел = { имя: "Вася" };
        гыы р = чел?.имя;
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::String("Вася".to_string())));
}

#[test]
fn optional_chain_member_on_null() {
    let interp = run_code(
        r#"
        гыы чел = ноль;
        гыы р = чел?.имя;
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::Undefined));
}

#[test]
fn optional_chain_member_on_undefined() {
    let interp = run_code(
        r#"
        гыы чел = неибу;
        гыы р = чел?.имя;
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::Undefined));
}

#[test]
fn optional_chain_nested() {
    let interp = run_code(
        r#"
        гыы данные = { а: { б: 42 } };
        гыы р = данные?.а?.б;
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::Number(42.0)));
}

#[test]
fn optional_chain_nested_null() {
    let interp = run_code(
        r#"
        гыы данные = { а: ноль };
        гыы р = данные?.а?.б;
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::Undefined));
}

#[test]
fn optional_chain_index() {
    let interp = run_code(
        r#"
        гыы арр = [10, 20, 30];
        гыы р = арр?.[1];
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::Number(20.0)));
}

#[test]
fn optional_chain_index_on_null() {
    let interp = run_code(
        r#"
        гыы арр = ноль;
        гыы р = арр?.[0];
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::Undefined));
}

#[test]
fn optional_chain_call() {
    let interp = run_code(
        r#"
        гыы ф = () => { отвечаю 42; };
        гыы р = ф?.();
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::Number(42.0)));
}

#[test]
fn optional_chain_call_on_null() {
    let interp = run_code(
        r#"
        гыы ф = ноль;
        гыы р = ф?.();
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::Undefined));
}

#[test]
fn logical_and_assign_truthy() {
    let interp = run_code(
        r#"
        гыы а = 1;
        а &&= 42;
        "#,
    );
    assert_eq!(interp.get("а"), Some(Value::Number(42.0)));
}

#[test]
fn logical_and_assign_falsy() {
    let interp = run_code(
        r#"
        гыы а = 0;
        а &&= 42;
        "#,
    );
    assert_eq!(interp.get("а"), Some(Value::Number(0.0)));
}

#[test]
fn logical_or_assign_falsy() {
    let interp = run_code(
        r#"
        гыы а = 0;
        а ||= 42;
        "#,
    );
    assert_eq!(interp.get("а"), Some(Value::Number(42.0)));
}

#[test]
fn logical_or_assign_truthy() {
    let interp = run_code(
        r#"
        гыы а = 1;
        а ||= 42;
        "#,
    );
    assert_eq!(interp.get("а"), Some(Value::Number(1.0)));
}

#[test]
fn numeric_separator() {
    let interp = run_code(
        r#"
        гыы а = 1_000_000;
        гыы б = 1.23_45;
        "#,
    );
    assert_eq!(interp.get("а"), Some(Value::Number(1_000_000.0)));
    assert_eq!(interp.get("б"), Some(Value::Number(1.2345)));
}

#[test]
fn typeof_basic() {
    let interp = run_code(
        r#"
        гыы а = чезажижан 42;
        гыы б = чезажижан "привет";
        гыы в = чезажижан правда;
        гыы г = чезажижан ноль;
        гыы д = чезажижан неибу;
        "#,
    );
    assert_eq!(interp.get("а"), Some(Value::String("число".to_string())));
    assert_eq!(interp.get("б"), Some(Value::String("строка".to_string())));
    assert_eq!(interp.get("в"), Some(Value::String("булево".to_string())));
    assert_eq!(interp.get("г"), Some(Value::String("объект".to_string())));
    assert_eq!(interp.get("д"), Some(Value::String("неопределено".to_string())));
}

#[test]
fn typeof_undefined_variable() {
    let interp = run_code(
        r#"
        гыы р = чезажижан несуществует;
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::String("неопределено".to_string())));
}

#[test]
fn typeof_function() {
    let interp = run_code(
        r#"
        йопта ф() {}
        гыы р = чезажижан ф;
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::String("функция".to_string())));
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
    assert_eq!(interp.get("р"), Some(Value::String("мир".to_string())));
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
    assert_eq!(interp.get("р"), Some(Value::String("братан".to_string())));
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
    assert_eq!(interp.get("р"), Some(Value::Array(vec![Value::Number(2.0), Value::Number(3.0), Value::Number(4.0),])));
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
    assert_eq!(interp.get("р"), Some(Value::Array(vec![])));
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
    assert_eq!(interp.get("р"), Some(Value::Array(vec![Value::Number(1.0), Value::Number(2.0), Value::Number(3.0),])));
}

#[test]
fn rest_param_arrow_function() {
    let interp = run_code(
        r#"
        гыы фн = (...арг) => { отвечаю арг; };
        гыы р = фн(10, 20);
        "#,
    );
    assert_eq!(interp.get("р"), Some(Value::Array(vec![Value::Number(10.0), Value::Number(20.0)])));
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
fn spread_in_array() {
    let i = run_code(
        r#"
        гыы а = [1, 2, 3];
        гыы б = [0, ...а, 4];
        гыы длн = 0;
        го (гыы и = 0; и < 5; и++) {
            длн = длн + 1;
        }
        гыы рез = б[0] + б[1] + б[2] + б[3] + б[4];
        "#,
    );
    assert_eq!(i.get("рез"), Some(Value::Number(10.0)));
}

#[test]
fn spread_in_call() {
    let i = run_code(
        r#"
        йопта сумма(а, б, в) {
            отвечаю а + б + в;
        }
        гыы арг = [1, 2, 3];
        гыы рез = сумма(...арг);
        "#,
    );
    assert_eq!(i.get("рез"), Some(Value::Number(6.0)));
}

#[test]
fn spread_in_object() {
    let i = run_code(
        r#"
        гыы а = {x: 1, y: 2};
        гыы б = {...а, z: 3};
        гыы рез = б["x"] + б["y"] + б["z"];
        "#,
    );
    assert_eq!(i.get("рез"), Some(Value::Number(6.0)));
}

#[test]
fn computed_property_name() {
    let i = run_code(
        r#"
        гыы ключ = "привет";
        гыы о = {[ключ]: 42};
        гыы рез = о["привет"];
        "#,
    );
    assert_eq!(i.get("рез"), Some(Value::Number(42.0)));
}

#[test]
fn shorthand_property() {
    let i = run_code(
        r#"
        гыы х = 10;
        гыы у = 20;
        гыы о = {х, у};
        гыы рез = о["х"] + о["у"];
        "#,
    );
    assert_eq!(i.get("рез"), Some(Value::Number(30.0)));
}

#[test]
fn method_shorthand_in_object() {
    let i = run_code(
        r#"
        гыы о = {
            удвоить(н) {
                отвечаю н * 2;
            }
        };
        гыы рез = о.удвоить(5);
        "#,
    );
    assert_eq!(i.get("рез"), Some(Value::Number(10.0)));
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
fn delete_object_property() {
    let i = run_code(
        r#"
        гыы о = {а: 1, б: 2};
        ёбнуть о.а;
        гыы рез = чезажижан о["а"];
        "#,
    );
    assert_eq!(i.get("рез"), Some(Value::String("неопределено".to_string())));
}

#[test]
fn delete_array_index_creates_undefined_hole() {
    let i = run_code(
        r#"
        гыы а = [10, 20, 30];
        ёбнуть а[1];
        гыы н = а[1];
        гыы д = длина(а);
        "#,
    );
    assert_eq!(i.get("н"), Some(Value::Undefined));
    assert_eq!(i.get("д"), Some(Value::Number(3.0)));
}

#[test]
fn delete_array_preserves_other_elements() {
    let i = run_code(
        r#"
        гыы а = [10, 20, 30];
        ёбнуть а[1];
        гыы х = а[0];
        гыы з = а[2];
        "#,
    );
    assert_eq!(i.get("х"), Some(Value::Number(10.0)));
    assert_eq!(i.get("з"), Some(Value::Number(30.0)));
}

#[test]
fn delete_array_out_of_bounds_is_noop() {
    let i = run_code(
        r#"
        гыы а = [10, 20];
        ёбнуть а[10];
        гыы д = длина(а);
        "#,
    );
    assert_eq!(i.get("д"), Some(Value::Number(2.0)));
}

#[test]
fn delete_string_index_is_runtime_error() {
    let err = run_code_err(
        r#"
        гыы с = "абв";
        ёбнуть с[0];
        "#,
    );
    assert!(err.message.to_lowercase().contains("стро"), "ошибка должна упоминать строки, получено: {}", err.message);
}

#[test]
fn break_outside_loop_errors() {
    let err = run_code_err("харэ;");
    assert!(err.message.contains("'харэ'"), "got: {}", err.message);
    assert!(err.message.contains("вне цикла"), "got: {}", err.message);
}

#[test]
fn continue_outside_loop_errors() {
    let err = run_code_err("двигай;");
    assert!(err.message.contains("'двигай'"), "got: {}", err.message);
    assert!(err.message.contains("вне цикла"), "got: {}", err.message);
}

#[test]
fn division_by_zero_errors() {
    let err = run_code_err("гыы х = 5 / 0;");
    assert!(err.message.contains("Деление на ноль"), "got: {}", err.message);
}

#[test]
fn array_index_must_be_number() {
    let err = run_code_err(
        r#"
        гыы а = [1, 2, 3];
        а["ключ"] = 9;
        "#,
    );
    assert!(
        err.message.contains("индекс") || err.message.contains("Индекс") || err.message.contains("индексировать"),
        "got: {}",
        err.message
    );
}

#[test]
fn assignment_lhs_must_be_assignable() {
    let err = run_code_err("42 = 7;");
    assert!(err.message.contains("Левая сторона"), "got: {}", err.message);
}

#[test]
fn increment_on_non_variable_errors() {
    let err = run_code_err("42++;");
    assert!(err.message.contains("'++'") || err.message.contains("переменной"), "got: {}", err.message);
}

#[test]
fn this_outside_method_errors() {
    let err = run_code_err("гыы х = тырыпыры;");
    assert!(err.message.contains("тырыпыры") || err.message.contains("this"), "got: {}", err.message);
    assert!(err.message.contains("вне"), "got: {}", err.message);
}

#[test]
fn super_outside_subclass_errors() {
    let err = run_code_err(
        r#"
        клёво А {
            метод() { отвечаю яга.чтото(); }
        }
        гыы а = захуярить А();
        а.метод();
        "#,
    );
    assert!(err.message.contains("яга") || err.message.contains("super"), "got: {}", err.message);
}

#[test]
fn calling_non_function_errors() {
    let err = run_code_err(
        r#"
        гыы х = 5;
        гыы у = х();
        "#,
    );
    assert!(err.message.contains("не является функцией") || err.message.contains("функц"), "got: {}", err.message);
}

#[test]
fn unary_minus_on_string_errors() {
    let err = run_code_err(r#"гыы х = -"абв";"#);
    assert!(err.message.contains("'-'") || err.message.contains("тип"), "got: {}", err.message);
}

#[test]
fn increment_on_string_errors() {
    let err = run_code_err(
        r#"
        гыы х = "стр";
        х++;
        "#,
    );
    assert!(err.message.contains("число") || err.message.contains("'++'"), "got: {}", err.message);
}

#[test]
fn set_property_on_number_errors() {
    let err = run_code_err(
        r#"
        гыы х = 5;
        х.поле = 1;
        "#,
    );
    assert!(err.message.contains("свойство") || err.message.contains("Нельзя"), "got: {}", err.message);
}

#[test]
fn instanceof_operator_requires_class_on_right() {
    let err = run_code_err(
        r#"
        гыы рез = 42 шкура 10;
        "#,
    );
    assert!(err.message.contains("шкура"));
}

#[test]
fn in_operator() {
    let i = run_code(
        r#"
        гыы о = {х: 1, у: 2};
        гыы р1 = "х" из о;
        гыы р2 = "з" из о;
        "#,
    );
    assert_eq!(i.get("р1"), Some(Value::Boolean(true)));
    assert_eq!(i.get("р2"), Some(Value::Boolean(false)));
}

#[test]
fn pipeline_operator() {
    let i = run_code(
        r#"
        йопта удвоить(н) {
            отвечаю н * 2;
        }
        йопта прибавитьОдин(н) {
            отвечаю н + 1;
        }
        гыы рез = 5 |> удвоить |> прибавитьОдин;
        "#,
    );
    assert_eq!(i.get("рез"), Some(Value::Number(11.0)));
}

#[test]
fn void_operator() {
    let i = run_code(
        r#"
        гыы рез = куку 42;
        "#,
    );
    assert_eq!(i.get("рез"), Some(Value::Undefined));
}

#[test]
fn string_key_in_object() {
    let i = run_code(
        r#"
        гыы о = {"моё имя": 42};
        гыы рез = о["моё имя"];
        "#,
    );
    assert_eq!(i.get("рез"), Some(Value::Number(42.0)));
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
fn class_basic_constructor_and_fields() {
    let i = run_code(
        r#"
        клёво Чел {
            Чел(имя, возраст) {
                тырыпыры.имя = имя;
                тырыпыры.возраст = возраст;
            }
        }
        гыы п = захуярить Чел("Вася", 25);
        гыы имя = п.имя;
        гыы возраст = п.возраст;
        "#,
    );
    assert_eq!(i.get("имя"), Some(Value::String("Вася".to_string())));
    assert_eq!(i.get("возраст"), Some(Value::Number(25.0)));
}

#[test]
fn class_method_call() {
    let i = run_code(
        r#"
        клёво Кот {
            Кот(имя) {
                тырыпыры.имя = имя;
            }
            мяукнуть() {
                отвечаю тырыпыры.имя;
            }
        }
        гыы к = захуярить Кот("Барсик");
        гыы рез = к.мяукнуть();
        "#,
    );
    assert_eq!(i.get("рез"), Some(Value::String("Барсик".to_string())));
}

#[test]
fn class_inheritance() {
    let i = run_code(
        r#"
        клёво Животное {
            Животное(имя) {
                тырыпыры.имя = имя;
            }
            представиться() {
                отвечаю тырыпыры.имя;
            }
        }
        клёво Собака батя Животное {
            Собака(имя, порода) {
                тырыпыры.имя = имя;
                тырыпыры.вид = порода;
            }
            получитьВид() {
                отвечаю тырыпыры.вид;
            }
        }
        гыы с = захуярить Собака("Шарик", "дворняга");
        гыы имя = с.представиться();
        гыы вид = с.получитьВид();
        "#,
    );
    assert_eq!(i.get("имя"), Some(Value::String("Шарик".to_string())));
    assert_eq!(i.get("вид"), Some(Value::String("дворняга".to_string())));
}

#[test]
fn class_implicit_constructor_forwards_to_parent() {
    let i = run_code(
        r#"
        клёво Машина {
            Машина(модель) {
                тырыпыры.модель = модель;
            }
        }
        клёво Грузовик батя Машина {
        }
        гыы г = захуярить Грузовик("Камаз");
        гыы рез = г.модель;
        "#,
    );
    assert_eq!(i.get("рез"), Some(Value::String("Камаз".to_string())));
}

#[test]
fn class_implicit_constructor_preserves_class_tag() {
    let i = run_code(
        r#"
        клёво Базовый {
            Базовый(значение) {
                тырыпыры.значение = значение;
            }
        }
        клёво Производный батя Базовый {
        }
        гыы э = захуярить Производный(42);
        гыы знач = э.значение;
        гыы класс = э.__class__;
        "#,
    );
    assert_eq!(i.get("знач"), Some(Value::Number(42.0)));
    assert_eq!(i.get("класс"), Some(Value::String("Производный".to_string())));
}

#[test]
fn catch_receives_runtime_error_as_object() {
    let i = run_code(
        r#"
        гыы имя = "";
        гыы текст = "";
        хапнуть {
            гыы х = неопределённая_переменная;
        } гоп(е) {
            имя = е.name;
            текст = е.message;
        }
        "#,
    );
    assert_eq!(i.get("имя"), Some(Value::String("Косяк".to_string())));
    match i.get("текст") {
        Some(Value::String(s)) => assert!(s.contains("неопределённая_переменная")),
        other => panic!("ожидалась строка с сообщением, получено {other:?}"),
    }
}

#[test]
fn catch_thrown_kosyak_object_preserves_fields() {
    let i = run_code(
        r#"
        гыы имя = "";
        гыы текст = "";
        хапнуть {
            кидай захуярить Косяк("плохо");
        } гоп(е) {
            имя = е.name;
            текст = е.message;
        }
        "#,
    );
    assert_eq!(i.get("имя"), Some(Value::String("Косяк".to_string())));
    assert_eq!(i.get("текст"), Some(Value::String("плохо".to_string())));
}

#[test]
fn catch_thrown_string_passes_through() {
    let i = run_code(
        r#"
        гыы рез = "";
        хапнуть {
            кидай "плоская строка";
        } гоп(е) {
            рез = е;
        }
        "#,
    );
    assert_eq!(i.get("рез"), Some(Value::String("плоская строка".to_string())));
}

#[test]
fn instanceof_distinguishes_unrelated_classes() {
    let i = run_code(
        r#"
        клёво А { А() {} }
        клёво Б { Б() {} }
        гыы а = захуярить А();
        гыы тот = а шкура А;
        гыы нетот = а шкура Б;
        "#,
    );
    assert_eq!(i.get("тот"), Some(Value::Boolean(true)));
    assert_eq!(i.get("нетот"), Some(Value::Boolean(false)));
}

#[test]
fn instanceof_walks_parent_chain() {
    let i = run_code(
        r#"
        клёво Животное { Животное() {} }
        клёво Собака батя Животное { Собака() {} }
        клёво Овчарка батя Собака { Овчарка() {} }
        гыы о = захуярить Овчарка();
        гыы есть_овчарка = о шкура Овчарка;
        гыы есть_собака = о шкура Собака;
        гыы есть_животное = о шкура Животное;
        "#,
    );
    assert_eq!(i.get("есть_овчарка"), Some(Value::Boolean(true)));
    assert_eq!(i.get("есть_собака"), Some(Value::Boolean(true)));
    assert_eq!(i.get("есть_животное"), Some(Value::Boolean(true)));
}

#[test]
fn instanceof_false_for_non_instance() {
    let i = run_code(
        r#"
        клёво К { К() {} }
        гыы х = 42;
        гыы строка = "abc";
        гыы массив = [1, 2];
        гыы а = х шкура К;
        гыы б = строка шкура К;
        гыы в = массив шкура К;
        "#,
    );
    assert_eq!(i.get("а"), Some(Value::Boolean(false)));
    assert_eq!(i.get("б"), Some(Value::Boolean(false)));
    assert_eq!(i.get("в"), Some(Value::Boolean(false)));
}

#[test]
fn class_static_method() {
    let i = run_code(
        r#"
        клёво Матема {
            попонятия двойка() {
                отвечаю 2;
            }
        }
        гыы рез = Матема.двойка();
        "#,
    );
    assert_eq!(i.get("рез"), Some(Value::Number(2.0)));
}

#[test]
fn class_new_without_args() {
    let i = run_code(
        r#"
        клёво Пустой {
            Пустой() {
                тырыпыры.х = 42;
            }
        }
        гыы о = захуярить Пустой();
        гыы рез = о.х;
        "#,
    );
    assert_eq!(i.get("рез"), Some(Value::Number(42.0)));
}

#[test]
fn class_method_with_args() {
    let i = run_code(
        r#"
        клёво Калькулятор {
            Калькулятор() {}
            сложить(а, б) {
                отвечаю а + б;
            }
        }
        гыы к = захуярить Калькулятор();
        гыы рез = к.сложить(3, 4);
        "#,
    );
    assert_eq!(i.get("рез"), Some(Value::Number(7.0)));
}

#[test]
fn class_instanceof_check() {
    let i = run_code(
        r#"
        клёво Тест {
            Тест() {
                тырыпыры.вал = 1;
            }
        }
        гыы т = захуярить Тест();
        гыы рез = чезажижан т;
        "#,
    );
    assert_eq!(i.get("рез"), Some(Value::String("объект".to_string())));
}

#[test]
fn private_field_access_inside_class() {
    let i = run_code(
        r#"
        клёво Счёт {
            Счёт(нач) {
                тырыпыры.#баланс = нач;
            }
            получить() {
                отвечаю тырыпыры.#баланс;
            }
            добавить(с) {
                тырыпыры.#баланс = тырыпыры.#баланс + с;
            }
        }
        гыы с = захуярить Счёт(100);
        с.добавить(50);
        гыы рез = с.получить();
        "#,
    );
    assert_eq!(i.get("рез"), Some(Value::Number(150.0)));
}

#[test]
fn private_field_access_outside_class_fails() {
    let err = run_code_err(
        r#"
        клёво Кошелёк {
            Кошелёк() {
                тырыпыры.#бабки = 500;
            }
        }
        гыы к = захуярить Кошелёк();
        гыы х = к.#бабки;
        "#,
    );
    assert!(err.message.contains("приватному полю"));
}

#[test]
fn private_field_declaration() {
    let i = run_code(
        r#"
        клёво Бокс {
            #значение = 42;
            получить() {
                отвечаю тырыпыры.#значение;
            }
        }
        гыы б = захуярить Бокс();
        гыы рез = б.получить();
        "#,
    );
    assert_eq!(i.get("рез"), Some(Value::Number(42.0)));
}

#[test]
fn class_getter() {
    let i = run_code(
        r#"
        клёво Круг {
            Круг(р) {
                тырыпыры.радиус = р;
            }
            get площадь() {
                отвечаю 3 * тырыпыры.радиус * тырыпыры.радиус;
            }
        }
        гыы к = захуярить Круг(10);
        гыы рез = к.площадь;
        "#,
    );
    assert_eq!(i.get("рез"), Some(Value::Number(300.0)));
}

#[test]
fn class_setter() {
    let i = run_code(
        r#"
        клёво Ящик {
            Ящик() {
                тырыпыры.ширина = 0;
                тырыпыры.высота = 0;
            }
            get площадь() {
                отвечаю тырыпыры.ширина * тырыпыры.высота;
            }
            set размер(с) {
                тырыпыры.ширина = с;
                тырыпыры.высота = с;
            }
        }
        гыы я = захуярить Ящик();
        я.размер = 5;
        гыы рез = я.площадь;
        "#,
    );
    assert_eq!(i.get("рез"), Some(Value::Number(25.0)));
}

#[test]
fn object_getter_setter() {
    let i = run_code(
        r#"
        гыы об = {
            _имя: "мир",
            get имя() {
                отвечаю тырыпыры._имя;
            },
            set имя(н) {
                тырыпыры._имя = н;
            }
        };
        гыы до = об.имя;
        об.имя = "всем";
        гыы после = об.имя;
        "#,
    );
    assert_eq!(i.get("до"), Some(Value::String("мир".to_string())));
    assert_eq!(i.get("после"), Some(Value::String("всем".to_string())));
}

#[test]
fn static_getter() {
    let i = run_code(
        r#"
        клёво Конфиг {
            попонятия #версия = 1;
            попонятия get версия() {
                отвечаю 42;
            }
        }
        гыы рез = Конфиг.версия;
        "#,
    );
    assert_eq!(i.get("рез"), Some(Value::Number(42.0)));
}

#[test]
fn test_method_decorator() {
    let interp = run_code(
        r#"
        йопта обёртка(метод, контекст) {
            отвечаю (...аргс) => {
                отвечаю метод(аргс[0], аргс[1]) * 2;
            };
        }

        клёво К {
            @обёртка
            сложить(а, б) {
                отвечаю а + б;
            }
        }

        гыы к = захуярить К();
        гыы рез = к.сложить(3, 4);
        "#,
    );
    assert_eq!(interp.get("рез"), Some(Value::Number(14.0)));
}

#[test]
fn test_field_decorator() {
    let interp = run_code(
        r#"
        йопта удвоить(_, контекст) {
            отвечаю (начальное) => {
                отвечаю начальное * 2;
            };
        }

        клёво К {
            @удвоить
            значение = 21;
        }

        гыы к = захуярить К();
        гыы рез = к.значение;
        "#,
    );
    assert_eq!(interp.get("рез"), Some(Value::Number(42.0)));
}

#[test]
fn test_class_decorator() {
    let interp = run_code(
        r#"
        гыы сохр = ноль;
        йопта запомнить(класс, контекст) {
            сохр = контекст;
            отвечаю класс;
        }

        @запомнить
        клёво МойКласс { }
        "#,
    );
    let ctx = interp.get("сохр").unwrap();
    match ctx {
        Value::Object(map) => {
            assert_eq!(map.get("вид"), Some(&Value::String("класс".to_string())));
            assert_eq!(map.get("имя"), Some(&Value::String("МойКласс".to_string())));
            assert_eq!(map.get("статичное"), Some(&Value::Boolean(false)));
            assert_eq!(map.get("приватное"), Some(&Value::Boolean(false)));
        }
        _ => panic!("Expected Object context"),
    }
}

#[test]
fn test_class_decorator_passthrough() {
    let interp = run_code(
        r#"
        йопта нуп(класс, контекст) {
            отвечаю класс;
        }

        @нуп
        клёво К {
            метод() { отвечаю 42; }
        }

        гыы к = захуярить К();
        гыы рез = к.метод();
        "#,
    );
    assert_eq!(interp.get("рез"), Some(Value::Number(42.0)));
}

#[test]
fn test_add_initializer_instance() {
    let interp = run_code(
        r#"
        гыы счётчик = 0;
        йопта отслеживание(метод, контекст) {
            контекст.добавитьИнициализатор(() => {
                счётчик += 1;
            });
            отвечаю метод;
        }

        клёво К {
            @отслеживание
            метод() { }
        }

        гыы к1 = захуярить К();
        гыы к2 = захуярить К();
        гыы рез = счётчик;
        "#,
    );
    assert_eq!(interp.get("рез"), Some(Value::Number(2.0)));
}

#[test]
fn test_add_initializer_static() {
    let interp = run_code(
        r#"
        гыы инициализирован = лож;
        йопта регистрация(_, контекст) {
            контекст.добавитьИнициализатор(() => {
                инициализирован = правда;
            });
        }

        клёво К {
            @регистрация
            попонятия х = 1;
        }

        гыы рез = инициализирован;
        "#,
    );
    assert_eq!(interp.get("рез"), Some(Value::Boolean(true)));
}

#[test]
fn test_decorator_execution_order() {
    let interp = run_code(
        r#"
        гыы журнал = [];
        йопта д(тег) {
            журнал = втолкнуть(журнал, "выч:" + тег);
            отвечаю (значение, контекст) => {
                журнал = втолкнуть(журнал, "прим:" + тег + ">" + контекст.вид);
                отвечаю значение;
            };
        }

        @д("класс")
        клёво К {
            @д("метод")
            м() { }

            @д("поле")
            х = 1;
        }

        гыы рез = журнал;
        "#,
    );
    let log = interp.get("рез").unwrap();
    match log {
        Value::Array(items) => {
            let strs: Vec<String> = items.iter().map(|v| v.to_string()).collect();
            assert_eq!(
                strs,
                vec!["выч:класс", "выч:метод", "выч:поле", "прим:метод>метод", "прим:поле>поле", "прим:класс>класс",]
            );
        }
        _ => panic!("Expected Array"),
    }
}

#[test]
fn test_multiple_decorators_order() {
    let interp = run_code(
        r#"
        гыы журнал = [];
        йопта первый(м, к) { журнал = втолкнуть(журнал, "первый"); отвечаю м; }
        йопта второй(м, к) { журнал = втолкнуть(журнал, "второй"); отвечаю м; }

        клёво К {
            @первый
            @второй
            метод() { }
        }

        гыы рез = журнал;
        "#,
    );
    let log = interp.get("рез").unwrap();
    match log {
        Value::Array(items) => {
            let strs: Vec<String> = items.iter().map(|v| v.to_string()).collect();
            assert_eq!(strs, vec!["второй", "первый"]);
        }
        _ => panic!("Expected Array"),
    }
}

#[test]
fn test_stdlib_math_basic() {
    let interp = run_code(
        r#"
        гыы а = Матан.пол(3.7);
        гыы б = Матан.потолок(3.2);
        гыы в = Матан.округлить(3.5);
        гыы г = Матан.модуль(-5);
        гыы д = Матан.мин(1, 2, 3);
        гыы е = Матан.макс(1, 2, 3);
        гыы ё = Матан.степень(2, 10);
        гыы ж = Матан.корень(16);
        "#,
    );
    assert_eq!(interp.get("а"), Some(Value::Number(3.0)));
    assert_eq!(interp.get("б"), Some(Value::Number(4.0)));
    assert_eq!(interp.get("в"), Some(Value::Number(4.0)));
    assert_eq!(interp.get("г"), Some(Value::Number(5.0)));
    assert_eq!(interp.get("д"), Some(Value::Number(1.0)));
    assert_eq!(interp.get("е"), Some(Value::Number(3.0)));
    assert_eq!(interp.get("ё"), Some(Value::Number(1024.0)));
    assert_eq!(interp.get("ж"), Some(Value::Number(4.0)));
}

#[test]
fn test_stdlib_math_constants() {
    let interp = run_code(
        r#"
        гыы пи = Матан.ПИ;
        гыы е = Матан.Е;
        "#,
    );
    assert_eq!(interp.get("пи"), Some(Value::Number(std::f64::consts::PI)));
    assert_eq!(interp.get("е"), Some(Value::Number(std::f64::consts::E)));
}

#[test]
fn test_stdlib_array_push_pop() {
    let interp = run_code(
        r#"
        гыы а = [1, 2];
        а.push(3);
        а.push(4);
        гыы последний = а.pop();
        "#,
    );
    assert_eq!(interp.get("а"), Some(Value::Array(vec![Value::Number(1.0), Value::Number(2.0), Value::Number(3.0)])));
    assert_eq!(interp.get("последний"), Some(Value::Number(4.0)));
}

#[test]
fn test_stdlib_array_length_property() {
    let interp = run_code(
        r#"
        гыы а = [1, 2, 3, 4, 5];
        гыы д = а.length;
        гыы д2 = а.длина;
        "#,
    );
    assert_eq!(interp.get("д"), Some(Value::Number(5.0)));
    assert_eq!(interp.get("д2"), Some(Value::Number(5.0)));
}

#[test]
fn test_stdlib_array_map_filter_reduce() {
    let interp = run_code(
        r#"
        гыы а = [1, 2, 3, 4, 5];
        гыы у = а.map((x) => x * 2);
        гыы ф = а.filter((x) => x > 2);
        гыы с = а.reduce((а, б) => а + б, 0);
        "#,
    );
    assert_eq!(
        interp.get("у"),
        Some(Value::Array(vec![
            Value::Number(2.0),
            Value::Number(4.0),
            Value::Number(6.0),
            Value::Number(8.0),
            Value::Number(10.0),
        ]))
    );
    assert_eq!(interp.get("ф"), Some(Value::Array(vec![Value::Number(3.0), Value::Number(4.0), Value::Number(5.0)])));
    assert_eq!(interp.get("с"), Some(Value::Number(15.0)));
}

#[test]
fn test_stdlib_array_find_includes_indexof() {
    let interp = run_code(
        r#"
        гыы а = [10, 20, 30, 40];
        гыы н = а.find((x) => x > 15);
        гыы и = а.includes(30);
        гыы ин = а.indexOf(40);
        гыы ин2 = а.indexOf(99);
        "#,
    );
    assert_eq!(interp.get("н"), Some(Value::Number(20.0)));
    assert_eq!(interp.get("и"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("ин"), Some(Value::Number(3.0)));
    assert_eq!(interp.get("ин2"), Some(Value::Number(-1.0)));
}

#[test]
fn test_stdlib_array_join_slice_reverse() {
    let interp = run_code(
        r#"
        гыы а = [1, 2, 3];
        гыы дж = а.join("-");
        гыы ср = а.slice(1, 3);
        гыы пр = а.toReversed();
        "#,
    );
    assert_eq!(interp.get("дж"), Some(Value::String("1-2-3".to_string())));
    assert_eq!(interp.get("ср"), Some(Value::Array(vec![Value::Number(2.0), Value::Number(3.0)])));
    assert_eq!(interp.get("пр"), Some(Value::Array(vec![Value::Number(3.0), Value::Number(2.0), Value::Number(1.0)])));
}

#[test]
fn test_stdlib_array_at() {
    let interp = run_code(
        r#"
        гыы а = [10, 20, 30];
        гыы п = а.at(0);
        гыы пос = а.at(-1);
        гыы внеДиапазона = а.at(99);
        "#,
    );
    assert_eq!(interp.get("п"), Some(Value::Number(10.0)));
    assert_eq!(interp.get("пос"), Some(Value::Number(30.0)));
    assert_eq!(interp.get("внеДиапазона"), Some(Value::Undefined));
}

#[test]
fn test_stdlib_array_flat() {
    let interp = run_code(
        r#"
        гыы а = [1, [2, [3, [4]]]];
        гыы пл1 = а.flat();
        гыы пл2 = а.flat(2);
        "#,
    );
    assert_eq!(
        interp.get("пл1"),
        Some(Value::Array(vec![
            Value::Number(1.0),
            Value::Number(2.0),
            Value::Array(vec![Value::Number(3.0), Value::Array(vec![Value::Number(4.0)])]),
        ]))
    );
    assert_eq!(
        interp.get("пл2"),
        Some(Value::Array(vec![
            Value::Number(1.0),
            Value::Number(2.0),
            Value::Number(3.0),
            Value::Array(vec![Value::Number(4.0)]),
        ]))
    );
}

#[test]
fn test_stdlib_string_basic() {
    let interp = run_code(
        r#"
        гыы с = "Привет, Мир";
        гыы в = с.toUpperCase();
        гыы н = с.toLowerCase();
        гыы и = с.indexOf("Мир");
        гыы вкл = с.includes("Привет");
        "#,
    );
    assert_eq!(interp.get("в"), Some(Value::String("ПРИВЕТ, МИР".to_string())));
    assert_eq!(interp.get("н"), Some(Value::String("привет, мир".to_string())));
    assert_eq!(interp.get("и"), Some(Value::Number(8.0)));
    assert_eq!(interp.get("вкл"), Some(Value::Boolean(true)));
}

#[test]
fn test_stdlib_string_slice_trim_split() {
    let interp = run_code(
        r#"
        гыы с = "  привет  ";
        гыы об = с.trim();
        гыы сл = "a,b,c".split(",");
        гыы отр = "hello".slice(1, 4);
        "#,
    );
    assert_eq!(interp.get("об"), Some(Value::String("привет".to_string())));
    assert_eq!(
        interp.get("сл"),
        Some(Value::Array(vec![
            Value::String("a".to_string()),
            Value::String("b".to_string()),
            Value::String("c".to_string()),
        ]))
    );
    assert_eq!(interp.get("отр"), Some(Value::String("ell".to_string())));
}

#[test]
fn test_stdlib_string_length() {
    let interp = run_code(
        r#"
        гыы с = "Привет";
        гыы д = с.length;
        гыы д2 = с.длина;
        "#,
    );
    assert_eq!(interp.get("д"), Some(Value::Number(6.0)));
    assert_eq!(interp.get("д2"), Some(Value::Number(6.0)));
}

#[test]
fn test_stdlib_string_repeat_pad() {
    let interp = run_code(
        r#"
        гыы а = "abc".repeat(3);
        гыы б = "5".padStart(3, "0");
        гыы в = "5".padEnd(4, "-");
        "#,
    );
    assert_eq!(interp.get("а"), Some(Value::String("abcabcabc".to_string())));
    assert_eq!(interp.get("б"), Some(Value::String("005".to_string())));
    assert_eq!(interp.get("в"), Some(Value::String("5---".to_string())));
}

#[test]
fn test_stdlib_object_keys_values_entries() {
    let interp = run_code(
        r#"
        гыы о = { а: 1, б: 2 };
        гыы к = Кент.ключи(о);
        гыы з = Кент.значения(о);
        "#,
    );
    if let Some(Value::Array(mut keys)) = interp.get("к") {
        keys.sort_by_key(|v| v.to_string());
        assert_eq!(keys, vec![Value::String("а".to_string()), Value::String("б".to_string())]);
    } else {
        panic!("Expected Array");
    }
    if let Some(Value::Array(mut values)) = interp.get("з") {
        values.sort_by_key(|v| v.to_string());
        assert_eq!(values, vec![Value::Number(1.0), Value::Number(2.0)]);
    } else {
        panic!("Expected Array");
    }
}

#[test]
fn test_stdlib_json_stringify_parse_roundtrip() {
    let interp = run_code(
        r#"
        гыы о = { имя: "Саня", возраст: 25, активен: правда };
        гыы с = Жсон.вСтроку(о);
        гыы об = Жсон.разобрать(с);
        гыы имя = об.имя;
        гыы возраст = об.возраст;
        гыы активен = об.активен;
        "#,
    );
    assert_eq!(interp.get("имя"), Some(Value::String("Саня".to_string())));
    assert_eq!(interp.get("возраст"), Some(Value::Number(25.0)));
    assert_eq!(interp.get("активен"), Some(Value::Boolean(true)));
}

#[test]
fn test_stdlib_json_parse_array() {
    let interp = run_code(
        r#"
        гыы а = Жсон.разобрать("[1, 2, 3]");
        "#,
    );
    assert_eq!(interp.get("а"), Some(Value::Array(vec![Value::Number(1.0), Value::Number(2.0), Value::Number(3.0)])));
}

#[test]
fn json_parse_rejects_trailing_chars() {
    let err = run_code_err(r#"гыы а = Жсон.разобрать("{} мусор");"#);
    assert!(err.message.contains("Лишние символы"), "got: {}", err.message);
}

#[test]
fn json_parse_object_missing_colon() {
    let err = run_code_err(r#"гыы а = Жсон.разобрать("{\"к\" 1}");"#);
    assert!(err.message.contains("':'") || err.message.contains("JSON"), "got: {}", err.message);
}

#[test]
fn json_parse_array_missing_comma() {
    let err = run_code_err(r#"гыы а = Жсон.разобрать("[1 2]");"#);
    assert!(err.message.contains("','") || err.message.contains("']'"), "got: {}", err.message);
}

#[test]
fn json_parse_unexpected_token() {
    let err = run_code_err(r#"гыы а = Жсон.разобрать("чушь");"#);
    assert!(err.message.contains("JSON"), "got: {}", err.message);
}

#[test]
fn json_parse_incomplete_unicode_escape() {
    let err = run_code_err(r#"гыы а = Жсон.разобрать("\"\\u00\"");"#);
    assert!(err.message.contains("\\u") || err.message.contains("escape"), "got: {}", err.message);
}

#[test]
fn json_stringify_rejects_function() {
    let err = run_code_err(
        r#"
        гыы ф = () => 1;
        гыы с = Жсон.вСтроку(ф);
        "#,
    );
    assert!(err.message.contains("Функции") || err.message.contains("JSON"), "got: {}", err.message);
}

#[test]
fn json_stringify_rejects_symbol() {
    let err = run_code_err(
        r#"
        гыы с = Симбол("х");
        гыы стр = Жсон.вСтроку(с);
        "#,
    );
    assert!(err.message.contains("Символ") || err.message.contains("JSON"), "got: {}", err.message);
}

#[test]
fn object_keys_rejects_non_object() {
    let err = run_code_err(r#"гыы к = Кент.ключи(42);"#);
    assert!(err.message.contains("Кент.ключи"), "got: {}", err.message);
}

#[test]
fn promise_constructor_requires_function() {
    let err = run_code_err(r#"гыы p = захуярить СловоПацана(5);"#);
    assert!(err.message.contains("исполнитель") || err.message.contains("СловоПацана"), "got: {}", err.message);
}

#[test]
fn promise_race_rejects_empty_array() {
    let err = run_code_err(r#"гыы p = СловоПацана.гонка([]);"#);
    assert!(err.message.contains("гонка") || err.message.contains("пуст"), "got: {}", err.message);
}

#[test]
fn array_reduce_empty_without_initial_errors() {
    let err = run_code_err(
        r#"
        гыы а = [];
        гыы р = а.свернуть((а, в) => а + в);
        "#,
    );
    assert!(err.message.contains("reduce") || err.message.contains("пуст"), "got: {}", err.message);
}

#[test]
fn iterator_reduce_empty_without_initial_errors() {
    let err = run_code_err(
        r#"
        гыы и = Итератор.от([]);
        гыы р = и.свернуть((а, в) => а + в);
        "#,
    );
    assert!(err.message.contains("reduce") || err.message.contains("пуст"), "got: {}", err.message);
}

#[test]
fn string_repeat_negative_count_errors() {
    let err = run_code_err(
        r#"
        гыы с = "а";
        гыы р = с.повторить(-1);
        "#,
    );
    assert!(err.message.contains("повторений") || err.message.contains("Некорректное"), "got: {}", err.message);
}

#[test]
fn test_stdlib_number_checks() {
    let interp = run_code(
        r#"
        гыы кон = Хуйня.конечна(5);
        гыы кон2 = Хуйня.конечна(5.5);
        гыы цел = Хуйня.целая(5);
        гыы цел2 = Хуйня.целая(5.5);
        "#,
    );
    assert_eq!(interp.get("кон"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("кон2"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("цел"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("цел2"), Some(Value::Boolean(false)));
}

#[test]
fn test_stdlib_array_is_array() {
    let interp = run_code(
        r#"
        гыы а = Помойка.являетсяПомойкой([1, 2]);
        гыы б = Помойка.являетсяПомойкой("строка");
        "#,
    );
    assert_eq!(interp.get("а"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("б"), Some(Value::Boolean(false)));
}

#[test]
fn test_stdlib_array_sort() {
    let interp = run_code(
        r#"
        гыы а = [3, 1, 4, 1, 5, 9, 2, 6];
        а.sort((л, п) => л - п);
        "#,
    );
    assert_eq!(
        interp.get("а"),
        Some(Value::Array(vec![
            Value::Number(1.0),
            Value::Number(1.0),
            Value::Number(2.0),
            Value::Number(3.0),
            Value::Number(4.0),
            Value::Number(5.0),
            Value::Number(6.0),
            Value::Number(9.0),
        ]))
    );
}

#[test]
fn test_stdlib_array_splice() {
    let interp = run_code(
        r#"
        гыы а = [1, 2, 3, 4, 5];
        гыы удалённые = а.splice(1, 2, 9, 9);
        "#,
    );
    assert_eq!(
        interp.get("а"),
        Some(Value::Array(vec![
            Value::Number(1.0),
            Value::Number(9.0),
            Value::Number(9.0),
            Value::Number(4.0),
            Value::Number(5.0),
        ]))
    );
    assert_eq!(interp.get("удалённые"), Some(Value::Array(vec![Value::Number(2.0), Value::Number(3.0)])));
}

#[test]
fn test_stdlib_array_to_spliced() {
    let interp = run_code(
        r#"
        гыы а = [1, 2, 3, 4];
        гыы б = а.toSpliced(1, 1, 8, 9);
        "#,
    );
    assert_eq!(
        interp.get("а"),
        Some(Value::Array(vec![Value::Number(1.0), Value::Number(2.0), Value::Number(3.0), Value::Number(4.0),]))
    );
    assert_eq!(
        interp.get("б"),
        Some(Value::Array(vec![
            Value::Number(1.0),
            Value::Number(8.0),
            Value::Number(9.0),
            Value::Number(3.0),
            Value::Number(4.0),
        ]))
    );
}

#[test]
fn test_karta_get_or_insert() {
    let interp = run_code(
        r#"
        гыы м = захуярить Карта();
        м.set("а", 1);
        гыы существ = м.getOrInsert("а", 99);
        гыы новое = м.getOrInsert("б", 7);
        гыы итог = м.get("б");
        "#,
    );
    assert_eq!(interp.get("существ"), Some(Value::Number(1.0)));
    assert_eq!(interp.get("новое"), Some(Value::Number(7.0)));
    assert_eq!(interp.get("итог"), Some(Value::Number(7.0)));
}

#[test]
fn test_karta_get_or_insert_computed() {
    let interp = run_code(
        r#"
        гыы м = захуярить Карта();
        гыы вызовов = 0;
        гыы вычислить = (к) => {
            вызовов += 1;
            отвечаю к + "!";
        };
        гыы а = м.getOrInsertComputed("привет", вычислить);
        гыы б = м.getOrInsertComputed("привет", вычислить);
        "#,
    );
    assert_eq!(interp.get("а"), Some(Value::String("привет!".to_string())));
    assert_eq!(interp.get("б"), Some(Value::String("привет!".to_string())));
    assert_eq!(interp.get("вызовов"), Some(Value::Number(1.0)));
}

#[test]
fn test_kosyak_construct() {
    let interp = run_code(
        r#"
        гыы е = захуярить Косяк("плохо");
        гыы имя = е.name;
        гыы сообщ = е.message;
        "#,
    );
    assert_eq!(interp.get("имя"), Some(Value::String("Косяк".to_string())));
    assert_eq!(interp.get("сообщ"), Some(Value::String("плохо".to_string())));
}

#[test]
fn test_kosyak_without_new() {
    let interp = run_code(
        r#"
        гыы е = Косяк("сообщение");
        гыы имя = е.name;
        "#,
    );
    assert_eq!(interp.get("имя"), Some(Value::String("Косяк".to_string())));
}

#[test]
fn test_kosyak_throw_catch() {
    let interp = run_code(
        r#"
        гыы пойман = ноль;
        хапнуть {
            кидай захуярить Косяк("упало");
        } гоп (е) {
            пойман = е.message;
        }
        "#,
    );
    assert_eq!(interp.get("пойман"), Some(Value::String("упало".to_string())));
}

#[test]
fn test_kosyak_with_cause() {
    let interp = run_code(
        r#"
        гыы первый = захуярить Косяк("первая ошибка");
        гыы второй = захуярить Косяк("обёртка", { cause: первый });
        гыы причина = второй.cause;
        гыы сообщ = причина.message;
        "#,
    );
    assert_eq!(interp.get("сообщ"), Some(Value::String("первая ошибка".to_string())));
}

#[test]
fn test_kent_group_by() {
    let interp = run_code(
        r#"
        гыы числа = [1, 2, 3, 4, 5, 6, 7];
        гыы по_чётности = Кент.группировать(числа, (n) => n % 2 === 0 ? "чётные" : "нечётные");
        гыы чётные = по_чётности["чётные"];
        гыы нечётные = по_чётности["нечётные"];
        "#,
    );
    assert_eq!(
        interp.get("чётные"),
        Some(Value::Array(vec![Value::Number(2.0), Value::Number(4.0), Value::Number(6.0)]))
    );
    assert_eq!(
        interp.get("нечётные"),
        Some(Value::Array(vec![Value::Number(1.0), Value::Number(3.0), Value::Number(5.0), Value::Number(7.0),]))
    );
}

#[test]
fn test_huynya_parse_int() {
    let interp = run_code(
        r#"
        гыы а = Хуйня.разобратьЦелое("42");
        гыы б = Хуйня.разобратьЦелое("  -17  ");
        гыы в = Хуйня.разобратьЦелое("1010", 2);
        гыы г = Хуйня.разобратьЦелое("ff", 16);
        гыы д = Хуйня.разобратьЦелое("123abc");
        гыы е = Хуйня.разобратьЦелое("xyz");
        "#,
    );
    assert_eq!(interp.get("а"), Some(Value::Number(42.0)));
    assert_eq!(interp.get("б"), Some(Value::Number(-17.0)));
    assert_eq!(interp.get("в"), Some(Value::Number(10.0)));
    assert_eq!(interp.get("г"), Some(Value::Number(255.0)));
    assert_eq!(interp.get("д"), Some(Value::Number(123.0)));
    if let Some(Value::Number(n)) = interp.get("е") {
        assert!(n.is_nan(), "ожидалось NaN, получено {n}");
    } else {
        panic!("ожидалось Number(NaN)");
    }
}

#[test]
fn test_huynya_parse_float() {
    let interp = run_code(
        r#"
        гыы а = Хуйня.разобратьЧисло("2.5");
        гыы б = Хуйня.разобратьЧисло("  -2.5e2  ");
        гыы в = Хуйня.разобратьЧисло("123abc");
        гыы г = Хуйня.разобратьЧисло("");
        "#,
    );
    assert_eq!(interp.get("а"), Some(Value::Number(2.5)));
    assert_eq!(interp.get("б"), Some(Value::Number(-250.0)));
    if let Some(Value::Number(n)) = interp.get("г") {
        assert!(n.is_nan());
    } else {
        panic!("ожидалось NaN");
    }
    let _ = interp.get("в");
}

#[test]
fn test_spread_set_into_array() {
    let interp = run_code(
        r#"
        гыы н = захуярить Набор([1, 2, 3]);
        гыы а = [0, ...н, 4];
        "#,
    );
    assert_eq!(
        interp.get("а"),
        Some(Value::Array(vec![
            Value::Number(0.0),
            Value::Number(1.0),
            Value::Number(2.0),
            Value::Number(3.0),
            Value::Number(4.0),
        ]))
    );
}

#[test]
fn test_spread_map_into_array() {
    let interp = run_code(
        r#"
        гыы к = захуярить Карта([["а", 1], ["б", 2]]);
        гыы а = [...к];
        "#,
    );
    assert_eq!(
        interp.get("а"),
        Some(Value::Array(vec![
            Value::Array(vec![Value::String("а".to_string()), Value::Number(1.0)]),
            Value::Array(vec![Value::String("б".to_string()), Value::Number(2.0)]),
        ]))
    );
}

#[test]
fn test_spread_string_into_array() {
    let interp = run_code(
        r#"
        гыы а = [..."абв"];
        "#,
    );
    assert_eq!(
        interp.get("а"),
        Some(Value::Array(vec![
            Value::String("а".to_string()),
            Value::String("б".to_string()),
            Value::String("в".to_string()),
        ]))
    );
}

#[test]
fn test_spread_set_into_args() {
    let interp = run_code(
        r#"
        гыы н = захуярить Набор([1, 5, 3, 9, 2]);
        гыы макс = Матан.макс(...н);
        "#,
    );
    assert_eq!(interp.get("макс"), Some(Value::Number(9.0)));
}

#[test]
fn test_for_of_set() {
    let interp = run_code(
        r#"
        гыы н = захуярить Набор([10, 20, 30]);
        гыы сумма = 0;
        го (х сашаГрей н) {
            сумма += х;
        }
        "#,
    );
    assert_eq!(interp.get("сумма"), Some(Value::Number(60.0)));
}

#[test]
fn test_for_of_map_yields_pairs() {
    let interp = run_code(
        r#"
        гыы к = захуярить Карта([["а", 1], ["б", 2], ["в", 3]]);
        гыы ключи = [];
        гыы суммаЗнч = 0;
        го (пара сашаГрей к) {
            ключи.push(пара[0]);
            суммаЗнч += пара[1];
        }
        "#,
    );
    assert_eq!(
        interp.get("ключи"),
        Some(Value::Array(vec![
            Value::String("а".to_string()),
            Value::String("б".to_string()),
            Value::String("в".to_string()),
        ]))
    );
    assert_eq!(interp.get("суммаЗнч"), Some(Value::Number(6.0)));
}

#[test]
fn test_nabor_basic_operations() {
    let interp = run_code(
        r#"
        гыы н = захуярить Набор();
        н.add(1);
        н.add(2);
        н.add(2);
        гыы есть = н.has(1);
        гыы размер = н.size;
        "#,
    );
    assert_eq!(interp.get("есть"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("размер"), Some(Value::Number(2.0)));
}

#[test]
fn test_nabor_construct_from_array() {
    let interp = run_code(
        r#"
        гыы н = захуярить Набор([1, 2, 2, 3, 3, 3]);
        гыы размер = н.size;
        "#,
    );
    assert_eq!(interp.get("размер"), Some(Value::Number(3.0)));
}

#[test]
fn test_nabor_delete_clear() {
    let interp = run_code(
        r#"
        гыы н = захуярить Набор([1, 2, 3]);
        гыы убрал = н.delete(2);
        гыы естьЛи = н.has(2);
        н.clear();
        гыы пустой = н.size;
        "#,
    );
    assert_eq!(interp.get("убрал"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("естьЛи"), Some(Value::Boolean(false)));
    assert_eq!(interp.get("пустой"), Some(Value::Number(0.0)));
}

#[test]
fn test_nabor_values() {
    let interp = run_code(
        r#"
        гыы н = захуярить Набор([3, 1, 2]);
        гыы зн = н.values();
        "#,
    );
    assert_eq!(interp.get("зн"), Some(Value::Array(vec![Value::Number(3.0), Value::Number(1.0), Value::Number(2.0)])));
}

#[test]
fn test_nabor_for_each() {
    let interp = run_code(
        r#"
        гыы н = захуярить Набор([10, 20, 30]);
        гыы сумма = 0;
        н.forEach((x) => { сумма += x; });
        "#,
    );
    assert_eq!(interp.get("сумма"), Some(Value::Number(60.0)));
}

#[test]
fn test_nabor_set_operations() {
    let interp = run_code(
        r#"
        гыы а = захуярить Набор([1, 2, 3]);
        гыы б = захуярить Набор([3, 4, 5]);
        гыы пересечение = а.intersection(б).size;
        гыы объединение = а.union(б).size;
        гыы разница = а.difference(б).size;
        гыы симРазн = а.symmetricDifference(б).size;
        "#,
    );
    assert_eq!(interp.get("пересечение"), Some(Value::Number(1.0)));
    assert_eq!(interp.get("объединение"), Some(Value::Number(5.0)));
    assert_eq!(interp.get("разница"), Some(Value::Number(2.0)));
    assert_eq!(interp.get("симРазн"), Some(Value::Number(4.0)));
}

#[test]
fn test_nabor_subset_superset_disjoint() {
    let interp = run_code(
        r#"
        гыы малый = захуярить Набор([1, 2]);
        гыы большой = захуярить Набор([1, 2, 3, 4]);
        гыы отдельный = захуярить Набор([99]);
        гыы под = малый.isSubsetOf(большой);
        гыы над = большой.isSupersetOf(малый);
        гыы непер = малый.isDisjointFrom(отдельный);
        "#,
    );
    assert_eq!(interp.get("под"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("над"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("непер"), Some(Value::Boolean(true)));
}

#[test]
fn test_karta_basic_operations() {
    let interp = run_code(
        r#"
        гыы к = захуярить Карта();
        к.set("а", 1);
        к.set("б", 2);
        гыы есть = к.has("а");
        гыы значение = к.get("б");
        гыы размер = к.size;
        "#,
    );
    assert_eq!(interp.get("есть"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("значение"), Some(Value::Number(2.0)));
    assert_eq!(interp.get("размер"), Some(Value::Number(2.0)));
}

#[test]
fn test_karta_construct_from_pairs() {
    let interp = run_code(
        r#"
        гыы к = захуярить Карта([["а", 1], ["б", 2]]);
        гыы а = к.get("а");
        гыы б = к.get("б");
        "#,
    );
    assert_eq!(interp.get("а"), Some(Value::Number(1.0)));
    assert_eq!(interp.get("б"), Some(Value::Number(2.0)));
}

#[test]
fn test_karta_delete_clear() {
    let interp = run_code(
        r#"
        гыы к = захуярить Карта([["а", 1], ["б", 2], ["в", 3]]);
        к.delete("б");
        гыы есть = к.has("б");
        гыы рдо = к.size;
        к.clear();
        гыы рпосле = к.size;
        "#,
    );
    assert_eq!(interp.get("есть"), Some(Value::Boolean(false)));
    assert_eq!(interp.get("рдо"), Some(Value::Number(2.0)));
    assert_eq!(interp.get("рпосле"), Some(Value::Number(0.0)));
}

#[test]
fn test_karta_keys_values_entries_preserve_insertion_order() {
    let interp = run_code(
        r#"
        гыы к = захуярить Карта();
        к.set("первый", 1);
        к.set("второй", 2);
        к.set("третий", 3);
        гыы клч = к.keys();
        гыы знч = к.values();
        "#,
    );
    assert_eq!(
        interp.get("клч"),
        Some(Value::Array(vec![
            Value::String("первый".to_string()),
            Value::String("второй".to_string()),
            Value::String("третий".to_string()),
        ]))
    );
    assert_eq!(interp.get("знч"), Some(Value::Array(vec![Value::Number(1.0), Value::Number(2.0), Value::Number(3.0)])));
}

#[test]
fn test_karta_overwrite_keeps_position() {
    let interp = run_code(
        r#"
        гыы к = захуярить Карта();
        к.set("а", 1);
        к.set("б", 2);
        к.set("а", 99);
        гыы знч = к.values();
        "#,
    );
    assert_eq!(interp.get("знч"), Some(Value::Array(vec![Value::Number(99.0), Value::Number(2.0)])));
}

#[test]
fn test_karta_static_ot_par() {
    let interp = run_code(
        r#"
        гыы к = Карта.отПар([["x", 10], ["y", 20]]);
        гыы x = к.get("x");
        "#,
    );
    assert_eq!(interp.get("x"), Some(Value::Number(10.0)));
}

#[test]
fn test_karta_for_each() {
    let interp = run_code(
        r#"
        гыы к = захуярить Карта([["а", 1], ["б", 2]]);
        гыы сумма = 0;
        к.forEach((значение, ключ) => {
            сумма += значение;
        });
        "#,
    );
    assert_eq!(interp.get("сумма"), Some(Value::Number(3.0)));
}

#[test]
fn test_karta_keys_supports_non_string() {
    let interp = run_code(
        r#"
        гыы к = захуярить Карта();
        к.set(1, "один");
        к.set(2, "два");
        гыы а = к.get(1);
        гыы б = к.get(2);
        "#,
    );
    assert_eq!(interp.get("а"), Some(Value::String("один".to_string())));
    assert_eq!(interp.get("б"), Some(Value::String("два".to_string())));
}

#[test]
fn test_eto_kosyak() {
    let interp = run_code(
        r#"
        гыы а = этоКосяк(захуярить Косяк("ой"));
        гыы б = этоКосяк("просто строка");
        гыы в = этоКосяк({ name: "Другое" });
        "#,
    );
    assert_eq!(interp.get("а"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("б"), Some(Value::Boolean(false)));
    assert_eq!(interp.get("в"), Some(Value::Boolean(false)));
}

#[test]
fn using_disposes_on_scope_exit() {
    let interp = run_code(
        r#"
        гыы счёт = 0;
        {
            юзай р = { расход: () => { счёт = счёт + 1; } };
        }
        "#,
    );
    assert_eq!(interp.get("счёт"), Some(Value::Number(1.0)));
}

#[test]
fn using_disposes_in_lifo_order() {
    let interp = run_code(
        r#"
        гыы лог = [];
        {
            юзай а = { расход: () => { лог.push("а"); } };
            юзай б = { расход: () => { лог.push("б"); } };
            юзай в = { расход: () => { лог.push("в"); } };
        }
        "#,
    );
    let log = interp.get("лог").unwrap();
    let Value::Array(items) = log else { panic!("expected array") };
    assert_eq!(items.len(), 3);
    assert_eq!(items[0], Value::String("в".to_string()));
    assert_eq!(items[1], Value::String("б".to_string()));
    assert_eq!(items[2], Value::String("а".to_string()));
}

#[test]
fn using_skips_null_resource() {
    let interp = run_code(
        r#"
        гыы счёт = 0;
        {
            юзай р = ноль;
        }
        "#,
    );
    assert_eq!(interp.get("счёт"), Some(Value::Number(0.0)));
}

#[test]
fn using_requires_dispose_method() {
    let err = run_code_err(
        r#"
        {
            юзай р = { данные: 42 };
        }
        "#,
    );
    assert!(err.message.contains("расход"));
}

#[test]
fn using_with_class_instance() {
    let interp = run_code(
        r#"
        гыы счёт = 0;
        клёво Файл {
            расход() {
                счёт = счёт + 10;
            }
        }
        {
            юзай ф = захуярить Файл();
        }
        "#,
    );
    assert_eq!(interp.get("счёт"), Some(Value::Number(10.0)));
}

#[test]
fn symbol_create_and_typeof() {
    let interp = run_code(
        r#"
        гыы с = Симбол("привет");
        гыы т = чезажижан с;
        "#,
    );
    assert_eq!(interp.get("т"), Some(Value::String("символ".to_string())));
}

#[test]
fn symbol_unique_identity() {
    let interp = run_code(
        r#"
        гыы а = Симбол("ключ");
        гыы б = Симбол("ключ");
        гыы равны = а === б;
        гыы самСебя = а === а;
        "#,
    );
    assert_eq!(interp.get("равны"), Some(Value::Boolean(false)));
    assert_eq!(interp.get("самСебя"), Some(Value::Boolean(true)));
}

#[test]
fn symbol_for_returns_shared() {
    let interp = run_code(
        r#"
        гыы а = Симбол.для("общий");
        гыы б = Симбол.для("общий");
        гыы в = Симбол.для("другой");
        гыы равны1 = а === б;
        гыы равны2 = а === в;
        "#,
    );
    assert_eq!(interp.get("равны1"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("равны2"), Some(Value::Boolean(false)));
}

#[test]
fn symbol_description_property() {
    let interp = run_code(
        r#"
        гыы с = Симбол("моёОписание");
        гыы оп = с.описание;
        "#,
    );
    assert_eq!(interp.get("оп"), Some(Value::String("моёОписание".to_string())));
}

#[test]
fn symbol_well_known_iterator_dispose() {
    let interp = run_code(
        r#"
        гыы и1 = Симбол.итератор;
        гыы и2 = Симбол.итератор;
        гыы р1 = Симбол.расход;
        гыы итерРасх = и1 === р1;
        гыы итерИтер = и1 === и2;
        "#,
    );
    assert_eq!(interp.get("итерРасх"), Some(Value::Boolean(false)));
    assert_eq!(interp.get("итерИтер"), Some(Value::Boolean(true)));
}

#[test]
fn symbol_to_string_method() {
    let interp = run_code(
        r#"
        гыы с = Симбол("м");
        гыы стр = с.вСтроку();
        "#,
    );
    assert_eq!(interp.get("стр"), Some(Value::String("Симбол(м)".to_string())));
}

#[test]
fn dict_alias_chounastoot_for_in() {
    let interp = run_code(
        r#"
        гыы об = { а: 1, б: 2 };
        гыы ключи = [];
        го (гыы к чоунастут об) {
            ключи.push(к);
        }
        гыы длина = ключи.length;
        "#,
    );
    assert_eq!(interp.get("длина"), Some(Value::Number(2.0)));
}

#[test]
fn dict_alias_nan_global() {
    let interp = run_code(
        r#"
        гыы н = нихуя;
        гыы это_нан = Хуйня.нихуя(н);
        "#,
    );
    assert_eq!(interp.get("это_нан"), Some(Value::Boolean(true)));
}

#[test]
fn dict_alias_nan_is_const() {
    let err = run_code_err(
        r#"
        нихуя = 1;
        "#,
    );
    assert!(err.message.contains("константу") || err.message.contains("const"));
}

#[test]
fn dict_modifier_private_field_blocks_outside_access() {
    let err = run_code_err(
        r#"
        клёво К {
            Кошелёк() {}
            мой бабки = 500;
        }
        гыы к = захуярить К();
        гыы х = к.#бабки;
        "#,
    );
    assert!(err.message.contains("приватному полю"));
}

#[test]
fn dict_modifier_private_field_accessible_inside_method() {
    let i = run_code(
        r#"
        клёво К {
            мой значение = 42;
            читать() { отвечаю тырыпыры.#значение; }
        }
        гыы о = захуярить К();
        гыы р = о.читать();
        "#,
    );
    assert_eq!(i.get("р"), Some(Value::Number(42.0)));
}

#[test]
fn dict_modifier_public_protected_parse_as_public() {
    let i = run_code(
        r#"
        клёво К {
            ебанное публ = 1;
            подкрыша прот = 2;
            ебанное взять() { отвечаю тырыпыры.публ + тырыпыры.прот; }
        }
        гыы о = захуярить К();
        гыы а = о.публ;
        гыы б = о.прот;
        гыы в = о.взять();
        "#,
    );
    assert_eq!(i.get("а"), Some(Value::Number(1.0)));
    assert_eq!(i.get("б"), Some(Value::Number(2.0)));
    assert_eq!(i.get("в"), Some(Value::Number(3.0)));
}

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
    assert_eq!(i.get("рез"), Some(Value::Array(vec![Value::Number(0.0), Value::Number(1.0), Value::Number(2.0)])));
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
    assert_eq!(i.get("рез"), Some(Value::Array(vec![Value::Undefined, Value::Undefined])));
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
    assert_eq!(
        i.get("рез"),
        Some(Value::Array(vec![Value::Number(1.0), Value::Number(10.0), Value::Number(20.0), Value::Number(2.0),]))
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
    assert_eq!(i.get("рез"), Some(Value::Array(vec![Value::Number(1.0)])));
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
    assert_eq!(
        i.get("рез"),
        Some(Value::Array(vec![Value::Number(0.0), Value::Number(1.0), Value::Number(2.0), Value::Number(3.0),]))
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
    assert_eq!(i.get("рез"), Some(Value::Array(vec![Value::Number(0.0), Value::Number(2.0), Value::Number(4.0)])));
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
        assert_eq!(m.get("значение"), Some(&Value::String("а".to_string())));
        assert_eq!(m.get("готово"), Some(&Value::Boolean(false)));
    } else {
        panic!("ожидался объект, получено {r1:?}");
    }
    if let Value::Object(m) = r2 {
        assert_eq!(m.get("значение"), Some(&Value::String("б".to_string())));
        assert_eq!(m.get("готово"), Some(&Value::Boolean(false)));
    } else {
        panic!();
    }
    if let Value::Object(m) = r3 {
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
    assert_eq!(
        i.get("рез"),
        Some(Value::Array(vec![
            Value::Number(0.0),
            Value::Number(1.0),
            Value::Number(2.0),
            Value::Number(3.0),
            Value::Number(4.0),
            Value::Number(99.0),
        ]))
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
    assert_eq!(
        i.get("рез"),
        Some(Value::Array(vec![Value::Number(1.0), Value::Number(2.0), Value::Number(4.0), Value::Number(5.0),]))
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
    assert_eq!(
        i.get("рез"),
        Some(Value::Array(vec![Value::Number(1.0), Value::String("бум".to_string()), Value::Number(2.0),]))
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
    assert_eq!(i.get("рез"), Some(Value::Array(vec![Value::Number(0.0), Value::Number(4.0), Value::Number(16.0)])));
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
fn dict_debugger_is_noop() {
    let interp = run_code(
        r#"
        гыы а = 1;
        логопед;
        гыы б = 2;
        "#,
    );
    assert_eq!(interp.get("а"), Some(Value::Number(1.0)));
    assert_eq!(interp.get("б"), Some(Value::Number(2.0)));
}

#[test]
fn test_stdlib_chained_methods() {
    let interp = run_code(
        r#"
        гыы рез = [1, 2, 3, 4, 5]
            .filter((x) => x > 1)
            .map((x) => x * x)
            .reduce((а, б) => а + б, 0);
        "#,
    );
    assert_eq!(interp.get("рез"), Some(Value::Number(54.0)));
}

#[test]
fn test_async_function_then_runs_without_explicit_await() {
    let interp = run_code(
        r#"
        ассо йопта f() { отвечаю 42; }
        гыы итог = 0;
        f().потом((v) => { итог = v; });
        "#,
    );
    assert_eq!(interp.get("итог"), Some(Value::Number(42.0)));
}

#[test]
fn test_async_function_returns_promise() {
    let interp = run_code(
        r#"
        ассо йопта f() { отвечаю 42; }
        гыы p = f();
        гыы т = тип(p);
        "#,
    );
    assert_eq!(interp.get("т"), Some(Value::String("обещание".to_string())));
    match interp.get("p") {
        Some(Value::Promise { .. }) => {}
        other => panic!("Ожидался Promise, получено {other:?}"),
    }
}

#[test]
fn test_async_await_chain_then() {
    let interp = run_code(
        r#"
        ассо йопта f() { отвечаю 1; }
        ассо йопта g() {
            гыы x = сидетьНахуй f();
            отвечаю x + 1;
        }
        гыы итог = 0;
        g().потом((v) => { итог = v; });
        "#,
    );
    assert_eq!(interp.get("итог"), Some(Value::Number(2.0)));
}

#[test]
fn test_promise_resolve_then() {
    let interp = run_code(
        r#"
        гыы итог = 0;
        СловоПацана.решить(5).потом((v) => { итог = v; });
        "#,
    );
    assert_eq!(interp.get("итог"), Some(Value::Number(5.0)));
}

#[test]
fn test_promise_all_resolved() {
    let interp = run_code(
        r#"
        гыы итог = ноль;
        СловоПацана.всех([СловоПацана.решить(1), СловоПацана.решить(2)]).потом((v) => { итог = v; });
        "#,
    );
    assert_eq!(interp.get("итог"), Some(Value::Array(vec![Value::Number(1.0), Value::Number(2.0)])));
}

#[test]
fn test_await_rejected_throws_catchable() {
    let interp = run_code(
        r#"
        ассо йопта плохо() {
            кидай "беда";
        }
        ассо йопта тест() {
            хапнуть {
                сидетьНахуй плохо();
                отвечаю "ок";
            } гоп (e) {
                отвечаю "поймал";
            }
        }
        гыы итог = "пусто";
        тест().потом((v) => { итог = v; });
        "#,
    );
    assert_eq!(interp.get("итог"), Some(Value::String("поймал".to_string())));
}

#[test]
fn test_promise_then_on_resolved_executes_once() {
    let interp = run_code(
        r#"
        гыы счёт = 0;
        гыы p = СловоПацана.решить(1);
        p.потом((v) => { счёт = счёт + v; });
        "#,
    );
    assert_eq!(interp.get("счёт"), Some(Value::Number(1.0)));
}

#[test]
fn test_promise_then_stored_then_chained() {
    let interp = run_code(
        r#"
        гыы итог = 0;
        гыы p = СловоПацана.решить(10);
        p.потом((v) => { итог = v; });
        "#,
    );
    assert_eq!(interp.get("итог"), Some(Value::Number(10.0)));
}

#[test]
fn test_promise_catch_on_rejected() {
    let interp = run_code(
        r#"
        гыы итог = "нет";
        СловоПацана.отвергнуть("ошибка").ловить((e) => { итог = e; });
        "#,
    );
    assert_eq!(interp.get("итог"), Some(Value::String("ошибка".to_string())));
}

#[test]
fn test_promise_finally_on_fulfilled() {
    let interp = run_code(
        r#"
        гыы итог = 0;
        СловоПацана.решить(7).наконец(() => { итог = 1; });
        "#,
    );
    assert_eq!(interp.get("итог"), Some(Value::Number(1.0)));
}

#[test]
fn test_promise_then_chained_twice() {
    let interp = run_code(
        r#"
        гыы итог = 0;
        СловоПацана.решить(3)
            .потом((v) => { отвечаю v + 1; })
            .потом((v) => { итог = v; });
        "#,
    );
    assert_eq!(interp.get("итог"), Some(Value::Number(4.0)));
}

#[test]
fn test_iterator_from_array_to_array() {
    let interp = run_code(
        r#"
        гыы и = Итератор.от([1, 2, 3]);
        гыы рез = и.вМассив();
        "#,
    );
    assert_eq!(interp.get("рез"), Some(Value::Array(vec![Value::Number(1.0), Value::Number(2.0), Value::Number(3.0)])));
}

#[test]
fn test_iterator_map_lazy() {
    let interp = run_code(
        r#"
        гыы рез = Итератор.от([1, 2, 3]).преобразовать((х) => х * 10).вМассив();
        "#,
    );
    assert_eq!(
        interp.get("рез"),
        Some(Value::Array(vec![Value::Number(10.0), Value::Number(20.0), Value::Number(30.0)]))
    );
}

#[test]
fn test_iterator_filter_take_drop_chain() {
    let interp = run_code(
        r#"
        гыы рез = Итератор.от([1, 2, 3, 4, 5, 6, 7, 8])
            .отфильтровать((х) => х % 2 == 0)
            .пропустить(1)
            .взять(2)
            .вМассив();
        "#,
    );
    assert_eq!(interp.get("рез"), Some(Value::Array(vec![Value::Number(4.0), Value::Number(6.0)])));
}

#[test]
fn test_iterator_reduce() {
    let interp = run_code(
        r#"
        гыы сумма = Итератор.от([1, 2, 3, 4]).свернуть((а, б) => а + б, 0);
        "#,
    );
    assert_eq!(interp.get("сумма"), Some(Value::Number(10.0)));
}

#[test]
fn test_iterator_for_of_drains() {
    let interp = run_code(
        r#"
        гыы итог = 0;
        го (х сашаГрей Итератор.от([10, 20, 30])) {
            итог = итог + х;
        }
        "#,
    );
    assert_eq!(interp.get("итог"), Some(Value::Number(60.0)));
}

#[test]
fn test_iterator_concat() {
    let interp = run_code(
        r#"
        гыы рез = Итератор.склеить([1, 2], [3, 4], [5]).вМассив();
        "#,
    );
    assert_eq!(
        interp.get("рез"),
        Some(Value::Array(vec![
            Value::Number(1.0),
            Value::Number(2.0),
            Value::Number(3.0),
            Value::Number(4.0),
            Value::Number(5.0)
        ]))
    );
}

#[test]
fn test_iterator_some_every_find() {
    let interp = run_code(
        r#"
        гыы есть = Итератор.от([1, 2, 3]).некоторые((х) => х > 2);
        гыы все = Итератор.от([2, 4, 6]).все((х) => х % 2 == 0);
        гыы первое = Итератор.от([1, 2, 3, 4]).найти((х) => х > 2);
        "#,
    );
    assert_eq!(interp.get("есть"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("все"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("первое"), Some(Value::Number(3.0)));
}

#[test]
fn test_iterator_next_protocol() {
    let interp = run_code(
        r#"
        гыы и = Итератор.от([7, 8]);
        гыы а = и.следующий();
        гыы б = и.следующий();
        гыы в = и.следующий();
        "#,
    );
    let a = interp.get("а").unwrap();
    let b = interp.get("б").unwrap();
    let c = interp.get("в").unwrap();
    if let Value::Object(m) = a {
        assert_eq!(m.get("значение"), Some(&Value::Number(7.0)));
        assert_eq!(m.get("готово"), Some(&Value::Boolean(false)));
    } else {
        panic!("expected Object");
    }
    if let Value::Object(m) = b {
        assert_eq!(m.get("значение"), Some(&Value::Number(8.0)));
        assert_eq!(m.get("готово"), Some(&Value::Boolean(false)));
    } else {
        panic!("expected Object");
    }
    if let Value::Object(m) = c {
        assert_eq!(m.get("значение"), Some(&Value::Undefined));
        assert_eq!(m.get("готово"), Some(&Value::Boolean(true)));
    } else {
        panic!("expected Object");
    }
}

#[test]
fn test_iterator_from_string() {
    let interp = run_code(
        r#"
        гыы рез = Итератор.от("abc").вМассив();
        "#,
    );
    assert_eq!(
        interp.get("рез"),
        Some(Value::Array(vec![
            Value::String("a".to_string()),
            Value::String("b".to_string()),
            Value::String("c".to_string())
        ]))
    );
}

#[test]
fn test_iterator_for_of_break_stops_lazy_chain() {
    let interp = run_code(
        r#"
        гыы счёт = 0;
        го (х сашаГрей Итератор.от([1, 2, 3, 4, 5]).преобразовать((в) => { счёт = счёт + 1; отвечаю в; })) {
            вилкойвглаз (х == 3) { харэ; }
        }
        "#,
    );
    assert_eq!(interp.get("счёт"), Some(Value::Number(3.0)));
}

#[test]
fn test_iterator_for_of_yields_in_order_without_materializing() {
    let interp = run_code(
        r#"
        гыы итог = "";
        го (х сашаГрей Итератор.от([10, 20, 30]).преобразовать((в) => в + 1)) {
            итог = итог + строка(х) + ",";
        }
        "#,
    );
    assert_eq!(interp.get("итог"), Some(Value::String("11,21,31,".to_string())));
}

#[test]
fn test_iterator_spread_into_array() {
    let interp = run_code(
        r#"
        гыы рез = [0, ...Итератор.от([1, 2, 3]).преобразовать((х) => х + 1), 99];
        "#,
    );
    assert_eq!(
        interp.get("рез"),
        Some(Value::Array(vec![
            Value::Number(0.0),
            Value::Number(2.0),
            Value::Number(3.0),
            Value::Number(4.0),
            Value::Number(99.0)
        ]))
    );
}

#[test]
fn test_for_await_of_plain_values() {
    let interp = run_code(
        r#"
        ассо йопта тест() {
            гыы сумма = 0;
            го сидетьНахуй (х сашаГрей [1, 2, 3, 4]) {
                сумма += х;
            }
            отвечаю сумма;
        }
        гыы итог = 0;
        тест().потом((v) => { итог = v; });
        "#,
    );
    assert_eq!(interp.get("итог"), Some(Value::Number(10.0)));
}

#[test]
fn test_for_await_of_promises_in_array() {
    let interp = run_code(
        r#"
        ассо йопта тест() {
            гыы массив = [СловоПацана.решить(10), СловоПацана.решить(20), СловоПацана.решить(30)];
            гыы сумма = 0;
            го сидетьНахуй (х сашаГрей массив) {
                сумма += х;
            }
            отвечаю сумма;
        }
        гыы итог = 0;
        тест().потом((v) => { итог = v; });
        "#,
    );
    assert_eq!(interp.get("итог"), Some(Value::Number(60.0)));
}

#[test]
fn test_for_await_of_rejects_for_in() {
    use yps_lexer::{Lexer, SourceFile};
    use yps_parser::Parser;
    let source = SourceFile::new(
        "test".to_string(),
        r#"
        ассо йопта тест() {
            го сидетьНахуй (к чоунастут [1, 2]) { }
        }
        "#
        .to_string(),
    );
    let (tokens, _) = Lexer::new(&source).tokenize();
    let (_, parse_diags) = Parser::new(&tokens, &source).parse_program();
    assert!(
        parse_diags.iter().any(|d| d.message.contains("сидетьНахуй") || d.message.contains("сашаГрей")),
        "Парсер должен отклонять 'го сидетьНахуй (... чоунастут ...)', получено: {parse_diags:?}"
    );
}

#[test]
fn test_for_await_of_break_and_continue() {
    let interp = run_code(
        r#"
        ассо йопта тест() {
            гыы собрано = [];
            го сидетьНахуй (х сашаГрей [1, 2, 3, 4, 5]) {
                вилкойвглаз (х == 2) { двигай; }
                вилкойвглаз (х == 4) { харэ; }
                собрано.push(х);
            }
            отвечаю собрано;
        }
        гыы итог = ноль;
        тест().потом((v) => { итог = v; });
        "#,
    );
    assert_eq!(interp.get("итог"), Some(Value::Array(vec![Value::Number(1.0), Value::Number(3.0)])));
}

#[test]
fn test_kosyak_static_is_error() {
    let interp = run_code(
        r#"
        гыы а = Косяк.этоКосяк(захуярить Косяк("ой"));
        гыы б = Косяк.этоКосяк("строка");
        гыы в = Косяк.этоКосяк({ name: "Другое" });
        гыы г = Косяк.этоКосяк(ноль);
        "#,
    );
    assert_eq!(interp.get("а"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("б"), Some(Value::Boolean(false)));
    assert_eq!(interp.get("в"), Some(Value::Boolean(false)));
    assert_eq!(interp.get("г"), Some(Value::Boolean(false)));
}

#[test]
fn test_kosyak_static_is_error_english_alias() {
    let interp = run_code(
        r#"
        гыы а = Косяк.isError(захуярить Косяк("ой"));
        гыы б = Косяк.isError(42);
        "#,
    );
    assert_eq!(interp.get("а"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("б"), Some(Value::Boolean(false)));
}

#[test]
fn test_kosyak_unknown_static_method() {
    let err = run_code_err(
        r#"
        Косяк.несуществует("х");
        "#,
    );
    assert!(
        err.message.contains("Косяк") && err.message.contains("несуществует"),
        "Сообщение должно упоминать неизвестный метод, получено: {}",
        err.message
    );
}

fn run_with_module(module_src: &str, main_src: &str) -> Interpreter {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let id = SEQ.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("yps_dynimp_{}_{id}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("модуль.yop"), module_src).unwrap();

    let source = SourceFile::new("test".to_string(), main_src.to_string());
    let (tokens, lex_diags) = Lexer::new(&source).tokenize();
    assert!(lex_diags.is_empty(), "Ошибки лексера: {lex_diags:?}");
    let (program, parse_diags) = Parser::new(&tokens, &source).parse_program();
    assert!(parse_diags.is_empty(), "Ошибки парсера: {parse_diags:?}");
    let mut interp = Interpreter::new();
    interp.set_base_path(dir.clone());
    interp.run(&program).expect("Ошибка интерпретатора");
    let _ = std::fs::remove_dir_all(&dir);
    interp
}

#[test]
fn dynamic_import_returns_namespace() {
    let module = r#"
        предъява гыы х = 42;
        предъява йопта удвоить(а) { отвечаю а * 2; }
    "#;
    let main = r#"
        ассо йопта главная() {
            гыы м = сидетьНахуй спиздить("./модуль");
            результат_х = м.х;
            результат_удв = м.удвоить(10);
        }
        гыы результат_х = 0;
        гыы результат_удв = 0;
        главная();
    "#;
    let interp = run_with_module(module, main);
    assert_eq!(interp.get("результат_х"), Some(Value::Number(42.0)));
    assert_eq!(interp.get("результат_удв"), Some(Value::Number(20.0)));
}

#[test]
fn dynamic_import_missing_module_rejects() {
    let source = SourceFile::new(
        "test".to_string(),
        r#"
        ассо йопта главная() {
            хапнуть {
                сидетьНахуй спиздить("./нету_такого");
            } гоп (е) {
                поймано = правда;
            }
        }
        гыы поймано = лож;
        главная();
        "#
        .to_string(),
    );
    let (tokens, _) = Lexer::new(&source).tokenize();
    let (program, _) = Parser::new(&tokens, &source).parse_program();
    let mut interp = Interpreter::new();
    interp.set_base_path(std::env::temp_dir());
    interp.run(&program).expect("await rejected promise превращается в ошибку, поймано в try/catch");
    assert_eq!(interp.get("поймано"), Some(Value::Boolean(true)));
}

#[test]
fn dynamic_import_at_expr_position_parses() {
    let source = SourceFile::new("test".to_string(), r#"гыы пром = спиздить("./нет");"#.to_string());
    let (tokens, lex_diags) = Lexer::new(&source).tokenize();
    assert!(lex_diags.is_empty());
    let (_program, parse_diags) = Parser::new(&tokens, &source).parse_program();
    assert!(parse_diags.is_empty(), "должно парситься как выражение: {parse_diags:?}");
}

fn run_with_data_file(filename: &str, content: &str, main_src: &str) -> Interpreter {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let id = SEQ.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("yps_import_attr_{}_{id}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(filename), content).unwrap();

    let source = SourceFile::new("test".to_string(), main_src.to_string());
    let (tokens, lex_diags) = Lexer::new(&source).tokenize();
    assert!(lex_diags.is_empty(), "Ошибки лексера: {lex_diags:?}");
    let (program, parse_diags) = Parser::new(&tokens, &source).parse_program();
    assert!(parse_diags.is_empty(), "Ошибки парсера: {parse_diags:?}");
    let mut interp = Interpreter::new();
    interp.set_base_path(dir.clone());
    interp.run(&program).expect("Ошибка интерпретатора");
    let _ = std::fs::remove_dir_all(&dir);
    interp
}

#[test]
fn import_json_with_type_attribute() {
    let json = r#"{ "имя": "Вася", "возраст": 25, "хобби": ["а", "б"] }"#;
    let main = r#"
        спиздить data из "./d.json" with { type: "json" };
        гыы имя = data.имя;
        гыы возраст = data.возраст;
        гыы первое = data.хобби[0];
    "#;
    let interp = run_with_data_file("d.json", json, main);
    assert_eq!(interp.get("имя"), Some(Value::String("Вася".to_string())));
    assert_eq!(interp.get("возраст"), Some(Value::Number(25.0)));
    assert_eq!(interp.get("первое"), Some(Value::String("а".to_string())));
}

#[test]
fn import_attributes_russian_alias_satr() {
    let json = r#"{ "ключ": 7 }"#;
    let main = r#"
        спиздить д из "./a.json" сатр { type: "json" };
        гыы значение = д.ключ;
    "#;
    let interp = run_with_data_file("a.json", json, main);
    assert_eq!(interp.get("значение"), Some(Value::Number(7.0)));
}

#[test]
fn regex_literal_test_and_find() {
    let interp = run_code(
        r#"
        гыы шаблон = /\d+/;
        гыы есть = шаблон.проверить("номер 42");
        гыы найдено = шаблон.найти("abc 123 def");
        гыы первое = найдено["0"];
        гыы idx = найдено.index;
        "#,
    );
    assert_eq!(interp.get("есть"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("первое"), Some(Value::String("123".to_string())));
    assert_eq!(interp.get("idx"), Some(Value::Number(4.0)));
}

#[test]
fn regex_str_match_no_g() {
    let interp = run_code(
        r#"
        гыы r = "abc 123 def".совпадает(/\d+/);
        гыы m = r["0"];
        "#,
    );
    assert_eq!(interp.get("m"), Some(Value::String("123".to_string())));
}

#[test]
fn regex_str_match_no_match() {
    let interp = run_code(
        r#"
        гыы r = "abc".совпадает(/\d+/);
        "#,
    );
    assert_eq!(interp.get("r"), Some(Value::Null));
}

#[test]
fn regex_str_match_global() {
    let interp = run_code(
        r#"
        гыы r = "a1 b2 c3".совпадает(/\d/g);
        гыы a = r[0];
        гыы b = r[1];
        гыы c = r[2];
        "#,
    );
    assert_eq!(interp.get("a"), Some(Value::String("1".to_string())));
    assert_eq!(interp.get("b"), Some(Value::String("2".to_string())));
    assert_eq!(interp.get("c"), Some(Value::String("3".to_string())));
}

#[test]
fn regex_str_match_all() {
    let interp = run_code(
        r#"
        гыы first = "";
        гыы second_g1 = "";
        гыы i = 0;
        го (м сашаГрей "a1 b2".найтиВсе(/(\w)(\d)/g)) {
            вилкойвглаз (i == 0) { first = м["0"]; }
            вилкойвглаз (i == 1) { second_g1 = м["1"]; }
            i = i + 1;
        }
        "#,
    );
    assert_eq!(interp.get("first"), Some(Value::String("a1".to_string())));
    assert_eq!(interp.get("second_g1"), Some(Value::String("b".to_string())));
}

#[test]
fn regex_matchall_lazy_iterator() {
    let interp = run_code(
        r#"
        гыы out = "";
        го (м сашаГрей "a1 b2 c3".найтиВсе(/\d/g)) {
            out = out + м["0"];
        }
        "#,
    );
    assert_eq!(interp.get("out"), Some(Value::String("123".to_string())));
}

#[test]
fn regex_matchall_returns_iterator_type() {
    let interp = run_code(
        r#"
        гыы t = тип("x".найтиВсе(/x/g));
        "#,
    );
    assert_eq!(interp.get("t"), Some(Value::String("итератор".to_string())));
}

#[test]
fn regex_str_replace() {
    let interp = run_code(
        r#"
        гыы r = "hello world".заменить(/world/, "yopta");
        "#,
    );
    assert_eq!(interp.get("r"), Some(Value::String("hello yopta".to_string())));
}

#[test]
fn regex_str_replace_global() {
    let interp = run_code(
        r#"
        гыы r = "a-b-c".заменить(/-/g, "_");
        "#,
    );
    assert_eq!(interp.get("r"), Some(Value::String("a_b_c".to_string())));
}

#[test]
fn regex_str_replace_backref() {
    let interp = run_code(
        r#"
        гыы r = "John Smith".заменить(/(\w+) (\w+)/, "$2 $1");
        "#,
    );
    assert_eq!(interp.get("r"), Some(Value::String("Smith John".to_string())));
}

#[test]
fn regex_str_replace_dollar_escape() {
    let interp = run_code(
        r#"
        гыы r = "abc".заменить(/b/, "$$");
        "#,
    );
    assert_eq!(interp.get("r"), Some(Value::String("a$c".to_string())));
}

#[test]
fn regex_str_replace_named_backref() {
    let interp = run_code(
        r#"
        гыы r = "John Smith".заменить(/(?<first>\w+) (?<last>\w+)/, "$<last> $<first>");
        "#,
    );
    assert_eq!(interp.get("r"), Some(Value::String("Smith John".to_string())));
}

#[test]
fn regex_replace_with_fn() {
    let interp = run_code(
        r#"
        гыы r = "a1b2".заменить(/\d/g, (m) => число(m) * 10 + "");
        "#,
    );
    assert_eq!(interp.get("r"), Some(Value::String("a10b20".to_string())));
}

#[test]
fn regex_replace_with_fn_groups() {
    let interp = run_code(
        r#"
        гыы r = "foo bar".заменить(/(\w+) (\w+)/, (m, a, b) => b + " " + a);
        "#,
    );
    assert_eq!(interp.get("r"), Some(Value::String("bar foo".to_string())));
}

#[test]
fn regex_replace_with_fn_offset() {
    let interp = run_code(
        r#"
        гыы r = "abc".заменить(/./g, (m, off) => off + "");
        "#,
    );
    assert_eq!(interp.get("r"), Some(Value::String("012".to_string())));
}

#[test]
fn regex_replace_with_fn_no_g_only_first() {
    let interp = run_code(
        r#"
        гыы r = "a1b2c3".заменить(/\d/, (m) => "X");
        "#,
    );
    assert_eq!(interp.get("r"), Some(Value::String("aXb2c3".to_string())));
}

#[test]
fn regex_replace_all_with_fn() {
    let interp = run_code(
        r#"
        гыы r = "a1b2".заменитьВсе(/\d/g, (m) => число(m) + 1 + "");
        "#,
    );
    assert_eq!(interp.get("r"), Some(Value::String("a2b3".to_string())));
}

#[test]
fn regex_str_replace_multi_digit_backref() {
    let interp = run_code(
        r#"
        гыы a = "abc".заменить(/(a)(b)(c)/, "$3$2$1");
        гыы b = "XabcdefghijY".заменить(/(a)(b)(c)(d)(e)(f)(g)(h)(i)(j)/, "$10$9$8");
        гыы c = "aZ".заменить(/(a)/, "$10");
        "#,
    );
    assert_eq!(interp.get("a"), Some(Value::String("cba".to_string())));
    assert_eq!(interp.get("b"), Some(Value::String("XjihY".to_string())));
    assert_eq!(interp.get("c"), Some(Value::String("a0Z".to_string())));
}

#[test]
fn regex_str_split() {
    let interp = run_code(
        r#"
        гыы r = "a, b,  c,d".разбить(/,\s*/);
        гыы a = r[0];
        гыы b = r[1];
        гыы c = r[2];
        гыы d = r[3];
        "#,
    );
    assert_eq!(interp.get("a"), Some(Value::String("a".to_string())));
    assert_eq!(interp.get("b"), Some(Value::String("b".to_string())));
    assert_eq!(interp.get("c"), Some(Value::String("c".to_string())));
    assert_eq!(interp.get("d"), Some(Value::String("d".to_string())));
}

#[test]
fn regex_str_search() {
    let interp = run_code(
        r#"
        гыы a = "abc 123".найтиИндекс(/\d+/);
        гыы b = "abc".найтиИндекс(/\d+/);
        "#,
    );
    assert_eq!(interp.get("a"), Some(Value::Number(4.0)));
    assert_eq!(interp.get("b"), Some(Value::Number(-1.0)));
}

#[test]
fn regex_exec_object_shape() {
    let interp = run_code(
        r#"
        гыы r = /(\w+)/.найти("hello");
        гыы whole = r["0"];
        гыы g1 = r["1"];
        гыы idx = r.index;
        гыы inp = r.input;
        гыы grp = r.groups;
        "#,
    );
    assert_eq!(interp.get("whole"), Some(Value::String("hello".to_string())));
    assert_eq!(interp.get("g1"), Some(Value::String("hello".to_string())));
    assert_eq!(interp.get("idx"), Some(Value::Number(0.0)));
    assert_eq!(interp.get("inp"), Some(Value::String("hello".to_string())));
    assert_eq!(interp.get("grp"), Some(Value::Null));
}

#[test]
fn regex_exec_named_groups() {
    let interp = run_code(
        r#"
        гыы r = /(?<word>\w+)/.найти("hi");
        гыы w = r.groups.word;
        "#,
    );
    assert_eq!(interp.get("w"), Some(Value::String("hi".to_string())));
}

#[test]
fn regex_lastindex_global_advances() {
    let interp = run_code(
        r#"
        гыы re = /\d/g;
        гыы r1 = re.найти("a1b2");
        гыы li1 = re.последнийИндекс;
        гыы r2 = re.найти("a1b2");
        гыы li2 = re.последнийИндекс;
        гыы r3 = re.найти("a1b2");
        гыы li3 = re.последнийИндекс;
        "#,
    );
    assert_eq!(interp.get("li1"), Some(Value::Number(2.0)));
    assert_eq!(interp.get("li2"), Some(Value::Number(4.0)));
    assert_eq!(interp.get("li3"), Some(Value::Number(0.0)));
    assert_eq!(interp.get("r3"), Some(Value::Null));
}

#[test]
fn regex_new_regexp_with_flags() {
    let interp = run_code(
        r#"
        гыы re = RegExp("hello", "i");
        гыы ok = re.проверить("HELLO");
        "#,
    );
    assert_eq!(interp.get("ok"), Some(Value::Boolean(true)));
}

#[test]
fn regex_new_regexp_no_flags() {
    let interp = run_code(
        r#"
        гыы re = RegExp("abc");
        гыы ok = re.проверить("xabcx");
        гыы fl = re.флаги;
        "#,
    );
    assert_eq!(interp.get("ok"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("fl"), Some(Value::String(String::new())));
}

#[test]
fn regex_new_regexp_from_regex_with_flags() {
    let interp = run_code(
        r#"
        гыы base = /hello/;
        гыы re = RegExp(base, "i");
        гыы ok = re.проверить("HELLO");
        гыы fl = re.флаги;
        "#,
    );
    assert_eq!(interp.get("ok"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("fl"), Some(Value::String("i".to_string())));
}

#[test]
fn regex_new_regexp_invalid_pattern() {
    let err = run_code_err(
        r#"
        RegExp("(unclosed");
        "#,
    );
    assert!(err.message.contains("(unclosed"), "got: {}", err.message);
}

#[test]
fn regex_lookbehind_rejected() {
    let err = run_code_err(
        r#"
        RegExp("(?<=foo)bar");
        "#,
    );
    assert!(err.message.contains("lookbehind"), "got: {}", err.message);
}

#[test]
fn regex_backref_rejected() {
    let err = run_code_err(
        r#"
        RegExp("(a)\\1");
        "#,
    );
    assert!(err.message.contains("backreferences"), "got: {}", err.message);
}

#[test]
fn regex_sticky_match_at_position() {
    let interp = run_code(
        r#"
        гыы re = RegExp("\\d", "y");
        re.последнийИндекс = 1;
        гыы r = re.найти("a1b2");
        гыы matched = r["0"];
        гыы li = re.последнийИндекс;
        "#,
    );
    assert_eq!(interp.get("matched"), Some(Value::String("1".to_string())));
    assert_eq!(interp.get("li"), Some(Value::Number(2.0)));
}

#[test]
fn regex_sticky_mismatch_resets() {
    let interp = run_code(
        r#"
        гыы re = RegExp("\\d", "y");
        гыы r = re.найти("a1b2");
        гыы li = re.последнийИндекс;
        "#,
    );
    assert_eq!(interp.get("r"), Some(Value::Null));
    assert_eq!(interp.get("li"), Some(Value::Number(0.0)));
}

#[test]
fn regex_indices_in_exec() {
    let interp = run_code(
        r#"
        гыы re = RegExp("(\\d)(\\w)", "d");
        гыы r = re.найти("a1b");
        гыы pair0 = r.indices["0"];
        гыы pair1 = r.indices["1"];
        гыы pair2 = r.indices["2"];
        гыы s0 = pair0[0];
        гыы e0 = pair0[1];
        гыы s1 = pair1[0];
        гыы e1 = pair1[1];
        гыы s2 = pair2[0];
        гыы e2 = pair2[1];
        "#,
    );
    assert_eq!(interp.get("s0"), Some(Value::Number(1.0)));
    assert_eq!(interp.get("e0"), Some(Value::Number(3.0)));
    assert_eq!(interp.get("s1"), Some(Value::Number(1.0)));
    assert_eq!(interp.get("e1"), Some(Value::Number(2.0)));
    assert_eq!(interp.get("s2"), Some(Value::Number(2.0)));
    assert_eq!(interp.get("e2"), Some(Value::Number(3.0)));
}

#[test]
fn regex_indices_groups() {
    let interp = run_code(
        r#"
        гыы re = RegExp("(?<n>\\d+)", "d");
        гыы r = re.найти("a42b");
        гыы pair = r.indices.groups.n;
        гыы s = pair[0];
        гыы e = pair[1];
        "#,
    );
    assert_eq!(interp.get("s"), Some(Value::Number(1.0)));
    assert_eq!(interp.get("e"), Some(Value::Number(3.0)));
}

#[test]
fn regex_lastindex_property_read() {
    let interp = run_code(
        r#"
        гыы re = /\d+/g;
        re.найти("abc 123 def");
        гыы li = re.последнийИндекс;
        "#,
    );
    assert_eq!(interp.get("li"), Some(Value::Number(7.0)));
}

#[test]
fn regex_sticky_property_flag() {
    let interp = run_code(
        r#"
        гыы re = RegExp("a", "y");
        гыы s = re.sticky;
        гыы s2 = re.липкий;
        "#,
    );
    assert_eq!(interp.get("s"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("s2"), Some(Value::Boolean(true)));
}

#[test]
fn regex_hasindices_property_flag() {
    let interp = run_code(
        r#"
        гыы re = RegExp("a", "d");
        гыы h = re.hasIndices;
        гыы h2 = re.имеетИндексы;
        "#,
    );
    assert_eq!(interp.get("h"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("h2"), Some(Value::Boolean(true)));
}

#[test]
fn regex_lastindex_property_write() {
    let interp = run_code(
        r#"
        гыы re = /\d/g;
        re.последнийИндекс = 2;
        гыы r = re.найти("a1b2");
        гыы matched = r["0"];
        "#,
    );
    assert_eq!(interp.get("matched"), Some(Value::String("2".to_string())));
}

#[test]
fn regex_escape_safe_literal_paren() {
    let interp = run_code(
        r#"
        гыы re = RegExp("\\(\\?<x>foo\\)");
        гыы ok = re.проверить("(?<x>foo)");
        "#,
    );
    assert_eq!(interp.get("ok"), Some(Value::Boolean(true)));
}

#[test]
fn regex_escape_safe_char_class() {
    let interp = run_code(
        r#"
        гыы re = RegExp("[(?<x>]");
        гыы a = re.проверить("(");
        гыы b = re.проверить("?");
        гыы c = re.проверить("<");
        гыы d = re.проверить("x");
        гыы e = re.проверить(">");
        "#,
    );
    assert_eq!(interp.get("a"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("b"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("c"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("d"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("e"), Some(Value::Boolean(true)));
}

#[test]
fn regex_case_insensitive_flag() {
    let interp = run_code(
        r#"
        гыы p = /hello/i;
        гыы r = p.проверить("Hello World");
        "#,
    );
    assert_eq!(interp.get("r"), Some(Value::Boolean(true)));
}

#[test]
fn regex_source_and_flags_properties() {
    let interp = run_code(
        r#"
        гыы p = /foo/gi;
        гыы src = p.источник;
        гыы fl = p.флаги;
        "#,
    );
    assert_eq!(interp.get("src"), Some(Value::String("foo".to_string())));
    assert_eq!(interp.get("fl"), Some(Value::String("gi".to_string())));
}

#[test]
fn regex_division_disambiguation() {
    let interp = run_code(
        r#"
        гыы a = 10;
        гыы b = 2;
        гыы c = a / b / 1;
        "#,
    );
    assert_eq!(interp.get("c"), Some(Value::Number(5.0)));
}

#[test]
fn bigint_literal() {
    let interp = run_code(
        r#"
        гыы a = 123n;
        "#,
    );
    assert_eq!(interp.get("a"), Some(Value::BigInt(123)));
}

#[test]
fn bigint_arithmetic() {
    let interp = run_code(
        r#"
        гыы a = 1000000000000n;
        гыы b = 2n;
        гыы s = a + b;
        гыы d = a - b;
        гыы m = a * b;
        гыы q = a / b;
        гыы r = a % b;
        гыы p = b ** 10n;
        "#,
    );
    assert_eq!(interp.get("s"), Some(Value::BigInt(1_000_000_000_002)));
    assert_eq!(interp.get("d"), Some(Value::BigInt(999_999_999_998)));
    assert_eq!(interp.get("m"), Some(Value::BigInt(2_000_000_000_000)));
    assert_eq!(interp.get("q"), Some(Value::BigInt(500_000_000_000)));
    assert_eq!(interp.get("r"), Some(Value::BigInt(0)));
    assert_eq!(interp.get("p"), Some(Value::BigInt(1024)));
}

#[test]
fn bigint_unary_minus() {
    let interp = run_code(
        r#"
        гыы a = -7n;
        "#,
    );
    assert_eq!(interp.get("a"), Some(Value::BigInt(-7)));
}

#[test]
fn bigint_compare() {
    let interp = run_code(
        r#"
        гыы a = 5n < 7n;
        гыы b = 5n == 5n;
        гыы c = 5n > 7n;
        "#,
    );
    assert_eq!(interp.get("a"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("b"), Some(Value::Boolean(true)));
    assert_eq!(interp.get("c"), Some(Value::Boolean(false)));
}

#[test]
fn bigint_typeof() {
    let interp = run_code(
        r#"
        гыы t = чезажижан 9n;
        "#,
    );
    assert_eq!(interp.get("t"), Some(Value::String("бигцелое".to_string())));
}

#[test]
fn bigint_mixed_with_number_errors() {
    let err = run_code_err("гыы x = 1n + 2;");
    assert!(err.message.contains("Нельзя смешивать"));
}

#[test]
fn bigint_div_by_zero_errors() {
    let err = run_code_err("гыы x = 5n / 0n;");
    assert!(err.message.contains("ноль"));
}

#[test]
fn bigint_constructor_from_string() {
    let interp = run_code(
        r#"
        гыы a = БигЦелое("999");
        "#,
    );
    assert_eq!(interp.get("a"), Some(Value::BigInt(999)));
}

#[test]
fn bigint_constructor_from_number() {
    let interp = run_code(
        r#"
        гыы a = БигЦелое(42);
        "#,
    );
    assert_eq!(interp.get("a"), Some(Value::BigInt(42)));
}

#[test]
fn bigint_constructor_rejects_fractional() {
    let err = run_code_err(r#"гыы x = БигЦелое(1.5);"#);
    assert!(err.message.contains("целое"));
}

#[test]
fn proto_create_and_lookup() {
    let interp = run_code(
        r#"
        гыы родитель = { привет: "ку" };
        гыы потомок = Кент.создать(родитель);
        гыы рез = потомок.привет;
        "#,
    );
    assert_eq!(interp.get("рез"), Some(Value::String("ку".to_string())));
}

#[test]
fn proto_own_shadows_parent() {
    let interp = run_code(
        r#"
        гыы родитель = { x: 1 };
        гыы потомок = Кент.создать(родитель);
        потомок.x = 2;
        гыы a = потомок.x;
        гыы b = родитель.x;
        "#,
    );
    assert_eq!(interp.get("a"), Some(Value::Number(2.0)));
    assert_eq!(interp.get("b"), Some(Value::Number(1.0)));
}

#[test]
fn proto_get_proto_returns_null_when_none() {
    let interp = run_code(
        r#"
        гыы o = { a: 1 };
        гыы p = Кент.прототип(o);
        "#,
    );
    assert_eq!(interp.get("p"), Some(Value::Null));
}

#[test]
fn proto_set_proto_changes_lookup() {
    let interp = run_code(
        r#"
        гыы a = { ключ: "ааа" };
        гыы b = { ключ: "ббб" };
        гыы o = Кент.создать(a);
        o = Кент.назначитьПрототип(o, b);
        гыы r = o.ключ;
        "#,
    );
    assert_eq!(interp.get("r"), Some(Value::String("ббб".to_string())));
}

#[test]
fn proto_chain_two_levels() {
    let interp = run_code(
        r#"
        гыы дед = { имя: "дед" };
        гыы отец = Кент.создать(дед);
        гыы сын = Кент.создать(отец);
        гыы рез = сын.имя;
        "#,
    );
    assert_eq!(interp.get("рез"), Some(Value::String("дед".to_string())));
}

#[test]
fn class_prototype_has_methods() {
    let interp = run_code(
        r#"
        клёво К {
            привет() { отвечаю "хай"; }
        }
        гыы proto = К.прототип;
        гыы m = proto.привет;
        гыы t = чезажижан m;
        "#,
    );
    assert_eq!(interp.get("t"), Some(Value::String("функция".to_string())));
}

#[test]
fn instance_constructor_returns_class() {
    let interp = run_code(
        r#"
        клёво К {}
        гыы x = захуярить К();
        гыы c = x.конструктор;
        гыы тот = c == К;
        "#,
    );
    assert_eq!(interp.get("тот"), Some(Value::Boolean(true)));
}

#[test]
fn чутка_fires_in_deadline_order() {
    let interp = run_code(
        r#"
        гыы лог = [];
        чутка(() => { лог = втолкнуть(лог, "A"); }, 30);
        чутка(() => { лог = втолкнуть(лог, "B"); }, 5);
        "#,
    );
    let log = interp.get("лог").unwrap();
    assert_eq!(log, Value::Array(vec![Value::String("B".into()), Value::String("A".into())]));
}

#[test]
#[allow(non_snake_case)]
fn отменаЧутки_prevents_callback() {
    let interp = run_code(
        r#"
        гыы лог = [];
        гыы ид = чутка(() => { лог = втолкнуть(лог, "X"); }, 5);
        отменаЧутки(ид);
        "#,
    );
    assert_eq!(interp.get("лог"), Some(Value::Array(vec![])));
}

#[test]
fn интервал_fires_multiple_times_then_cancels() {
    let interp = run_code(
        r#"
        гыы счёт = 0;
        гыы ид = ноль;
        ид = интервал(() => {
            счёт = счёт + 1;
            вилкойвглаз (счёт >= 3) {
                отменаИнтервала(ид);
            }
        }, 1);
        "#,
    );
    assert_eq!(interp.get("счёт"), Some(Value::Number(3.0)));
}

#[test]
fn сразу_runs_before_macrotask() {
    let interp = run_code(
        r#"
        гыы лог = [];
        чутка(() => { лог = втолкнуть(лог, "макро"); }, 0);
        сразу(() => { лог = втолкнуть(лог, "микро"); });
        "#,
    );
    assert_eq!(
        interp.get("лог"),
        Some(Value::Array(vec![Value::String("микро".into()), Value::String("макро".into())]))
    );
}

#[test]
#[allow(non_snake_case)]
fn наСледующемТике_has_priority_over_сразу() {
    let interp = run_code(
        r#"
        гыы лог = [];
        чутка(() => {
            сразу(() => { лог = втолкнуть(лог, "обычная"); });
            наСледующемТике(() => { лог = втолкнуть(лог, "приоритет"); });
        }, 0);
        "#,
    );
    assert_eq!(
        interp.get("лог"),
        Some(Value::Array(vec![Value::String("приоритет".into()), Value::String("обычная".into())]))
    );
}

#[test]
fn await_parks_on_pending_promise_resolved_by_chutka() {
    let interp = run_code(
        r#"
        ассо йопта получить() {
            отвечаю захуярить СловоПацана((решить, _) => {
                чутка(() => решить(42), 5);
            });
        }
        ассо йопта главное() {
            гыы х = сидетьНахуй получить();
            отвечаю х;
        }
        гыы p = главное();
        гыы итог = ноль;
        p.потом((v) => { итог = v; });
        "#,
    );
    assert_eq!(interp.get("итог"), Some(Value::Number(42.0)));
}

#[test]
fn top_level_await_drives_loop() {
    let interp = run_code(
        r#"
        гыы р = захуярить СловоПацана((решить, _) => {
            чутка(() => решить("готово"), 5);
        });
        гыы значение = сидетьНахуй р;
        "#,
    );
    assert_eq!(interp.get("значение"), Some(Value::String("готово".into())));
}

#[test]
fn await_chain_across_delays() {
    let interp = run_code(
        r#"
        йопта задержанный(значение, мс) {
            отвечаю захуярить СловоПацана((решить, _) => {
                чутка(() => решить(значение), мс);
            });
        }
        ассо йопта суммаЧерезЗадержки() {
            гыы а = сидетьНахуй задержанный(10, 5);
            гыы б = сидетьНахуй задержанный(20, 5);
            гыы в = сидетьНахуй задержанный(30, 5);
            отвечаю а + б + в;
        }
        гыы итог = ноль;
        суммаЧерезЗадержки().потом((v) => { итог = v; });
        "#,
    );
    assert_eq!(interp.get("итог"), Some(Value::Number(60.0)));
}

#[test]
fn await_rejected_promise_caught_by_try_catch() {
    let interp = run_code(
        r#"
        ассо йопта плохо() {
            гыы p = захуярить СловоПацана((_, отвергнуть) => {
                чутка(() => отвергнуть("боль"), 5);
            });
            сидетьНахуй p;
        }
        ассо йопта главное() {
            гыы пойман = ноль;
            хапнуть {
                сидетьНахуй плохо();
            } гоп (e) {
                пойман = e;
            }
            отвечаю пойман;
        }
        гыы итог = ноль;
        главное().потом((v) => { итог = v; });
        "#,
    );
    assert_eq!(interp.get("итог"), Some(Value::String("боль".into())));
}

#[test]
fn timer_callback_throw_does_not_kill_loop() {
    let interp = run_code(
        r#"
        гыы лог = [];
        чутка(() => { кидай "плохой коллбэк"; }, 5);
        чутка(() => { лог = втолкнуть(лог, "выжил"); }, 10);
        "#,
    );
    assert_eq!(interp.get("лог"), Some(Value::Array(vec![Value::String("выжил".into())])));
}

#[test]
fn interval_continues_after_throwing_tick() {
    let interp = run_code(
        r#"
        гыы счёт = 0;
        гыы ид = ноль;
        ид = интервал(() => {
            счёт = счёт + 1;
            вилкойвглаз (счёт == 1) {
                кидай "сбой";
            }
            вилкойвглаз (счёт >= 3) {
                отменаИнтервала(ид);
            }
        }, 1);
        "#,
    );
    assert_eq!(interp.get("счёт"), Some(Value::Number(3.0)));
}

#[test]
fn await_on_non_promise_returns_value() {
    let interp = run_code(
        r#"
        ассо йопта f() {
            отвечаю сидетьНахуй 42;
        }
        гыы итог = ноль;
        f().потом((v) => { итог = v; });
        "#,
    );
    assert_eq!(interp.get("итог"), Some(Value::Number(42.0)));
}

#[test]
fn async_fn_returns_pending_immediately() {
    let interp = run_code(
        r#"
        гыы маркер = "before";
        ассо йопта работа() {
            маркер = "inside";
            отвечаю 1;
        }
        гыы пара = [работа(), маркер];
        "#,
    );
    let pair = interp.get("пара").unwrap();
    let Value::Array(items) = pair else { panic!("expected array") };
    assert!(matches!(items[0], Value::Promise { .. }), "first element must be a Promise");
    assert_eq!(items[1], Value::String("before".into()));
    assert_eq!(interp.get("маркер"), Some(Value::String("inside".into())));
}

#[test]
fn async_fn_returning_pending_promise_adopts() {
    let interp = run_code(
        r#"
        ассо йопта внутри() {
            отвечаю захуярить СловоПацана((решить, _) => {
                чутка(() => решить(99), 5);
            });
        }
        ассо йопта снаружи() {
            отвечаю внутри();
        }
        гыы итог = ноль;
        снаружи().потом((v) => { итог = v; });
        "#,
    );
    assert_eq!(interp.get("итог"), Some(Value::Number(99.0)));
}

#[test]
fn non_callable_to_chutka_errors_with_call_site() {
    let err = run_code_err(r#"чутка(42, 10);"#);
    assert!(err.message.contains("'чутка'"), "got: {}", err.message);
    assert!(err.message.contains("функцию"), "got: {}", err.message);
}

#[test]
fn non_callable_to_srazu_errors() {
    let err = run_code_err(r#"сразу("не функция");"#);
    assert!(err.message.contains("'сразу'"), "got: {}", err.message);
}

#[test]
fn await_depth_limit_caught() {
    let interp = run_code(
        r#"
        ассо йопта глубина(н) {
            вилкойвглаз (н <= 0) {
                отвечаю 0;
            }
            гыы p = захуярить СловоПацана((решить, _) => {
                чутка(() => решить(н), 0);
            });
            гыы значение = сидетьНахуй p;
            гыы дальше = сидетьНахуй глубина(н - 1);
            отвечаю значение + дальше;
        }
        гыы пойман = ноль;
        ассо йопта запуск() {
            хапнуть {
                сидетьНахуй глубина(30);
            } гоп (e) {
                пойман = e.message;
            }
        }
        запуск();
        "#,
    );
    let caught = interp.get("пойман").unwrap();
    if let Value::String(s) = caught {
        assert!(s.contains("глубин"), "expected depth-limit message, got: {s}");
    } else {
        panic!("expected error string, got {caught:?}");
    }
}

#[test]
fn nested_intervals_independent_cancellation() {
    let interp = run_code(
        r#"
        гыы лог = [];
        гыы внешнийИд = ноль;
        гыы внутреннийИд = ноль;
        гыы внешнийСчёт = 0;
        внешнийИд = интервал(() => {
            внешнийСчёт = внешнийСчёт + 1;
            лог = втолкнуть(лог, "внешний");
            вилкойвглаз (внешнийСчёт == 1) {
                гыы внутреннийСчёт = 0;
                внутреннийИд = интервал(() => {
                    внутреннийСчёт = внутреннийСчёт + 1;
                    лог = втолкнуть(лог, "внутренний");
                    вилкойвглаз (внутреннийСчёт >= 2) {
                        отменаИнтервала(внутреннийИд);
                    }
                }, 1);
            }
            вилкойвглаз (внешнийСчёт >= 3) {
                отменаИнтервала(внешнийИд);
            }
        }, 5);
        "#,
    );
    let log = interp.get("лог").unwrap();
    let Value::Array(items) = log else { panic!("expected array") };
    let labels: Vec<&str> = items.iter().map(|v| if let Value::String(s) = v { s.as_str() } else { "?" }).collect();
    assert_eq!(labels.iter().filter(|l| **l == "внешний").count(), 3);
    assert_eq!(labels.iter().filter(|l| **l == "внутренний").count(), 2);
}

#[test]
fn interval_cancelled_mid_callback_fires_exactly_once() {
    let interp = run_code(
        r#"
        гыы счёт = 0;
        гыы ид = ноль;
        ид = интервал(() => {
            счёт = счёт + 1;
            отменаИнтервала(ид);
        }, 1);
        "#,
    );
    assert_eq!(interp.get("счёт"), Some(Value::Number(1.0)));
}

#[test]
fn rejected_promise_then_catch_path() {
    let interp = run_code(
        r#"
        гыы пойман = ноль;
        захуярить СловоПацана((_, отвергнуть) => {
            чутка(() => отвергнуть("ошибка"), 5);
        }).ловить((e) => { пойман = e; });
        "#,
    );
    assert_eq!(interp.get("пойман"), Some(Value::String("ошибка".into())));
}

#[test]
fn promise_all_resolves_after_async_delays() {
    let interp = run_code(
        r#"
        йопта задержка(значение, мс) {
            отвечаю захуярить СловоПацана((решить, _) => {
                чутка(() => решить(значение), мс);
            });
        }
        гыы итог = ноль;
        СловоПацана.всех([задержка(1, 10), задержка(2, 5), задержка(3, 15)])
            .потом((v) => { итог = v; });
        "#,
    );
    assert_eq!(
        interp.get("итог"),
        Some(Value::Array(vec![Value::Number(1.0), Value::Number(2.0), Value::Number(3.0)]))
    );
}

#[test]
fn promise_all_rejects_on_first_failure() {
    let interp = run_code(
        r#"
        гыы пойман = ноль;
        СловоПацана.всех([
            захуярить СловоПацана((решить, _) => { чутка(() => решить(1), 20); }),
            захуярить СловоПацана((_, отвергнуть) => { чутка(() => отвергнуть("плохо"), 5); })
        ]).ловить((e) => { пойман = e; });
        "#,
    );
    assert_eq!(interp.get("пойман"), Some(Value::String("плохо".into())));
}

#[test]
fn promise_race_takes_first_settled() {
    let interp = run_code(
        r#"
        гыы итог = ноль;
        СловоПацана.гонка([
            захуярить СловоПацана((решить, _) => { чутка(() => решить("медленно"), 20); }),
            захуярить СловоПацана((решить, _) => { чутка(() => решить("быстро"), 5); })
        ]).потом((v) => { итог = v; });
        "#,
    );
    assert_eq!(interp.get("итог"), Some(Value::String("быстро".into())));
}

#[test]
fn promise_any_skips_rejections_returns_first_success() {
    let interp = run_code(
        r#"
        гыы итог = ноль;
        СловоПацана.любой([
            захуярить СловоПацана((_, отвергнуть) => { чутка(() => отвергнуть("a"), 5); }),
            захуярить СловоПацана((решить, _) => { чутка(() => решить("ок"), 10); }),
            захуярить СловоПацана((_, отвергнуть) => { чутка(() => отвергнуть("b"), 15); })
        ]).потом((v) => { итог = v; });
        "#,
    );
    assert_eq!(interp.get("итог"), Some(Value::String("ок".into())));
}

#[test]
fn promise_any_all_rejected_emits_aggregate_error() {
    let interp = run_code(
        r#"
        гыы имя = ноль;
        гыы ошибки = ноль;
        СловоПацана.любой([
            захуярить СловоПацана((_, отвергнуть) => { чутка(() => отвергнуть("a"), 5); }),
            захуярить СловоПацана((_, отвергнуть) => { чутка(() => отвергнуть("b"), 10); })
        ]).ловить((e) => { имя = e.name; ошибки = e.errors; });
        "#,
    );
    assert_eq!(interp.get("имя"), Some(Value::String("ВсёОбосралось".into())));
    assert_eq!(interp.get("ошибки"), Some(Value::Array(vec![Value::String("a".into()), Value::String("b".into())])));
}

#[test]
fn promise_all_settled_collects_all_outcomes() {
    let interp = run_code(
        r#"
        гыы статусы = ноль;
        гыы первое = ноль;
        гыы причина = ноль;
        гыы третье = ноль;
        СловоПацана.всехУстаканить([
            захуярить СловоПацана((решить, _) => { чутка(() => решить(1), 5); }),
            захуярить СловоПацана((_, отвергнуть) => { чутка(() => отвергнуть("плохо"), 10); }),
            захуярить СловоПацана((решить, _) => { чутка(() => решить(3), 15); })
        ]).потом((v) => {
            статусы = [v[0].статус, v[1].статус, v[2].статус];
            первое = v[0].значение;
            причина = v[1].причина;
            третье = v[2].значение;
        });
        "#,
    );
    assert_eq!(
        interp.get("статусы"),
        Some(Value::Array(vec![
            Value::String("выполнено".into()),
            Value::String("отклонено".into()),
            Value::String("выполнено".into()),
        ]))
    );
    assert_eq!(interp.get("первое"), Some(Value::Number(1.0)));
    assert_eq!(interp.get("причина"), Some(Value::String("плохо".into())));
    assert_eq!(interp.get("третье"), Some(Value::Number(3.0)));
}

#[test]
fn promise_all_empty_array_resolves_to_empty() {
    let interp = run_code(
        r#"
        гыы итог = ноль;
        СловоПацана.всех([]).потом((v) => { итог = v; });
        "#,
    );
    assert_eq!(interp.get("итог"), Some(Value::Array(Vec::new())));
}

#[test]
fn promise_any_empty_array_rejects_with_aggregate() {
    let interp = run_code(
        r#"
        гыы имя = ноль;
        СловоПацана.любой([]).ловить((e) => { имя = e.name; });
        "#,
    );
    assert_eq!(interp.get("имя"), Some(Value::String("ВсёОбосралось".into())));
}

#[test]
fn promise_race_with_rejection_first_propagates() {
    let interp = run_code(
        r#"
        гыы пойман = ноль;
        СловоПацана.гонка([
            захуярить СловоПацана((решить, _) => { чутка(() => решить("поздно"), 20); }),
            захуярить СловоПацана((_, отвергнуть) => { чутка(() => отвергнуть("рано"), 5); })
        ]).ловить((e) => { пойман = e; });
        "#,
    );
    assert_eq!(interp.get("пойман"), Some(Value::String("рано".into())));
}

#[test]
fn test_await_podozhdat_blocks_min_50ms() {
    let start = std::time::Instant::now();
    let interp = run_code(
        r#"
        гыы готово = лож;
        ассо йопта главное() {
            сидетьНахуй подождать(50);
            готово = правда;
        }
        главное();
        "#,
    );
    let elapsed = start.elapsed();
    assert_eq!(interp.get("готово"), Some(Value::Boolean(true)));
    assert!(elapsed.as_millis() >= 40, "ожидалось >=40мс, было {}мс", elapsed.as_millis());
    assert!(elapsed.as_millis() < 500, "ожидалось <500мс, было {}мс", elapsed.as_millis());
}

#[test]
fn test_await_podozhdat_aborts_via_signal() {
    let interp = run_code(
        r#"
        гыы пойман = ноль;
        ассо йопта главное() {
            гыы к = захуярить КонтроллёрОтмены();
            чутка(() => к.отменить({ name: "ОшибкаОтмены", message: "сигнал" }), 5);
            хапнуть {
                сидетьНахуй подождать(500, { сигнал: к.сигнал });
            } гоп (e) {
                пойман = e.message;
            }
        }
        главное();
        "#,
    );
    assert_eq!(interp.get("пойман"), Some(Value::String("сигнал".into())));
}

#[test]
fn test_sochereit_runs_before_macrotask() {
    let interp = run_code(
        r#"
        гыы лог = [];
        чутка(() => { лог = втолкнуть(лог, "макро"); }, 0);
        сОчередить(() => { лог = втолкнуть(лог, "микро"); });
        "#,
    );
    assert_eq!(
        interp.get("лог"),
        Some(Value::Array(vec![Value::String("микро".into()), Value::String("макро".into())]))
    );
}

#[test]
fn test_promise_race_picks_shortest_timer() {
    let interp = run_code(
        r#"
        гыы итог = ноль;
        ассо йопта главное() {
            итог = сидетьНахуй СловоПацана.гонка([подождать(50), подождать(5), подождать(100)]);
        }
        главное();
        "#,
    );
    assert_eq!(interp.get("итог"), Some(Value::Undefined));
}

#[test]
fn test_abort_signal_ot_vremeni_rejects() {
    let interp = run_code(
        r#"
        гыы имя = ноль;
        гыы сообщ = ноль;
        ассо йопта главное() {
            гыы с = СигналОтмены.отВремени(10);
            хапнуть {
                сидетьНахуй с.обещание;
            } гоп (e) {
                имя = e.name;
                сообщ = e.message;
            }
        }
        главное();
        "#,
    );
    assert_eq!(interp.get("имя"), Some(Value::String("ОшибкаОтмены".into())));
    assert_eq!(interp.get("сообщ"), Some(Value::String("Тайм-аут".into())));
}

#[test]
fn test_signal_promise_cached_no_listener_leak() {
    let interp = run_code(
        r#"
        гыы к = захуярить КонтроллёрОтмены();
        гыы сиг = к.сигнал;
        гыы и = 0;
        потрещим (и < 100) {
            гыы _ = сиг.обещание;
            и = и + 1;
        }
        "#,
    );
    let sig = interp.get("сиг").expect("сиг должен быть определён");
    let state = match sig {
        Value::AbortSignal { state } => state,
        other => panic!("ожидался AbortSignal, получено {other:?}"),
    };
    let count = state.borrow().listeners.len();
    assert!(count <= 2, "ожидалось <=2 слушателей, было {count}");
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
    assert_eq!(i.get("зн"), Some(Value::String("упс".to_string())));
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
    assert_eq!(err.thrown, Some(Value::String("бах".to_string())));
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
    assert_eq!(err.thrown, Some(Value::String("после конца".to_string())));
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
