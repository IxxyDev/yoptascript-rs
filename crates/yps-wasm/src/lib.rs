//! Обёртка YoptaScript для браузера: лексер → парсер → интерпретатор, вывод `сказать`
//! собирается в буфер вместо stdout.
//!
//! Границы (проверено на wasm32-unknown-unknown):
//! - `ФС`/`Сеть` дают обычную ловимую ошибку времени выполнения
//!   («operation not supported on this platform»), а не падение;
//! - `Процесс.переменные` пуст, `Процесс.рабочаяПапка` — пустая строка;
//! - `сказать.время`/`сказать.времяСтоп` дают ловимую ошибку времени выполнения
//!   («недоступен в браузере»), а не падение;
//! - системные часы иначе не поддерживаются: `Дата.сейчас()` и таймеры (`чутка`/`интервал`)
//!   паникуют и прерывают вызов исключением на стороне JS
//!   (модуль после этого остаётся работоспособным); `захуярить Дата(мс)` работает;
//! - глубокая рекурсия упирается в стек JS раньше, чем в `MAX_CALL_DEPTH`, и тоже
//!   приходит в JS исключением; бесконечный цикл вешает вкладку — выполнение синхронное.
//!
//! Развёртывание страницы (хостинг) — отдельное решение, в этот код не входит.

use wasm_bindgen::prelude::*;

use yps_interpreter::{BufferSink, Interpreter};
use yps_lexer::{Diagnostic, Lexer, SourceFile};
use yps_parser::Parser;

const SOURCE_NAME: &str = "песочница";

/// Ошибки возвращаются текстом в формате yps-cli: `имя:строка:колонка: сообщение`.
pub fn run_source(code: &str) -> Result<String, String> {
    let source = SourceFile::new(SOURCE_NAME.to_string(), code.to_string());

    let (tokens, lex_diagnostics) = Lexer::new(&source).tokenize();
    if !lex_diagnostics.is_empty() {
        return Err(render_diagnostics(&source, &lex_diagnostics));
    }

    let (program, parse_diagnostics) = Parser::new(&tokens, &source).parse_program();
    if !parse_diagnostics.is_empty() {
        return Err(render_diagnostics(&source, &parse_diagnostics));
    }

    let buffer = BufferSink::new();
    let mut interp = Interpreter::new();
    interp.set_output_sink(Box::new(buffer.clone()));

    match interp.run(&program) {
        Ok(()) => Ok(buffer.take()),
        Err(e) => {
            let (line, col) = source.position(e.span.start);
            let mut text = format!("{SOURCE_NAME}:{line}:{col}: {e}");
            for frame in &e.stack {
                let (fl, fc) = source.position(frame.span.start);
                text.push_str(&format!("\n  в {}:{SOURCE_NAME}:{fl}:{fc}", frame.name));
            }
            let printed = buffer.take();
            if printed.is_empty() { Err(text) } else { Err(format!("{printed}{text}")) }
        }
    }
}

fn render_diagnostics(source: &SourceFile, diagnostics: &[Diagnostic]) -> String {
    diagnostics
        .iter()
        .map(|d| {
            let (line, col) = source.position(d.span.start);
            format!("{SOURCE_NAME}:{line}:{col}: {:?}: {}", d.severity, d.message)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
}

#[wasm_bindgen]
pub fn run_yopta(code: &str) -> Result<String, JsValue> {
    run_source(code).map_err(|e| JsValue::from_str(&e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prints_captured_output() {
        assert_eq!(run_source("сказать(\"привет\");").unwrap(), "привет\n");
    }

    #[test]
    fn console_family_captured() {
        let out = run_source("сказать.инфо(1); сказать.ошибка(2);").unwrap();
        assert_eq!(out, "1\n2\n");
    }

    #[test]
    fn parse_error_is_reported() {
        let err = run_source("гыы = ;").unwrap_err();
        assert!(err.starts_with("песочница:1:"), "получено {err:?}");
    }

    #[test]
    fn lex_error_is_reported() {
        let err = run_source("гыы а = \"незакрытая;").unwrap_err();
        assert!(err.contains("песочница:1:"), "получено {err:?}");
    }

    #[test]
    fn runtime_error_keeps_partial_output() {
        let err = run_source("сказать(\"до\"); неизвестнаяФункция();").unwrap_err();
        assert!(err.starts_with("до\n"), "получено {err:?}");
        assert!(err.contains("песочница:1:"), "получено {err:?}");
    }

    #[test]
    fn infinite_recursion_is_a_runtime_error_not_a_crash() {
        let err = run_source("йопта ф() { отвечаю ф(); } ф();").unwrap_err();
        assert!(err.contains("глубина рекурсии"), "получено {err:?}");
    }

    #[test]
    fn filesystem_access_is_a_catchable_runtime_error() {
        let out =
            run_source("хапнуть { ФС.прочитать(\"/нет/такого\"); } гоп (е) { сказать(\"поймали:\", е.message); }")
                .unwrap();
        assert!(out.starts_with("поймали: 'ФС.прочитать'"), "получено {out:?}");
    }
}
