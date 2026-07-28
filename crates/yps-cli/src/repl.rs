use std::env;
use std::io::{self, BufRead, IsTerminal};
use std::path::PathBuf;
use std::process;

use rustyline::Editor;
use rustyline::error::ReadlineError;
use rustyline::history::DefaultHistory;

use yps_interpreter::Interpreter;
use yps_lexer::{Lexer, SourceFile};
use yps_parser::{Parser, Program};

use crate::completion::YpsHelper;
use crate::{print_diagnostics, print_runtime_error};

type YpsEditor = Editor<YpsHelper, DefaultHistory>;

fn history_path_from(env_override: Option<String>, home: Option<String>) -> Option<PathBuf> {
    if let Some(p) = env_override {
        return Some(PathBuf::from(p));
    }
    home.map(|home| PathBuf::from(home).join(".yps_history"))
}

fn history_path() -> Option<PathBuf> {
    history_path_from(env::var("YPS_HISTORY_FILE").ok(), env::var("HOME").ok())
}

enum CheckOutcome {
    Ready(Program),
    Incomplete,
    LexError,
    ParseError,
}

fn check_and_report(source: &SourceFile, report_incomplete: bool) -> CheckOutcome {
    let lexer = Lexer::new(source);
    let (tokens, lex_diags) = lexer.tokenize();
    if !lex_diags.is_empty() {
        print_diagnostics(source, &lex_diags, "<repl>");
        return CheckOutcome::LexError;
    }

    let parser = Parser::new(&tokens, source);
    let (program, parse_diags, unexpected_eof) = parser.parse_program_extended();
    if !parse_diags.is_empty() {
        if unexpected_eof && !report_incomplete {
            return CheckOutcome::Incomplete;
        }
        print_diagnostics(source, &parse_diags, "<repl>");
        return if unexpected_eof { CheckOutcome::Incomplete } else { CheckOutcome::ParseError };
    }

    CheckOutcome::Ready(program)
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
    let trimmed = input.trim();
    match trimmed {
        ":выход" => Some(ReplCommand::Exit),
        ":история" => Some(ReplCommand::History),
        ":сброс" => Some(ReplCommand::Reset),
        ":отмена" => Some(ReplCommand::Cancel),
        s if s.starts_with('!') => {
            let num_str = s[1..].trim();
            num_str.parse::<usize>().ok().filter(|&n| n >= 1).map(ReplCommand::Repeat)
        }
        _ => None,
    }
}

fn format_history_line(i: usize, entry: &str) -> String {
    format!("{}: {}", i + 1, entry)
}

fn push_line(buffer: &mut String, line: &str) {
    buffer.push_str(line);
    buffer.push('\n');
}

fn print_history(history: &[String]) {
    for (i, entry) in history.iter().enumerate() {
        println!("{}", format_history_line(i, entry));
    }
}

enum LineEvent {
    Line(String),
    Cancelled,
    Eof,
}

enum InputSource {
    Tty(Box<YpsEditor>),
    Piped(io::Stdin),
}

impl InputSource {
    fn read_line(&mut self, continuation: bool) -> LineEvent {
        match self {
            InputSource::Tty(editor) => {
                let prompt = if continuation { "....> " } else { "йопта> " };
                match editor.readline(prompt) {
                    Ok(line) => {
                        if !line.trim().is_empty() {
                            let _ = editor.add_history_entry(&line);
                        }
                        LineEvent::Line(line)
                    }
                    Err(ReadlineError::Interrupted) => LineEvent::Cancelled,
                    Err(ReadlineError::Eof) => LineEvent::Eof,
                    Err(_) => {
                        eprintln!("Ошибка чтения ввода.");
                        LineEvent::Eof
                    }
                }
            }
            InputSource::Piped(stdin) => {
                let mut line = String::new();
                match stdin.lock().read_line(&mut line) {
                    Ok(0) => LineEvent::Eof,
                    Ok(_) => LineEvent::Line(line.trim_end_matches('\n').trim_end_matches('\r').to_string()),
                    Err(_) => {
                        eprintln!("Ошибка чтения ввода.");
                        LineEvent::Eof
                    }
                }
            }
        }
    }

    fn record_declarations(&self, source_text: &str) {
        if let InputSource::Tty(editor) = self
            && let Some(helper) = editor.helper()
        {
            helper.record_declarations(source_text);
        }
    }

    fn reset_locals(&self) {
        if let InputSource::Tty(editor) = self
            && let Some(helper) = editor.helper()
        {
            helper.reset_locals();
        }
    }

    fn save_history(&mut self) {
        if let InputSource::Tty(editor) = self
            && let Some(path) = history_path()
        {
            let _ = editor.save_history(&path);
        }
    }
}

