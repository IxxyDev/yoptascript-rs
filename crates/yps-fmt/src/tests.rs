#[cfg(test)]
mod suite {
    use crate::{FormatError, format_source};
    use yps_lexer::{Lexer, SourceFile};
    use yps_parser::Parser;

    fn parse_and_format(src: &str) -> String {
        format_source(src).unwrap_or_else(|e| panic!("format_source failed: {e}")).text
    }

    fn programs_equivalent_str(a: &str, b: &str) -> bool {
        let sf_a = SourceFile::new("<a>".to_string(), a.to_string());
        let (tok_a, _, _) = Lexer::new(&sf_a).tokenize_with_trivia();
        let (prog_a, diags_a) = Parser::new(&tok_a, &sf_a).parse_program();
        if !diags_a.is_empty() {
            return false;
        }

        let sf_b = SourceFile::new("<b>".to_string(), b.to_string());
        let (tok_b, _, _) = Lexer::new(&sf_b).tokenize_with_trivia();
        let (prog_b, diags_b) = Parser::new(&tok_b, &sf_b).parse_program();
        if !diags_b.is_empty() {
            return false;
        }

        crate::normalize::programs_equivalent(&prog_a, &prog_b)
    }

    fn example_path(name: &str) -> std::path::PathBuf {
        let mut p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        p.pop();
        p.pop();
        p.push("examples");
        p.push(name);
        p
    }

    fn read_example(name: &str) -> String {
        std::fs::read_to_string(example_path(name)).unwrap_or_else(|e| panic!("не удалось прочитать {name}: {e}"))
    }

    // === (a) round-trip на 20 examples ===

    fn assert_round_trip(name: &str) {
        let src = read_example(name);
        let formatted = parse_and_format(&src);
        assert!(
            programs_equivalent_str(&src, &formatted),
            "round-trip не прошёл для {name}: parse(fmt(src)) ≢ parse(src)"
        );
    }

    #[test]
    fn round_trip_abort() {
        assert_round_trip("abort.yopta");
    }

    #[test]
    fn round_trip_async_timers() {
        assert_round_trip("async_timers.yopta");
    }

    #[test]
    fn round_trip_date() {
        assert_round_trip("date.yopta");
    }

    #[test]
    fn round_trip_decorators() {
        assert_round_trip("decorators.yopta");
    }

    #[test]
    fn round_trip_destructuring_defaults() {
        assert_round_trip("destructuring_defaults.yopta");
    }

    #[test]
    fn round_trip_dynamic_import() {
        assert_round_trip("dynamic_import.yopta");
    }

    #[test]
    fn round_trip_event_loop() {
        assert_round_trip("event_loop.yopta");
    }

    #[test]
    fn round_trip_for_await_of() {
        assert_round_trip("for_await_of.yopta");
    }

    #[test]
    fn round_trip_hello() {
        assert_round_trip("hello.yopta");
    }

    #[test]
    fn round_trip_hoisting() {
        assert_round_trip("hoisting.yopta");
    }

    #[test]
    fn round_trip_import_json() {
        assert_round_trip("import_json.yopta");
    }

    #[test]
    fn round_trip_io_demo() {
        assert_round_trip("io_demo.yopta");
    }

    #[test]
    fn round_trip_iterator_helpers() {
        assert_round_trip("iterator_helpers.yopta");
    }

    #[test]
    fn round_trip_labeled_loops() {
        assert_round_trip("labeled_loops.yopta");
    }

    #[test]
    fn round_trip_promise_smoke1() {
        assert_round_trip("promise_smoke1.yopta");
    }

    #[test]
    fn round_trip_promise_smoke2() {
        assert_round_trip("promise_smoke2.yopta");
    }

    #[test]
    fn round_trip_regex() {
        assert_round_trip("regex.yopta");
    }

    #[test]
    fn round_trip_stack_trace() {
        assert_round_trip("stack_trace.yopta");
    }

    #[test]
    fn round_trip_stdlib() {
        assert_round_trip("stdlib.yopta");
    }

    #[test]
    fn round_trip_tagged_templates() {
        assert_round_trip("tagged_templates.yopta");
    }

    // === (b) идемпотентность побайтно на 20 examples ===

    fn assert_idempotent(name: &str) {
        let src = read_example(name);
        let first = parse_and_format(&src);
        let second = parse_and_format(&first);
        assert_eq!(first, second, "идемпотентность нарушена для {name}: fmt(fmt(src)) ≠ fmt(src)");
    }

    #[test]
    fn idempotent_abort() {
        assert_idempotent("abort.yopta");
    }

    #[test]
    fn idempotent_async_timers() {
        assert_idempotent("async_timers.yopta");
    }

    #[test]
    fn idempotent_date() {
        assert_idempotent("date.yopta");
    }

