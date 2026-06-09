use super::*;

#[test]
fn test_iterator_from_array_to_array() {
    let interp = run_code(
        r#"
        гыы и = Итератор.от([1, 2, 3]);
        гыы рез = и.вМассив();
        "#,
    );
    assert_struct_eq(interp.get("рез"), Value::array(vec![Value::Number(1.0), Value::Number(2.0), Value::Number(3.0)]));
}

#[test]
fn test_iterator_map_lazy() {
    let interp = run_code(
        r#"
        гыы рез = Итератор.от([1, 2, 3]).преобразовать((х) => х * 10).вМассив();
        "#,
    );
    assert_struct_eq(
        interp.get("рез"),
        Value::array(vec![Value::Number(10.0), Value::Number(20.0), Value::Number(30.0)]),
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
    assert_struct_eq(interp.get("рез"), Value::array(vec![Value::Number(4.0), Value::Number(6.0)]));
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
    assert_struct_eq(
        interp.get("рез"),
        Value::array(vec![
            Value::Number(1.0),
            Value::Number(2.0),
            Value::Number(3.0),
            Value::Number(4.0),
            Value::Number(5.0),
        ]),
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
        let m = m.borrow();
        assert_eq!(m.get("значение"), Some(&Value::Number(7.0)));
        assert_eq!(m.get("готово"), Some(&Value::Boolean(false)));
    } else {
        panic!("expected Object");
    }
    if let Value::Object(m) = b {
        let m = m.borrow();
        assert_eq!(m.get("значение"), Some(&Value::Number(8.0)));
        assert_eq!(m.get("готово"), Some(&Value::Boolean(false)));
    } else {
        panic!("expected Object");
    }
    if let Value::Object(m) = c {
        let m = m.borrow();
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
    assert_struct_eq(
        interp.get("рез"),
        Value::array(vec![
            Value::String("a".to_string()),
            Value::String("b".to_string()),
            Value::String("c".to_string()),
        ]),
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
    assert_struct_eq(
        interp.get("рез"),
        Value::array(vec![
            Value::Number(0.0),
            Value::Number(2.0),
            Value::Number(3.0),
            Value::Number(4.0),
            Value::Number(99.0),
        ]),
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
    assert_struct_eq(interp.get("итог"), Value::array(vec![Value::Number(1.0), Value::Number(3.0)]));
}
