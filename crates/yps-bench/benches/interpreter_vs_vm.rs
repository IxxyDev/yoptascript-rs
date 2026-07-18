use criterion::{Criterion, criterion_group, criterion_main};
use yps_interpreter::Interpreter;
use yps_lexer::{Lexer, SourceFile};
use yps_parser::Parser;
use yps_parser::ast::Program;

const FIB: &str = include_str!("programs/fib.yopta");
const STRINGS: &str = include_str!("programs/strings.yopta");
const OBJECTS: &str = include_str!("programs/objects.yopta");
const CLOSURES: &str = include_str!("programs/closures.yopta");
const ARRAYS: &str = include_str!("programs/arrays.yopta");

fn parse(name: &str, source: &str) -> Program {
    let file = SourceFile::new(name.to_string(), source.to_string());
    let (tokens, lex_diagnostics) = Lexer::new(&file).tokenize();
    assert!(lex_diagnostics.is_empty(), "{name}: lexer diagnostics: {lex_diagnostics:?}");
    let (program, parse_diagnostics) = Parser::new(&tokens, &file).parse_program();
    assert!(parse_diagnostics.is_empty(), "{name}: parser diagnostics: {parse_diagnostics:?}");
    program
}

fn bench_program(c: &mut Criterion, name: &str, source: &str) {
    let program = parse(name, source);

    let mut group = c.benchmark_group(name);

    group.bench_function("interpreter", |b| {
        b.iter(|| {
            let mut interpreter = Interpreter::new();
            interpreter.run(&program).expect("interpreter run failed");
        });
    });

    group.bench_function("vm", |b| {
        b.iter(|| {
            yps_vm::execute(&program).expect("vm run failed");
        });
    });

    group.finish();
}

fn bench_fib(c: &mut Criterion) {
    bench_program(c, "fib", FIB);
}

fn bench_strings(c: &mut Criterion) {
    bench_program(c, "strings", STRINGS);
}

fn bench_objects(c: &mut Criterion) {
    bench_program(c, "objects", OBJECTS);
}

fn bench_closures(c: &mut Criterion) {
    bench_program(c, "closures", CLOSURES);
}

fn bench_arrays(c: &mut Criterion) {
    bench_program(c, "arrays", ARRAYS);
}

criterion_group!(benches, bench_fib, bench_strings, bench_objects, bench_closures, bench_arrays);
criterion_main!(benches);
