use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use yps_lexer::Span;
use yps_parser::ast::Program;

use crate::error::RuntimeError;
use crate::value::Value;

use super::Interpreter;

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

    pub(super) fn load_module(&mut self, source: &str, span: Span) -> Result<HashMap<String, Value>, RuntimeError> {
        let resolved = self.resolve_module_path(source, span)?;

        if let Some(cached) = self.module_cache.borrow().get(&resolved) {
            return Ok(cached.clone());
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

        let mut sub = Interpreter::new();
        sub.module_cache = Rc::clone(&self.module_cache);
        sub.base_path = resolved.parent().map(Path::to_path_buf);

        let exports = sub.run_module(&program, &resolved)?;
        Ok(exports)
    }

    pub fn run_module(&mut self, program: &Program, path: &Path) -> Result<HashMap<String, Value>, RuntimeError> {
        self.run(program)?;
        let exports = std::mem::take(&mut self.current_exports);
        self.module_cache.borrow_mut().insert(path.to_path_buf(), exports.clone());
        Ok(exports)
    }
}
