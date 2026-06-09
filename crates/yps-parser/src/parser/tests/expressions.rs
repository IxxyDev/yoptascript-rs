use super::*;

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
