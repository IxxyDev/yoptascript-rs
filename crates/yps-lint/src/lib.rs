mod linter;

use yps_lexer::{Diagnostic, Lexer, Severity, SourceFile, Span};
use yps_parser::Parser;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rule {
    UnusedVariable,
    UnreachableCode,
    ShadowedDeclaration,
}

impl Rule {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::UnusedVariable => "unused-variable",
            Self::UnreachableCode => "unreachable-code",
            Self::ShadowedDeclaration => "shadowed-declaration",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LintSeverity {
    Warning,
    Hint,
}

#[derive(Debug, Clone)]
pub struct LintDiagnostic {
    pub span: Span,
    pub rule: Rule,
    pub severity: LintSeverity,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct LintResult {
    pub diagnostics: Vec<LintDiagnostic>,
    pub parse_errors: Vec<Diagnostic>,
}

#[must_use]
pub fn lint_source(source: &str) -> LintResult {
    let sf = SourceFile::new("<lint>".to_string(), source.to_string());
    let (tokens, lex_diags) = Lexer::new(&sf).tokenize();
    let (program, parse_diags) = Parser::new(&tokens, &sf).parse_program();

    let mut parse_errors = lex_diags;
    parse_errors.extend(parse_diags);

    if parse_errors.iter().any(|d| d.severity == Severity::Error) {
        return LintResult { diagnostics: Vec::new(), parse_errors };
    }

    let diagnostics = linter::lint_program(&program);
    LintResult { diagnostics, parse_errors }
}

#[cfg(test)]
mod tests {
    use super::{LintDiagnostic, Rule, lint_source};

    fn diagnostics(src: &str) -> Vec<LintDiagnostic> {
        let result = lint_source(src);
        assert!(result.parse_errors.is_empty(), "неожиданные ошибки разбора: {:?}", result.parse_errors);
        result.diagnostics
    }

    fn count(src: &str, rule: Rule) -> usize {
        diagnostics(src).iter().filter(|d| d.rule == rule).count()
    }

    fn only(src: &str, rule: Rule) -> usize {
        let all = diagnostics(src);
        let of_rule = all.iter().filter(|d| d.rule == rule).count();
        assert_eq!(all.len(), of_rule, "ожидались только диагностики {:?}, получено: {all:?}", rule.code());
        of_rule
    }

