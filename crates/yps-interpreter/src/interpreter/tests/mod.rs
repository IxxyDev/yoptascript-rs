use super::*;
use yps_lexer::{Lexer, SourceFile};
use yps_parser::Parser;

fn run_code(src: &str) -> Interpreter {
    let source = SourceFile::new("test".to_string(), src.to_string());
    let (tokens, lex_diags) = Lexer::new(&source).tokenize();
    assert!(lex_diags.is_empty(), "Ошибки лексера: {lex_diags:?}");
    let (program, parse_diags) = Parser::new(&tokens, &source).parse_program();
    assert!(parse_diags.is_empty(), "Ошибки парсера: {parse_diags:?}");
    let mut interp = Interpreter::new();
    interp.run(&program).expect("Ошибка интерпретатора");
    interp
}

fn run_code_err(src: &str) -> RuntimeError {
    let source = SourceFile::new("test".to_string(), src.to_string());
    let (tokens, _) = Lexer::new(&source).tokenize();
    let (program, _) = Parser::new(&tokens, &source).parse_program();
    let mut interp = Interpreter::new();
    interp.run(&program).unwrap_err()
}

fn structural_eq(a: &Value, b: &Value) -> bool {
    let mut seen: std::collections::HashSet<(*const (), *const ())> = std::collections::HashSet::new();
    structural_eq_inner(a, b, &mut seen)
}

fn structural_eq_inner(a: &Value, b: &Value, seen: &mut std::collections::HashSet<(*const (), *const ())>) -> bool {
    match (a, b) {
        (Value::Array(x), Value::Array(y)) => {
            let key = (std::rc::Rc::as_ptr(x) as *const (), std::rc::Rc::as_ptr(y) as *const ());
            if !seen.insert(key) {
                return true;
            }
            let xb = x.borrow();
            let yb = y.borrow();
            let res = xb.len() == yb.len() && xb.iter().zip(yb.iter()).all(|(p, q)| structural_eq_inner(p, q, seen));
            seen.remove(&key);
            res
        }
        (Value::Object(x), Value::Object(y)) => {
            let key = (std::rc::Rc::as_ptr(x) as *const (), std::rc::Rc::as_ptr(y) as *const ());
            if !seen.insert(key) {
                return true;
            }
            let xb = x.borrow();
            let yb = y.borrow();
            let res = xb.len() == yb.len()
                && xb.iter().all(|(k, v)| match yb.get(k) {
                    Some(w) => structural_eq_inner(v, w, seen),
                    None => false,
                });
            seen.remove(&key);
            res
        }
        (Value::Map(x), Value::Map(y)) => {
            let xb = x.borrow();
            let yb = y.borrow();
            xb.len() == yb.len()
                && xb.iter().zip(yb.iter()).all(|((k1, v1), (k2, v2))| {
                    structural_eq_inner(k1.as_value(), k2.as_value(), seen) && structural_eq_inner(v1, v2, seen)
                })
        }
        (Value::Set(x), Value::Set(y)) => {
            let xb = x.borrow();
            let yb = y.borrow();
            xb.len() == yb.len()
                && xb.iter().zip(yb.iter()).all(|(p, q)| structural_eq_inner(p.as_value(), q.as_value(), seen))
        }
        _ => a == b,
    }
}

#[track_caller]
fn assert_struct_eq(actual: Option<Value>, expected: Value) {
    let actual = actual.expect("значение не найдено");
    assert!(structural_eq(&actual, &expected), "структурное несовпадение: actual={actual:?}, expected={expected:?}");
}

fn parse_src(src: &str) -> yps_parser::ast::Program {
    let source = SourceFile::new("repl".to_string(), src.to_string());
    let (tokens, lex_diags) = Lexer::new(&source).tokenize();
    assert!(lex_diags.is_empty(), "Ошибки лексера: {lex_diags:?}");
    let (program, parse_diags) = Parser::new(&tokens, &source).parse_program();
    assert!(parse_diags.is_empty(), "Ошибки парсера: {parse_diags:?}");
    program
}

fn run_more(interp: &mut Interpreter, src: &str) -> Option<Value> {
    let source = SourceFile::new("test".to_string(), src.to_string());
    let (tokens, lex_diags) = Lexer::new(&source).tokenize();
    assert!(lex_diags.is_empty(), "Ошибки лексера: {lex_diags:?}");
    let (program, parse_diags) = Parser::new(&tokens, &source).parse_program();
    assert!(parse_diags.is_empty(), "Ошибки парсера: {parse_diags:?}");
    interp.run_repl(&program).expect("Ошибка интерпретатора")
}

mod aliases;
mod assign;
mod async_promise;
mod builtins_misc;
mod classes;
mod coercion;
mod collections;
mod control_flow;
mod decorators;
mod destructure;
mod event_loop;
mod expressions;
mod functions;
mod gc;
mod generators;
mod iterators;
mod limits;
mod modules;
mod operators;
mod proto;
mod proxy;
mod ref_semantics;
mod regex;
mod repl;
mod runtime_errors;
mod stack_traces;
mod stdlib_core;
mod strings;
mod try_catch;
mod typed_arrays;
mod using_symbol;
