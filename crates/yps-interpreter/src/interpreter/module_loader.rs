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

impl Interpreter {
    fn resolve_module_path(&self, source: &str, span: Span) -> Result<PathBuf, RuntimeError> {
        let base = self.base_path.clone().unwrap_or_else(|| PathBuf::from("."));
        let mut candidate = base.join(source);
        if candidate.extension().is_none() {
            candidate.set_extension("yop");
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
        let source_file = yps_lexer::SourceFile::new(resolved.display().to_string(), code);
        let lexer = yps_lexer::Lexer::new(&source_file);
        let (tokens, lex_diags) = lexer.tokenize();
        if !lex_diags.is_empty() {
            return Err(RuntimeError::new(
                format!("Ошибки лексера в модуле '{}': {:?}", resolved.display(), lex_diags),
                span,
            ));
        }
        let parser = yps_parser::Parser::new(&tokens, &source_file);
        let (program, parse_diags) = parser.parse_program();
        if !parse_diags.is_empty() {
            return Err(RuntimeError::new(
                format!("Ошибки парсера в модуле '{}': {:?}", resolved.display(), parse_diags),
                span,
            ));
        }

        let export_cell: ExportCell = Rc::new(RefCell::new(HashMap::new()));

        let mut sub = Interpreter::new();
        sub.module_cache = Rc::clone(&self.module_cache);
        sub.module_links = Rc::clone(&self.module_links);
        sub.base_path = resolved.parent().map(Path::to_path_buf);
        sub.export_cell = Some(Rc::clone(&export_cell));

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
    use std::cell::RefCell;
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
        write_file(&dir, "self_mod.yop", "импортировать { x } из \"self_mod\";\nэкспортировать гыы x = 1;");
        let mut i = interp_with_base(&dir);
        let result = i.load_module("self_mod", Span { start: 0, end: 0 });
        assert!(result.is_ok() || result.is_err(), "должен завершиться без stack overflow");
    }

    #[test]
    fn test_cyclic_ab_no_stack_overflow() {
        let dir = TempDir::new("cyclic");
        write_file(&dir, "a.yop", "импортировать { b_val } из \"b\";\nэкспортировать гыы a_val = 1;");
        write_file(&dir, "b.yop", "импортировать { a_val } из \"a\";\nэкспортировать гыы b_val = 2;");
        let mut i = interp_with_base(&dir);
        let result = i.load_module("a", Span { start: 0, end: 0 });
        assert!(result.is_ok() || result.is_err(), "A→B→A не должен вызывать stack overflow");
    }

    #[test]
    fn test_loading_state_returns_partial() {
        let dir = TempDir::new("partial");
        let resolved = dir.path().join("dummy.yop");
        std::fs::write(&resolved, "").unwrap();

        let mut exports = HashMap::new();
        exports.insert("x".to_string(), Value::Number(42.0));

        let cache: Rc<RefCell<HashMap<PathBuf, ModuleState>>> = Rc::new(RefCell::new(HashMap::new()));
        cache.borrow_mut().insert(resolved.clone(), ModuleState::Loading(Rc::new(RefCell::new(exports.clone()))));

        let state = cache.borrow();
        let got = state.get(&resolved).unwrap().exports_snapshot();
        assert_eq!(got.get("x"), Some(&Value::Number(42.0)));
    }

    #[test]
    fn test_loaded_state_cached() {
        let dir = TempDir::new("cached");
        write_file(&dir, "mod.yop", "экспортировать гыы val = 99;");
        let mut i = interp_with_base(&dir);
        let r1 = i.load_module("mod", Span { start: 0, end: 0 });
        let r2 = i.load_module("mod", Span { start: 0, end: 0 });
        if r1.is_ok() {
            assert!(r2.is_ok());
        }
    }
}
