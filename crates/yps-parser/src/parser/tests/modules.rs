use super::*;

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