pub fn run_repl() {
    let is_tty = io::stdin().is_terminal();
    let mut interpreter = Interpreter::new();
    if let Ok(cwd) = env::current_dir() {
        interpreter.set_base_path(cwd);
    }

    if is_tty {
        let version = env!("CARGO_PKG_VERSION");
        println!("ЙоптаСкрипт v{version}");
        println!("Введите `:выход` для выхода, `:история` для истории, `:сброс` для сброса состояния.");
    }

    let mut input = if is_tty {
        match YpsEditor::new() {
            Ok(mut editor) => {
                editor.set_helper(Some(YpsHelper::new()));
                if let Some(path) = history_path() {
                    let _ = editor.load_history(&path);
                }
                InputSource::Tty(Box::new(editor))
            }
            Err(_) => InputSource::Piped(io::stdin()),
        }
    } else {
        InputSource::Piped(io::stdin())
    };
    let mut history: Vec<String> = Vec::new();
    let mut buffer = String::new();

    loop {
        let line = match input.read_line(!buffer.is_empty()) {
            LineEvent::Eof => break,
            LineEvent::Cancelled => {
                buffer.clear();
                println!("Ввод отменён.");
                continue;
            }
            LineEvent::Line(l) => l,
        };

        if let Some(cmd) = parse_repl_command(&line) {
            if cmd == ReplCommand::Cancel {
                buffer.clear();
                if is_tty {
                    println!("Ввод отменён.");
                }
                continue;
            }

            if buffer.is_empty() {
                match cmd {
                    ReplCommand::Exit => {
                        if is_tty {
                            println!();
                        }
                        input.save_history();
                        process::exit(0);
                    }
                    ReplCommand::History => {
                        print_history(&history);
                        continue;
                    }
                    ReplCommand::Reset => {
                        interpreter = Interpreter::new();
                        if let Ok(cwd) = env::current_dir() {
                            interpreter.set_base_path(cwd);
                        }
                        input.reset_locals();
                        if is_tty {
                            println!("Состояние сброшено.");
                        }
                        continue;
                    }
                    ReplCommand::Cancel => unreachable!(),
                    ReplCommand::Repeat(n) => {
                        if n > history.len() {
                            eprintln!("Нет записи с номером {n} в истории.");
                            continue;
                        }
                        let repeated = history[n - 1].clone();
                        if is_tty {
                            println!("{repeated}");
                        }
                        push_line(&mut buffer, &repeated);
                    }
                }
            } else {
                push_line(&mut buffer, &line);
            }
        } else {
            if buffer.is_empty() && line.trim().is_empty() {
                continue;
            }
            push_line(&mut buffer, &line);
        }

        let source = SourceFile::new("<repl>".to_string(), buffer.clone());

        let program = match check_and_report(&source, false) {
            CheckOutcome::Incomplete => continue,
            CheckOutcome::LexError => {
                buffer.clear();
                continue;
            }
            CheckOutcome::ParseError => {
                let completed_input = buffer.trim_end_matches('\n').to_string();
                history.push(completed_input);
                buffer.clear();
                continue;
            }
            CheckOutcome::Ready(program) => program,
        };

        let completed_input = buffer.trim_end_matches('\n').to_string();
        input.record_declarations(&completed_input);
        history.push(completed_input);
        buffer.clear();

        match interpreter.run_repl(&program) {
            Ok(Some(value)) => println!("{value}"),
            Ok(None) => {}
            Err(e) => {
                print_runtime_error(&source, &e, "<repl>");
            }
        }
    }

    input.save_history();

    if !buffer.is_empty() {
        let source = SourceFile::new("<repl>".to_string(), buffer.clone());
        let has_errors = !matches!(check_and_report(&source, true), CheckOutcome::Ready(_));
        if is_tty {
            println!();
        } else if has_errors {
            process::exit(1);
        }
    } else if is_tty {
        println!();
    }
}

#[cfg(test)]
mod tests {
    use rustyline::history::History;

    use super::*;

    #[test]
    fn history_path_prefers_env_override() {
        let path = history_path_from(Some("/tmp/custom_history".to_string()), Some("/home/user".to_string()));
        assert_eq!(path, Some(PathBuf::from("/tmp/custom_history")));
    }

    #[test]
    fn history_path_falls_back_to_home() {
        let path = history_path_from(None, Some("/home/user".to_string()));
        assert_eq!(path, Some(PathBuf::from("/home/user/.yps_history")));
    }

    #[test]
    fn history_path_none_when_nothing_available() {
        let path = history_path_from(None, None);
        assert_eq!(path, None);
    }

    #[test]
    fn history_round_trip_persists_entries_across_editor_instances() {
        let mut history_file = std::env::temp_dir();
        history_file.push(format!("yps_repl_history_test_{}", process::id()));
        let _ = std::fs::remove_file(&history_file);

        {
            let mut editor = YpsEditor::new().unwrap();
            editor.add_history_entry("гыы а = 1;").unwrap();
            editor.add_history_entry("сказать(а);").unwrap();
            editor.save_history(&history_file).unwrap();
        }

        {
            let mut editor = YpsEditor::new().unwrap();
            editor.load_history(&history_file).unwrap();
            let first = editor.history().get(0, rustyline::history::SearchDirection::Forward).unwrap().unwrap();
            let second = editor.history().get(1, rustyline::history::SearchDirection::Forward).unwrap().unwrap();
            assert_eq!(first.entry, "гыы а = 1;");
            assert_eq!(second.entry, "сказать(а);");
        }

        let _ = std::fs::remove_file(&history_file);
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
    fn format_history_line_first() {
        assert_eq!(format_history_line(0, "гыы х = 1;"), "1: гыы х = 1;");
    }

    #[test]
    fn format_history_line_second() {
        assert_eq!(format_history_line(1, "х + 2;"), "2: х + 2;");
    }
}