    #[test]
    fn idempotent_decorators() {
        assert_idempotent("decorators.yopta");
    }

    #[test]
    fn idempotent_destructuring_defaults() {
        assert_idempotent("destructuring_defaults.yopta");
    }

    #[test]
    fn idempotent_dynamic_import() {
        assert_idempotent("dynamic_import.yopta");
    }

    #[test]
    fn idempotent_event_loop() {
        assert_idempotent("event_loop.yopta");
    }

    #[test]
    fn idempotent_for_await_of() {
        assert_idempotent("for_await_of.yopta");
    }

    #[test]
    fn idempotent_hello() {
        assert_idempotent("hello.yopta");
    }

    #[test]
    fn idempotent_hoisting() {
        assert_idempotent("hoisting.yopta");
    }

    #[test]
    fn idempotent_import_json() {
        assert_idempotent("import_json.yopta");
    }

    #[test]
    fn idempotent_io_demo() {
        assert_idempotent("io_demo.yopta");
    }

    #[test]
    fn idempotent_iterator_helpers() {
        assert_idempotent("iterator_helpers.yopta");
    }

    #[test]
    fn idempotent_labeled_loops() {
        assert_idempotent("labeled_loops.yopta");
    }

    #[test]
    fn idempotent_promise_smoke1() {
        assert_idempotent("promise_smoke1.yopta");
    }

    #[test]
    fn idempotent_promise_smoke2() {
        assert_idempotent("promise_smoke2.yopta");
    }

    #[test]
    fn idempotent_regex() {
        assert_idempotent("regex.yopta");
    }

    #[test]
    fn idempotent_stack_trace() {
        assert_idempotent("stack_trace.yopta");
    }

    #[test]
    fn idempotent_stdlib() {
        assert_idempotent("stdlib.yopta");
    }

    #[test]
    fn idempotent_tagged_templates() {
        assert_idempotent("tagged_templates.yopta");
    }

    // === (c) property-тест по операторам ===

    fn lcg_next(seed: &mut u64) -> u64 {
        *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        *seed
    }

