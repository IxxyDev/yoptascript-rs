use std::cell::RefCell;
use std::env;
use std::ffi::OsString;
use std::fmt::Write as _;
use std::fs;
use std::io::{self, Read as _};
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::rc::Rc;

use yps_interpreter::{Interpreter, RuntimeError};
use yps_lexer::{Diagnostic, Lexer, SourceFile, Sources};
use yps_parser::{Parser, Program};

use crate::args::{Command, FmtMode, RunSource};
use crate::write::{write_atomic, write_stdout};

mod args;
mod completion;
mod repl;
mod write;

const FAILURE_EXIT_CODE: u8 = 1;
const USAGE_EXIT_CODE: u8 = 2;
const INTERNAL_ERROR_EXIT_CODE: u8 = 70;

#[derive(Debug)]
pub(crate) struct Failure {
    code: u8,
    message: Option<String>,
}

impl Failure {
    pub(crate) fn error(message: impl Into<String>) -> Self {
        Self { code: FAILURE_EXIT_CODE, message: Some(message.into()) }
    }

    pub(crate) const fn reported() -> Self {
        Self { code: FAILURE_EXIT_CODE, message: None }
    }

    fn usage(message: impl Into<String>) -> Self {
        Self { code: USAGE_EXIT_CODE, message: Some(message.into()) }
    }

    fn internal(message: impl Into<String>) -> Self {
        Self { code: INTERNAL_ERROR_EXIT_CODE, message: Some(message.into()) }
    }
}

pub(crate) fn guarded<T>(run: impl FnOnce() -> T) -> Option<T> {
    panic::catch_unwind(AssertUnwindSafe(run)).ok()
}

pub(crate) fn abandon_after_panic<T>(broken: T) {
    std::mem::forget(broken);
}

pub(crate) fn print_diagnostics(source: &SourceFile, diagnostics: &[Diagnostic]) {
    for diagnostic in diagnostics {
        eprintln!("{}", source.describe(diagnostic));
    }
}

pub(crate) fn print_runtime_error(e: &RuntimeError, locate: impl Fn(usize) -> String) {
    eprintln!("{}: {e}", locate(e.span.start));
    for frame in &e.stack {
        eprintln!("  в {}:{}", frame.name, locate(frame.span.start));
    }
}

fn locate(sources: &RefCell<Sources>, main: &SourceFile, offset: usize) -> String {
    let sources = sources.borrow();
    let file = sources.lookup(offset).map_or(main, AsRef::as_ref);
    let (line, col) = file.position(offset);
    format!("{}:{line}:{col}", file.name)
}

fn main() -> ExitCode {
    let args: Vec<OsString> = env::args_os().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(failure) => {
            if let Some(message) = &failure.message {
                eprintln!("{message}");
            }
            ExitCode::from(failure.code)
        }
    }
}

fn run(args: &[OsString]) -> Result<(), Failure> {
    match args::parse(args).map_err(|e| Failure::usage(e.0))? {
        Command::Help(text) => write_stdout(&format!("{text}\n")),
        Command::Version => write_stdout(&format!("yps {}\n", env!("CARGO_PKG_VERSION"))),
        Command::Repl => {
            yps_interpreter::set_script_args(Vec::new());
            repl::run_repl()
        }
        Command::Run { source, use_vm, script_args } => run_program(source, use_vm, script_args),
        Command::Fmt { file, mode, source_map } => run_fmt(&file, mode, source_map),
        Command::Ast { file } => run_ast(&file),
        Command::Disasm { file } => run_disasm(&file),
        Command::Lint { file } => run_lint(&file),
        Command::Transpile { file, output } => run_transpile(&file, output.as_deref()),
    }
}

fn read_file(path: &Path) -> Result<String, Failure> {
    fs::read_to_string(path).map_err(|e| Failure::error(format!("Не удалось прочитать файл '{}': {e}", path.display())))
}

fn read_source(path: &Path) -> Result<SourceFile, Failure> {
    Ok(SourceFile::new(path.display().to_string(), read_file(path)?))
}

fn parse(source: &SourceFile) -> Result<Program, Failure> {
    let (tokens, lex_diagnostics) = Lexer::new(source).tokenize();
    if !lex_diagnostics.is_empty() {
        print_diagnostics(source, &lex_diagnostics);
        return Err(Failure::reported());
    }

    let (program, parse_diagnostics) = Parser::new(&tokens, source).parse_program();
    if !parse_diagnostics.is_empty() {
        print_diagnostics(source, &parse_diagnostics);
        return Err(Failure::reported());
    }

    Ok(program)
}

