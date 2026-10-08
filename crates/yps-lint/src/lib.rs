//! Статический линтер программ `.yopta`: обходит AST из `yps-parser` и выдаёт [`LintDiagnostic`].
//!
//! Правила (код — серьёзность — что проверяется):
//!
//! * `unused-variable` — предупреждение — переменная (`гыы`/`участковый`), константа
//!   (`ясенХуй`, в том числе переменная цикла `го (ясенХуй … сашаГрей …)`) или параметр
//!   функции объявлены, но ни разу не прочитаны. Запись (`х = 1`, `х += 1`, `х++` как
//!   отдельная инструкция) чтением не считается.
//! * `unreachable-code` — предупреждение — первая инструкция после `отвечаю`, `кидай`, `харэ`
//!   или `двигай` в том же списке инструкций (объявления функций и пустые инструкции не в счёт).
//! * `shadowed-declaration` — подсказка — объявление затеняет имя из внешней области.
//!   Имя функционального выражения не затеняет ничего снаружи (`гыы ф = йопта ф() { … }`).
//! * `unused-import` — предупреждение — импортированное имя ни разу не прочитано.
//! * `duplicate-object-key` — предупреждение — ключ-идентификатор повторяется в объектном
//!   литерале; геттер и сеттер с одним ключом — не повтор, вычисляемые ключи не проверяются.
//! * `self-assignment` — предупреждение — `х = х`, `х ??= х`, `х ||= х`, `х &&= х`.
//! * `duplicate-param` — предупреждение — простой параметр повторяется в списке параметров.
//!
//! Области видимости повторяют рантайм. Повторное объявление имени в той же области — это
//! та же самая привязка, а не новая: `гыы х = 1; гыы х = х + 1;` читает и пишет один слот.
//! Параметры и тело функции — одна область, параметр `гоп` и его блок — тоже, поэтому
//! `гыы` в теле с именем параметра — не затенение, а та же привязка. Если имя сначала
//! объявлено без проверки на неиспользование (функция, класс, параметр `гоп`, имя
//! функционального выражения), а потом переобъявлено через `гыы`/`ясенХуй` или параметром,
//! привязка проверяется как переобъявленная.
//!
//! Имена, начинающиеся с `_`, никогда не считаются неиспользованными. Экспортированные
//! объявления (`предъява гыы …`, `предъява { … }`) тоже.
//!
//! Неиспользованные параметры считаются по правилу «после последнего использованного»:
//! простой параметр левее прочитанного не репортится (его нельзя убрать, не сломав позиции
//! остальных), rest-параметр не репортится никогда, а имена из деструктурирующего параметра
//! репортятся всегда.
//!
//! Если лексер или парсер выдали хотя бы одну ошибку, [`lint_source`] возвращает их и ничего
//! не линтит.

mod linter;

use yps_lexer::{Diagnostic, Lexer, SourceFile, Span};
use yps_parser::{Parser, Program};

/// Правило линтера. Стабильный машиночитаемый идентификатор — [`Rule::code`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Rule {
    UnusedVariable,
    UnreachableCode,
    ShadowedDeclaration,
    UnusedImport,
    DuplicateObjectKey,
    SelfAssignment,
    DuplicateParam,
}

impl Rule {
    pub const ALL: [Self; 7] = [
        Self::UnusedVariable,
        Self::UnreachableCode,
        Self::ShadowedDeclaration,
        Self::UnusedImport,
        Self::DuplicateObjectKey,
        Self::SelfAssignment,
        Self::DuplicateParam,
    ];

    #[must_use]
    pub const fn severity(self) -> LintSeverity {
        match self {
            Self::ShadowedDeclaration => LintSeverity::Hint,
            Self::UnusedVariable
            | Self::UnreachableCode
            | Self::UnusedImport
            | Self::DuplicateObjectKey
            | Self::SelfAssignment
            | Self::DuplicateParam => LintSeverity::Warning,
        }
    }

    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::UnusedVariable => "unused-variable",
            Self::UnreachableCode => "unreachable-code",
            Self::ShadowedDeclaration => "shadowed-declaration",
            Self::UnusedImport => "unused-import",
            Self::DuplicateObjectKey => "duplicate-object-key",
            Self::SelfAssignment => "self-assignment",
            Self::DuplicateParam => "duplicate-param",
        }
    }
}

