use super::*;

#[test]
fn test_parse_var_decl_gyy() {
    let source = SourceFile::new("test.yopta".to_string(), "гыы x = 5;".to_string());
    let lexer = yps_lexer::Lexer::new(&source);
    let (tokens, lex_diags) = lexer.tokenize();
    assert!(lex_diags.is_empty());

    let parser = Parser::new(&tokens, &source);
    let (program, diags) = parser.parse_program();

    assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
    assert_eq!(program.items.len(), 1);

    match &program.items[0] {
        Stmt::VarDecl { pattern: Pattern::Identifier(name), init, .. } => {
            assert_eq!(name.name, "x");
            assert!(matches!(init, Expr::Literal(Literal::Number { .. })));
        }
        _ => panic!("Expected VarDecl, got: {:?}", program.items[0]),
    }
}

#[test]
fn test_parse_labeled_statement() {
    let source = SourceFile::new("test.yopta".to_string(), "внешний: потрещим (правда) { харэ внешний; }".to_string());
    let lexer = yps_lexer::Lexer::new(&source);
    let (tokens, lex_diags) = lexer.tokenize();
    assert!(lex_diags.is_empty());

    let parser = Parser::new(&tokens, &source);
    let (program, diags) = parser.parse_program();
    assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");

    match &program.items[0] {
        Stmt::Labeled { label, body, .. } => {
            assert_eq!(label.name, "внешний");
            assert!(matches!(body.as_ref(), Stmt::While { .. }));
        }
        other => panic!("Expected Labeled, got: {other:?}"),
    }
}

#[test]
fn test_parse_break_with_label() {
    let source = SourceFile::new("test.yopta".to_string(), "харэ метка;".to_string());
    let lexer = yps_lexer::Lexer::new(&source);
    let (tokens, lex_diags) = lexer.tokenize();
    assert!(lex_diags.is_empty());

    let parser = Parser::new(&tokens, &source);
    let (program, diags) = parser.parse_program();
    assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");

    match &program.items[0] {
        Stmt::Break { label: Some(l), .. } => assert_eq!(l.name, "метка"),
        other => panic!("Expected Break with label, got: {other:?}"),
    }
}

#[test]
fn test_parse_break_without_label() {
    let source = SourceFile::new("test.yopta".to_string(), "харэ;".to_string());
    let lexer = yps_lexer::Lexer::new(&source);
    let (tokens, _) = lexer.tokenize();
    let parser = Parser::new(&tokens, &source);
    let (program, diags) = parser.parse_program();
    assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
    assert!(matches!(program.items[0], Stmt::Break { label: None, .. }));
}

#[test]
fn test_parse_object_pattern_default() {
    let source = SourceFile::new("test.yopta".to_string(), "гыы { х = 5, а: б = 7 } = obj;".to_string());
    let lexer = yps_lexer::Lexer::new(&source);
    let (tokens, lex_diags) = lexer.tokenize();
    assert!(lex_diags.is_empty());

    let parser = Parser::new(&tokens, &source);
    let (program, diags) = parser.parse_program();
    assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");

    match &program.items[0] {
        Stmt::VarDecl { pattern: Pattern::Object { properties, .. }, .. } => {
            assert_eq!(properties.len(), 2);
            assert!(matches!(properties[0].value, Some(Pattern::Default { .. })), "shorthand с default");
            assert!(matches!(properties[1].value, Some(Pattern::Default { .. })), "rename с default");
        }
        other => panic!("Expected object pattern VarDecl, got: {other:?}"),
    }
}

#[test]
fn test_parse_array_pattern_default() {
    let source = SourceFile::new("test.yopta".to_string(), "гыы [а = 1, б] = arr;".to_string());
    let lexer = yps_lexer::Lexer::new(&source);
    let (tokens, lex_diags) = lexer.tokenize();
    assert!(lex_diags.is_empty());

    let parser = Parser::new(&tokens, &source);
    let (program, diags) = parser.parse_program();
    assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");

    match &program.items[0] {
        Stmt::VarDecl { pattern: Pattern::Array { elements, .. }, .. } => {
            assert!(matches!(elements[0], Some(Pattern::Default { .. })));
            assert!(matches!(elements[1], Some(Pattern::Identifier(_))));
        }
        other => panic!("Expected array pattern VarDecl, got: {other:?}"),
    }
}