    fn gen_simple_ident(seed: &mut u64) -> &'static str {
        let idents = ["а", "б", "в", "г", "д", "е", "ж", "з"];
        idents[(lcg_next(seed) as usize) % idents.len()]
    }

    fn gen_simple_number(seed: &mut u64) -> String {
        let n = (lcg_next(seed) % 9) + 1;
        format!("{n}")
    }

    fn binary_op_str(op_idx: usize) -> &'static str {
        const OPS: &[&str] = &[
            "+",
            "-",
            "*",
            "/",
            "%",
            "**",
            "=",
            "+=",
            "-=",
            "*=",
            "/=",
            "**=",
            "==",
            "===",
            "!=",
            "!==",
            "<",
            ">",
            "<=",
            ">=",
            "&&",
            "||",
            "??",
            "??=",
            "&&=",
            "||=",
            "|>",
            "шкура",
            "из",
        ];
        OPS[op_idx % OPS.len()]
    }

    fn unary_op_str(op_idx: usize) -> &'static str {
        const OPS: &[&str] = &["+", "-", "!", "чезажижан", "ёбнуть", "куку"];
        OPS[op_idx % OPS.len()]
    }

    fn postfix_op_str(op_idx: usize) -> &'static str {
        const OPS: &[&str] = &["++", "--"];
        OPS[op_idx % OPS.len()]
    }

    #[test]
    fn property_all_binary_ops_round_trip() {
        let mut seed: u64 = 42;
        for op_idx in 0..29 {
            let op = binary_op_str(op_idx);
            let lhs = gen_simple_ident(&mut seed);
            let rhs = gen_simple_number(&mut seed);
            let src = format!("гыы {lhs} = 1;\n{lhs} {op} {rhs};\n");
            let result = format_source(&src);
            match result {
                Ok(outcome) => {
                    assert!(
                        programs_equivalent_str(&src, &outcome.text),
                        "round-trip не прошёл для бинарного оп '{op}': fmt вывод: {:?}",
                        outcome.text
                    );
                }
                Err(e) => {
                    panic!("format_source failed для оп '{op}': {e}");
                }
            }
        }
    }

    #[test]
    fn property_all_unary_ops_round_trip() {
        let mut seed: u64 = 137;
        for op_idx in 0..6 {
            let op = unary_op_str(op_idx);
            let operand = gen_simple_ident(&mut seed);
            let src = format!("гыы {operand} = 1;\n{op} {operand};\n");
            let result = format_source(&src);
            match result {
                Ok(outcome) => {
                    assert!(
                        programs_equivalent_str(&src, &outcome.text),
                        "round-trip не прошёл для унарного оп '{op}': fmt вывод: {:?}",
                        outcome.text
                    );
                }
                Err(e) => {
                    panic!("format_source failed для унарного оп '{op}': {e}");
                }
            }
        }
    }

    #[test]
    fn property_all_postfix_ops_round_trip() {
        for op_idx in 0..2 {
            let op = postfix_op_str(op_idx);
            let src = format!("гыы а = 1;\nа{op};\n");
            let result = format_source(&src);
            match result {
                Ok(outcome) => {
                    assert!(
                        programs_equivalent_str(&src, &outcome.text),
                        "round-trip не прошёл для постфиксного оп '{op}': fmt вывод: {:?}",
                        outcome.text
                    );
                }
                Err(e) => {
                    panic!("format_source failed для постфиксного оп '{op}': {e}");
                }
            }
        }
    }

    // Обязательный набор ассоциативных пар (план TODO-7c)

    #[test]
    fn assoc_exp_right_assoc_no_extra_parens() {
        let src = "2 ** 3 ** 2;\n";
        let out = parse_and_format(src);
        assert!(programs_equivalent_str(src, &out), "round-trip нарушен для 2**3**2");
        assert!(!out.contains("(3 ** 2)"), "правоассоциативность: лишние скобки вокруг 3**2");
    }

    #[test]
    fn assoc_unary_over_exp_parens() {
        let src = "гыы х = 1;\n(-х) ** 2;\n";
        let out = parse_and_format(src);
        assert!(programs_equivalent_str(src, &out), "round-trip нарушен для (-х)**2");
    }

    #[test]
    fn assoc_unary_wraps_exp_arg() {
        let src = "гыы х = 1;\n-(х ** 2);\n";
        let out = parse_and_format(src);
        assert!(programs_equivalent_str(src, &out), "round-trip нарушен для -(х**2)");
        assert!(out.contains("(х ** 2)"), "скобки вокруг х**2 должны присутствовать: {out:?}");
    }

    #[test]
    fn assoc_left_assoc_right_operand_needs_parens() {
        let src = "а - (б - в);\n";
        let out = parse_and_format(src);
        assert!(programs_equivalent_str(src, &out), "round-trip нарушен для а - (б - в)");
        assert!(out.contains("(б - в)"), "скобки вокруг (б - в) должны присутствовать: {out:?}");
    }

    #[test]
    fn assoc_left_assoc_no_parens() {
        let src = "а - б - в;\n";
        let out = parse_and_format(src);
        assert!(programs_equivalent_str(src, &out), "round-trip нарушен для а - б - в");
    }

    #[test]
    fn assoc_nullish_or_no_extra_parens() {
        let src = "гыы а = ноль;\nгыы б = ноль;\nгыы в = 1;\nа ?? б || в;\n";
        let out = parse_and_format(src);
        assert!(programs_equivalent_str(src, &out), "round-trip нарушен для а ?? б || в");
    }

    // === (d) snapshot канонического вывода ===

    #[test]
    fn snapshot_var_decl() {
        let src = "гыы   х   =   42 ;\n";
        let out = parse_and_format(src);
        assert_eq!(out, "гыы х = 42;\n");
    }

    #[test]
    fn snapshot_const_decl() {
        let src = "ясенХуй   ПИ=3.14;\n";
        let out = parse_and_format(src);
        assert_eq!(out, "ясенХуй ПИ = 3.14;\n");
    }

    #[test]
    fn snapshot_if_else() {
        let src = "вилкойвглаз(правда){сказать(1);}иливжопураз{сказать(2);}\n";
        let out = parse_and_format(src);
        assert_eq!(out, "вилкойвглаз (правда) {\n    сказать(1);\n} иливжопураз {\n    сказать(2);\n}\n");
    }

    #[test]
    fn snapshot_function_decl() {
        let src = "йопта сложить(а,б){отвечаю а+б;}\n";
        let out = parse_and_format(src);
        assert_eq!(out, "йопта сложить(а, б) {\n    отвечаю а + б;\n}\n");
    }

    #[test]
    fn snapshot_while_loop() {
        let src = "гыы и=0;потрещим(и<5){и++;}\n";
        let out = parse_and_format(src);
        assert_eq!(out, "гыы и = 0;\nпотрещим (и < 5) {\n    и++;\n}\n");
    }

    #[test]
    fn snapshot_binary_precedence_parens() {
        let src = "(а + б) * в;\n";
        let out = parse_and_format(src);
        assert_eq!(out, "(а + б) * в;\n");
    }

    #[test]
    fn snapshot_for_loop_with_var_init() {
        let src = "го(гыы и=0;и<3;и++){сказать(и);}\n";
        let out = parse_and_format(src);
        assert_eq!(out, "го (гыы и = 0; и < 3; и++) {\n    сказать(и);\n}\n");
    }

    #[test]
    fn snapshot_class_static_method_and_getter() {
        let src = "клёво К{попонятия статМетод(){отвечаю 1;}get значение(){отвечаю 2;}}\n";
        let out = parse_and_format(src);
        assert_eq!(
            out,
            "клёво К {\n    попонятия статМетод() {\n        отвечаю 1;\n    }\n\n    get значение() {\n        отвечаю 2;\n    }\n}\n"
        );
    }

    // === (e) негативные тесты ===

    #[test]
    fn negative_parse_error_rejected() {
        let src = "гыы х = ;\n";
        let err = format_source(src).unwrap_err();
        assert!(matches!(err, FormatError::ParseError(_)), "ожидался ParseError, получен: {err:?}");
    }

    #[test]
    fn negative_parse_error_incomplete() {
        let src = "гыы х = ";
        let err = format_source(src).unwrap_err();
        assert!(matches!(err, FormatError::ParseError(_)));
    }

    #[test]
    fn negative_dangling_comment_refused() {
        let src = "йопта ф() {\n    // одинокий комментарий внутри пустого блока\n}\n";
        let err = format_source(src).unwrap_err();
        assert!(
            matches!(err, FormatError::CommentRefused(_)),
            "ожидался CommentRefused для dangling комментария, получен: {err:?}"
        );
    }

    #[test]
    fn negative_slash_in_regex_not_comment() {
        let src = "гыы р = /https?:\\/\\//;\n";
        let out = format_source(src);
        assert!(out.is_ok(), "// внутри regex ошибочно принят за комментарий: {out:?}");
    }

    #[test]
    fn negative_slash_in_string_not_comment() {
        let src = "гыы у = \"http://пример.ру\";\n";
        let out = format_source(src).unwrap();
        assert!(out.text.contains("http://пример.ру"));
    }

    #[test]
    fn negative_slash_in_template_not_comment() {
        let src = "гыы у = `http://пример`;\n";
        let out = format_source(src).unwrap();
        assert!(out.text.contains("http://пример"));
    }

    // === (f) пустой Program → один trailing newline ===

    #[test]
    fn empty_program_single_trailing_newline() {
        let src = "";
        let out = parse_and_format(src);
        assert_eq!(out, "\n", "пустой Program должен давать ровно один trailing newline");
    }

    #[test]
    fn whitespace_only_program_single_trailing_newline() {
        let src = "   \n\n  \n";
        let out = parse_and_format(src);
        assert_eq!(out, "\n");
    }

    // Дополнительные тесты корректности check-режима

    #[test]
    fn already_formatted_flag_true_on_canonical() {
        let src = "гыы х = 42;\n";
        let out = format_source(src).unwrap();
        assert!(out.already_formatted, "флаг already_formatted должен быть true для канонического вывода");
    }

    #[test]
    fn already_formatted_flag_false_on_non_canonical() {
        let src = "гыы   х   =   42 ;\n";
        let out = format_source(src).unwrap();
        assert!(!out.already_formatted, "флаг already_formatted должен быть false для неканонического ввода");
    }

    #[test]
    fn function_expr_anon_prints_canonically() {
        let src = "гыы ф = йопта(х) { отвечаю х; };\n";
        let out = parse_and_format(src);
        assert_eq!(out, "гыы ф = йопта(х) {\n    отвечаю х;\n};\n");
        assert!(programs_equivalent_str(src, &out));
    }

    #[test]
    fn function_expr_named_prints_name() {
        let src = "гыы ф = йопта фиб(н) { отвечаю н; };\n";
        let out = parse_and_format(src);
        assert_eq!(out, "гыы ф = йопта фиб(н) {\n    отвечаю н;\n};\n");
        assert!(programs_equivalent_str(src, &out));
    }

    #[test]
    fn function_expr_async_prints_asso_prefix() {
        let src = "гыы ф = ассо йопта(х) { отвечаю х; };\n";
        let out = parse_and_format(src);
        assert_eq!(out, "гыы ф = ассо йопта(х) {\n    отвечаю х;\n};\n");
        assert!(programs_equivalent_str(src, &out));
    }

    #[test]
    fn function_expr_in_call_arg_round_trips() {
        let src = "чутка(йопта() { сказать(1); }, 10);\n";
        let out = parse_and_format(src);
        assert!(programs_equivalent_str(src, &out));
        assert_eq!(out, parse_and_format(&out), "идемпотентность нарушена для function expression в аргументе");
    }

    #[test]
    fn import_namespace_round_trips() {
        let src = "спиздить * как всё из \"./модуль\";\n";
        let out = parse_and_format(src);
        assert!(programs_equivalent_str(src, &out), "round-trip нарушен для спиздить * как всё из ...");
        assert!(out.contains("* как всё"), "форматтер должен печатать 'как', а не 'as': {out:?}");
        assert_eq!(out, parse_and_format(&out), "идемпотентность нарушена для namespace import");
    }
}