/// Серьёзность диагностики линтера; у каждого правила она фиксирована ([`Rule::severity`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LintSeverity {
    Warning,
    Hint,
}

impl std::fmt::Display for LintSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Warning => "Предупреждение",
            Self::Hint => "Подсказка",
        })
    }
}

/// Одна находка линтера: байтовый диапазон в исходнике, правило, его серьёзность и
/// сообщение на русском.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LintDiagnostic {
    pub span: Span,
    pub rule: Rule,
    pub severity: LintSeverity,
    pub message: String,
}

/// Линтит уже разобранную программу. Диагностики отсортированы по началу диапазона.
#[must_use]
pub fn lint_program(program: &Program) -> Vec<LintDiagnostic> {
    linter::lint_program(program)
}

/// Лексит, парсит и линтит исходник.
///
/// # Errors
///
/// Возвращает диагностики лексера и парсера, если они есть; в этом случае линтинг не
/// выполняется.
pub fn lint_source(source: &str) -> Result<Vec<LintDiagnostic>, Vec<Diagnostic>> {
    let sf = SourceFile::new("<lint>".to_string(), source.to_string());
    let (tokens, mut diags) = Lexer::new(&sf).tokenize();
    let (program, parse_diags) = Parser::new(&tokens, &sf).parse_program();
    diags.extend(parse_diags);

    if diags.is_empty() { Ok(lint_program(&program)) } else { Err(diags) }
}

#[cfg(test)]
mod tests {
    use super::{LintDiagnostic, LintSeverity, Rule, lint_source};

    #[test]
    fn severity_displays_in_russian() {
        assert_eq!(LintSeverity::Warning.to_string(), "Предупреждение");
        assert_eq!(LintSeverity::Hint.to_string(), "Подсказка");
    }

    fn diagnostics(src: &str) -> Vec<LintDiagnostic> {
        lint_source(src).unwrap_or_else(|errors| panic!("неожиданные ошибки разбора: {errors:?}"))
    }

    fn of_rule(src: &str, rule: Rule) -> Vec<LintDiagnostic> {
        diagnostics(src).into_iter().filter(|d| d.rule == rule).collect()
    }

    fn count(src: &str, rule: Rule) -> usize {
        of_rule(src, rule).len()
    }

    fn spans_of(src: &str, rule: Rule) -> Vec<&str> {
        of_rule(src, rule).iter().map(|d| &src[d.span.start..d.span.end]).collect()
    }

    fn messages(src: &str, rule: Rule) -> Vec<String> {
        of_rule(src, rule).into_iter().map(|d| d.message).collect()
    }

    fn assert_silent(src: &str) {
        let diags = diagnostics(src);
        assert!(diags.is_empty(), "ожидалась тишина, получено: {diags:?}");
    }

    fn only(src: &str, rule: Rule) -> usize {
        let all = diagnostics(src);
        let of_rule = all.iter().filter(|d| d.rule == rule).count();
        assert_eq!(all.len(), of_rule, "ожидались только диагностики {:?}, получено: {all:?}", rule.code());
        of_rule
    }

    #[test]
    fn parse_error_yields_no_lint() {
        assert!(lint_source("гыы = ;\n").is_err());
    }

    #[test]
    fn rule_codes_are_stable() {
        let codes: Vec<&str> = Rule::ALL.iter().map(|rule| rule.code()).collect();
        assert_eq!(
            codes,
            [
                "unused-variable",
                "unreachable-code",
                "shadowed-declaration",
                "unused-import",
                "duplicate-object-key",
                "self-assignment",
                "duplicate-param",
            ]
        );
        let unique: std::collections::HashSet<&str> = codes.iter().copied().collect();
        assert_eq!(unique.len(), codes.len());
    }