#[test]
fn test_parse_var_decl_yasen_huy() {
    let source = SourceFile::new("test.yopta".to_string(), "ясенХуй y = \"hello\";".to_string());
    let lexer = yps_lexer::Lexer::new(&source);
    let (tokens, lex_diags) = lexer.tokenize();
    assert!(lex_diags.is_empty());

    let parser = Parser::new(&tokens, &source);
    let (program, diags) = parser.parse_program();

    assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
    assert_eq!(program.items.len(), 1);

    match &program.items[0] {
        Stmt::VarDecl { pattern: Pattern::Identifier(name), init, .. } => {
            assert_eq!(name.name, "y");
            assert!(matches!(init, Expr::Literal(Literal::String { .. })));
        }
        _ => panic!("Expected VarDecl"),
    }
}

#[test]
fn test_parse_expr_stmt() {
    let source = SourceFile::new("test.yopta".to_string(), "x + 5;".to_string());
    let lexer = yps_lexer::Lexer::new(&source);
    let (tokens, lex_diags) = lexer.tokenize();
    assert!(lex_diags.is_empty());

    let parser = Parser::new(&tokens, &source);
    let (program, diags) = parser.parse_program();

    assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
    assert_eq!(program.items.len(), 1);

    match &program.items[0] {
        Stmt::Expr { expr, .. } => {
            assert!(matches!(expr, Expr::Binary { op: BinaryOp::Add, .. }));
        }
        _ => panic!("Expected Expr statement"),
    }
}

#[test]
fn test_parse_empty_stmt() {
    let source = SourceFile::new("test.yopta".to_string(), ";".to_string());
    let lexer = yps_lexer::Lexer::new(&source);
    let (tokens, lex_diags) = lexer.tokenize();
    assert!(lex_diags.is_empty());

    let parser = Parser::new(&tokens, &source);
    let (program, diags) = parser.parse_program();

    assert!(diags.is_empty());
    assert_eq!(program.items.len(), 1);
    assert!(matches!(program.items[0], Stmt::Empty { .. }));
}

#[test]
fn test_parse_multiple_statements() {
    let source = SourceFile::new("test.yopta".to_string(), "гыы x = 5;\nучастковый y = 10;\nx + y;".to_string());
    let lexer = yps_lexer::Lexer::new(&source);
    let (tokens, lex_diags) = lexer.tokenize();
    assert!(lex_diags.is_empty());

    let parser = Parser::new(&tokens, &source);
    let (program, diags) = parser.parse_program();

    assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
    assert_eq!(program.items.len(), 3);

    assert!(matches!(program.items[0], Stmt::VarDecl { .. }));
    assert!(matches!(program.items[1], Stmt::VarDecl { .. }));
    assert!(matches!(program.items[2], Stmt::Expr { .. }));
}

#[test]
fn test_parse_if_stmt() {
    let source = SourceFile::new("test.yopta".to_string(), "вилкойвглаз (x > 5) x = 10;".to_string());
    let lexer = yps_lexer::Lexer::new(&source);
    let (tokens, lex_diags) = lexer.tokenize();
    assert!(lex_diags.is_empty());

    let parser = Parser::new(&tokens, &source);
    let (program, diags) = parser.parse_program();

    assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
    assert_eq!(program.items.len(), 1);

    match &program.items[0] {
        Stmt::If { condition, then_branch, else_branch, .. } => {
            assert!(matches!(condition, Expr::Binary { op: BinaryOp::Greater, .. }));
            assert!(matches!(then_branch.as_ref(), Stmt::Expr { .. }));
            assert!(else_branch.is_none());
        }
        _ => panic!("Expected If statement"),
    }
}

#[test]
fn test_parse_if_else_stmt() {
    let source =
        SourceFile::new("test.yopta".to_string(), "вилкойвглаз (x > 5) x = 10; иливжопураз x = 0;".to_string());
    let lexer = yps_lexer::Lexer::new(&source);
    let (tokens, lex_diags) = lexer.tokenize();
    assert!(lex_diags.is_empty());

    let parser = Parser::new(&tokens, &source);
    let (program, diags) = parser.parse_program();

    assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
    assert_eq!(program.items.len(), 1);

    match &program.items[0] {
        Stmt::If { condition, then_branch, else_branch, .. } => {
            assert!(matches!(condition, Expr::Binary { op: BinaryOp::Greater, .. }));
            assert!(matches!(then_branch.as_ref(), Stmt::Expr { .. }));
            assert!(else_branch.is_some());
            assert!(matches!(else_branch.as_ref().unwrap().as_ref(), Stmt::Expr { .. }));
        }
        _ => panic!("Expected If statement"),
    }
}

