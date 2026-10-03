use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use yps_lexer::Span;
use yps_parser::ast::Program;

use crate::environment::EnvFrame;
use crate::error::RuntimeError;
use crate::stdlib::json;
use crate::value::Value;

use super::Interpreter;

pub(crate) type ExportCell = Rc<RefCell<HashMap<String, Value>>>;

pub(crate) enum ModuleState {
    Loading(ExportCell),
    Loaded(HashMap<String, Value>),
}

impl ModuleState {
    pub(crate) fn exports_snapshot(&self) -> HashMap<String, Value> {
        match self {
            ModuleState::Loading(cell) => cell.borrow().clone(),
            ModuleState::Loaded(e) => e.clone(),
        }
    }

    pub(crate) fn for_each_export_value(&self, mut f: impl FnMut(&Value)) {
        match self {
            ModuleState::Loading(cell) => cell.borrow().values().for_each(&mut f),
            ModuleState::Loaded(e) => e.values().for_each(&mut f),
        }
    }
}

pub(crate) struct DeferredLink {
    pub(crate) module: PathBuf,
    pub(crate) target_env: Rc<RefCell<EnvFrame>>,
    pub(crate) local: String,
    pub(crate) imported: String,
}

fn described(source: &yps_lexer::SourceFile, diagnostics: &[yps_lexer::Diagnostic]) -> String {
    diagnostics.iter().map(|diagnostic| format!("\n  {}", source.describe(diagnostic))).collect()
}

impl Interpreter {
    fn resolve_module_path(&self, source: &str, span: Span) -> Result<PathBuf, RuntimeError> {
        let base = self.base_path.clone().unwrap_or_else(|| PathBuf::from("."));
        let mut candidate = base.join(source);
        if candidate.extension().is_none() {
            candidate.set_extension("yopta");
        }
        candidate
            .canonicalize()
            .map_err(|e| RuntimeError::new(format!("Не удалось разрешить путь модуля '{source}': {e}"), span))
    }

    pub(super) fn load_json_module(
        &mut self,
        source: &str,
        span: Span,
    ) -> Result<HashMap<String, Value>, RuntimeError> {
        let base = self.base_path.clone().unwrap_or_else(|| PathBuf::from("."));
        let resolved = base
            .join(source)
            .canonicalize()
            .map_err(|e| RuntimeError::new(format!("Не удалось разрешить путь модуля '{source}': {e}"), span))?;

        if let Some(state) = self.module_cache.borrow().get(&resolved) {
            return Ok(state.exports_snapshot());
        }

        let code = std::fs::read_to_string(&resolved).map_err(|e| {
            RuntimeError::new(format!("Не удалось прочитать JSON модуль '{}': {e}", resolved.display()), span)
        })?;
        let value = json::parse_str(&code, span)?;
        let mut exports = HashMap::new();
        exports.insert("default".to_string(), value);
        self.module_cache.borrow_mut().insert(resolved, ModuleState::Loaded(exports.clone()));
        Ok(exports)
    }

    pub(super) fn load_module(&mut self, source: &str, span: Span) -> Result<HashMap<String, Value>, RuntimeError> {
        let resolved = self.resolve_module_path(source, span)?;

        {
            let cache = self.module_cache.borrow();
            if let Some(state) = cache.get(&resolved) {
                return Ok(state.exports_snapshot());
            }
        }

        let code = std::fs::read_to_string(&resolved).map_err(|e| {
            RuntimeError::new(format!("Не удалось прочитать модуль '{}': {e}", resolved.display()), span)
        })?;
        let name = resolved.display().to_string();
        let source_file = match &self.sources {
            Some(sources) => sources.borrow_mut().add(name, code),
            None => Rc::new(yps_lexer::SourceFile::new(name, code)),
        };
        let lexer = yps_lexer::Lexer::new(&source_file);
        let (tokens, lex_diags) = lexer.tokenize();
        if !lex_diags.is_empty() {
            return Err(RuntimeError::new(
                format!("Ошибки лексера в модуле '{}':{}", resolved.display(), described(&source_file, &lex_diags)),
                span,
            ));
        }
        let parser = yps_parser::Parser::new(&tokens, &source_file);
        let (program, parse_diags) = parser.parse_program();
        if !parse_diags.is_empty() {
            return Err(RuntimeError::new(
                format!("Ошибки парсера в модуле '{}':{}", resolved.display(), described(&source_file, &parse_diags)),
                span,
            ));
        }

        let export_cell: ExportCell = Rc::new(RefCell::new(HashMap::new()));

        let mut sub = Interpreter::new();
        sub.module_cache = Rc::clone(&self.module_cache);
        sub.module_links = Rc::clone(&self.module_links);
        sub.base_path = resolved.parent().map(Path::to_path_buf);
        sub.export_cell = Some(Rc::clone(&export_cell));
        sub.output_sink = self.output_sink.clone();
        sub.stdin_blocked = self.stdin_blocked.clone();
        sub.sources = self.sources.clone();

        self.module_cache.borrow_mut().insert(resolved.clone(), ModuleState::Loading(Rc::clone(&export_cell)));
        match sub.run_module(&program, &resolved) {
            Ok(exports) => Ok(exports),
            Err(e) => {
                self.module_cache.borrow_mut().remove(&resolved);
                Err(e)
            }
        }
    }