    #[test]
    fn severity_table() {
        assert_eq!(Rule::ShadowedDeclaration.severity(), LintSeverity::Hint);
        for rule in Rule::ALL.into_iter().filter(|rule| *rule != Rule::ShadowedDeclaration) {
            assert_eq!(rule.severity(), LintSeverity::Warning, "{rule:?}");
        }
    }

    #[test]
    fn lexer_error_yields_no_lint() {
        let errors = lint_source("гыы х = \"abc;\n").expect_err("ожидалась ошибка лексера");
        assert!(errors.iter().any(|e| e.message.contains("Незакрытая строка")), "{errors:?}");
    }

    #[test]
    fn diagnostics_are_sorted_by_span() {
        let src = "йопта ф(а) { отвечаю 1; сказать(2); }\nгыы о = {к: 1, к: 2};\nгыы х = 1;\nх = х;\nгыы у = 1;\n";
        let diags = diagnostics(src);
        assert!(diags.len() >= 4, "{diags:?}");
        let spans: Vec<_> = diags.iter().map(|d| (d.span.start, d.span.end)).collect();
        let mut sorted = spans.clone();
        sorted.sort_unstable();
        assert_eq!(spans, sorted);
    }

    #[test]
    fn unused_spans_cover_identifier_only() {
        assert_eq!(spans_of("гыы хвост = 1;\n", Rule::UnusedVariable), ["хвост"]);
        assert_eq!(spans_of("йопта ф(арг) { отвечаю 1; }\n", Rule::UnusedVariable), ["арг"]);
        assert_eq!(spans_of("спиздить кент из \"./модуль\";\n", Rule::UnusedImport), ["кент"]);
    }

    #[test]
    fn unreachable_span_covers_first_dead_statement() {
        let src = "йопта ф() { отвечаю 1; сказать(2); сказать(3); }\n";
        let spans = spans_of(src, Rule::UnreachableCode);
        assert_eq!(spans.len(), 1);
        assert!(spans[0].starts_with("сказать(2)"), "{spans:?}");
    }

    #[test]
    fn duplicate_spans_point_at_second_occurrence() {
        let src = "гыы о = {ключ: 1, ключ: 2};\nсказать(о);\n";
        let diags = diagnostics(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].span.start, src.rfind("ключ").expect("второй ключ"));
        assert_eq!(&src[diags[0].span.start..diags[0].span.end], "ключ");

