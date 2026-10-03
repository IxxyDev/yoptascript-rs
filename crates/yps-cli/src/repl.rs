use std::cell::RefCell;
use std::env;
use std::io::{self, BufRead, IsTerminal};
use std::path::PathBuf;
use std::rc::Rc;

use rustyline::Editor;
use rustyline::error::ReadlineError;
use rustyline::history::DefaultHistory;

use yps_interpreter::{Interpreter, RuntimeError, Value};
use yps_lexer::{Diagnostic, Lexer, SourceFile, Sources};
use yps_parser::{Parser, Program};

use crate::completion::{self, YpsHelper};
use crate::{Failure, abandon_after_panic, guarded, print_diagnostics, print_runtime_error};

type YpsEditor = Editor<YpsHelper, DefaultHistory>;

const REPL_NAME: &str = "<repl>";

fn history_path_from(env_override: Option<PathBuf>, home: Option<PathBuf>) -> Option<PathBuf> {
    env_override.filter(|path| !path.as_os_str().is_empty()).or_else(|| home.map(|home| home.join(".yps_history")))
}

fn history_path() -> Option<PathBuf> {
    history_path_from(env::var_os("YPS_HISTORY_FILE").map(PathBuf::from), env::home_dir())
}

#[derive(Debug, PartialEq)]
enum ReplCommand {
    Exit,
    History,
    Reset,
    Cancel,
    Repeat(usize),
}

fn parse_repl_command(input: &str) -> Option<ReplCommand> {
    match input.trim() {
        ":выход" => Some(ReplCommand::Exit),
        ":история" => Some(ReplCommand::History),
        ":сброс" => Some(ReplCommand::Reset),
        ":отмена" => Some(ReplCommand::Cancel),
        other => other.strip_prefix('!')?.trim().parse().ok().filter(|&n| n >= 1).map(ReplCommand::Repeat),
    }
}

enum Check {
    Ready(Program),
    Incomplete { diagnostics: Vec<Diagnostic>, in_literal: bool },
    Invalid(Vec<Diagnostic>),
}

fn check(source: &SourceFile) -> Check {
    let (tokens, diagnostics, in_literal) = Lexer::new(source).tokenize_extended();
    if in_literal {
        return Check::Incomplete { diagnostics, in_literal };
    }
    if !diagnostics.is_empty() {
        return Check::Invalid(diagnostics);
    }
    let (program, diagnostics, unexpected_eof) = Parser::new(&tokens, source).parse_program_extended();
    if unexpected_eof {
        return Check::Incomplete { diagnostics, in_literal: false };
    }
    if !diagnostics.is_empty() {
        return Check::Invalid(diagnostics);
    }
    Check::Ready(program)
}

#[derive(Debug, PartialEq)]
enum Outcome {
    Nothing,
    Exit,
    Reset,
    Declared(Vec<String>),
}

struct Session {
    interpreter: Interpreter,
    sources: Rc<RefCell<Sources>>,
    history: Vec<String>,
    buffer: String,
    in_literal: bool,
    interactive: bool,
    failed: bool,
}

impl Session {
    fn new(interactive: bool) -> Self {
        let sources = Rc::new(RefCell::new(Sources::default()));
        Self {
            interpreter: fresh_interpreter(&sources),
            sources,
            history: Vec::new(),
            buffer: String::new(),
            in_literal: false,
            interactive,
            failed: false,
        }
    }

    fn is_mid_input(&self) -> bool {
        !self.buffer.is_empty()
    }

    fn discard_input(&mut self) {
        self.buffer.clear();
        self.in_literal = false;
    }

    fn cancel(&mut self) {
        self.discard_input();
        if self.interactive {
            println!("Ввод отменён.");
        }
    }

    fn command_in(&self, line: &str) -> Option<ReplCommand> {
        parse_repl_command(line).filter(|command| !self.in_literal || *command == ReplCommand::Cancel)
    }

