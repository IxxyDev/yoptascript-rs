use std::env;
use std::io::{self, BufRead, IsTerminal, Write as IoWrite};
use std::process;

use yps_interpreter::Interpreter;
use yps_lexer::{Lexer, SourceFile};
use yps_parser::Parser;

use crate::{print_diagnostics, print_runtime_error};

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

fn print_history(history: &[String]) {
    for (i, entry) in history.iter().enumerate() {
        println!("{}", format_history_line(i, entry));
    }
}

fn prompt(is_tty: bool, continuation: bool) {
    if is_tty {
        if continuation {
            print!("....> ");
        } else {
            print!("йопта> ");
        }
        let _ = io::stdout().flush();
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

    let stdin = io::stdin();
    let mut history: Vec<String> = Vec::new();
    let mut buffer = String::new();

    loop {
        prompt(is_tty, !buffer.is_empty());

        let mut line = String::new();
        let result = stdin.lock().read_line(&mut line);
        let line = match result {
            Ok(0) => break,
            Ok(_) => line.trim_end_matches('\n').trim_end_matches('\r').to_string(),
            Err(_) => {
                eprintln!("Ошибка чтения ввода.");
                break;
            }
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
                        buffer.push_str(&repeated);
                        buffer.push('\n');
                    }
                }
            } else {
                buffer.push_str(&line);
                buffer.push('\n');
            }
        } else if buffer.is_empty() {
            if line.trim().is_empty() {
                continue;
            }
            buffer.push_str(&line);
            buffer.push('\n');
        } else {
            buffer.push_str(&line);
            buffer.push('\n');
        }

        let source = SourceFile::new("<repl>".to_string(), buffer.clone());
        let lexer = Lexer::new(&source);
        let (tokens, lex_diags) = lexer.tokenize();

        if !lex_diags.is_empty() {
            print_diagnostics(&source, &lex_diags, "<repl>");
            buffer.clear();
            continue;
        }

        let parser = Parser::new(&tokens, &source);
        let (program, parse_diags, unexpected_eof) = parser.parse_program_extended();

        if !parse_diags.is_empty() {
            if unexpected_eof {
                continue;
            }
            print_diagnostics(&source, &parse_diags, "<repl>");
            let completed_input = buffer.trim_end_matches('\n').to_string();
            history.push(completed_input);
            buffer.clear();
            continue;
        }

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
        let lexer = Lexer::new(&source);
        let (tokens, lex_diags) = lexer.tokenize();
        let has_errors;
        if !lex_diags.is_empty() {
            print_diagnostics(&source, &lex_diags, "<repl>");
            has_errors = true;
        } else {
            let parser = Parser::new(&tokens, &source);
            let (_, parse_diags, _) = parser.parse_program_extended();
            if !parse_diags.is_empty() {
                print_diagnostics(&source, &parse_diags, "<repl>");
                has_errors = true;
            } else {
                has_errors = false;
            }
        }
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
