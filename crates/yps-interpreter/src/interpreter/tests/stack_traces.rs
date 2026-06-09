use super::*;

fn stack_names(err: &RuntimeError) -> Vec<String> {
    err.stack.iter().map(|f| f.name.to_string()).collect()
}

#[test]
fn stack_nested_calls_bottom_up() {
    let err = run_code_err(
        r#"
        йопта в() { кидай "бум"; }
        йопта б() { отвечаю в(); }
        йопта а() { отвечаю б(); }
        а();
        "#,
    );
    assert_eq!(stack_names(&err), vec!["в", "б", "а"]);
}

#[test]
fn stack_clean_after_try_catch() {
    let err = run_code_err(
        r#"
        йопта плохо() { кидай "ой"; }
        хапнуть {
            плохо();
        } гоп (e) {}
        йопта д() { кидай "снова"; }
        д();
        "#,
    );
    assert_eq!(stack_names(&err), vec!["д"]);
}

#[test]
fn stack_throw_carries_stack() {
    let err = run_code_err(
        r#"
        йопта внутр() { кидай "тут"; }
        йопта внешн() { отвечаю внутр(); }
        внешн();
        "#,
    );
    assert!(!err.stack.is_empty());
    assert!(stack_names(&err).contains(&"внутр".to_string()));
}

#[test]
fn stack_user_callback_frame_present() {
    let err = run_code_err(
        r#"
        йопта взорви(х) { кидай "колбэк"; }
        гыы а = [1, 2, 3];
        а.map(взорви);
        "#,
    );
    let names = stack_names(&err);
    assert!(names.contains(&"взорви".to_string()));
    assert!(!names.iter().any(|n| n.contains("map")));
}

#[test]
fn stack_async_function_frame() {
    let err = run_code_err(
        r#"
        ассо йопта работа() {
            нетакойвызов();
        }
        работа();
        "#,
    );
    let names = stack_names(&err);
    assert_eq!(names, vec!["работа".to_string()]);
}

#[test]
fn stack_display_unchanged() {
    let err = run_code_err(
        r#"
        йопта в() { кидай "бум"; }
        в();
        "#,
    );
    let text = format!("{err}");
    assert!(text.starts_with("Ошибка:"));
    assert!(!text.contains("  в "));
}

#[test]
fn stack_constructor_single_frame() {
    let err = run_code_err(
        r#"
        клёво Бомба {
            Бомба() {
                кидай "взрыв";
            }
        }
        захуярить Бомба();
        "#,
    );
    let names = stack_names(&err);
    assert_eq!(names.iter().filter(|n| n.as_str() == "Бомба").count(), 1);
    assert_eq!(names, vec!["Бомба".to_string()]);
}

#[test]
fn stack_generator_frame() {
    let err = run_code_err(
        r#"
        пиздюли ген() {
            поебалу 1;
            кидай "генбум";
        }
        гыы сумма = 0;
        го (гыы х сашаГрей ген()) {
            сумма += х;
        }
        "#,
    );
    let names = stack_names(&err);
    assert_eq!(names, vec!["ген".to_string()]);
}

#[test]
fn stack_capped_at_max_depth() {
    let handle = std::thread::Builder::new()
        .stack_size(256 * 1024 * 1024)
        .spawn(|| {
            let err = run_code_err(
                r#"
                йопта рек(н) {
                    вилкойвглаз (н <= 0) { кидай "дно"; }
                    отвечаю рек(н - 1);
                }
                рек(80);
                "#,
            );
            err.stack.len()
        })
        .unwrap();
    let depth = handle.join().unwrap();
    assert!(depth <= crate::error::MAX_STACK_DEPTH);
    assert_eq!(depth, crate::error::MAX_STACK_DEPTH);
}

#[test]
fn stack_method_and_setter_frames() {
    let method_err = run_code_err(
        r#"
        клёво Объ {
            метод() { кидай "метод"; }
        }
        гыы о = захуярить Объ();
        о.метод();
        "#,
    );
    assert!(stack_names(&method_err).contains(&"метод".to_string()));

    let setter_err = run_code_err(
        r#"
        клёво Кор {
            set значение(в) { кидай "сеттер"; }
        }
        гыы к = захуярить Кор();
        к.значение = 5;
        "#,
    );
    assert!(stack_names(&setter_err).contains(&"значение".to_string()));
}

#[test]
fn stack_balanced_after_nested_try_catch() {
    let err = run_code_err(
        r#"
        клёво Кл {
            метод() { кидай "из метода"; }
            Кл() { кидай "из конструктора"; }
        }
        хапнуть {
            гыы о = захуярить Кл();
        } гоп (e) {}
        хапнуть {
            клёво Безопасный {
                метод() { кидай "м"; }
            }
            гыы с = захуярить Безопасный();
            с.метод();
        } гоп (e) {}
        йопта д() { кидай "финал"; }
        д();
        "#,
    );
    assert_eq!(stack_names(&err), vec!["д"]);
}
