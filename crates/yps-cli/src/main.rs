use std::env;
use std::fs;
use std::io::{self, Write as IoWrite};
use std::path::PathBuf;
use std::process;

use yps_interpreter::Interpreter;
use yps_lexer::{Lexer, SourceFile};
use yps_parser::Parser;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Использование: yps <файл.yop> | yps fmt <файл.yop> [--write|-w] [--check]");
        process::exit(1);
    }

    if args[1] == "fmt" {
        run_fmt(&args[2..]);
    } else {
        run_interpret(&args[1]);
    }
}

fn run_fmt(args: &[String]) {
    if args.is_empty() {
        eprintln!("Использование: yps fmt <файл.yop> [--write|-w] [--check]");
        process::exit(1);
    }

    let filename = &args[0];
    let mut write_in_place = false;
    let mut check_only = false;

    for flag in &args[1..] {
        match flag.as_str() {
            "--write" | "-w" => write_in_place = true,
            "--check" => check_only = true,
            other => {
                eprintln!("Неизвестный флаг: {other}");
                process::exit(1);
            }
        }
    }

    let source = match fs::read_to_string(filename) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Не удалось прочитать файл '{filename}': {e}");
            process::exit(1);
        }
    };

    let outcome = match yps_fmt::format_source(&source) {
        Ok(o) => o,
        Err(yps_fmt::FormatError::ParseError(diags)) => {
            let sf = SourceFile::new(filename.clone(), source.clone());
            for d in &diags {
                let (line, col) = sf.position(d.span.start);
                eprintln!("{filename}:{line}:{col}: {:?}: {}", d.severity, d.message);
            }
            eprintln!("Форматирование отклонено: файл содержит синтаксические ошибки");
            process::exit(1);
        }
        Err(yps_fmt::FormatError::RoundTripFailed(msg)) => {
            eprintln!("Форматирование отклонено: самопроверка не прошла: {msg}");
            process::exit(1);
        }
        Err(yps_fmt::FormatError::CommentRefused(msg)) => {
            eprintln!("Форматирование отклонено: {msg}");
            process::exit(1);
        }
    };

    if check_only {
        if outcome.already_formatted {
            process::exit(0);
        } else {
            process::exit(1);
        }
    }

    if write_in_place {
        let tmp_path = format!("{filename}.fmt_tmp");
        if let Err(e) = fs::write(&tmp_path, &outcome.text) {
            eprintln!("Не удалось записать временный файл '{tmp_path}': {e}");
            process::exit(1);
        }
        if let Err(e) = fs::rename(&tmp_path, filename) {
            eprintln!("Не удалось переименовать '{tmp_path}' в '{filename}': {e}");
            let _ = fs::remove_file(&tmp_path);
            process::exit(1);
        }
    } else {
        let stdout = io::stdout();
        let mut handle = stdout.lock();
        if let Err(e) = handle.write_all(outcome.text.as_bytes()) {
            eprintln!("Ошибка записи в stdout: {e}");
            process::exit(1);
        }
    }
}

fn run_interpret(filename: &str) {
    let code = match fs::read_to_string(filename) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Не удалось прочитать файл '{filename}': {e}");
            process::exit(1);
        }
    };

    let source = SourceFile::new(filename.to_string(), code);

    let lexer = Lexer::new(&source);
    let (tokens, lex_diagnostics) = lexer.tokenize();

    if !lex_diagnostics.is_empty() {
        for d in &lex_diagnostics {
            let (line, col) = source.position(d.span.start);
            eprintln!("{filename}:{line}:{col}: {:?}: {}", d.severity, d.message);
        }
        process::exit(1);
    }

    let parser = Parser::new(&tokens, &source);
    let (program, parse_diagnostics) = parser.parse_program();

    if !parse_diagnostics.is_empty() {
        for d in &parse_diagnostics {
            let (line, col) = source.position(d.span.start);
            eprintln!("{filename}:{line}:{col}: {:?}: {}", d.severity, d.message);
        }
        process::exit(1);
    }

    let mut interpreter = Interpreter::new();
    if let Some(parent) = PathBuf::from(filename).parent().map(PathBuf::from) {
        interpreter.set_base_path(parent);
    }
    if let Err(e) = interpreter.run(&program) {
        let (line, col) = source.position(e.span.start);
        eprintln!("{filename}:{line}:{col}: {e}");
        for frame in &e.stack {
            let (fl, fc) = source.position(frame.span.start);
            eprintln!("  в {}:{filename}:{fl}:{fc}", frame.name);
        }
        process::exit(1);
    }
}
