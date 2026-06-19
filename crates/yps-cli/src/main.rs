use std::env;
use std::fs;
use std::io::{self, Write as IoWrite};
use std::path::PathBuf;
use std::process;

use yps_interpreter::{Interpreter, RuntimeError};
use yps_lexer::{Diagnostic, Lexer, SourceFile};
use yps_parser::Parser;

mod repl;

pub(crate) fn print_diagnostics(source: &SourceFile, diagnostics: &[Diagnostic], name: &str) {
    for d in diagnostics {
        let (line, col) = source.position(d.span.start);
        eprintln!("{name}:{line}:{col}: {:?}: {}", d.severity, d.message);
    }
}

pub(crate) fn print_runtime_error(source: &SourceFile, e: &RuntimeError, name: &str) {
    let (line_n, col) = source.position(e.span.start);
    eprintln!("{name}:{line_n}:{col}: {e}");
    for frame in &e.stack {
        let (fl, fc) = source.position(frame.span.start);
        eprintln!("  в {}:{name}:{fl}:{fc}", frame.name);
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        repl::run_repl();
        return;
    }

    if args[1] == "fmt" {
        run_fmt(&args[2..]);
    } else if args[1] == "repl" {
        repl::run_repl();
    } else {
        let use_vm = args[1..].iter().any(|a| a == "--vm");
        match args[1..].iter().find(|a| !a.starts_with("--")) {
            Some(file) if use_vm => run_vm(file),
            Some(file) => run_interpret(file),
            None => {
                eprintln!("Не указан файл для выполнения");
                process::exit(1);
            }
        }
    }
}

fn run_vm(filename: &str) {
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
        print_diagnostics(&source, &lex_diagnostics, filename);
        process::exit(1);
    }

    let parser = Parser::new(&tokens, &source);
    let (program, parse_diagnostics) = parser.parse_program();
    if !parse_diagnostics.is_empty() {
        print_diagnostics(&source, &parse_diagnostics, filename);
        process::exit(1);
    }

    let base = PathBuf::from(filename).parent().map(PathBuf::from);
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| yps_vm::execute_with_base(&program, base)));
    match outcome {
        Ok(Ok(())) => {}
        Ok(Err(e)) => {
            let (line, col) = source.position(e.span().start);
            eprintln!("{filename}:{line}:{col}: {e}");
            process::exit(1);
        }
        Err(_) => {
            eprintln!("Внутренняя ошибка VM: выполнение прервано");
            process::exit(70);
        }
    }
}

fn run_fmt(args: &[String]) {
    if args.is_empty() {
        eprintln!("Использование: yps fmt <файл.yop> [--write|-w] [--check] [--source-map]");
        process::exit(1);
    }

    let filename = &args[0];
    let mut write_in_place = false;
    let mut check_only = false;
    let mut source_map = false;

    for flag in &args[1..] {
        match flag.as_str() {
            "--write" | "-w" => write_in_place = true,
            "--check" => check_only = true,
            "--source-map" => source_map = true,
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

    let handle_fmt_err = |e: yps_fmt::FormatError| -> ! {
        match e {
            yps_fmt::FormatError::ParseError(diags) => {
                let sf = SourceFile::new(filename.clone(), source.clone());
                for d in &diags {
                    let (line, col) = sf.position(d.span.start);
                    eprintln!("{filename}:{line}:{col}: {:?}: {}", d.severity, d.message);
                }
                eprintln!("Форматирование отклонено: файл содержит синтаксические ошибки");
            }
            yps_fmt::FormatError::RoundTripFailed(msg) => {
                eprintln!("Форматирование отклонено: самопроверка не прошла: {msg}");
            }
            yps_fmt::FormatError::CommentRefused(msg) => {
                eprintln!("Форматирование отклонено: {msg}");
            }
        }
        process::exit(1)
    };

    if source_map {
        let (outcome, mut map) = match yps_fmt::format_source_with_map(&source) {
            Ok(r) => r,
            Err(e) => handle_fmt_err(e),
        };

        let map_path = format!("{filename}.map");
        map.file = map_path.clone();
        map.source_name = filename.clone();

        if check_only {
            process::exit(if outcome.already_formatted { 0 } else { 1 });
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
            if let Err(e) = fs::write(&map_path, map.to_json()) {
                eprintln!("Не удалось записать source map '{map_path}': {e}");
                process::exit(1);
            }
        } else {
            let stdout = io::stdout();
            let mut handle = stdout.lock();
            if let Err(e) = handle.write_all(outcome.text.as_bytes()) {
                eprintln!("Ошибка записи в stdout: {e}");
                process::exit(1);
            }
            if let Err(e) = writeln!(handle, "{}", map.to_json()) {
                eprintln!("Ошибка записи source map в stdout: {e}");
                process::exit(1);
            }
        }
        return;
    }

    let outcome = match yps_fmt::format_source(&source) {
        Ok(o) => o,
        Err(e) => handle_fmt_err(e),
    };

    if check_only {
        process::exit(if outcome.already_formatted { 0 } else { 1 });
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
        print_diagnostics(&source, &lex_diagnostics, filename);
        process::exit(1);
    }

    let parser = Parser::new(&tokens, &source);
    let (program, parse_diagnostics) = parser.parse_program();

    if !parse_diagnostics.is_empty() {
        print_diagnostics(&source, &parse_diagnostics, filename);
        process::exit(1);
    }

    let mut interpreter = Interpreter::new();
    if let Some(parent) = PathBuf::from(filename).parent().map(PathBuf::from) {
        interpreter.set_base_path(parent);
    }
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| interpreter.run(&program)));
    match outcome {
        Ok(Ok(())) => {}
        Ok(Err(e)) => {
            print_runtime_error(&source, &e, filename);
            process::exit(1);
        }
        Err(_) => {
            eprintln!("Внутренняя ошибка интерпретатора: выполнение прервано");
            process::exit(70);
        }
    }
}