    fn feed(&mut self, line: &str) -> Outcome {
        match self.command_in(line) {
            Some(ReplCommand::Cancel) => {
                self.cancel();
                return Outcome::Nothing;
            }
            Some(ReplCommand::Exit) => {
                self.discard_input();
                return Outcome::Exit;
            }
            Some(ReplCommand::History) => {
                for (index, entry) in self.history.iter().enumerate() {
                    println!("{}: {entry}", index + 1);
                }
                return Outcome::Nothing;
            }
            Some(ReplCommand::Reset) => {
                self.discard_input();
                self.interpreter = fresh_interpreter(&self.sources);
                if self.interactive {
                    println!("Состояние сброшено.");
                }
                return Outcome::Reset;
            }
            Some(ReplCommand::Repeat(number)) if !self.is_mid_input() => {
                let Some(entry) = self.history.get(number - 1) else {
                    eprintln!("Нет записи с номером {number} в истории.");
                    self.failed = true;
                    return Outcome::Nothing;
                };
                if self.interactive {
                    println!("{entry}");
                }
                self.buffer.clone_from(entry);
            }
            _ if !self.is_mid_input() && line.trim().is_empty() => return Outcome::Nothing,
            _ => self.buffer.push_str(line),
        }
        self.buffer.push('\n');
        self.evaluate_buffer()
    }

    fn pending_source(&self) -> SourceFile {
        SourceFile::with_base(REPL_NAME.to_string(), self.buffer.clone(), self.sources.borrow().next_base())
    }

    fn evaluate_buffer(&mut self) -> Outcome {
        let mut source = self.pending_source();
        let program = match check(&source) {
            Check::Incomplete { in_literal, .. } => {
                self.in_literal = in_literal;
                return Outcome::Nothing;
            }
            Check::Invalid(diagnostics) => {
                print_diagnostics(&source, &diagnostics);
                self.failed = true;
                self.remember_input();
                return Outcome::Nothing;
            }
            Check::Ready(program) => program,
        };

        let declared = completion::declared_names(&program);
        source.name = format!("<repl#{}>", self.remember_input());
        let current = self.sources.borrow_mut().insert(source);
        if self.execute(current.base(), |interpreter| interpreter.run_repl(&program)) {
            Outcome::Declared(declared)
        } else {
            Outcome::Reset
        }
    }

    fn remember_input(&mut self) -> usize {
        self.history.push(self.buffer.trim_end_matches('\n').to_string());
        self.discard_input();
        self.history.len()
    }

    fn execute(
        &mut self,
        current_base: usize,
        run: impl FnOnce(&mut Interpreter) -> Result<Option<Value>, RuntimeError>,
    ) -> bool {
        match guarded(|| run(&mut self.interpreter)) {
            Some(Ok(Some(value))) => println!("{value}"),
            Some(Ok(None)) => {}
            Some(Err(e)) => {
                self.failed = true;
                print_runtime_error(&e, |offset| self.locate(offset, current_base));
            }
            None => {
                self.failed = true;
                eprintln!("Внутренняя ошибка интерпретатора: состояние REPL сброшено");
                abandon_after_panic(std::mem::replace(&mut self.interpreter, fresh_interpreter(&self.sources)));
                return false;
            }
        }
        true
    }

    fn locate(&self, offset: usize, current_base: usize) -> String {
        let sources = self.sources.borrow();
        let Some(file) = sources.lookup(offset) else {
            return format!("{REPL_NAME}:1:1");
        };
        let (line, col) = file.position(offset);
        let name = if file.base() == current_base { REPL_NAME } else { &file.name };
        format!("{name}:{line}:{col}")
    }

    fn finish(&mut self) {
        if !self.is_mid_input() {
            return;
        }
        let source = self.pending_source();
        if let Check::Incomplete { diagnostics, .. } | Check::Invalid(diagnostics) = check(&source) {
            print_diagnostics(&source, &diagnostics);
            self.failed = true;
        }
    }
}

fn fresh_interpreter(sources: &Rc<RefCell<Sources>>) -> Interpreter {
    let mut interpreter = Interpreter::new();
    interpreter.set_sources(Rc::clone(sources));
    interpreter
}

enum LineEvent {
    Line(String),
    Cancelled,
    Eof,
    Failed,
}

enum InputSource {
    Tty(Box<YpsEditor>),
    Piped(io::Stdin),
}

impl InputSource {
    fn open(interactive: bool) -> Self {
        let Some(mut editor) = interactive.then(YpsEditor::new).and_then(Result::ok) else {
            return Self::Piped(io::stdin());
        };
        editor.set_helper(Some(YpsHelper::default()));
        if let Some(path) = history_path() {
            let _ = editor.load_history(&path);
        }
        Self::Tty(Box::new(editor))
    }