#[test]
fn test_parse_if_with_block() {
    let source = SourceFile::new("test.yopta".to_string(), "вилкойвглаз (x > 5) { x = 10; }".to_string());
    let lexer = yps_lexer::Lexer::new(&source);
    let (tokens, lex_diags) = lexer.tokenize();
    assert!(lex_diags.is_empty());

    let parser = Parser::new(&tokens, &source);
    let (program, diags) = parser.parse_program();

    assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
    assert_eq!(program.items.len(), 1);

    match &program.items[0] {
        Stmt::If { then_branch, .. } => {
            assert!(matches!(then_branch.as_ref(), Stmt::Block(_)));
        }
        _ => panic!("Expected If statement"),
    }
}

#[test]
fn test_parse_while_stmt() {
    let source = SourceFile::new("test.yopta".to_string(), "потрещим (x > 0) x = x - 1;".to_string());
    let lexer = yps_lexer::Lexer::new(&source);
    let (tokens, lex_diags) = lexer.tokenize();
    assert!(lex_diags.is_empty());
    let parser = Parser::new(&tokens, &source);

    let (program, diags) = parser.parse_program();

    assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
    assert_eq!(program.items.len(), 1);
    match &program.items[0] {
        Stmt::While { condition, body, .. } => {
            assert!(matches!(condition, Expr::Binary { op: BinaryOp::Greater, .. }));
            assert!(matches!(body.as_ref(), Stmt::Expr { .. }));
        }
        _ => panic!("Expected While statement"),
    }
}

#[test]
fn test_parse_while_with_block() {
    let source = SourceFile::new("test.yopta".to_string(), "потрещим (x > 0) { x = x - 1; }".to_string());
    let lexer = yps_lexer::Lexer::new(&source);
    let (tokens, lex_diags) = lexer.tokenize();
    assert!(lex_diags.is_empty());
    let parser = Parser::new(&tokens, &source);

    let (program, diags) = parser.parse_program();

    assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
    assert_eq!(program.items.len(), 1);
    match &program.items[0] {
        Stmt::While { body, .. } => {
            assert!(matches!(body.as_ref(), Stmt::Block(_)));
        }
        _ => panic!("Expected While statement"),
    }
}

#[test]
fn test_parse_nested_while() {
    let source = SourceFile::new("test.yopta".to_string(), "потрещим (x > 0) потрещим (y > 0) y = y - 1;".to_string());
    let lexer = yps_lexer::Lexer::new(&source);
    let (tokens, lex_diags) = lexer.tokenize();
    assert!(lex_diags.is_empty());
    let parser = Parser::new(&tokens, &source);

    let (program, diags) = parser.parse_program();

    assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
    assert_eq!(program.items.len(), 1);
    match &program.items[0] {
        Stmt::While { body, .. } => {
            assert!(matches!(body.as_ref(), Stmt::While { .. }));
        }
        _ => panic!("Expected While statement"),
    }
}

#[test]
fn test_parse_for_stmt() {
    let source = SourceFile::new("test.yopta".to_string(), "го (гыы i = 0; i < 10; i = i + 1) x = x + i;".to_string());
    let lexer = yps_lexer::Lexer::new(&source);
    let (tokens, lex_diags) = lexer.tokenize();
    assert!(lex_diags.is_empty());
    let parser = Parser::new(&tokens, &source);

    let (program, diags) = parser.parse_program();

    assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
    assert_eq!(program.items.len(), 1);
    match &program.items[0] {
        Stmt::For { init, condition, update, body, .. } => {
            assert!(init.is_some());
            assert!(condition.is_some());
            assert!(update.is_some());
            assert!(matches!(body.as_ref(), Stmt::Expr { .. }));
        }
        _ => panic!("Expected For statement"),
    }
}

#[test]
fn test_parse_for_with_block() {
    let source =
        SourceFile::new("test.yopta".to_string(), "го (гыы i = 0; i < 10; i = i + 1) { x = x + i; }".to_string());
    let lexer = yps_lexer::Lexer::new(&source);
    let (tokens, lex_diags) = lexer.tokenize();
    assert!(lex_diags.is_empty());
    let parser = Parser::new(&tokens, &source);

    let (program, diags) = parser.parse_program();

    assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
    assert_eq!(program.items.len(), 1);
    match &program.items[0] {
        Stmt::For { body, .. } => {
            assert!(matches!(body.as_ref(), Stmt::Block(_)));
        }
        _ => panic!("Expected For statement"),
    }
}

