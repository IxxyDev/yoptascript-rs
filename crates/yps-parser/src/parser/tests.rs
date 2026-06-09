use super::*;

fn parse_expr_from_source(src: &str) -> Result<Expr, Vec<Diagnostic>> {
    let source = SourceFile::new("test.yop".to_string(), src.to_string());
    let lexer = yps_lexer::Lexer::new(&source);
    let (tokens, lex_diags) = lexer.tokenize();

    if !lex_diags.is_empty() {
        return Err(lex_diags);
    }

    let mut parser = Parser::new(&tokens, &source);
    match parser.parse_expr() {
        Ok(expr) => Ok(expr),
        Err(()) => Err(parser.diagnostics),
    }
}

#[test]
fn test_parse_number() {
    let expr = parse_expr_from_source("42").unwrap();
    assert!(matches!(expr, Expr::Literal(Literal::Number { .. })));
}

#[test]
fn test_parse_string() {
    let expr = parse_expr_from_source("\"hello\"").unwrap();
    assert!(matches!(expr, Expr::Literal(Literal::String { .. })));
}

#[test]
fn test_parse_string_escape_newline() {
    let expr = parse_expr_from_source(r#""hello\nworld""#).unwrap();
    match expr {
        Expr::Literal(Literal::String { value, .. }) => assert_eq!(value, "hello\nworld"),
        _ => panic!("expected string literal"),
    }
}

#[test]
fn test_parse_string_escape_tab() {
    let expr = parse_expr_from_source(r#""a\tb""#).unwrap();
    match expr {
        Expr::Literal(Literal::String { value, .. }) => assert_eq!(value, "a\tb"),
        _ => panic!("expected string literal"),
    }
}

#[test]
fn test_parse_string_escape_backslash() {
    let expr = parse_expr_from_source(r#""a\\b""#).unwrap();
    match expr {
        Expr::Literal(Literal::String { value, .. }) => assert_eq!(value, "a\\b"),
        _ => panic!("expected string literal"),
    }
}

#[test]
fn test_parse_string_escape_quote() {
    let expr = parse_expr_from_source(r#""say \"yo\"""#).unwrap();
    match expr {
        Expr::Literal(Literal::String { value, .. }) => assert_eq!(value, "say \"yo\""),
        _ => panic!("expected string literal"),
    }
}

#[test]
fn test_parse_string_escape_multiple() {
    let expr = parse_expr_from_source(r#""a\nb\tc\r\0""#).unwrap();
    match expr {
        Expr::Literal(Literal::String { value, .. }) => assert_eq!(value, "a\nb\tc\r\0"),
        _ => panic!("expected string literal"),
    }
}

#[test]
fn test_parse_string_unknown_escape_preserved() {
    let expr = parse_expr_from_source(r#""a\xb""#).unwrap();
    match expr {
        Expr::Literal(Literal::String { value, .. }) => assert_eq!(value, "a\\xb"),
        _ => panic!("expected string literal"),
    }
}

#[test]
fn test_parse_identifier() {
    let expr = parse_expr_from_source("foo").unwrap();
    assert!(matches!(expr, Expr::Identifier(_)));
}

#[test]
fn test_parse_grouping() {
    let expr = parse_expr_from_source("(5)").unwrap();
    assert!(matches!(expr, Expr::Grouping { .. }));
}

#[test]
fn test_parse_unary_minus() {
    let expr = parse_expr_from_source("-5").unwrap();
    match expr {
        Expr::Unary { op, .. } => assert_eq!(op, UnaryOp::Minus),
        _ => panic!("Expected Unary expression"),
    }
}

#[test]
fn test_parse_unary_plus() {
    let expr = parse_expr_from_source("+5").unwrap();
    match expr {
        Expr::Unary { op, .. } => assert_eq!(op, UnaryOp::Plus),
        _ => panic!("Expected Unary expression"),
    }
}

#[test]
fn test_parse_unary_not() {
    let expr = parse_expr_from_source("!true").unwrap();
    match expr {
        Expr::Unary { op, .. } => assert_eq!(op, UnaryOp::Not),
        _ => panic!("Expected Unary expression"),
    }
}

#[test]
fn test_parse_binary_add() {
    let expr = parse_expr_from_source("2 + 3").unwrap();
    match expr {
        Expr::Binary { op, .. } => assert_eq!(op, BinaryOp::Add),
        _ => panic!("Expected Binary expression"),
    }
}

#[test]
fn test_parse_binary_multiply() {
    let expr = parse_expr_from_source("2 * 3").unwrap();
    match expr {
        Expr::Binary { op, .. } => assert_eq!(op, BinaryOp::Mul),
        _ => panic!("Expected Binary expression"),
    }
}

#[test]
fn test_precedence_mul_over_add() {
    let expr = parse_expr_from_source("2 + 3 * 4").unwrap();
    match expr {
        Expr::Binary { op: BinaryOp::Add, lhs, rhs, .. } => {
            assert!(matches!(*lhs, Expr::Literal(Literal::Number { .. })));
            assert!(matches!(*rhs, Expr::Binary { op: BinaryOp::Mul, .. }));
        }
        _ => panic!("Expected Add at top level with Mul on right"),
    }
}

#[test]
fn test_precedence_parentheses() {
    let expr = parse_expr_from_source("(2 + 3) * 4").unwrap();
    match expr {
        Expr::Binary { op: BinaryOp::Mul, lhs, rhs, .. } => {
            assert!(matches!(*lhs, Expr::Grouping { .. }));
            assert!(matches!(*rhs, Expr::Literal(Literal::Number { .. })));
        }
        _ => panic!("Expected Mul at top level with Grouping on left"),
    }
}

#[test]
fn test_comparison_less() {
    let expr = parse_expr_from_source("x < 5").unwrap();
    match expr {
        Expr::Binary { op, .. } => assert_eq!(op, BinaryOp::Less),
        _ => panic!("Expected Binary expression"),
    }
}

#[test]
fn test_comparison_greater_or_equal() {
    let expr = parse_expr_from_source("x >= 10").unwrap();
    match expr {
        Expr::Binary { op, .. } => assert_eq!(op, BinaryOp::GreaterOrEqual),
        _ => panic!("Expected Binary expression"),
    }
}

#[test]
fn test_logical_and() {
    let expr = parse_expr_from_source("x && y").unwrap();
    match expr {
        Expr::Binary { op, .. } => assert_eq!(op, BinaryOp::And),
        _ => panic!("Expected Binary expression"),
    }
}

#[test]
fn test_logical_or() {
    let expr = parse_expr_from_source("x || y").unwrap();
    match expr {
        Expr::Binary { op, .. } => assert_eq!(op, BinaryOp::Or),
        _ => panic!("Expected Binary expression"),
    }
}

#[test]
fn test_equality() {
    let expr = parse_expr_from_source("x == 5").unwrap();
    match expr {
        Expr::Binary { op, .. } => assert_eq!(op, BinaryOp::Equals),
        _ => panic!("Expected Binary expression"),
    }
}

#[test]
fn test_strict_equality() {
    let expr = parse_expr_from_source("x === 5").unwrap();
    match expr {
        Expr::Binary { op, .. } => assert_eq!(op, BinaryOp::StrictEquals),
        _ => panic!("Expected Binary expression"),
    }
}

#[test]
fn test_complex_expression() {
    let expr = parse_expr_from_source("2 + 3 * 4 - 5 / 2").unwrap();
    assert!(matches!(expr, Expr::Binary { op: BinaryOp::Sub, .. }));
}

#[test]
fn test_precedence_logical_over_comparison() {
    let expr = parse_expr_from_source("x > 5 && y < 10").unwrap();
    match expr {
        Expr::Binary { op: BinaryOp::And, lhs, rhs, .. } => {
            assert!(matches!(*lhs, Expr::Binary { op: BinaryOp::Greater, .. }));
            assert!(matches!(*rhs, Expr::Binary { op: BinaryOp::Less, .. }));
        }
        _ => panic!("Expected And at top level with comparisons as operands"),
    }
}

#[test]
fn test_parse_var_decl_gyy() {
    let source = SourceFile::new("test.yop".to_string(), "гыы x = 5;".to_string());
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
    let source = SourceFile::new("test.yop".to_string(), "внешний: потрещим (правда) { харэ внешний; }".to_string());
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
    let source = SourceFile::new("test.yop".to_string(), "харэ метка;".to_string());
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
    let source = SourceFile::new("test.yop".to_string(), "харэ;".to_string());
    let lexer = yps_lexer::Lexer::new(&source);
    let (tokens, _) = lexer.tokenize();
    let parser = Parser::new(&tokens, &source);
    let (program, diags) = parser.parse_program();
    assert!(diags.is_empty(), "Expected no errors, got: {diags:?}");
    assert!(matches!(program.items[0], Stmt::Break { label: None, .. }));
}

#[test]
fn test_parse_tagged_template() {
    let expr = parse_expr_from_source("тег`привет ${имя}!`").unwrap();
    match expr {
        Expr::TaggedTemplate { tag, quasis, expressions, .. } => {
            assert!(matches!(*tag, Expr::Identifier(_)));
            assert_eq!(quasis.len(), 2);
            assert_eq!(expressions.len(), 1);
            assert_eq!(quasis[0].cooked, "привет ");
            assert_eq!(quasis[1].cooked, "!");
        }
        other => panic!("Expected TaggedTemplate, got: {other:?}"),
    }
}

#[test]
fn test_parse_tagged_template_nosub() {
    let expr = parse_expr_from_source("тег`без подстановок`").unwrap();
    match expr {
        Expr::TaggedTemplate { quasis, expressions, .. } => {
            assert_eq!(quasis.len(), 1);
            assert!(expressions.is_empty());
            assert_eq!(quasis[0].cooked, "без подстановок");
        }
        other => panic!("Expected TaggedTemplate, got: {other:?}"),
    }
}

#[test]
fn test_parse_object_pattern_default() {
    let source = SourceFile::new("test.yop".to_string(), "гыы { х = 5, а: б = 7 } = obj;".to_string());
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
    let source = SourceFile::new("test.yop".to_string(), "гыы [а = 1, б] = arr;".to_string());
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
    let source = SourceFile::new("test.yop".to_string(), "ясенХуй y = \"hello\";".to_string());
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
    let source = SourceFile::new("test.yop".to_string(), "x + 5;".to_string());
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
    let source = SourceFile::new("test.yop".to_string(), ";".to_string());
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
    let source = SourceFile::new("test.yop".to_string(), "гыы x = 5;\nясенХуй y = 10;\nx + y;".to_string());
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
    let source = SourceFile::new("test.yop".to_string(), "вилкойвглаз (x > 5) x = 10;".to_string());
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
    let source = SourceFile::new("test.yop".to_string(), "вилкойвглаз (x > 5) x = 10; иливжопураз x = 0;".to_string());
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
    let source = SourceFile::new("test.yop".to_string(), "вилкойвглаз (x > 5) { x = 10; }".to_string());
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
    let source = SourceFile::new("test.yop".to_string(), "потрещим (x > 0) x = x - 1;".to_string());
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
    let source = SourceFile::new("test.yop".to_string(), "потрещим (x > 0) { x = x - 1; }".to_string());
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
    let source = SourceFile::new("test.yop".to_string(), "потрещим (x > 0) потрещим (y > 0) y = y - 1;".to_string());
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
    let source = SourceFile::new("test.yop".to_string(), "го (гыы i = 0; i < 10; i = i + 1) x = x + i;".to_string());
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
        SourceFile::new("test.yop".to_string(), "го (гыы i = 0; i < 10; i = i + 1) { x = x + i; }".to_string());
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
    let source = SourceFile::new("test.yop".to_string(), "го (; i < 10; i = i + 1) x = x + i;".to_string());
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
    let source = SourceFile::new("test.yop".to_string(), "го (гыы i = 0; ; i = i + 1) x = x + i;".to_string());
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
    let source = SourceFile::new("test.yop".to_string(), "го (гыы i = 0; i < 10;) x = x + i;".to_string());
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
    let source = SourceFile::new("test.yop".to_string(), "го (;;) x = x + 1;".to_string());
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
        "test.yop".to_string(),
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
    let source = SourceFile::new("test.yop".to_string(), "харэ;".to_string());
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
    let source = SourceFile::new("test.yop".to_string(), "двигай;".to_string());
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
    let source = SourceFile::new("test.yop".to_string(), "потрещим (x > 0) { харэ; }".to_string());
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
    let source = SourceFile::new("test.yop".to_string(), "го (гыы i = 0; i < 10; i = i + 1) { двигай; }".to_string());
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
fn test_parse_function_decl() {
    let source = SourceFile::new("test.yop".to_string(), "йопта foo(x, y) { x + y; }".to_string());
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
    let source = SourceFile::new("test.yop".to_string(), "йопта bar() { отвечаю 42; }".to_string());
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
    let source = SourceFile::new("test.yop".to_string(), "отвечаю 42;".to_string());
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
    let source = SourceFile::new("test.yop".to_string(), "отвечаю;".to_string());
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
    let source = SourceFile::new("test.yop".to_string(), "foo(1, 2);".to_string());
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
    let source = SourceFile::new("test.yop".to_string(), "bar();".to_string());
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
    let source = SourceFile::new("test.yop".to_string(), "foo(bar(1), 2);".to_string());
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

#[test]
fn test_parse_ternary_simple() {
    let expr = parse_expr_from_source("правда ? 1 : 2").unwrap();
    assert!(matches!(expr, Expr::Conditional { .. }));
}

#[test]
fn test_parse_ternary_with_comparison() {
    let expr = parse_expr_from_source("x > 5 ? 10 : 20").unwrap();
    match &expr {
        Expr::Conditional { condition, .. } => {
            assert!(matches!(condition.as_ref(), Expr::Binary { .. }));
        }
        _ => panic!("Expected Conditional"),
    }
}

#[test]
fn test_parse_ternary_nested_else() {
    let expr = parse_expr_from_source("a ? 1 : b ? 2 : 3").unwrap();
    match &expr {
        Expr::Conditional { else_expr, .. } => {
            assert!(matches!(else_expr.as_ref(), Expr::Conditional { .. }));
        }
        _ => panic!("Expected nested Conditional"),
    }
}

#[test]
fn test_parse_ternary_missing_colon() {
    let result = parse_expr_from_source("правда ? 1 2");
    assert!(result.is_err());
}

fn parse_program_from_source(src: &str) -> (Program, Vec<Diagnostic>) {
    let source = SourceFile::new("test.yop".to_string(), src.to_string());
    let (tokens, _) = yps_lexer::Lexer::new(&source).tokenize();
    Parser::new(&tokens, &source).parse_program()
}

#[test]
fn test_parse_class_decorator() {
    let (program, diags) = parse_program_from_source("@лог клёво Животное { }");
    assert!(diags.is_empty(), "Parse errors: {diags:?}");
    match &program.items[0] {
        Stmt::ClassDecl { decorators, .. } => assert_eq!(decorators.len(), 1),
        other => panic!("Expected ClassDecl, got {other:?}"),
    }
}

#[test]
fn test_parse_member_decorator() {
    let (program, diags) = parse_program_from_source("клёво Ж { @лог метод() { } }");
    assert!(diags.is_empty(), "Parse errors: {diags:?}");
    match &program.items[0] {
        Stmt::ClassDecl { members, .. } => match &members[0] {
            ClassMember::Method { decorators, .. } => assert_eq!(decorators.len(), 1),
            other => panic!("Expected Method, got {other:?}"),
        },
        other => panic!("Expected ClassDecl, got {other:?}"),
    }
}

#[test]
fn test_parse_multiple_decorators() {
    let (program, diags) = parse_program_from_source("@а @б клёво К { @в @г метод() { } }");
    assert!(diags.is_empty(), "Parse errors: {diags:?}");
    match &program.items[0] {
        Stmt::ClassDecl { decorators, members, .. } => {
            assert_eq!(decorators.len(), 2);
            match &members[0] {
                ClassMember::Method { decorators, .. } => assert_eq!(decorators.len(), 2),
                other => panic!("Expected Method, got {other:?}"),
            }
        }
        other => panic!("Expected ClassDecl, got {other:?}"),
    }
}

#[test]
fn test_parse_decorator_with_args() {
    let (program, diags) = parse_program_from_source("@лог(\"инфо\") клёво К { }");
    assert!(diags.is_empty(), "Parse errors: {diags:?}");
    match &program.items[0] {
        Stmt::ClassDecl { decorators, .. } => {
            assert_eq!(decorators.len(), 1);
            assert!(matches!(decorators[0], Expr::Call { .. }));
        }
        other => panic!("Expected ClassDecl, got {other:?}"),
    }
}

#[test]
fn test_parse_import_default() {
    let (program, diags) = parse_program_from_source(r#"спиздить кент из "./модуль";"#);
    assert!(diags.is_empty(), "Parse errors: {diags:?}");
    match &program.items[0] {
        Stmt::Import { specifiers, source, .. } => {
            assert_eq!(source, "./модуль");
            assert_eq!(specifiers.len(), 1);
            assert!(matches!(&specifiers[0], crate::ast::ImportSpec::Default { local } if local.name == "кент"));
        }
        other => panic!("Expected Import, got {other:?}"),
    }
}

#[test]
fn test_parse_import_named() {
    let (program, diags) = parse_program_from_source(r#"спиздить { foo, bar } из "./м";"#);
    assert!(diags.is_empty(), "Parse errors: {diags:?}");
    match &program.items[0] {
        Stmt::Import { specifiers, source, .. } => {
            assert_eq!(source, "./м");
            assert_eq!(specifiers.len(), 2);
        }
        other => panic!("Expected Import, got {other:?}"),
    }
}

#[test]
fn test_parse_export_named() {
    let (program, diags) = parse_program_from_source(r#"предъява { foo, bar };"#);
    assert!(diags.is_empty(), "Parse errors: {diags:?}");
    match &program.items[0] {
        Stmt::Export { kind: crate::ast::ExportKind::Named(names), .. } => {
            assert_eq!(names.len(), 2);
            assert_eq!(names[0].name, "foo");
            assert_eq!(names[1].name, "bar");
        }
        other => panic!("Expected Export Named, got {other:?}"),
    }
}

#[test]
fn test_parse_export_declaration() {
    let (program, diags) = parse_program_from_source("предъява гыы x = 5;");
    assert!(diags.is_empty(), "Parse errors: {diags:?}");
    match &program.items[0] {
        Stmt::Export { kind: crate::ast::ExportKind::Declaration(decl), .. } => {
            assert!(matches!(decl.as_ref(), Stmt::VarDecl { .. }));
        }
        other => panic!("Expected Export Declaration, got {other:?}"),
    }
}

#[test]
fn test_parse_export_function_decl() {
    let (program, diags) = parse_program_from_source("предъява йопта приветствие() { }");
    assert!(diags.is_empty(), "Parse errors: {diags:?}");
    match &program.items[0] {
        Stmt::Export { kind: crate::ast::ExportKind::Declaration(decl), .. } => {
            assert!(matches!(decl.as_ref(), Stmt::FunctionDecl { .. }));
        }
        other => panic!("Expected Export Declaration, got {other:?}"),
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
fn test_parse_await_expr() {
    let expr = parse_expr_from_source("сидетьНахуй p").unwrap();
    assert!(matches!(expr, Expr::Await { .. }));
}

#[test]
fn test_parse_async_arrow() {
    let expr = parse_expr_from_source("ассо (x) => x").unwrap();
    match expr {
        Expr::ArrowFunction { is_async, .. } => assert!(is_async),
        other => panic!("Expected async ArrowFunction, got {other:?}"),
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
fn test_parser_recovers_from_unknown_keyword_after_brace() {
    let (_, diags) = parse_program_from_source("{ } йопта 5;");
    assert!(!diags.is_empty());
}

#[test]
#[should_panic(expected = "TokenKind::Eof")]
fn parser_new_rejects_empty_tokens() {
    let source = SourceFile::new("test.yop".to_string(), String::new());
    Parser::new(&[], &source);
}

#[test]
#[should_panic(expected = "TokenKind::Eof")]
fn parser_new_rejects_tokens_without_eof() {
    let source = SourceFile::new("test.yop".to_string(), "1".to_string());
    let tokens = vec![Token { kind: TokenKind::Number, span: Span { start: 0, end: 1 } }];
    Parser::new(&tokens, &source);
}

#[test]
fn parser_new_accepts_eof_only_tokens() {
    let source = SourceFile::new("test.yop".to_string(), String::new());
    let tokens = vec![Token { kind: TokenKind::Eof, span: Span { start: 0, end: 0 } }];
    let parser = Parser::new(&tokens, &source);
    let (program, diags) = parser.parse_program();
    assert!(program.items.is_empty());
    assert!(diags.is_empty());
}

fn diag_messages(diags: &[Diagnostic]) -> Vec<&str> {
    diags.iter().map(|d| d.message.as_str()).collect()
}

#[test]
fn diag_unclosed_paren_in_grouping() {
    let (_, diags) = parse_program_from_source("гыы х = (1 + 2;");
    assert!(
        diags.iter().any(|d| d.message.contains("Ожидался ')'")),
        "expected unclosed-paren diagnostic, got: {:?}",
        diag_messages(&diags)
    );
}

#[test]
fn diag_unclosed_bracket_in_array() {
    let (_, diags) = parse_program_from_source("гыы а = [1, 2;");
    assert!(
        diags.iter().any(|d| d.message.contains("Ожидался ']'")),
        "expected unclosed-bracket diagnostic, got: {:?}",
        diag_messages(&diags)
    );
}

#[test]
fn diag_object_missing_colon_after_key() {
    let (_, diags) = parse_program_from_source(r#"гыы о = {"к" 1};"#);
    assert!(
        diags.iter().any(|d| d.message.contains("':'")),
        "expected missing-colon diagnostic, got: {:?}",
        diag_messages(&diags)
    );
}

#[test]
fn diag_var_decl_missing_equals() {
    let (_, diags) = parse_program_from_source("гыы х;");
    assert!(
        diags.iter().any(|d| d.message.contains("'='")),
        "expected missing-equals diagnostic, got: {:?}",
        diag_messages(&diags)
    );
}

#[test]
fn diag_var_decl_missing_semicolon() {
    let (_, diags) = parse_program_from_source("гыы х = 1\nйопта ф() {}");
    assert!(
        diags.iter().any(|d| d.message.contains("';'")),
        "expected missing-semicolon diagnostic, got: {:?}",
        diag_messages(&diags)
    );
}

#[test]
fn diag_if_missing_open_paren() {
    let (_, diags) = parse_program_from_source("вилкойвглаз 1) {}");
    assert!(
        diags.iter().any(|d| d.message.contains("вилкойвглаз")),
        "expected if-open-paren diagnostic, got: {:?}",
        diag_messages(&diags)
    );
}

#[test]
fn diag_while_missing_open_paren() {
    let (_, diags) = parse_program_from_source("потрещим 1) {}");
    assert!(
        diags.iter().any(|d| d.message.contains("потрещим")),
        "expected while-open-paren diagnostic, got: {:?}",
        diag_messages(&diags)
    );
}

#[test]
fn diag_decorator_on_non_class() {
    let (_, diags) = parse_program_from_source("@дек\nгыы х = 1;");
    assert!(
        diags.iter().any(|d| d.message.contains("Декораторы")),
        "expected decorator-on-non-class diagnostic, got: {:?}",
        diag_messages(&diags)
    );
}

#[test]
fn synchronize_does_not_hang_on_missing_semi_in_arrow_block() {
    let (_, diags) = parse_program_from_source("ф(() => { z = 1 });\n");
    assert!(!diags.is_empty(), "ожидалась диагностика на пропущенную ';' в теле стрелки");
}

fn parse_extended(src: &str) -> (Program, Vec<Diagnostic>, bool) {
    let source = SourceFile::new("<test>".to_string(), src.to_string());
    let lexer = yps_lexer::Lexer::new(&source);
    let (tokens, _) = lexer.tokenize();
    Parser::new(&tokens, &source).parse_program_extended()
}

#[test]
fn unexpected_eof_unclosed_block() {
    let (_, diags, eof) = parse_extended("гыы x = {");
    assert!(!diags.is_empty());
    assert!(eof, "незакрытый блок должен давать unexpected_eof=true");
}

#[test]
fn unexpected_eof_unclosed_paren() {
    let (_, diags, eof) = parse_extended("вилкойвглаз (x");
    assert!(!diags.is_empty());
    assert!(eof, "незакрытая скобка должна давать unexpected_eof=true");
}

#[test]
fn unexpected_eof_false_for_mid_error() {
    let (_, diags, eof) = parse_extended("гыы x = ;");
    assert!(!diags.is_empty());
    assert!(!eof, "ошибка в середине не должна давать unexpected_eof=true");
}

#[test]
fn unexpected_eof_false_for_valid() {
    let (_, diags, eof) = parse_extended("гыы x = 1;");
    assert!(diags.is_empty());
    assert!(!eof, "валидная программа не должна давать unexpected_eof=true");
}

#[test]
fn unexpected_eof_false_for_bigint_overflow() {
    let (_, diags, eof) = parse_extended("99999999999999999999999999999999999999999999n");
    assert!(!diags.is_empty(), "ожидалась диагностика BigInt");
    assert!(!eof, "BigInt-переполнение не должно давать unexpected_eof=true");
}

#[test]
fn unexpected_eof_false_for_asso_without_func() {
    let (_, diags, eof) = parse_extended("ассо");
    assert!(!diags.is_empty());
    assert!(!eof, "'ассо' без продолжения не должно давать unexpected_eof=true");
}

#[test]
fn unexpected_eof_true_for_unclosed_if_block() {
    let (_, diags, eof) = parse_extended("вилкойвглаз (х > 5) {");
    assert!(!diags.is_empty());
    assert!(eof, "незакрытый блок if должен давать unexpected_eof=true");
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

#[test]
fn deeply_nested_parens_yield_diagnostic_not_crash() {
    let src = format!("гыы а = {}1{};", "(".repeat(10_000), ")".repeat(10_000));
    let (_, diags) = parse_program_from_source(&src);
    assert!(
        diags.iter().any(|d| d.message.contains("вложенность")),
        "ожидалась диагностика о вложенности: {:?}",
        diag_messages(&diags)
    );
}

#[test]
fn deeply_nested_arrays_yield_diagnostic_not_crash() {
    let src = format!("гыы а = {}1{};", "[".repeat(10_000), "]".repeat(10_000));
    let (_, diags) = parse_program_from_source(&src);
    assert!(
        diags.iter().any(|d| d.message.contains("вложенность")),
        "ожидалась диагностика о вложенности: {:?}",
        diag_messages(&diags)
    );
}

#[test]
fn deeply_nested_blocks_yield_diagnostic_not_crash() {
    let src = format!("{}{}", "вилкойвглаз (правда) { ".repeat(10_000), "}".repeat(10_000));
    let (_, diags) = parse_program_from_source(&src);
    assert!(
        diags.iter().any(|d| d.message.contains("вложенность")),
        "ожидалась диагностика о вложенности: {:?}",
        diag_messages(&diags)
    );
}

#[test]
fn nesting_within_limit_parses_without_diagnostics() {
    let src = format!("гыы а = {}1{};", "(".repeat(100), ")".repeat(100));
    let (_, diags) = parse_program_from_source(&src);
    assert!(diags.is_empty(), "ошибок не ожидается: {:?}", diag_messages(&diags));
}

#[test]
fn very_long_member_chain_yields_diagnostic_not_crash() {
    let src = format!("гыы о = {{}}; о{};", ".х".repeat(50_000));
    let (_, diags) = parse_program_from_source(&src);
    assert!(
        diags.iter().any(|d| d.message.contains("цепочка")),
        "ожидалась диагностика о длине цепочки: {:?}",
        diag_messages(&diags)
    );
}

#[test]
fn very_long_binary_chain_yields_diagnostic_not_crash() {
    let src = format!("гыы а = 1{};", " + 1".repeat(50_000));
    let (_, diags) = parse_program_from_source(&src);
    assert!(
        diags.iter().any(|d| d.message.contains("цепочка")),
        "ожидалась диагностика о длине цепочки: {:?}",
        diag_messages(&diags)
    );
}