    fn read_line(&mut self, continuation: bool) -> LineEvent {
        let line = match self {
            Self::Tty(editor) => match editor.readline(if continuation { "....> " } else { "йопта> " }) {
                Ok(line) => Ok(Some(line)),
                Err(ReadlineError::Interrupted) => return LineEvent::Cancelled,
                Err(ReadlineError::Eof) => Ok(None),
                Err(_) => Err(()),
            },
            Self::Piped(stdin) => {
                let mut line = String::new();
                match stdin.lock().read_line(&mut line) {
                    Ok(0) => Ok(None),
                    Ok(_) => Ok(Some(line.trim_end_matches('\n').trim_end_matches('\r').to_string())),
                    Err(_) => Err(()),
                }
            }
        };
        match line {
            Ok(Some(line)) => LineEvent::Line(line),
            Ok(None) => LineEvent::Eof,
            Err(()) => {
                eprintln!("Ошибка чтения ввода.");
                LineEvent::Failed
            }
        }
    }

    fn remember(&mut self, line: &str) {
        if let Self::Tty(editor) = self
            && !line.trim().is_empty()
            && parse_repl_command(line).is_none()
        {
            let _ = editor.add_history_entry(line);
        }
    }

    fn helper_mut(&mut self) -> Option<&mut YpsHelper> {
        match self {
            Self::Tty(editor) => editor.helper_mut(),
            Self::Piped(_) => None,
        }
    }

    fn save_history(&mut self) {
        if let Self::Tty(editor) = self
            && let Some(path) = history_path()
        {
            let _ = editor.save_history(&path);
        }
    }
}

