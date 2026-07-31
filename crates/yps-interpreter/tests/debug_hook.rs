use std::cell::RefCell;
use std::rc::Rc;

use yps_interpreter::value::Value;
use yps_interpreter::{DebugAction, DebugEvent, DebugHook, Interpreter};
use yps_lexer::{Lexer, SourceFile};
use yps_parser::Parser;

#[derive(Debug, Clone)]
struct Stop {
    line: usize,
    depth: usize,
    locals: Vec<(String, Value)>,
}

struct ScriptedHook {
    source: SourceFile,
    breakpoint_lines: Vec<usize>,
    plan: Vec<DebugAction>,
    next: usize,
    stops: Rc<RefCell<Vec<Stop>>>,
}

impl DebugHook for ScriptedHook {
    fn on_statement(&mut self, event: DebugEvent<'_>) -> Option<DebugAction> {
        let (line, _) = self.source.position(event.span.start);
        if !event.step_complete && !self.breakpoint_lines.contains(&line) {
            return None;
        }
        self.stops.borrow_mut().push(Stop { line, depth: event.depth, locals: event.interp.debug_visible_locals() });
        let action = self.plan.get(self.next).copied().unwrap_or(DebugAction::Continue);
        self.next += 1;
        Some(action)
    }
}

fn run_debug(src: &str, breakpoint_lines: Vec<usize>, plan: Vec<DebugAction>, arm: DebugAction) -> Vec<Stop> {
    let source = SourceFile::new("test.yopta".to_string(), src.to_string());
    let (tokens, diags) = Lexer::new(&source).tokenize();
    assert!(diags.is_empty(), "лексер: {diags:?}");
    let (program, diags) = Parser::new(&tokens, &source).parse_program();
    assert!(diags.is_empty(), "парсер: {diags:?}");

    let stops = Rc::new(RefCell::new(Vec::new()));
    let mut interp = Interpreter::new();
    interp.set_debug_hook(Box::new(ScriptedHook {
        source: source.clone(),
        breakpoint_lines,
        plan,
        next: 0,
        stops: Rc::clone(&stops),
    }));
    if arm == DebugAction::Continue {
        interp.set_debug_resume(DebugAction::Continue);
    }
    interp.run(&program).expect("скрипт должен отработать");
    stops.borrow().clone()
}

fn local(stop: &Stop, name: &str) -> Option<Value> {
    stop.locals.iter().find(|(n, _)| n == name).map(|(_, v)| v.clone())
}

const LOOP_SRC: &str =
    "гыы всего = 0;\nго (гыы i = 0; i < 3; i = i + 1) {\n    всего = всего + i;\n}\nгыы итог = всего;\n";

const CALL_SRC: &str =
    "йопта удвоить(a) {\n    гыы b = a * 2;\n    отвечаю b;\n}\nгыы x = удвоить(4);\nгыы y = x + 1;\n";

#[test]
fn hook_is_not_invoked_without_a_debug_hook() {
    let source = SourceFile::new("test.yopta".to_string(), LOOP_SRC.to_string());
    let (tokens, _) = Lexer::new(&source).tokenize();
    let (program, _) = Parser::new(&tokens, &source).parse_program();
    let mut interp = Interpreter::new();
    interp.run(&program).unwrap();
    assert_eq!(interp.get("итог"), Some(Value::Number(3.0)));
}

#[test]
fn breakpoint_pauses_on_the_requested_line() {
    let stops = run_debug(LOOP_SRC, vec![3], Vec::new(), DebugAction::Continue);
    assert_eq!(stops.len(), 3, "тело цикла выполняется трижды");
    assert!(stops.iter().all(|s| s.line == 3));
    assert_eq!(local(&stops[0], "i"), Some(Value::Number(0.0)));
    assert_eq!(local(&stops[1], "i"), Some(Value::Number(1.0)));
    assert_eq!(local(&stops[2], "i"), Some(Value::Number(2.0)));
    assert_eq!(local(&stops[2], "всего"), Some(Value::Number(1.0)));
}