    #[test]
    fn parse_error_yields_no_lint() {
        let result = lint_source("гыы = ;\n");
        assert!(!result.parse_errors.is_empty());
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn rule_codes_are_stable() {
        assert_eq!(Rule::UnusedVariable.code(), "unused-variable");
        assert_eq!(Rule::UnreachableCode.code(), "unreachable-code");
        assert_eq!(Rule::ShadowedDeclaration.code(), "shadowed-declaration");
    }

    #[test]
    fn unused_fires_on_toplevel_var() {
        assert_eq!(only("гыы х = 1;\n", Rule::UnusedVariable), 1);
    }

    #[test]
    fn unused_fires_on_const() {
        assert_eq!(only("ясенХуй к = 5;\n", Rule::UnusedVariable), 1);
    }

    #[test]
    fn unused_fires_on_local_var() {
        let src = "йопта ф() { гыы у = 2; отвечаю 1; }\n";
        assert_eq!(count(src, Rule::UnusedVariable), 1);
    }

    #[test]
    fn unused_fires_on_sole_param() {
        let src = "йопта ф(а) { отвечаю 1; }\n";
        assert_eq!(count(src, Rule::UnusedVariable), 1);
    }

    #[test]
    fn unused_fires_on_trailing_param() {
        let src = "йопта ф(а, б) { отвечаю а; }\n";
        assert_eq!(count(src, Rule::UnusedVariable), 1);
    }

    #[test]
    fn unused_fires_on_write_only_var() {
        let src = "гыы х = 1;\nх = 2;\n";
        assert_eq!(only(src, Rule::UnusedVariable), 1);
    }

    #[test]
    fn unused_fires_on_destructured_param_before_used_param() {
        let src = "йопта ф({а}, б) { отвечаю б; }\nсказать(ф({а: 1}, 2));\n";
        assert_eq!(count(src, Rule::UnusedVariable), 1);
    }

    #[test]
    fn unused_silent_on_compound_assignment() {
        let src = "гыы х = 1;\nх += 2;\n";
        assert_eq!(count(src, Rule::UnusedVariable), 0);
    }

    #[test]
    fn unused_silent_when_read() {
        assert_eq!(count("гыы х = 1;\nсказать(х);\n", Rule::UnusedVariable), 0);
    }

    #[test]
    fn unused_silent_when_read_in_closure() {
        let src = "гыы счёт = 0;\nйопта инк() { счёт++; }\nсказать(инк());\n";
        assert_eq!(count(src, Rule::UnusedVariable), 0);
    }

    #[test]
    fn unused_silent_when_read_in_template() {
        let src = "гыы имя = \"мир\";\nсказать(`привет ${имя}`);\n";
        assert_eq!(count(src, Rule::UnusedVariable), 0);
    }

    #[test]
    fn unused_silent_for_used_destructuring() {
        let src = "гыы [а, б] = список;\nсказать(а);\nсказать(б);\n";
        assert_eq!(count(src, Rule::UnusedVariable), 0);
    }

    #[test]
    fn unused_silent_for_underscore_prefix() {
        assert_eq!(count("гыы _х = 1;\n", Rule::UnusedVariable), 0);
    }

    #[test]
    fn unused_silent_for_param_before_used() {
        let src = "йопта ф(а, б) { отвечаю б; }\n";
        assert_eq!(count(src, Rule::UnusedVariable), 0);
    }

    #[test]
    fn unused_silent_for_param_read_in_default() {
        let src = "йопта ф(а, б = а) { отвечаю б; }\n";
        assert_eq!(count(src, Rule::UnusedVariable), 0);
    }

    #[test]
    fn unused_silent_for_exported_named() {
        let src = "гыы а = 1;\nпредъява { а };\n";
        assert_eq!(count(src, Rule::UnusedVariable), 0);
    }

    #[test]
    fn unused_silent_for_exported_declaration() {
        let src = "предъява гыы б = 2;\n";
        assert_eq!(count(src, Rule::UnusedVariable), 0);
    }

    #[test]
    fn unused_silent_for_rest_param() {
        let src = "йопта ф(...остаток) { отвечаю 1; }\n";
        assert_eq!(count(src, Rule::UnusedVariable), 0);
    }

    #[test]
    fn unreachable_fires_after_return() {
        let src = "йопта ф() { отвечаю 1; сказать(2); }\n";
        assert_eq!(count(src, Rule::UnreachableCode), 1);
    }

    #[test]
    fn unreachable_fires_after_throw() {
        let src = "йопта ф() { кидай 1; сказать(2); }\n";
        assert_eq!(count(src, Rule::UnreachableCode), 1);
    }

    #[test]
    fn unreachable_fires_after_break() {
        let src = "потрещим (правда) { харэ; сказать(2); }\n";
        assert_eq!(count(src, Rule::UnreachableCode), 1);
    }

    #[test]
    fn unreachable_fires_after_continue() {
        let src = "го (;;) { двигай; сказать(1); }\n";
        assert_eq!(count(src, Rule::UnreachableCode), 1);
    }

    #[test]
    fn unreachable_silent_for_normal_flow() {
        let src = "йопта ф() { сказать(1); отвечаю 2; }\n";
        assert_eq!(count(src, Rule::UnreachableCode), 0);
    }

    #[test]
    fn unreachable_silent_for_tail_function_decl() {
        let src = "йопта ф() { отвечаю 1; йопта г() { отвечаю 2; } }\n";
        assert_eq!(count(src, Rule::UnreachableCode), 0);
    }

    #[test]
    fn unreachable_silent_for_return_in_branch() {
        let src = "йопта ф(х) { вилкойвглаз (х) { отвечаю 1; } сказать(2); }\n";
        assert_eq!(count(src, Rule::UnreachableCode), 0);
    }

    #[test]
    fn unreachable_silent_for_trailing_empty() {
        let src = "йопта ф() { отвечаю 1; ; }\n";
        assert_eq!(count(src, Rule::UnreachableCode), 0);
    }

    #[test]
    fn shadow_fires_for_nested_var() {
        let src = "гыы х = 1;\nсказать(х);\nйопта ф() { гыы х = 2; сказать(х); }\n";
        assert_eq!(count(src, Rule::ShadowedDeclaration), 1);
    }

    #[test]
    fn shadow_fires_for_param() {
        let src = "гыы у = 1;\nсказать(у);\nйопта ф(у) { сказать(у); }\n";
        assert_eq!(count(src, Rule::ShadowedDeclaration), 1);
    }

    #[test]
    fn shadow_fires_for_block() {
        let src = "гыы з = 1;\n{ гыы з = 2; сказать(з); }\nсказать(з);\n";
        assert_eq!(count(src, Rule::ShadowedDeclaration), 1);
    }

    #[test]
    fn shadow_fires_for_catch_param() {
        let src = "гыы е = 1;\nсказать(е);\nхапнуть { сказать(1); } гоп (е) { сказать(е); }\n";
        assert_eq!(count(src, Rule::ShadowedDeclaration), 1);
    }

    #[test]
    fn shadow_fires_for_loop_header() {
        let src = "гыы и = 1;\nсказать(и);\nго (гыы и = 0; и < 3; и++) { сказать(и); }\n";
        assert_eq!(count(src, Rule::ShadowedDeclaration), 1);
    }

    #[test]
    fn shadow_silent_for_sibling_blocks() {
        let src = "{ гыы к = 1; сказать(к); }\n{ гыы к = 2; сказать(к); }\n";
        assert_eq!(count(src, Rule::ShadowedDeclaration), 0);
    }

    #[test]
    fn shadow_silent_for_builtin_name() {
        let src = "гыы длина = 5;\nсказать(длина);\n";
        assert_eq!(count(src, Rule::ShadowedDeclaration), 0);
    }

    #[test]
    fn shadow_silent_without_outer_binding() {
        let src = "йопта ф() { гыы м = 1; сказать(м); }\n";
        assert_eq!(count(src, Rule::ShadowedDeclaration), 0);
    }

    #[test]
    fn shadow_silent_for_distinct_names() {
        let src = "гыы а = 1;\nсказать(а);\nйопта ф() { гыы б = 2; сказать(б); }\n";
        assert_eq!(count(src, Rule::ShadowedDeclaration), 0);
    }
}