pub(crate) fn run_repl() -> Result<(), Failure> {
    let interactive = io::stdin().is_terminal();
    let mut input = InputSource::open(interactive);
    let mut session = Session::new(interactive);

    if interactive {
        println!("ЙоптаСкрипт v{}", env!("CARGO_PKG_VERSION"));
        println!("Введите `:выход` для выхода, `:история` для истории, `:сброс` для сброса состояния.");
    }

    loop {
        let line = match input.read_line(session.is_mid_input()) {
            LineEvent::Eof => break,
            LineEvent::Failed => {
                session.failed = true;
                break;
            }
            LineEvent::Cancelled => {
                session.cancel();
                continue;
            }
            LineEvent::Line(line) => line,
        };
        input.remember(&line);

        match session.feed(&line) {
            Outcome::Nothing => {}
            Outcome::Exit => break,
            Outcome::Reset => {
                if let Some(helper) = input.helper_mut() {
                    helper.reset_locals();
                }
            }
            Outcome::Declared(names) => {
                if let Some(helper) = input.helper_mut() {
                    helper.record_declarations(names);
                }
            }
        }
    }

    input.save_history();
    session.finish();

    if interactive {
        println!();
        Ok(())
    } else if session.failed {
        Err(Failure::reported())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use rustyline::history::History;

    use super::*;

    #[test]
    fn history_path_prefers_env_override() {
        let path = history_path_from(Some(PathBuf::from("/tmp/custom_history")), Some(PathBuf::from("/home/user")));

        assert_eq!(path, Some(PathBuf::from("/tmp/custom_history")));
    }

    #[test]
    fn history_path_falls_back_to_home() {
        let path = history_path_from(None, Some(PathBuf::from("/home/user")));

        assert_eq!(path, Some(PathBuf::from("/home/user/.yps_history")));
    }

    #[test]
    fn history_path_ignores_an_empty_override() {
        let path = history_path_from(Some(PathBuf::new()), Some(PathBuf::from("/home/user")));

        assert_eq!(path, Some(PathBuf::from("/home/user/.yps_history")));
    }

    #[test]
    fn history_path_none_when_nothing_available() {
        assert_eq!(history_path_from(None, None), None);
    }

    #[test]
    fn history_path_uses_the_platform_home_directory() {
        if env::var_os("YPS_HISTORY_FILE").is_some() {
            return;
        }

        assert_eq!(history_path(), env::home_dir().map(|home| home.join(".yps_history")));
    }

    #[test]
    fn parse_cmd_exit() {
        assert_eq!(parse_repl_command(":выход"), Some(ReplCommand::Exit));
    }

    #[test]
    fn parse_cmd_history() {
        assert_eq!(parse_repl_command(":история"), Some(ReplCommand::History));
    }

    #[test]
    fn parse_cmd_reset() {
        assert_eq!(parse_repl_command(":сброс"), Some(ReplCommand::Reset));
    }

    #[test]
    fn parse_cmd_cancel() {
        assert_eq!(parse_repl_command(":отмена"), Some(ReplCommand::Cancel));
    }

    #[test]
    fn parse_cmd_repeat() {
        assert_eq!(parse_repl_command("!3"), Some(ReplCommand::Repeat(3)));
    }

    #[test]
    fn parse_cmd_repeat_zero_is_none() {
        assert_eq!(parse_repl_command("!0"), None);
    }

    #[test]
    fn parse_cmd_code_is_none() {
        assert_eq!(parse_repl_command("гыы х = 1;"), None);
    }

    #[test]
    fn parse_cmd_unknown_is_none() {
        assert_eq!(parse_repl_command(":неизвестно"), None);
    }

    #[test]
    fn line_editor_history_keeps_code_and_skips_repl_commands() {
        let mut input = InputSource::Tty(Box::new(YpsEditor::new().unwrap()));

        for line in [":история", "гыы а = 1;", "!1", "   ", ":выход", ":сброс", ":отмена"] {
            input.remember(line);
        }

        let InputSource::Tty(editor) = &input else { unreachable!() };
        assert_eq!(editor.history().len(), 1);
        let kept = editor.history().get(0, rustyline::history::SearchDirection::Forward).unwrap().unwrap();
        assert_eq!(kept.entry, "гыы а = 1;");
    }

    #[test]
    fn feeding_a_declaration_reports_the_declared_names() {
        let mut session = Session::new(false);

        assert_eq!(session.feed("гыы { а, б } = { а: 1, б: 2 };"), Outcome::Declared(vec!["а".into(), "б".into()]));
        assert!(!session.failed);
    }

    #[test]
    fn an_unfinished_input_waits_for_more_lines() {
        let mut session = Session::new(false);

        assert_eq!(session.feed("йопта ф() {"), Outcome::Nothing);
        assert!(session.is_mid_input());
        assert_eq!(session.feed("}"), Outcome::Declared(vec!["ф".into()]));
        assert!(!session.is_mid_input());
        assert_eq!(session.history, ["йопта ф() {\n}"]);
    }

    #[test]
    fn exit_and_reset_work_in_the_middle_of_an_input() {
        let mut session = Session::new(false);
        session.feed("йопта ф() {");

        assert_eq!(session.feed(":сброс"), Outcome::Reset);
        assert!(!session.is_mid_input());

        session.feed("йопта ф() {");
        assert_eq!(session.feed(":выход"), Outcome::Exit);
        assert!(!session.is_mid_input());
        assert!(!session.failed);
    }

    #[test]
    fn reset_drops_the_interpreter_state() {
        let mut session = Session::new(false);
        session.feed("гыы z = 5;");
        assert!(session.interpreter.get("z").is_some());

        session.feed(":сброс");

        assert!(session.interpreter.get("z").is_none());
    }

    #[test]
    fn a_panic_during_evaluation_resets_the_session_instead_of_killing_it() {
        let mut session = Session::new(false);
        session.feed("гыы z = 5;");

        let survived = session.execute(0, |_| panic!("сбой интерпретатора"));

        assert!(!survived);
        assert!(session.failed);
        assert!(session.interpreter.get("z").is_none());
        assert_eq!(session.feed("гыы у = 1;"), Outcome::Declared(vec!["у".into()]));
    }

    #[test]
    fn errors_are_located_in_the_input_that_owns_the_offset() {
        let mut session = Session::new(false);
        session.feed("гыы первая = 1;");
        session.feed("гыы вторая = 2;");
        let second_base = session.sources.borrow().lookup("гыы первая = 1;\n".len() + 1).unwrap().base();

        assert_eq!(session.locate(7, second_base), "<repl#1>:1:5");
        assert_eq!(session.locate(second_base + 7, second_base), "<repl>:1:5");
    }

    #[test]
    fn a_rejected_input_marks_the_session_failed_and_stays_in_history() {
        let mut session = Session::new(false);

        assert_eq!(session.feed("гыы а = §;"), Outcome::Nothing);

        assert!(session.failed);
        assert_eq!(session.history, ["гыы а = §;"]);
        assert!(!session.is_mid_input());
    }
}