#[test]
fn test_parse_for_without_init() {
    let source = SourceFile::new("test.yopta".to_string(), "го (; i < 10; i = i + 1) x = x + i;".to_string());
    let lexer = yps_lexer::Lexer::new(&source);
    let (tokens, lex_diags) = lexer.tokenize();
    assert!(lex_diags.is_empty());
    let parser = Parser::new(&tokens, &source);

    let (program, diags) = parser.parse_program();

    assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
    assert_eq!(program.items.len(), 1);
    match &program.items[0] {
        Stmt::For { init, condition, update, .. } => {
            assert!(init.is_none());
            assert!(condition.is_some());
            assert!(update.is_some());
        }
        _ => panic!("Expected For statement"),
    }
}

#[test]
fn test_parse_for_without_condition() {
    let source = SourceFile::new("test.yopta".to_string(), "го (гыы i = 0; ; i = i + 1) x = x + i;".to_string());
    let lexer = yps_lexer::Lexer::new(&source);
    let (tokens, lex_diags) = lexer.tokenize();
    assert!(lex_diags.is_empty());
    let parser = Parser::new(&tokens, &source);

    let (program, diags) = parser.parse_program();

    assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
    assert_eq!(program.items.len(), 1);
    match &program.items[0] {
        Stmt::For { init, condition, update, .. } => {
            assert!(init.is_some());
            assert!(condition.is_none());
            assert!(update.is_some());
        }
        _ => panic!("Expected For statement"),
    }
}

#[test]
fn test_parse_for_without_update() {
    let source = SourceFile::new("test.yopta".to_string(), "го (гыы i = 0; i < 10;) x = x + i;".to_string());
    let lexer = yps_lexer::Lexer::new(&source);
    let (tokens, lex_diags) = lexer.tokenize();
    assert!(lex_diags.is_empty());
    let parser = Parser::new(&tokens, &source);

    let (program, diags) = parser.parse_program();

    assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
    assert_eq!(program.items.len(), 1);
    match &program.items[0] {
        Stmt::For { init, condition, update, .. } => {
            assert!(init.is_some());
            assert!(condition.is_some());
            assert!(update.is_none());
        }
        _ => panic!("Expected For statement"),
    }
}

#[test]
fn test_parse_for_infinite_loop() {
    let source = SourceFile::new("test.yopta".to_string(), "го (;;) x = x + 1;".to_string());
    let lexer = yps_lexer::Lexer::new(&source);
    let (tokens, lex_diags) = lexer.tokenize();
    assert!(lex_diags.is_empty());
    let parser = Parser::new(&tokens, &source);

    let (program, diags) = parser.parse_program();

    assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
    assert_eq!(program.items.len(), 1);
    match &program.items[0] {
        Stmt::For { init, condition, update, .. } => {
            assert!(init.is_none());
            assert!(condition.is_none());
            assert!(update.is_none());
        }
        _ => panic!("Expected For statement"),
    }
}

#[test]
fn test_parse_nested_for() {
    let source = SourceFile::new(
        "test.yopta".to_string(),
        "го (гыы i = 0; i < 10; i = i + 1) го (гыы j = 0; j < 5; j = j + 1) x = x + 1;".to_string(),
    );
    let lexer = yps_lexer::Lexer::new(&source);
    let (tokens, lex_diags) = lexer.tokenize();
    assert!(lex_diags.is_empty());
    let parser = Parser::new(&tokens, &source);

    let (program, diags) = parser.parse_program();

    assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
    assert_eq!(program.items.len(), 1);
    match &program.items[0] {
        Stmt::For { body, .. } => {
            assert!(matches!(body.as_ref(), Stmt::For { .. }));
        }
        _ => panic!("Expected For statement"),
    }
}

#[test]
fn test_parse_break_stmt() {
    let source = SourceFile::new("test.yopta".to_string(), "харэ;".to_string());
    let lexer = yps_lexer::Lexer::new(&source);
    let (tokens, lex_diags) = lexer.tokenize();
    assert!(lex_diags.is_empty());
    let parser = Parser::new(&tokens, &source);

    let (program, diags) = parser.parse_program();

    assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
    assert_eq!(program.items.len(), 1);
    assert!(matches!(program.items[0], Stmt::Break { .. }));
}

