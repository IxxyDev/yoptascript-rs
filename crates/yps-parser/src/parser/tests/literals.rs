use super::*;

#[test]
fn test_parse_array_literal() {
    let source = SourceFile::new("test.yop".to_string(), "[1, 2, 3];".to_string());
    let lexer = yps_lexer::Lexer::new(&source);
    let (tokens, lex_diags) = lexer.tokenize();
    assert!(lex_diags.is_empty());
    let parser = Parser::new(&tokens, &source);

    let (program, diags) = parser.parse_program();

    assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
    assert_eq!(program.items.len(), 1);
    match &program.items[0] {
        Stmt::Expr { expr, .. } => match expr {
            Expr::Literal(Literal::Array { elements, .. }) => {
                assert_eq!(elements.len(), 3);
            }
            _ => panic!("Expected Array literal"),
        },
        _ => panic!("Expected Expr statement"),
    }
}

#[test]
fn test_parse_empty_array() {
    let source = SourceFile::new("test.yop".to_string(), "[];".to_string());
    let lexer = yps_lexer::Lexer::new(&source);
    let (tokens, lex_diags) = lexer.tokenize();
    assert!(lex_diags.is_empty());
    let parser = Parser::new(&tokens, &source);

    let (program, diags) = parser.parse_program();

    assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
    assert_eq!(program.items.len(), 1);
    match &program.items[0] {
        Stmt::Expr { expr, .. } => match expr {
            Expr::Literal(Literal::Array { elements, .. }) => {
                assert_eq!(elements.len(), 0);
            }
            _ => panic!("Expected Array literal"),
        },
        _ => panic!("Expected Expr statement"),
    }
}

#[test]
fn test_parse_array_index() {
    let source = SourceFile::new("test.yop".to_string(), "arr[0];".to_string());
    let lexer = yps_lexer::Lexer::new(&source);
    let (tokens, lex_diags) = lexer.tokenize();
    assert!(lex_diags.is_empty());
    let parser = Parser::new(&tokens, &source);

    let (program, diags) = parser.parse_program();

    assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
    assert_eq!(program.items.len(), 1);
    match &program.items[0] {
        Stmt::Expr { expr, .. } => {
            assert!(matches!(expr, Expr::Index { .. }));
        }
        _ => panic!("Expected Expr statement"),
    }
}

#[test]
fn test_parse_nested_array_index() {
    let source = SourceFile::new("test.yop".to_string(), "arr[i][j];".to_string());
    let lexer = yps_lexer::Lexer::new(&source);
    let (tokens, lex_diags) = lexer.tokenize();
    assert!(lex_diags.is_empty());
    let parser = Parser::new(&tokens, &source);

    let (program, diags) = parser.parse_program();

    assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
    assert_eq!(program.items.len(), 1);
    match &program.items[0] {
        Stmt::Expr { expr, .. } => match expr {
            Expr::Index { object, .. } => {
                assert!(matches!(object.as_ref(), Expr::Index { .. }));
            }
            _ => panic!("Expected Index expression"),
        },
        _ => panic!("Expected Expr statement"),
    }
}

#[test]
fn test_parse_nested_array_literal() {
    let source = SourceFile::new("test.yop".to_string(), "[[1, 2], [3, 4]];".to_string());
    let lexer = yps_lexer::Lexer::new(&source);
    let (tokens, lex_diags) = lexer.tokenize();
    assert!(lex_diags.is_empty());
    let parser = Parser::new(&tokens, &source);

    let (program, diags) = parser.parse_program();

    assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
    assert_eq!(program.items.len(), 1);
    match &program.items[0] {
        Stmt::Expr { expr, .. } => match expr {
            Expr::Literal(Literal::Array { elements, .. }) => {
                assert_eq!(elements.len(), 2);
                assert!(matches!(elements[0], Expr::Literal(Literal::Array { .. })));
            }
            _ => panic!("Expected Array literal"),
        },
        _ => panic!("Expected Expr statement"),
    }
}

#[test]
fn test_parse_object_literal() {
    let source = SourceFile::new("test.yop".to_string(), "гыы obj = {x: 1, y: 2};".to_string());
    let lexer = yps_lexer::Lexer::new(&source);
    let (tokens, lex_diags) = lexer.tokenize();
    assert!(lex_diags.is_empty());
    let parser = Parser::new(&tokens, &source);

    let (program, diags) = parser.parse_program();

    assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
    assert_eq!(program.items.len(), 1);
    match &program.items[0] {
        Stmt::VarDecl { init, .. } => match init {
            Expr::Literal(Literal::Object { entries, .. }) => {
                assert_eq!(entries.len(), 2);
                match &entries[0] {
                    ObjectEntry::Property { key: PropKey::Identifier(id), .. } => assert_eq!(id.name, "x"),
                    _ => panic!("Expected identifier key"),
                }
                match &entries[1] {
                    ObjectEntry::Property { key: PropKey::Identifier(id), .. } => assert_eq!(id.name, "y"),
                    _ => panic!("Expected identifier key"),
                }
            }
            _ => panic!("Expected Object literal"),
        },
        _ => panic!("Expected VarDecl statement"),
    }
}