    pub fn run_module(&mut self, program: &Program, path: &Path) -> Result<HashMap<String, Value>, RuntimeError> {
        self.run(program)?;
        let exports = std::mem::take(&mut self.current_exports);
        self.export_cell = None;
        self.module_cache.borrow_mut().insert(path.to_path_buf(), ModuleState::Loaded(exports.clone()));
        self.apply_module_links(path, &exports);
        Ok(exports)
    }

    pub(super) fn record_export(&mut self, name: String, value: Value) {
        if let Some(cell) = &self.export_cell {
            cell.borrow_mut().insert(name.clone(), value.clone());
        }
        self.current_exports.insert(name, value);
    }

    pub(super) fn loading_module_path(&self, source: &str, span: Span) -> Option<PathBuf> {
        let resolved = self.resolve_module_path(source, span).ok()?;
        let cache = self.module_cache.borrow();
        match cache.get(&resolved) {
            Some(ModuleState::Loading(_)) => Some(resolved),
            _ => None,
        }
    }

    pub(super) fn register_module_link(&self, module: PathBuf, local: &str, imported: &str) {
        self.module_links.borrow_mut().push(DeferredLink {
            module,
            target_env: self.env.snapshot(),
            local: local.to_string(),
            imported: imported.to_string(),
        });
    }