#[test]
fn test_parse_continue_stmt() {
    let source = SourceFile::new("test.yopta".to_string(), "двигай;".to_string());
    let lexer = yps_lexer::Lexer::new(&source);
    let (tokens, lex_diags) = lexer.tokenize();
    assert!(lex_diags.is_empty());
    let parser = Parser::new(&tokens, &source);

    let (program, diags) = parser.parse_program();

    assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
    assert_eq!(program.items.len(), 1);
    assert!(matches!(program.items[0], Stmt::Continue { .. }));
}

#[test]
fn test_parse_break_in_while() {
    let source = SourceFile::new("test.yopta".to_string(), "потрещим (x > 0) { харэ; }".to_string());
    let lexer = yps_lexer::Lexer::new(&source);
    let (tokens, lex_diags) = lexer.tokenize();
    assert!(lex_diags.is_empty());
    let parser = Parser::new(&tokens, &source);

    let (program, diags) = parser.parse_program();

    assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
    assert_eq!(program.items.len(), 1);
    match &program.items[0] {
        Stmt::While { body, .. } => match body.as_ref() {
            Stmt::Block(Block { stmts, .. }) => {
                assert_eq!(stmts.len(), 1);
                assert!(matches!(stmts[0], Stmt::Break { .. }));
            }
            _ => panic!("Expected Block in While body"),
        },
        _ => panic!("Expected While statement"),
    }
}

#[test]
fn test_parse_continue_in_for() {
    let source = SourceFile::new("test.yopta".to_string(), "го (гыы i = 0; i < 10; i = i + 1) { двигай; }".to_string());
    let lexer = yps_lexer::Lexer::new(&source);
    let (tokens, lex_diags) = lexer.tokenize();
    assert!(lex_diags.is_empty());
    let parser = Parser::new(&tokens, &source);

    let (program, diags) = parser.parse_program();

    assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
    assert_eq!(program.items.len(), 1);
    match &program.items[0] {
        Stmt::For { body, .. } => match body.as_ref() {
            Stmt::Block(Block { stmts, .. }) => {
                assert_eq!(stmts.len(), 1);
                assert!(matches!(stmts[0], Stmt::Continue { .. }));
            }
            _ => panic!("Expected Block in For body"),
        },
        _ => panic!("Expected For statement"),
    }
}

#[test]
fn test_parse_using_await() {
    let (program, diags) = parse_program_from_source("юзай сидетьНахуй р = получить();");
    assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
    assert_eq!(program.items.len(), 1);
    match &program.items[0] {
        Stmt::Using { name, is_await, .. } => {
            assert_eq!(name.name, "р");
            assert!(*is_await);
        }
        other => panic!("Expected Using statement, got {other:?}"),
    }
}

#[test]
fn test_parse_using_sync_not_await() {
    let (program, diags) = parse_program_from_source("юзай р = получить();");
    assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
    match &program.items[0] {
        Stmt::Using { is_await, .. } => assert!(!*is_await),
        other => panic!("Expected Using statement, got {other:?}"),
    }
}

#[test]
fn test_parse_using_await_missing_name() {
    let (_, diags) = parse_program_from_source("юзай сидетьНахуй = получить();");
    let msgs = diag_messages(&diags);
    assert!(msgs.iter().any(|m| m.contains("Ожидался идентификатор")), "Expected identifier error, got: {msgs:?}");
}

#[test]
fn test_parse_for_of_plain_identifier() {
    let (program, diags) = parse_program_from_source("го (гыы х сашаГрей сп) { сказать(х); }");
    assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
    match &program.items[0] {
        Stmt::ForOf { variable: Pattern::Identifier(id), .. } => assert_eq!(id.name, "х"),
        other => panic!("Expected ForOf with identifier, got {other:?}"),
    }
}

#[test]
fn test_parse_for_of_array_pattern() {
    let (program, diags) = parse_program_from_source("го (гыы [а, б] сашаГрей пары) { сказать(а); }");
    assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
    match &program.items[0] {
        Stmt::ForOf { variable: Pattern::Array { elements, .. }, .. } => assert_eq!(elements.len(), 2),
        other => panic!("Expected ForOf with array pattern, got {other:?}"),
    }
}