#[test]
fn step_over_does_not_descend_into_a_call() {
    let stops = run_debug(CALL_SRC, Vec::new(), vec![DebugAction::StepOver; 8], DebugAction::StepIn);
    let lines: Vec<usize> = stops.iter().map(|s| s.line).collect();
    assert_eq!(lines, vec![1, 5, 6], "объявление функции, вызов, следующий оператор");
    assert!(stops.iter().all(|s| s.depth == 0));
}

#[test]
fn step_in_descends_into_a_call() {
    let stops = run_debug(
        CALL_SRC,
        Vec::new(),
        vec![DebugAction::StepOver, DebugAction::StepIn, DebugAction::StepOver, DebugAction::StepOver],
        DebugAction::StepIn,
    );
    let lines: Vec<usize> = stops.iter().map(|s| s.line).collect();
    assert_eq!(lines, vec![1, 5, 2, 3, 6]);
    assert_eq!(stops[2].depth, 1, "тело удвоить исполняется на глубине 1");
    assert_eq!(local(&stops[2], "a"), Some(Value::Number(4.0)));
}

#[test]
fn step_out_returns_to_the_caller_depth() {
    let stops = run_debug(
        CALL_SRC,
        Vec::new(),
        vec![DebugAction::StepOver, DebugAction::StepIn, DebugAction::StepOut, DebugAction::StepOver],
        DebugAction::StepIn,
    );
    let lines: Vec<usize> = stops.iter().map(|s| s.line).collect();
    assert_eq!(lines, vec![1, 5, 2, 6]);
    assert_eq!(stops[3].depth, 0);
}

#[test]
fn call_stack_is_visible_while_paused() {
    struct StackProbe {
        source: SourceFile,
        names: Rc<RefCell<Vec<Vec<String>>>>,
    }
    impl DebugHook for StackProbe {
        fn on_statement(&mut self, event: DebugEvent<'_>) -> Option<DebugAction> {
            let (line, _) = self.source.position(event.span.start);
            if line == 2 {
                self.names
                    .borrow_mut()
                    .push(event.interp.debug_call_stack().iter().map(|f| f.name.to_string()).collect());
            }
            None
        }
    }

    let source = SourceFile::new("test.yopta".to_string(), CALL_SRC.to_string());
    let (tokens, _) = Lexer::new(&source).tokenize();
    let (program, _) = Parser::new(&tokens, &source).parse_program();
    let names = Rc::new(RefCell::new(Vec::new()));
    let mut interp = Interpreter::new();
    interp.set_debug_hook(Box::new(StackProbe { source: source.clone(), names: Rc::clone(&names) }));
    interp.set_debug_resume(DebugAction::Continue);
    interp.run(&program).unwrap();
    assert_eq!(names.borrow().as_slice(), [vec!["удвоить".to_string()]]);
}

#[test]
fn reassigned_variable_is_reflected_at_the_pause() {
    let src = "гыы n = 1;\nn = 2;\nгыы m = n + 10;\n";
    let stops = run_debug(src, vec![3], Vec::new(), DebugAction::Continue);
    assert_eq!(stops.len(), 1);
    assert_eq!(local(&stops[0], "n"), Some(Value::Number(2.0)));
    assert_eq!(local(&stops[0], "m"), None, "m ещё не объявлена");
}

#[test]
fn terminate_aborts_the_program() {
    struct Killer;
    impl DebugHook for Killer {
        fn on_statement(&mut self, _event: DebugEvent<'_>) -> Option<DebugAction> {
            Some(DebugAction::Terminate)
        }
    }
    let source = SourceFile::new("test.yopta".to_string(), LOOP_SRC.to_string());
    let (tokens, _) = Lexer::new(&source).tokenize();
    let (program, _) = Parser::new(&tokens, &source).parse_program();
    let mut interp = Interpreter::new();
    interp.set_debug_hook(Box::new(Killer));
    let err = interp.run(&program).expect_err("должно прерваться");
    assert_eq!(err.message, yps_interpreter::DEBUG_TERMINATED);
    assert_eq!(interp.get("итог"), None);
}
