use super::*;

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
fn static_method_inherited_by_subclass() {
    let i = run_code(
        r#"
        клёво А {
            попонятия привет() { отвечаю "А-привет"; }
            попонятия счёт = 10;
        }
        клёво Б батя А {}
        гыы рез = Б.привет();
        гыы поле = Б.счёт;
        "#,
    );
    assert_eq!(i.get("рез"), Some(Value::String("А-привет".to_string())));
    assert_eq!(i.get("поле"), Some(Value::Number(10.0)));
}

#[test]
fn super_method_call_in_instance_method() {
    let i = run_code(
        r#"
        клёво Зверь {
            голос() { отвечаю "..."; }
        }
        клёво Пёс батя Зверь {
            голос() { отвечаю яга.голос() + "гав"; }
        }
        ясенХуй п = захуярить Пёс();
        гыы рез = п.голос();
        "#,
    );
    assert_eq!(i.get("рез"), Some(Value::String("...гав".to_string())));
}

#[test]
fn super_constructor_three_level_chain() {
    let i = run_code(
        r#"
        клёво А { А() { тырыпыры.а = "a"; } }
        клёво Б батя А { Б() { яга(); тырыпыры.б = "b"; } }
        клёво В батя Б { В() { яга(); тырыпыры.в = "c"; } }
        ясенХуй в = захуярить В();
        гыы поле_а = в.а;
        гыы поле_б = в.б;
        гыы поле_в = в.в;
        "#,
    );
    assert_eq!(i.get("поле_а"), Some(Value::String("a".to_string())));
    assert_eq!(i.get("поле_б"), Some(Value::String("b".to_string())));
    assert_eq!(i.get("поле_в"), Some(Value::String("c".to_string())));
}

#[test]
fn static_this_bound_to_class() {
    let i = run_code(
        r#"
        клёво Мат {
            попонятия ПИ = 3.14;
            попонятия описание() { отвечаю "Мат(" + тырыпыры.ПИ + ")"; }
        }
        гыы рез = Мат.описание();
        "#,
    );
    assert_eq!(i.get("рез"), Some(Value::String("Мат(3.14)".to_string())));
}

#[test]
fn arrow_field_captures_live_instance() {
    let i = run_code(
        r#"
        клёво Т {
            Т() { тырыпыры.х = 10; }
            стрела = () => тырыпыры.х;
        }
        ясенХуй т = захуярить Т();
        ясенХуй стр = т.стрела;
        гыы рез = стр();
        "#,
    );
    assert_eq!(i.get("рез"), Some(Value::Number(10.0)));
}

#[test]
fn static_field_mutation_via_this() {
    let i = run_code(
        r#"
        клёво Счёт {
            попонятия число = 0;
            попонятия тик() {
                тырыпыры.число = тырыпыры.число + 1;
            }
        }
        Счёт.тик();
        Счёт.тик();
        Счёт.тик();
        гыы рез = Счёт.число;
        "#,
    );
    assert_eq!(i.get("рез"), Some(Value::Number(3.0)));
}

#[test]
fn static_field_mutation_via_class_name() {
    let i = run_code(
        r#"
        клёво Счёт {
            попонятия число = 5;
        }
        Счёт.число = Счёт.число + 10;
        гыы рез = Счёт.число;
        "#,
    );
    assert_eq!(i.get("рез"), Some(Value::Number(15.0)));
}

#[test]
fn object_freeze_blocks_writes_and_deletes() {
    let i = run_code(
        r#"
        гыы о = { х: 1 };
        Кент.заморозить(о);
        о.х = 2;
        о.у = 3;
        гыы х = о.х;
        гыы у = о.у;
        гыы заморожен = Кент.заморожен(о);
        "#,
    );
    assert_eq!(i.get("х"), Some(Value::Number(1.0)));
    assert_eq!(i.get("у"), Some(Value::Undefined));
    assert_eq!(i.get("заморожен"), Some(Value::Boolean(true)));
}

#[test]
fn object_not_frozen_by_default() {
    let i = run_code(
        r#"
        гыы о = { х: 1 };
        гыы заморожен = Кент.заморожен(о);
        о.х = 7;
        гыы х = о.х;
        "#,
    );
    assert_eq!(i.get("заморожен"), Some(Value::Boolean(false)));
    assert_eq!(i.get("х"), Some(Value::Number(7.0)));
}
