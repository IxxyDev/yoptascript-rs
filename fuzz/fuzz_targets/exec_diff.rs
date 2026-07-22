#![no_main]

use libfuzzer_sys::fuzz_target;
use std::fs::OpenOptions;
use std::io::{Read, Seek, SeekFrom, Write as _};
use std::os::unix::io::AsRawFd;
use yps_interpreter::Interpreter;
use yps_lexer::{Lexer, SourceFile};
use yps_parser::Parser;
use yps_parser::ast::Program;

const TIMER_MARKERS: [&str; 3] = ["чутка", "интервал", "подождать"];

const KNOWN_DIVERGENCES: [&str; 2] = ["найтиВсе", "RegExp("];

const KNOWN_VM_ONLY_ERRORS: [&str; 1] = ["недопустимая цель присваивания в VM"];

const KNOWN_INTERP_ONLY_ERRORS: [&str; 1] = ["Операция требует числа"];

fn has_real_time_wait(source: &str) -> bool {
    TIMER_MARKERS.iter().any(|marker| source.contains(marker))
}

fn is_known_divergence(source: &str) -> bool {
    KNOWN_DIVERGENCES.iter().any(|marker| source.contains(marker))
}

fn run_interpreter_capturing_stdout(program: &Program) -> (bool, String, String) {
    let path = std::env::temp_dir().join(format!("yps_exec_diff_{}.out", std::process::id()));
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .read(true)
        .open(&path)
        .expect("не удалось создать временный файл для захвата stdout");
    let saved_stdout = unsafe { libc::dup(1) };
    unsafe { libc::dup2(file.as_raw_fd(), 1) };

    let mut interpreter = Interpreter::new();
    let result = interpreter.run(program);
    let ok = result.is_ok();
    let err = result.err().map(|e| e.message.clone()).unwrap_or_default();

    let _ = std::io::stdout().flush();
    unsafe { libc::dup2(saved_stdout, 1) };
    unsafe { libc::close(saved_stdout) };

    let mut file = file;
    let _ = file.seek(SeekFrom::Start(0));
    let mut buf = Vec::new();
    let _ = file.read_to_end(&mut buf);
    let _ = std::fs::remove_file(&path);

    (ok, String::from_utf8_lossy(&buf).into_owned(), err)
}

fuzz_target!(|data: &str| {
    if has_real_time_wait(data) || is_known_divergence(data) {
        return;
    }

    let source = SourceFile::new("fuzz".to_string(), data.to_string());
    let (tokens, lex_diags) = Lexer::new(&source).tokenize();
    if !lex_diags.is_empty() {
        return;
    }
    let (program, parse_diags) = Parser::new(&tokens, &source).parse_program();
    if !parse_diags.is_empty() {
        return;
    }

    let (interp_ok, interp_out, interp_err) = run_interpreter_capturing_stdout(&program);
    let (vm_ok, vm_out, vm_err) = match yps_vm::run_to_string(&program) {
        Ok(out) => (true, out, String::new()),
        Err(e) => (false, String::new(), e.to_string()),
    };

    if !vm_ok && KNOWN_VM_ONLY_ERRORS.iter().any(|marker| vm_err.contains(marker)) {
        return;
    }
    if !interp_ok && vm_ok && KNOWN_INTERP_ONLY_ERRORS.iter().any(|marker| interp_err.contains(marker)) {
        return;
    }

    if interp_ok != vm_ok {
        panic!(
            "расхождение бэкендов: успех интерпретатора={interp_ok}, успех vm={vm_ok}\nисходник: {data:?}\nвывод интерпретатора: {interp_out:?}\nвывод vm: {vm_out:?}"
        );
    }
    if interp_ok && interp_out != vm_out {
        panic!(
            "расхождение вывода бэкендов\nисходник: {data:?}\nвывод интерпретатора: {interp_out:?}\nвывод vm: {vm_out:?}"
        );
    }
});
