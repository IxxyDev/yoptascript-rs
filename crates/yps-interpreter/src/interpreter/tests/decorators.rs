use super::*;

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
            let map = map.borrow();
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
            let strs: Vec<String> = items.borrow().iter().map(|v| v.to_string()).collect();
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
            let strs: Vec<String> = items.borrow().iter().map(|v| v.to_string()).collect();
            assert_eq!(strs, vec!["второй", "первый"]);
        }
        _ => panic!("Expected Array"),
    }
}