    fn apply_module_links(&self, path: &Path, exports: &HashMap<String, Value>) {
        let pending: Vec<DeferredLink> = {
            let mut links = self.module_links.borrow_mut();
            let mut drained = Vec::new();
            links.retain(|link| {
                if link.module == path {
                    drained.push(DeferredLink {
                        module: link.module.clone(),
                        target_env: Rc::clone(&link.target_env),
                        local: link.local.clone(),
                        imported: link.imported.clone(),
                    });
                    false
                } else {
                    true
                }
            });
            drained
        };
        for link in pending {
            if let Some(value) = exports.get(&link.imported) {
                link.target_env.borrow_mut().rebind(link.local, value.clone());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(prefix: &str) -> Self {
            let n = COUNTER.fetch_add(1, Ordering::Relaxed);
            let dir = std::env::temp_dir().join(format!("yps_test_{prefix}_{n}"));
            std::fs::create_dir_all(&dir).unwrap();
            TempDir(dir)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn write_file(dir: &TempDir, name: &str, content: &str) -> PathBuf {
        let path = dir.path().join(name);
        std::fs::write(&path, content).unwrap();
        path
    }

    fn interp_with_base(dir: &TempDir) -> Interpreter {
        let mut i = Interpreter::new();
        i.set_base_path(dir.path().to_path_buf());
        i
    }

    #[test]
    fn test_self_import_no_stack_overflow() {
        let dir = TempDir::new("self");
        write_file(&dir, "self_mod.yopta", "спиздить { x } из \"self_mod\";\nпредъява гыы x = 1;");
        let mut i = interp_with_base(&dir);
        let result = i.load_module("self_mod", Span { start: 0, end: 0 });
        let exports = result.expect("самоимпорт должен завершиться без stack overflow и без ошибки");
        assert_eq!(exports.get("x"), Some(&Value::Number(1.0)));
    }

    #[test]
    fn test_cyclic_ab_no_stack_overflow() {
        let dir = TempDir::new("cyclic");
        write_file(&dir, "a.yopta", "спиздить { b_val } из \"b\";\nпредъява гыы a_val = 1;");
        write_file(&dir, "b.yopta", "спиздить { a_val } из \"a\";\nпредъява гыы b_val = 2;");
        let mut i = interp_with_base(&dir);
        let result = i.load_module("a", Span { start: 0, end: 0 });
        let exports = result.expect("A→B→A не должен вызывать stack overflow и должен успешно завершиться");
        assert_eq!(exports.get("a_val"), Some(&Value::Number(1.0)));
    }

    #[test]
    fn test_loading_state_returns_partial_exports_during_cycle() {
        let dir = TempDir::new("partial");
        write_file(
            &dir,
            "p.yopta",
            "предъява гыы раньше = 1;\nспиздить { видимое } из \"q\";\nпредъява гыы позже = 2;",
        );
        write_file(&dir, "q.yopta", "спиздить { раньше, позже } из \"p\";\nпредъява гыы видимое = раньше;");
        let mut i = interp_with_base(&dir);
        let exports = i.load_module("p", Span { start: 0, end: 0 }).expect("цикл должен загрузиться");
        assert_eq!(exports.get("раньше"), Some(&Value::Number(1.0)));
        assert_eq!(exports.get("позже"), Some(&Value::Number(2.0)));
    }

    #[test]
    fn test_loaded_state_cached_not_reparsed() {
        let dir = TempDir::new("cached");
        write_file(&dir, "mod.yopta", "предъява гыы val = 99;");
        let mut i = interp_with_base(&dir);
        let exports1 = i.load_module("mod", Span { start: 0, end: 0 }).expect("модуль должен успешно загрузиться");
        assert_eq!(exports1.get("val"), Some(&Value::Number(99.0)));

        write_file(&dir, "mod.yopta", "предъява гыы val = 111;");
        let exports2 = i.load_module("mod", Span { start: 0, end: 0 }).expect("повторная загрузка должна взять кэш");
        assert_eq!(exports2.get("val"), Some(&Value::Number(99.0)), "изменение файла не должно перечитываться");
    }

    fn parse_registered(sources: &Rc<RefCell<yps_lexer::Sources>>, name: &str, code: &str) -> Program {
        let source = sources.borrow_mut().add(name.to_string(), code.to_string());
        let (tokens, lex_diags) = yps_lexer::Lexer::new(&source).tokenize();
        assert!(lex_diags.is_empty(), "{lex_diags:?}");
        let (program, parse_diags) = yps_parser::Parser::new(&tokens, &source).parse_program();
        assert!(parse_diags.is_empty(), "{parse_diags:?}");
        program
    }

    #[test]
    fn error_inside_a_module_function_resolves_to_the_module_source() {
        let dir = TempDir::new("sources_fn");
        write_file(&dir, "m.yopta", "\nпредъява йопта упасть() {\n  гыы о = ноль;\n  отвечаю о.поле;\n}\n");
        let sources = Rc::new(RefCell::new(yps_lexer::Sources::default()));
        let program = parse_registered(&sources, "main.yopta", "спиздить { упасть } из \"m\";\nупасть();\n");
        let mut i = interp_with_base(&dir);
        i.set_sources(Rc::clone(&sources));

        let err = i.run(&program).expect_err("обращение к полю ноль должно упасть");

        let sources = sources.borrow();
        let owner = sources.lookup(err.span.start).expect("спан ошибки должен принадлежать исходнику");
        assert!(owner.name.ends_with("m.yopta"), "ошибка приписана {}", owner.name);
        assert_eq!(owner.position(err.span.start), (4, 11));
        let caller = sources.lookup(err.stack[0].span.start).expect("спан кадра");
        assert_eq!(caller.name, "main.yopta");
        assert_eq!(caller.position(err.stack[0].span.start), (2, 1));
    }

    #[test]
    fn error_at_module_top_level_resolves_to_the_module_source() {
        let dir = TempDir::new("sources_top");
        write_file(&dir, "m.yopta", "предъява гыы х = 1;\nгыы о = ноль;\nо.поле;\n");
        let sources = Rc::new(RefCell::new(yps_lexer::Sources::default()));
        let program = parse_registered(&sources, "main.yopta", "спиздить { х } из \"m\";\n");
        let mut i = interp_with_base(&dir);
        i.set_sources(Rc::clone(&sources));

        let err = i.run(&program).expect_err("модуль должен упасть при загрузке");

        let sources = sources.borrow();
        let owner = sources.lookup(err.span.start).expect("спан ошибки должен принадлежать исходнику");
        assert!(owner.name.ends_with("m.yopta"), "ошибка приписана {}", owner.name);
        assert_eq!(owner.position(err.span.start).0, 3);
    }

    #[test]
    fn nested_module_inherits_the_source_registry() {
        let dir = TempDir::new("sources_nested");
        write_file(&dir, "inner.yopta", "предъява йопта упасть() {\n  гыы о = ноль;\n  отвечаю о.поле;\n}\n");
        write_file(
            &dir,
            "outer.yopta",
            "спиздить { упасть } из \"inner\";\nпредъява йопта обёртка() { отвечаю упасть(); }\n",
        );
        let sources = Rc::new(RefCell::new(yps_lexer::Sources::default()));
        let program = parse_registered(&sources, "main.yopta", "спиздить { обёртка } из \"outer\";\nобёртка();\n");
        let mut i = interp_with_base(&dir);
        i.set_sources(Rc::clone(&sources));

        let err = i.run(&program).expect_err("вложенный модуль должен упасть");

        let sources = sources.borrow();
        let owner = sources.lookup(err.span.start).expect("спан ошибки должен принадлежать исходнику");
        assert!(owner.name.ends_with("inner.yopta"), "ошибка приписана {}", owner.name);
        assert_eq!(owner.position(err.span.start), (3, 11));
    }

    #[test]
    fn without_a_registry_module_spans_stay_local_to_the_module() {
        let dir = TempDir::new("sources_none");
        write_file(&dir, "m.yopta", "предъява йопта упасть() {\n  гыы о = ноль;\n  отвечаю о.поле;\n}\n");
        let source = yps_lexer::SourceFile::new(
            "main.yopta".to_string(),
            "спиздить { упасть } из \"m\";\nупасть();\n".to_string(),
        );
        let (tokens, _) = yps_lexer::Lexer::new(&source).tokenize();
        let (program, _) = yps_parser::Parser::new(&tokens, &source).parse_program();
        let mut i = interp_with_base(&dir);

        let err = i.run(&program).expect_err("обращение к полю ноль должно упасть");

        let module_text = std::fs::read_to_string(dir.path().join("m.yopta")).unwrap();
        assert!(err.span.start < module_text.len());
    }

    #[test]
    fn uncaught_throw_inside_a_module_function_resolves_to_the_module_source() {
        let dir = TempDir::new("sources_throw");
        write_file(&dir, "m.yopta", "\nпредъява йопта упасть() {\n  кидай \"из модуля\";\n}\n");
        let sources = Rc::new(RefCell::new(yps_lexer::Sources::default()));
        let program = parse_registered(&sources, "main.yopta", "спиздить { упасть } из \"m\";\nупасть();\n");
        let mut i = interp_with_base(&dir);
        i.set_sources(Rc::clone(&sources));

        let err = i.run(&program).expect_err("исключение должно дойти до верха");

        let sources = sources.borrow();
        let owner = sources.lookup(err.span.start).expect("спан ошибки должен принадлежать исходнику");
        assert!(owner.name.ends_with("m.yopta"), "ошибка приписана {}", owner.name);
        assert_eq!(owner.position(err.span.start), (3, 3));
    }

    #[test]
    fn uncaught_throw_at_module_top_level_resolves_to_the_module_source() {
        let dir = TempDir::new("sources_throw_top");
        write_file(&dir, "m.yopta", "предъява гыы х = 1;\n\n\nкидай \"бум\";\n");
        let sources = Rc::new(RefCell::new(yps_lexer::Sources::default()));
        let program = parse_registered(&sources, "main.yopta", "спиздить { х } из \"m\";\n");
        let mut i = interp_with_base(&dir);
        i.set_sources(Rc::clone(&sources));

        let err = i.run(&program).expect_err("модуль должен упасть при загрузке");

        let sources = sources.borrow();
        let owner = sources.lookup(err.span.start).expect("спан ошибки должен принадлежать исходнику");
        assert!(owner.name.ends_with("m.yopta"), "ошибка приписана {}", owner.name);
        assert_eq!(owner.position(err.span.start), (4, 1));
    }

    #[test]
    fn a_syntax_error_in_a_module_is_rendered_as_a_located_diagnostic() {
        let dir = TempDir::new("module_syntax");
        write_file(&dir, "bad.yopta", "предъява гыы х = 1;\nгыы у = ;\n");
        let mut i = interp_with_base(&dir);

        let err = i.load_module("bad", Span { start: 0, end: 0 }).expect_err("модуль с ошибкой разбора");

        assert!(err.message.contains("bad.yopta:2:9: Ошибка: "), "{}", err.message);
        assert!(!err.message.contains("Diagnostic {"), "{}", err.message);
    }

    #[test]
    fn a_lexer_error_in_a_module_is_rendered_as_a_located_diagnostic() {
        let dir = TempDir::new("module_lex");
        write_file(&dir, "bad.yopta", "предъява гыы х = 1;\nгыы у = §;\n");
        let mut i = interp_with_base(&dir);

        let err = i.load_module("bad", Span { start: 0, end: 0 }).expect_err("модуль с ошибкой лексера");

        assert!(err.message.contains("bad.yopta:2:9: Ошибка: Неизвестный символ"), "{}", err.message);
        assert!(!err.message.contains("Diagnostic {"), "{}", err.message);
    }
}
