use std::env;
use std::io::{self, BufRead, IsTerminal};
use std::process;

use rustyline::DefaultEditor;
use rustyline::error::ReadlineError;

use yps_interpreter::Interpreter;
use yps_lexer::{Lexer, SourceFile};
use yps_parser::{Parser, Program};

use crate::{print_diagnostics, print_runtime_error};

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
    Tty(Box<DefaultEditor>),
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
        match DefaultEditor::new() {
            Ok(editor) => InputSource::Tty(Box::new(editor)),
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
    use super::*;

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