fn run_ast(file: &Path) -> Result<(), Failure> {
    let program = parse(&read_source(file)?)?;
    write_stdout(&format!("{program:#?}\n"))
}

fn run_disasm(file: &Path) -> Result<(), Failure> {
    let source = read_source(file)?;
    let program = parse(&source)?;
    match yps_vm::compile_program(&program) {
        Ok(proto) => write_stdout(&format!("{}\n", yps_vm::disassemble(&proto))),
        Err(e) => {
            let (line, col) = source.position(e.span.start);
            Err(Failure::error(format!("{}:{line}:{col}: {e}", source.name)))
        }
    }
}

fn run_lint(file: &Path) -> Result<(), Failure> {
    let source = read_source(file)?;
    let result = yps_lint::lint_source(&source.source);

    if !result.parse_errors.is_empty() {
        print_diagnostics(&source, &result.parse_errors);
        return Err(Failure::reported());
    }
    if result.diagnostics.is_empty() {
        return Ok(());
    }

    let mut report = String::new();
    for d in &result.diagnostics {
        let (line, col) = source.position(d.span.start);
        let _ = writeln!(report, "{}:{line}:{col}: {} [{}]: {}", source.name, d.severity, d.rule.code(), d.message);
    }
    write_stdout(&report)?;
    Err(Failure::reported())
}

