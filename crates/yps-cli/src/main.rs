use std::env;
use std::fs;
use std::io::{self, Read as IoRead, Write as IoWrite};
use std::path::PathBuf;
use std::process;

use yps_interpreter::{Interpreter, RuntimeError};
use yps_lexer::{Diagnostic, Lexer, SourceFile};
use yps_parser::{Parser, Program};

mod repl;

const INTERNAL_ERROR_EXIT_CODE: i32 = 70;

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

const HELP_TEXT: &str = "Использование: yps [ФЛАГИ] [ФАЙЛ]
       yps repl
       yps fmt <файл.yopta> [--write|-w] [--check] [--source-map]

Выполнение программы:
  yps ФАЙЛ                  выполнить файл на дереве интерпретации
  yps --vm ФАЙЛ             выполнить файл на байткодовой VM
  yps -e \"код\", --eval \"код\"  выполнить код, переданный строкой
  yps -                     выполнить код, прочитанный из stdin
  yps repl                  запустить интерактивный REPL
  yps                       без аргументов — тоже REPL

Форматирование:
  yps fmt <файл.yopta>              напечатать отформатированный код в stdout
  yps fmt <файл.yopta> --write|-w   переписать файл на месте
  yps fmt <файл.yopta> --check      проверить, отформатирован ли файл (код выхода)
  yps fmt <файл.yopta> --source-map добавить source map к результату

Прочее:
  -h, --help       показать эту справку
  -V, --version    показать версию";

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        repl::run_repl();
        return;
    }

    match args[1].as_str() {
        "--version" | "-V" => {
            println!("yps {}", env!("CARGO_PKG_VERSION"));
        }
        "--help" | "-h" => {
            println!("{HELP_TEXT}");
        }
        "fmt" => run_fmt(&args[2..]),
        "repl" => repl::run_repl(),
        _ => run_program(&args[1..]),
    }
}

fn run_program(args: &[String]) {
    let mut use_vm = false;
    let mut eval_code: Option<String> = None;
    let mut use_stdin = false;
    let mut file: Option<String> = None;

    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_str();
        match arg {
            "--vm" => use_vm = true,
            "-e" | "--eval" => {
                i += 1;
                match args.get(i) {
                    Some(code) => eval_code = Some(code.clone()),
                    None => {
                        eprintln!("Флаг {arg} требует аргумент с кодом");
                        process::exit(1);
                    }
                }
            }
            "-" => use_stdin = true,
            other if other.starts_with('-') => {
                eprintln!("Неизвестный флаг: {other}");
                process::exit(1);
            }
            other => {
                if file.is_some() {
                    eprintln!("Указан более чем один файл: {other}");
                    process::exit(1);
                }
                file = Some(other.to_string());
            }
        }
        i += 1;
    }

    if let Some(code) = eval_code {
        let source = SourceFile::new("<eval>".to_string(), code);
        let program = parse_or_exit(&source);
        execute(source, program, None, use_vm);
        return;
    }

    if use_stdin {
        let mut code = String::new();
        if let Err(e) = io::stdin().read_to_string(&mut code) {
            eprintln!("Не удалось прочитать stdin: {e}");
            process::exit(1);
        }
        let source = SourceFile::new("<stdin>".to_string(), code);
        let program = parse_or_exit(&source);
        execute(source, program, None, use_vm);
        return;
    }

    match file {
        Some(filename) => {
            let (source, program) = load_program(&filename);
            let base = PathBuf::from(&filename).parent().map(PathBuf::from);
            execute(source, program, base, use_vm);
        }
        None => {
            eprintln!("Не указан файл для выполнения");
            process::exit(1);
        }
    }
}

fn execute(source: SourceFile, program: Program, base: Option<PathBuf>, use_vm: bool) {
    if use_vm {
        run_vm(source, program, base);
    } else {
        run_interpret(source, program, base);
    }
}

fn parse_or_exit(source: &SourceFile) -> Program {
    let (tokens, lex_diagnostics) = Lexer::new(source).tokenize();
    if !lex_diagnostics.is_empty() {
        print_diagnostics(source, &lex_diagnostics, &source.name);
        process::exit(1);
    }

    let (program, parse_diagnostics) = Parser::new(&tokens, source).parse_program();
    if !parse_diagnostics.is_empty() {
        print_diagnostics(source, &parse_diagnostics, &source.name);
        process::exit(1);
    }

    program
}

fn load_program(filename: &str) -> (SourceFile, Program) {
    let code = match fs::read_to_string(filename) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Не удалось прочитать файл '{filename}': {e}");
            process::exit(1);
        }
    };

    let source = SourceFile::new(filename.to_string(), code);
    let program = parse_or_exit(&source);
    (source, program)
}

fn run_vm(source: SourceFile, program: Program, base: Option<PathBuf>) {
    let name = source.name.clone();
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| yps_vm::execute_with_base(&program, base)));
    match outcome {
        Ok(Ok(())) => {}
        Ok(Err(e)) => {
            let (line, col) = source.position(e.span().start);
            eprintln!("{name}:{line}:{col}: {e}");
            process::exit(1);
        }
        Err(_) => {
            eprintln!("Внутренняя ошибка VM: выполнение прервано");
            process::exit(INTERNAL_ERROR_EXIT_CODE);
        }
    }
}

fn write_atomic(filename: &str, contents: &[u8]) {
    let tmp_path = format!("{filename}.fmt_tmp");
    if let Err(e) = fs::write(&tmp_path, contents) {
        eprintln!("Не удалось записать временный файл '{tmp_path}': {e}");
        process::exit(1);
    }
    if let Err(e) = fs::rename(&tmp_path, filename) {
        eprintln!("Не удалось переименовать '{tmp_path}' в '{filename}': {e}");
        let _ = fs::remove_file(&tmp_path);
        process::exit(1);
    }
}

fn run_fmt(args: &[String]) {
    if args.is_empty() {
        eprintln!("Использование: yps fmt <файл.yopta> [--write|-w] [--check] [--source-map]");
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
                print_diagnostics(&sf, &diags, filename);
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
            write_atomic(filename, outcome.text.as_bytes());
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
        write_atomic(filename, outcome.text.as_bytes());
    } else {
        let stdout = io::stdout();
        let mut handle = stdout.lock();
        if let Err(e) = handle.write_all(outcome.text.as_bytes()) {
            eprintln!("Ошибка записи в stdout: {e}");
            process::exit(1);
        }
    }
}

fn run_interpret(source: SourceFile, program: Program, base: Option<PathBuf>) {
    let name = source.name.clone();
    let mut interpreter = Interpreter::new();
    if let Some(parent) = base {
        interpreter.set_base_path(parent);
    }
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| interpreter.run(&program)));
    match outcome {
        Ok(Ok(())) => {}
        Ok(Err(e)) => {
            print_runtime_error(&source, &e, &name);
            process::exit(1);
        }
        Err(_) => {
            eprintln!("Внутренняя ошибка интерпретатора: выполнение прервано");
            process::exit(INTERNAL_ERROR_EXIT_CODE);
        }
    }
}
