use super::*;

#[test]
fn test_parse_function_decl() {
    let source = SourceFile::new("test.yopta".to_string(), "йопта foo(x, y) { x + y; }".to_string());
    let lexer = yps_lexer::Lexer::new(&source);
    let (tokens, lex_diags) = lexer.tokenize();
    assert!(lex_diags.is_empty());
    let parser = Parser::new(&tokens, &source);

    let (program, diags) = parser.parse_program();

    assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
    assert_eq!(program.items.len(), 1);
    match &program.items[0] {
        Stmt::FunctionDecl { name, params, body, .. } => {
            assert_eq!(name.name, "foo");
            assert_eq!(params.len(), 2);
            assert_eq!(params[0].name.name, "x");
            assert_eq!(params[1].name.name, "y");
            assert_eq!(body.stmts.len(), 1);
        }
        _ => panic!("Expected FunctionDecl statement"),
    }
}

#[test]
fn test_parse_function_decl_no_params() {
    let source = SourceFile::new("test.yopta".to_string(), "йопта bar() { отвечаю 42; }".to_string());
    let lexer = yps_lexer::Lexer::new(&source);
    let (tokens, lex_diags) = lexer.tokenize();
    assert!(lex_diags.is_empty());
    let parser = Parser::new(&tokens, &source);

    let (program, diags) = parser.parse_program();

    assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
    assert_eq!(program.items.len(), 1);
    match &program.items[0] {
        Stmt::FunctionDecl { name, params, body, .. } => {
            assert_eq!(name.name, "bar");
            assert_eq!(params.len(), 0);
            assert_eq!(body.stmts.len(), 1);
        }
        _ => panic!("Expected FunctionDecl statement"),
    }
}

#[test]
fn test_parse_return_stmt() {
    let source = SourceFile::new("test.yopta".to_string(), "отвечаю 42;".to_string());
    let lexer = yps_lexer::Lexer::new(&source);
    let (tokens, lex_diags) = lexer.tokenize();
    assert!(lex_diags.is_empty());
    let parser = Parser::new(&tokens, &source);

    let (program, diags) = parser.parse_program();

    assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
    assert_eq!(program.items.len(), 1);
    match &program.items[0] {
        Stmt::Return { value, .. } => {
            assert!(value.is_some());
        }
        _ => panic!("Expected Return statement"),
    }
}

#[test]
fn test_parse_return_stmt_no_value() {
    let source = SourceFile::new("test.yopta".to_string(), "отвечаю;".to_string());
    let lexer = yps_lexer::Lexer::new(&source);
    let (tokens, lex_diags) = lexer.tokenize();
    assert!(lex_diags.is_empty());
    let parser = Parser::new(&tokens, &source);

    let (program, diags) = parser.parse_program();

    assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
    assert_eq!(program.items.len(), 1);
    match &program.items[0] {
        Stmt::Return { value, .. } => {
            assert!(value.is_none());
        }
        _ => panic!("Expected Return statement"),
    }
}

#[test]
fn test_parse_function_call() {
    let source = SourceFile::new("test.yopta".to_string(), "foo(1, 2);".to_string());
    let lexer = yps_lexer::Lexer::new(&source);
    let (tokens, lex_diags) = lexer.tokenize();
    assert!(lex_diags.is_empty());
    let parser = Parser::new(&tokens, &source);

    let (program, diags) = parser.parse_program();

    assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
    assert_eq!(program.items.len(), 1);
    match &program.items[0] {
        Stmt::Expr { expr, .. } => match expr {
            Expr::Call { args, .. } => {
                assert_eq!(args.len(), 2);
            }
            _ => panic!("Expected Call expression"),
        },
        _ => panic!("Expected Expr statement"),
    }
}

#[test]
fn test_parse_function_call_no_args() {
    let source = SourceFile::new("test.yopta".to_string(), "bar();".to_string());
    let lexer = yps_lexer::Lexer::new(&source);
    let (tokens, lex_diags) = lexer.tokenize();
    assert!(lex_diags.is_empty());
    let parser = Parser::new(&tokens, &source);

    let (program, diags) = parser.parse_program();

    assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
    assert_eq!(program.items.len(), 1);
    match &program.items[0] {
        Stmt::Expr { expr, .. } => match expr {
            Expr::Call { args, .. } => {
                assert_eq!(args.len(), 0);
            }
            _ => panic!("Expected Call expression"),
        },
        _ => panic!("Expected Expr statement"),
    }
}

