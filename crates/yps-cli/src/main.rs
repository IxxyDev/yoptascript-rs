use std::env;
use std::fs;
use std::path::PathBuf;
use std::process;

use yps_interpreter::Interpreter;
use yps_lexer::{Lexer, SourceFile};
use yps_parser::Parser;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Использование: yps <файл.yop>");
        process::exit(1);
    }

    let filename = &args[1];
    let code = match fs::read_to_string(filename) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Не удалось прочитать файл '{filename}': {e}");
            process::exit(1);
        }
    };

    let source = SourceFile::new(filename.clone(), code);

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
        process::exit(1);
    }
}