#[test]
fn test_parse_empty_object() {
    let source = SourceFile::new("test.yop".to_string(), "гыы obj = {};".to_string());
    let lexer = yps_lexer::Lexer::new(&source);
    let (tokens, lex_diags) = lexer.tokenize();
    assert!(lex_diags.is_empty());
    let parser = Parser::new(&tokens, &source);

    let (program, diags) = parser.parse_program();

    assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
    assert_eq!(program.items.len(), 1);
    match &program.items[0] {
        Stmt::VarDecl { init, .. } => match init {
            Expr::Literal(Literal::Object { entries, .. }) => {
                assert_eq!(entries.len(), 0);
            }
            _ => panic!("Expected Object literal"),
        },
        _ => panic!("Expected VarDecl statement"),
    }
}

#[test]
fn test_parse_member_access() {
    let source = SourceFile::new("test.yop".to_string(), "obj.prop;".to_string());
    let lexer = yps_lexer::Lexer::new(&source);
    let (tokens, lex_diags) = lexer.tokenize();
    assert!(lex_diags.is_empty());
    let parser = Parser::new(&tokens, &source);

    let (program, diags) = parser.parse_program();

    assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
    assert_eq!(program.items.len(), 1);
    match &program.items[0] {
        Stmt::Expr { expr, .. } => {
            assert!(matches!(expr, Expr::Member { .. }));
        }
        _ => panic!("Expected Expr statement"),
    }
}

#[test]
fn test_parse_nested_member_access() {
    let source = SourceFile::new("test.yop".to_string(), "obj.prop.nested;".to_string());
    let lexer = yps_lexer::Lexer::new(&source);
    let (tokens, lex_diags) = lexer.tokenize();
    assert!(lex_diags.is_empty());
    let parser = Parser::new(&tokens, &source);

    let (program, diags) = parser.parse_program();

    assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
    assert_eq!(program.items.len(), 1);
    match &program.items[0] {
        Stmt::Expr { expr, .. } => match expr {
            Expr::Member { object, .. } => {
                assert!(matches!(object.as_ref(), Expr::Member { .. }));
            }
            _ => panic!("Expected Member expression"),
        },
        _ => panic!("Expected Expr statement"),
    }
}

#[test]
fn test_parse_nested_object_literal() {
    let source = SourceFile::new("test.yop".to_string(), "гыы obj = {x: {y: 1}};".to_string());
    let lexer = yps_lexer::Lexer::new(&source);
    let (tokens, lex_diags) = lexer.tokenize();
    assert!(lex_diags.is_empty());
    let parser = Parser::new(&tokens, &source);

    let (program, diags) = parser.parse_program();

    assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
    assert_eq!(program.items.len(), 1);
    match &program.items[0] {
        Stmt::VarDecl { init, .. } => match init {
            Expr::Literal(Literal::Object { entries, .. }) => {
                assert_eq!(entries.len(), 1);
                match &entries[0] {
                    ObjectEntry::Property { value, .. } => {
                        assert!(matches!(value, Expr::Literal(Literal::Object { .. })));
                    }
                    _ => panic!("Expected property"),
                }
            }
            _ => panic!("Expected Object literal"),
        },
        _ => panic!("Expected VarDecl statement"),
    }
}

#[test]
fn test_parse_method_call() {
    let source = SourceFile::new("test.yop".to_string(), "obj.method();".to_string());
    let lexer = yps_lexer::Lexer::new(&source);
    let (tokens, lex_diags) = lexer.tokenize();
    assert!(lex_diags.is_empty());
    let parser = Parser::new(&tokens, &source);

    let (program, diags) = parser.parse_program();

    assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
    assert_eq!(program.items.len(), 1);
    match &program.items[0] {
        Stmt::Expr { expr, .. } => match expr {
            Expr::Call { callee, .. } => {
                assert!(matches!(callee.as_ref(), Expr::Member { .. }));
            }
            _ => panic!("Expected Call expression"),
        },
        _ => panic!("Expected Expr statement"),
    }
}

#[test]
fn test_parse_array_of_objects() {
    let source = SourceFile::new("test.yop".to_string(), "[{x: 1}, {y: 2}];".to_string());
    let lexer = yps_lexer::Lexer::new(&source);
    let (tokens, lex_diags) = lexer.tokenize();
    assert!(lex_diags.is_empty());
    let parser = Parser::new(&tokens, &source);

    let (program, diags) = parser.parse_program();

    assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
    assert_eq!(program.items.len(), 1);
    match &program.items[0] {
        Stmt::Expr { expr, .. } => match expr {
            Expr::Literal(Literal::Array { elements, .. }) => {
                assert_eq!(elements.len(), 2);
                assert!(matches!(elements[0], Expr::Literal(Literal::Object { .. })));
                assert!(matches!(elements[1], Expr::Literal(Literal::Object { .. })));
            }
            _ => panic!("Expected Array literal"),
        },
        _ => panic!("Expected Expr statement"),
    }
}