#[test]
fn test_parse_nested_function_call() {
    let source = SourceFile::new("test.yopta".to_string(), "foo(bar(1), 2);".to_string());
    let lexer = yps_lexer::Lexer::new(&source);
    let (tokens, lex_diags) = lexer.tokenize();
    assert!(lex_diags.is_empty());
    let parser = Parser::new(&tokens, &source);

    let (program, diags) = parser.parse_program();

    assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
    assert_eq!(program.items.len(), 1);
    match &program.items[0] {
        Stmt::Expr { expr, .. } => match expr {
            Expr::Call { args, .. } => {
                assert_eq!(args.len(), 2);
                assert!(matches!(args[0], Expr::Call { .. }));
            }
            _ => panic!("Expected Call expression"),
        },
        _ => panic!("Expected Expr statement"),
    }
}

#[test]
fn test_parse_async_function_decl() {
    let (program, diags) = parse_program_from_source("ассо йопта foo() { отвечаю 42; }");
    assert!(diags.is_empty(), "Parse errors: {diags:?}");
    match &program.items[0] {
        Stmt::FunctionDecl { name, is_async, is_generator, .. } => {
            assert_eq!(name.name, "foo");
            assert!(*is_async);
            assert!(!*is_generator);
        }
        other => panic!("Expected async FunctionDecl, got {other:?}"),
    }
}

#[test]
fn test_parser_accepts_yopta_function_expr_in_call_arg() {
    let (prog, diags) = parse_program_from_source("сказать(йопта(v) {});");
    assert!(
        diags.is_empty(),
        "function expression в аргументе должен парситься без ошибок: {:?}",
        diag_messages(&diags)
    );
    assert_eq!(prog.items.len(), 1);
}

#[test]
fn function_expr_anon_in_call_arg() {
    let (prog, diags) = parse_program_from_source("чутка(йопта() { сказать(1); }, 10);");
    assert!(diags.is_empty(), "ошибок не ожидается: {:?}", diag_messages(&diags));
    assert_eq!(prog.items.len(), 1);
    let Stmt::Expr { expr: Expr::Call { args, .. }, .. } = &prog.items[0] else {
        panic!("Ожидался Stmt::Expr с вызовом, получено {:?}", prog.items[0]);
    };
    assert_eq!(args.len(), 2);
    assert!(matches!(&args[0], Expr::FunctionExpr { name: None, .. }));
}

#[test]
fn function_expr_anon_in_var_decl() {
    let (prog, diags) = parse_program_from_source("гыы ф = йопта() { отвечаю 1; };");
    assert!(diags.is_empty(), "ошибок не ожидается: {:?}", diag_messages(&diags));
    assert_eq!(prog.items.len(), 1);
    let Stmt::VarDecl { init, .. } = &prog.items[0] else {
        panic!("Ожидался Stmt::VarDecl, получено {:?}", prog.items[0]);
    };
    assert!(matches!(init, Expr::FunctionExpr { name: None, .. }));
}

#[test]
fn function_expr_named_in_var_decl() {
    let (prog, diags) = parse_program_from_source("гыы ф = йопта имя() { отвечаю 1; };");
    assert!(diags.is_empty(), "ошибок не ожидается: {:?}", diag_messages(&diags));
    assert_eq!(prog.items.len(), 1);
    let Stmt::VarDecl { init, .. } = &prog.items[0] else {
        panic!("Ожидался Stmt::VarDecl, получено {:?}", prog.items[0]);
    };
    let Expr::FunctionExpr { name: Some(name), .. } = init else {
        panic!("Ожидался именованный FunctionExpr, получено {init:?}");
    };
    assert_eq!(name.name, "имя");
}

#[test]
fn function_decl_top_level_still_works() {
    let (prog, diags) = parse_program_from_source("йопта ф() { отвечаю 1; }");
    assert!(diags.is_empty(), "ошибок не ожидается: {:?}", diag_messages(&diags));
    assert!(matches!(prog.items[0], crate::ast::Stmt::FunctionDecl { .. }));
}