        let src = "йопта ф(арг, арг) { отвечаю арг; }\n";
        let diags = diagnostics(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].span.start, src.find(", арг").expect("второй параметр") + 2);
        assert_eq!(&src[diags[0].span.start..diags[0].span.end], "арг");
    }

    #[test]
    fn self_assignment_span_covers_whole_expression() {
        assert_eq!(spans_of("гыы х = 1;\nх = х;\n", Rule::SelfAssignment), ["х = х"]);
        assert_eq!(spans_of("гыы х = 1;\nх ??= х;\nсказать(х);\n", Rule::SelfAssignment), ["х ??= х"]);
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
        let diags = diagnostics(src);
        let unused: Vec<_> = diags.iter().filter(|d| d.rule == Rule::UnusedVariable).collect();
        assert_eq!(unused.len(), 1);
        assert!(unused[0].message.contains('а'), "ожидалась диагностика для «а», получено: {}", unused[0].message);
        assert!(!unused[0].message.contains('б'), "не ожидалась диагностика для «б», получено: {}", unused[0].message);
    }

    #[test]
    fn unused_fires_on_compound_assignment_write_only() {
        let src = "гыы х = 1;\nх += 2;\n";
        assert_eq!(only(src, Rule::UnusedVariable), 1);
    }

    #[test]
    fn unused_silent_when_read() {
        assert_silent("гыы х = 1;\nсказать(х);\n");
    }

    #[test]
    fn unused_fires_on_postfix_write_only() {
        let src = "гыы счёт = 0;\nйопта инк() { счёт++; }\nсказать(инк());\n";
        assert_eq!(count(src, Rule::UnusedVariable), 1);
    }

    #[test]
    fn unused_silent_when_read_in_closure() {
        let src = "гыы счёт = 0;\nйопта инк() { отвечаю счёт; }\nсказать(инк());\n";
        assert_silent(src);
    }

    #[test]
    fn unused_silent_when_read_in_template() {
        let src = "гыы имя = \"мир\";\nсказать(`привет ${имя}`);\n";
        assert_silent(src);
    }

    #[test]
    fn unused_silent_for_used_destructuring() {
        let src = "гыы [а, б] = список;\nсказать(а);\nсказать(б);\n";
        assert_silent(src);
    }

    #[test]
    fn unused_silent_for_underscore_prefix() {
        assert_silent("гыы _х = 1;\n");
    }

    #[test]
    fn unused_silent_for_param_before_used() {
        let src = "йопта ф(а, б) { отвечаю б; }\n";
        assert_silent(src);
    }

    #[test]
    fn unused_silent_for_param_read_in_default() {
        let src = "йопта ф(а, б = а) { отвечаю б; }\n";
        assert_silent(src);
    }

    #[test]
    fn unused_silent_for_exported_named() {
        let src = "гыы а = 1;\nпредъява { а };\n";
        assert_silent(src);
    }

    #[test]
    fn unused_silent_for_exported_declaration() {
        let src = "предъява гыы б = 2;\n";
        assert_silent(src);
    }

    #[test]
    fn unused_silent_for_rest_param() {
        let src = "йопта ф(...остаток) { отвечаю 1; }\n";
        assert_silent(src);
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
        assert_silent(src);
    }

    #[test]
    fn unreachable_silent_for_tail_function_decl() {
        let src = "йопта ф() { отвечаю 1; йопта г() { отвечаю 2; } }\n";
        assert_silent(src);
    }

    #[test]
    fn unreachable_silent_for_return_in_branch() {
        let src = "йопта ф(х) { вилкойвглаз (х) { отвечаю 1; } сказать(2); }\n";
        assert_silent(src);
    }

    #[test]
    fn unreachable_silent_for_trailing_empty() {
        let src = "йопта ф() { отвечаю 1; ; }\n";
        assert_silent(src);
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
        assert_silent(src);
    }

    #[test]
    fn shadow_silent_without_outer_binding() {
        let src = "йопта ф() { гыы м = 1; сказать(м); }\n";
        assert_silent(src);
    }

    #[test]
    fn shadow_silent_for_distinct_names() {
        let src = "гыы а = 1;\nсказать(а);\nйопта ф() { гыы б = 2; сказать(б); }\n";
        assert_silent(src);
    }

    #[test]
    fn unused_import_fires_for_unread_default() {
        let src = "спиздить кент из \"./модуль\";\n";
        assert_eq!(only(src, Rule::UnusedImport), 1);
    }

    #[test]
    fn unused_import_fires_for_unread_named() {
        let src = "спиздить { фу, бар } из \"./м\";\nсказать(фу);\n";
        assert_eq!(only(src, Rule::UnusedImport), 1);
    }

    #[test]
    fn unused_import_silent_when_read() {
        let src = "спиздить кент из \"./модуль\";\nсказать(кент);\n";
        assert_silent(src);
    }

    #[test]
    fn unused_import_silent_when_reexported() {
        let src = "спиздить кент из \"./модуль\";\nпредъява { кент };\n";
        assert_silent(src);
    }

    #[test]
    fn duplicate_object_key_fires_for_repeated_identifier() {
        let src = "гыы о = {а: 1, а: 2};\nсказать(о);\n";
        assert_eq!(only(src, Rule::DuplicateObjectKey), 1);
    }

    #[test]
    fn duplicate_object_key_fires_once_per_extra_occurrence() {
        let src = "гыы о = {а: 1, а: 2, а: 3};\nсказать(о);\n";
        assert_eq!(count(src, Rule::DuplicateObjectKey), 2);
    }

    #[test]
    fn duplicate_object_key_silent_for_distinct_keys() {
        let src = "гыы о = {а: 1, б: 2};\nсказать(о);\n";
        assert_silent(src);
    }

    #[test]
    fn duplicate_object_key_silent_for_nested_literals() {
        let src = "гыы о = {а: {б: 1}, в: {б: 2}};\nсказать(о);\n";
        assert_silent(src);
    }

    #[test]
    fn duplicate_object_key_silent_for_computed_key() {
        let src = "гыы к = \"а\";\nгыы о = {[к]: 1, [к]: 2};\nсказать(о);\n";
        assert_silent(src);
    }

    #[test]
    fn self_assignment_fires_for_same_identifier() {
        let src = "гыы х = 1;\nх = х;\n";
        assert_eq!(only(src, Rule::SelfAssignment), 1);
    }

    #[test]
    fn self_assignment_silent_for_different_identifier() {
        let src = "гыы х = 1;\nгыы у = 2;\nх = у;\nсказать(х);\nсказать(у);\n";
        assert_silent(src);
    }

    #[test]
    fn self_assignment_silent_for_member_expression() {
        let src = "гыы о = {х: 1};\nо.х = о.х;\nсказать(о);\n";
        assert_silent(src);
    }

    #[test]
    fn self_assignment_silent_for_compound_assign() {
        let src = "гыы х = 1;\nх += х;\nсказать(х);\n";
        assert_silent(src);
    }

    #[test]
    fn duplicate_param_fires_for_repeated_name() {
        let src = "йопта ф(а, а) { отвечаю а; }\n";
        assert_eq!(only(src, Rule::DuplicateParam), 1);
    }

    #[test]
    fn duplicate_param_silent_for_distinct_names() {
        let src = "йопта ф(а, б) { отвечаю а + б; }\n";
        assert_silent(src);
    }

    #[test]
    fn duplicate_param_silent_for_destructured_params() {
        let src = "йопта ф({а}, {а: б}) { отвечаю б; }\n";
        assert_eq!(count(src, Rule::DuplicateParam), 0);
    }

    #[test]
    fn duplicate_param_fires_once_per_extra_occurrence() {
        let src = "йопта ф(а, а, а) { отвечаю а; }\n";
        assert_eq!(count(src, Rule::DuplicateParam), 2);
    }

    #[test]
    fn duplicate_param_silent_for_rest_param() {
        let src = "йопта ф(а, ...а) { отвечаю а; }\n";
        assert_silent(src);
    }

    #[test]
    fn deep_nested_chains_do_not_overflow_small_stack() {
        let depth = 8;
        let terms = 1000;
        let mut src = String::from("гыы х = ");
        src.push_str(&"(".repeat(depth));
        src.push('1');
        for _ in 0..depth {
            src.push_str(&"+1".repeat(terms));
            src.push(')');
        }
        src.push_str(";\nсказать(х);\n");
        let handle = std::thread::Builder::new()
            .stack_size(2 * 1024 * 1024)
            .spawn(move || lint_source(&src).map(|diags| diags.len()))
            .expect("поток линтера");
        let result = handle.join().expect("линтер не должен падать");
        assert_eq!(result.expect("цепочка должна разбираться"), 0);
    }

    #[test]
    fn switch_cases_have_separate_scopes() {
        let src = "базарпо (1) { тема 1: { гыы б = 1; } тема 2: { сказать(б); } }\n";
        assert_eq!(only(src, Rule::UnusedVariable), 1);
    }

    #[test]
    fn switch_cases_report_each_declaration_independently() {
        let src = "базарпо (1) { тема 1: { гыы а = 1; } тема 2: { гыы а = 2; } }\n";
        assert_eq!(only(src, Rule::UnusedVariable), 2);
        let src = "базарпо (1) { тема 1: { гыы а = 1; сказать(а); } тема 2: { гыы а = 2; } }\n";
        assert_eq!(only(src, Rule::UnusedVariable), 1);
    }

    #[test]
    fn body_local_redeclaring_param_is_the_same_binding() {
        let src = "йопта ф(а) { гыы а = 5; отвечаю 1; }\nсказать(ф(1));\n";
        assert_eq!(messages(src, Rule::UnusedVariable), ["параметр «а» не используется"]);
        assert_eq!(count(src, Rule::ShadowedDeclaration), 0);
    }

    #[test]
    fn body_local_reading_param_of_same_name_is_silent() {
        assert_silent("йопта ф(а) { гыы а = а + 1; отвечаю а; }\nсказать(ф(1));\n");
    }

    #[test]
    fn var_redeclaring_function_adopts_var_kind() {
        let src = "йопта ф() { отвечаю 1; }\nгыы ф = 1;\n";
        assert_eq!(messages(src, Rule::UnusedVariable), ["переменная «ф» объявлена, но не используется"]);
    }

    #[test]
    fn body_local_redeclaring_catch_param_is_reported() {
        let src = "хапнуть { сказать(1); } гоп (е) { гыы е = 2; }\n";
        assert_eq!(messages(src, Rule::UnusedVariable), ["переменная «е» объявлена, но не используется"]);
    }

    #[test]
    fn param_named_like_function_expression_is_reported() {
        let src = "гыы ф = йопта г(г) { отвечаю 1; };\nсказать(ф(1));\n";
        assert_eq!(messages(src, Rule::UnusedVariable), ["параметр «г» не используется"]);
        assert_eq!(count(src, Rule::ShadowedDeclaration), 0);
    }

    #[test]
    fn repeated_declaration_is_one_binding() {
        let src = "гыы х = 1;\nгыы х = 2;\n";
        assert_eq!(only(src, Rule::UnusedVariable), 1);
        assert_eq!(spans_of(src, Rule::UnusedVariable), ["х"]);
        assert_eq!(diagnostics(src)[0].span.start, src.find('х').expect("первое объявление"));
        assert_silent("гыы х = 1;\nсказать(х);\nгыы х = 2;\n");
    }

    #[test]
    fn closure_between_redeclarations_is_silent() {
        assert_silent("гыы х = 1;\nгыы ф = () => х;\nгыы х = 2;\nсказать(ф());\n");
    }

    #[test]
    fn redeclaration_reading_previous_value_is_silent() {
        assert_silent("гыы к = 1;\nгыы к = к + 1;\nсказать(к);\n");
    }

    #[test]
    fn destructuring_swap_is_silent() {
        assert_silent("гыы а = 1;\nгыы б = 2;\nгыы [а, б] = [б, а];\nсказать(а, б);\n");
    }

    #[test]
    fn shadow_fires_for_nested_function_param_in_default() {
        let src = "йопта ф(а, б = йопта(а) { отвечаю а; }) { отвечаю а + б(1); }\nсказать(ф(1));\n";
        assert_eq!(only(src, Rule::ShadowedDeclaration), 1);
    }

    #[test]
    fn shadow_silent_for_function_expression_named_like_its_variable() {
        let src = "гыы ф = йопта ф(н) { отвечаю н; };\nсказать(ф(1));\n";
        assert_silent(src);
    }

    #[test]
    fn duplicate_object_key_fires_for_getter_and_property() {
        let src = "гыы о = { get а() { отвечаю 1; }, а: 2 };\nсказать(о);\n";
        assert_eq!(only(src, Rule::DuplicateObjectKey), 1);
    }

    #[test]
    fn duplicate_object_key_fires_for_repeated_getter() {
        let src = "гыы о = { get а() { отвечаю 1; }, get а() { отвечаю 2; } };\nсказать(о);\n";
        assert_eq!(only(src, Rule::DuplicateObjectKey), 1);
    }

    #[test]
    fn duplicate_object_key_silent_for_getter_setter_pair() {
        let src = "гыы о = { get а() { отвечаю 1; }, set а(з) { сказать(з); } };\nсказать(о);\n";
        assert_silent(src);
    }

    #[test]
    fn self_assignment_fires_for_logical_assign() {
        let src = "гыы х = 1;\nх ??= х;\nх ||= х;\nх &&= х;\nсказать(х);\n";
        assert_eq!(only(src, Rule::SelfAssignment), 3);
    }

    #[test]
    fn const_loop_variable_is_reported_as_constant() {
        let src = "го (ясенХуй к сашаГрей [1]) { }\n";
        assert_eq!(messages(src, Rule::UnusedVariable), ["константа «к» объявлена, но не используется"]);
    }
}