#[test]
fn test_parse_for_of_object_pattern() {
    let (program, diags) = parse_program_from_source("го (гыы { х, у } сашаГрей точки) { сказать(х); }");
    assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
    match &program.items[0] {
        Stmt::ForOf { variable: Pattern::Object { properties, .. }, .. } => assert_eq!(properties.len(), 2),
        other => panic!("Expected ForOf with object pattern, got {other:?}"),
    }
}

#[test]
fn test_parse_for_of_array_pattern_rest_and_default() {
    let (program, diags) =
        parse_program_from_source("го (гыы [первый = 0, ...хвост] сашаГрей сп) { сказать(первый); }");
    assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
    match &program.items[0] {
        Stmt::ForOf { variable: Pattern::Array { elements, rest, .. }, .. } => {
            assert_eq!(elements.len(), 1);
            assert!(matches!(elements[0], Some(Pattern::Default { .. })));
            assert!(rest.is_some());
        }
        other => panic!("Expected ForOf with array pattern, got {other:?}"),
    }
}

#[test]
fn test_parse_for_of_nested_pattern() {
    let (program, diags) = parse_program_from_source("го (гыы { к: { л } } сашаГрей сп) { сказать(л); }");
    assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
    match &program.items[0] {
        Stmt::ForOf { variable: Pattern::Object { properties, .. }, .. } => {
            assert!(matches!(properties[0].value, Some(Pattern::Object { .. })));
        }
        other => panic!("Expected ForOf with nested pattern, got {other:?}"),
    }
}

#[test]
fn test_parse_for_in_array_pattern() {
    let (program, diags) = parse_program_from_source("го (гыы [а, б] чоунастут об) { сказать(а); }");
    assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
    match &program.items[0] {
        Stmt::ForIn { variable: Pattern::Array { elements, .. }, .. } => assert_eq!(elements.len(), 2),
        other => panic!("Expected ForIn with array pattern, got {other:?}"),
    }
}

#[test]
fn test_parse_for_await_of_object_pattern() {
    let (program, diags) = parse_program_from_source("го сидетьНахуй (гыы { х, у } сашаГрей ист) { сказать(х); }");
    assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
    match &program.items[0] {
        Stmt::ForAwaitOf { variable: Pattern::Object { properties, .. }, .. } => assert_eq!(properties.len(), 2),
        other => panic!("Expected ForAwaitOf with object pattern, got {other:?}"),
    }
}

#[test]
fn test_parse_for_classic_with_array_destructure_init_unchanged() {
    let (program, diags) = parse_program_from_source("го (гыы [а, б] = [1, 2]; а < 10; а = а + 1) { сказать(а); }");
    assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
    assert!(matches!(program.items[0], Stmt::For { .. }), "Expected classic For, got {:?}", program.items[0]);
}

#[test]
fn test_parse_for_classic_plain_still_for() {
    let (program, diags) = parse_program_from_source("го (гыы i = 0; i < 10; i = i + 1) { сказать(i); }");
    assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
    assert!(matches!(program.items[0], Stmt::For { .. }), "Expected classic For, got {:?}", program.items[0]);
}

#[test]
fn test_parse_class_static_block() {
    let (program, diags) = parse_program_from_source(
        "клёво К { попонятия а = 1; попонятия { тырыпыры.б = 2; } попонятия метод() { отвечаю 3; } }",
    );
    assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
    match &program.items[0] {
        Stmt::ClassDecl { members, .. } => {
            assert_eq!(members.len(), 3);
            assert!(matches!(members[0], ClassMember::Field { is_static: true, .. }));
            match &members[1] {
                ClassMember::StaticBlock { body, .. } => assert_eq!(body.stmts.len(), 1),
                other => panic!("Expected StaticBlock, got {other:?}"),
            }
            assert!(matches!(members[2], ClassMember::Method { is_static: true, .. }));
        }
        other => panic!("Expected ClassDecl, got {other:?}"),
    }
}

#[test]
fn test_parse_class_static_block_decorator_rejected() {
    let (_program, diags) = parse_program_from_source("клёво К { @дек попонятия { тырыпыры.а = 1; } }");
    assert!(!diags.is_empty(), "Expected diagnostic for decorated static block");
    assert!(
        diags.iter().any(|d| d.message.contains("статическому блоку")),
        "Expected static-block decorator diagnostic, got: {:?}",
        diag_messages(&diags)
    );
}