fn is_same_file(a: &Path, b: &Path) -> bool {
    match (fs::canonicalize(a), fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

fn run_transpile(file: &Path, output: Option<&Path>) -> Result<(), Failure> {
    if let Some(output) = output
        && is_same_file(file, output)
    {
        return Err(Failure::usage(format!("Выходной файл совпадает с исходным: {}", output.display())));
    }

    let source = read_source(file)?;
    let program = parse(&source)?;
    let js = yps_jsgen::transpile(&program).map_err(|e| {
        let (line, col) = source.position(e.span.start);
        Failure::error(format!("{}:{line}:{col}: {e}", source.name))
    })?;

    match output {
        Some(path) => write_atomic(path, js.as_bytes()),
        None => write_stdout(&js),
    }
}

fn format_failure(error: yps_fmt::FormatError, source: &SourceFile) -> Failure {
    match error {
        yps_fmt::FormatError::ParseError(diagnostics) => {
            print_diagnostics(source, &diagnostics);
            Failure::error("Форматирование отклонено: файл содержит синтаксические ошибки")
        }
        yps_fmt::FormatError::RoundTripFailed(msg) => {
            Failure::error(format!("Форматирование отклонено: самопроверка не прошла: {msg}"))
        }
        yps_fmt::FormatError::CommentRefused(msg) => Failure::error(format!("Форматирование отклонено: {msg}")),
    }
}

fn write_source_map(file: &Path, mut map: yps_fmt::SourceMap) -> Result<(), Failure> {
    let name = file.file_name().unwrap_or_default().to_string_lossy().into_owned();
    map.file.clone_from(&name);
    map.source_name = name;

    let mut map_path = file.as_os_str().to_owned();
    map_path.push(".map");
    write_atomic(Path::new(&map_path), map.to_json().as_bytes())
}

fn run_fmt(file: &Path, mode: FmtMode, source_map: bool) -> Result<(), Failure> {
    let source = read_source(file)?;
    let (outcome, map) = if source_map {
        let (outcome, map) = yps_fmt::format_source_with_map(&source.source).map_err(|e| format_failure(e, &source))?;
        (outcome, Some(map))
    } else {
        (yps_fmt::format_source(&source.source).map_err(|e| format_failure(e, &source))?, None)
    };

    match mode {
        FmtMode::Check if outcome.already_formatted => Ok(()),
        FmtMode::Check => Err(Failure::reported()),
        FmtMode::Print => write_stdout(&outcome.text),
        FmtMode::Write => {
            if !outcome.already_formatted {
                write_atomic(file, outcome.text.as_bytes())?;
            }
            map.map_or(Ok(()), |map| write_source_map(file, map))
        }
    }
}

fn run_program(source: RunSource, use_vm: bool, script_args: Vec<String>) -> Result<(), Failure> {
    let (name, code, base, argv0) = match source {
        RunSource::File(path) => {
            let name = path.display().to_string();
            (name.clone(), read_file(&path)?, path.parent().map(Path::to_path_buf), name)
        }
        RunSource::Eval(code) => ("<eval>".to_string(), code, None, "-e".to_string()),
        RunSource::Stdin => {
            let mut code = String::new();
            io::stdin()
                .read_to_string(&mut code)
                .map_err(|e| Failure::error(format!("Не удалось прочитать stdin: {e}")))?;
            ("<stdin>".to_string(), code, None, "-".to_string())
        }
    };
    yps_interpreter::set_script_args(std::iter::once(argv0).chain(script_args).collect());

    let sources = Rc::new(RefCell::new(Sources::default()));
    let main = sources.borrow_mut().add(name, code);
    let program = parse(&main)?;
    if use_vm { run_vm(&program, base, &sources, &main) } else { run_interpret(&program, base, &sources, &main) }
}

fn run_interpret(
    program: &Program,
    base: Option<PathBuf>,
    sources: &Rc<RefCell<Sources>>,
    main: &SourceFile,
) -> Result<(), Failure> {
    let mut interpreter = Interpreter::new();
    if let Some(base) = base {
        interpreter.set_base_path(base);
    }
    interpreter.set_sources(Rc::clone(sources));

    match guarded(|| interpreter.run(program)) {
        Some(Ok(())) => Ok(()),
        Some(Err(e)) => {
            print_runtime_error(&e, |offset| locate(sources, main, offset));
            Err(Failure::reported())
        }
        None => {
            abandon_after_panic(interpreter);
            Err(Failure::internal("Внутренняя ошибка интерпретатора: выполнение прервано"))
        }
    }
}

fn run_vm(
    program: &Program,
    base: Option<PathBuf>,
    sources: &Rc<RefCell<Sources>>,
    main: &SourceFile,
) -> Result<(), Failure> {
    let mut vm = yps_vm::Vm::new();
    if let Some(base) = base {
        vm.set_base_path(base);
    }
    vm.set_sources(Rc::clone(sources));

    let outcome = guarded(|| -> Result<(), yps_vm::ExecError> {
        let proto = yps_vm::compile_program(program)?;
        vm.run(proto)?;
        Ok(())
    });
    match outcome {
        Some(Ok(())) => Ok(()),
        Some(Err(e)) => Err(Failure::error(format!("{}: {e}", locate(sources, main, e.span().start)))),
        None => {
            abandon_after_panic(vm);
            Err(Failure::internal("Внутренняя ошибка VM: выполнение прервано"))
        }
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use std::fs;
    use std::path::{Path, PathBuf};

    pub(crate) struct Scratch(PathBuf);

    impl Scratch {
        pub(crate) fn new(tag: &str) -> Self {
            let dir = std::env::temp_dir().join(format!("yps_cli_unit_{tag}_{}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).expect("создать каталог");
            Self(dir)
        }

        pub(crate) fn path(&self) -> &Path {
            &self.0
        }

        pub(crate) fn entries(&self) -> Vec<String> {
            let mut names: Vec<String> = fs::read_dir(&self.0)
                .expect("прочитать каталог")
                .filter_map(Result::ok)
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .collect();
            names.sort();
            names
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::Scratch;
    use super::*;

    #[test]
    fn guarded_returns_the_value_of_a_normal_run() {
        assert_eq!(guarded(|| 7), Some(7));
    }

    #[test]
    fn guarded_turns_a_panic_into_none() {
        assert_eq!(guarded(|| -> u8 { panic!("сбой") }), None);
    }

    #[test]
    fn locate_names_the_source_that_owns_the_offset() {
        let sources = RefCell::new(Sources::default());
        let main = sources.borrow_mut().add("main.yopta".to_string(), "раз\nдва\n".to_string());
        let module = sources.borrow_mut().add("mod.yopta".to_string(), "\n\nтри\n".to_string());

        assert_eq!(locate(&sources, &main, main.base() + 7), "main.yopta:2:1");
        assert_eq!(locate(&sources, &main, module.base() + 2), "mod.yopta:3:1");
    }

    #[test]
    fn locate_falls_back_to_the_main_source_for_an_unknown_offset() {
        let sources = RefCell::new(Sources::default());
        let main = sources.borrow_mut().add("main.yopta".to_string(), "раз\n".to_string());

        assert_eq!(locate(&sources, &main, usize::MAX), "main.yopta:2:1");
    }

    #[test]
    fn a_file_is_the_same_as_itself_through_a_different_spelling() {
        let scratch = Scratch::new("same_file");
        let file = scratch.path().join("a.yopta");
        fs::write(&file, "").unwrap();

        assert!(is_same_file(&file, &scratch.path().join(".").join("a.yopta")));
        assert!(!is_same_file(&file, &scratch.path().join("нет.js")));
    }
}
