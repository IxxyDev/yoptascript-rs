use std::any::Any;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;

use yps_interpreter::{DebugAction, DebugEvent, DebugHook, Interpreter, OutputSink};
use yps_lexer::{Lexer, Severity, SourceFile, Sources};
use yps_parser::Parser;

use crate::line_index::LineIndex;

pub const MODULE_FRAME: &str = "(модуль)";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopReason {
    Entry,
    Breakpoint,
    Step,
    Pause,
}

impl StopReason {
    #[must_use]
    pub const fn as_dap(self) -> &'static str {
        match self {
            Self::Entry => "entry",
            Self::Breakpoint => "breakpoint",
            Self::Step => "step",
            Self::Pause => "pause",
        }
    }
}

#[derive(Debug, Clone)]
pub struct DapFrame {
    pub name: String,
    pub line: usize,
    pub column: usize,
    pub path: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DapVar {
    pub name: String,
    pub value: String,
    pub type_name: String,
}

#[derive(Debug, Clone)]
pub struct StopInfo {
    pub reason: StopReason,
    pub frames: Vec<DapFrame>,
    pub locals: Vec<DapVar>,
}

#[derive(Debug)]
pub enum DebugMsg {
    Stopped(Box<StopInfo>),
    Output { category: &'static str, text: String },
    Exited { error: Option<String> },
}

pub type Notify = Arc<dyn Fn(DebugMsg) + Send + Sync>;

struct NotifySink {
    notify: Notify,
}

impl OutputSink for NotifySink {
    fn write_line(&mut self, line: &str) {
        (self.notify)(DebugMsg::Output { category: "stdout", text: format!("{line}\n") });
    }

    fn write_error_line(&mut self, line: &str) {
        (self.notify)(DebugMsg::Output { category: "stderr", text: format!("{line}\n") });
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResumeCmd {
    Continue,
    Next,
    StepIn,
    StepOut,
    Terminate,
}

pub struct DebuggeeHandle {
    pub resume_tx: Sender<ResumeCmd>,
    pub pause_flag: Arc<AtomicBool>,
}

struct DapHook {
    source: SourceFile,
    lines: LineIndex,
    sources: Rc<RefCell<Sources>>,
    module_lines: HashMap<usize, LineIndex>,
    breakpoints: Arc<Mutex<HashSet<usize>>>,
    pause_flag: Arc<AtomicBool>,
    notify: Notify,
    resume_rx: Receiver<ResumeCmd>,
    entry_pending: bool,
}

impl DebugHook for DapHook {
    fn on_statement(&mut self, event: DebugEvent<'_>) -> Option<DebugAction> {
        let paused = self.pause_flag.swap(false, Ordering::SeqCst);
        let hit_breakpoint = self.source.contains(event.span.start)
            && self.breakpoints.lock().is_ok_and(|set| set.contains(&self.lines.line(event.span.start)));
        if !paused && !hit_breakpoint && !event.step_complete {
            return None;
        }

        let reason = if std::mem::take(&mut self.entry_pending) {
            StopReason::Entry
        } else if paused {
            StopReason::Pause
        } else if hit_breakpoint {
            StopReason::Breakpoint
        } else {
            StopReason::Step
        };

        let info = StopInfo {
            reason,
            frames: self.build_frames(&event),
            locals: event
                .interp
                .debug_visible_locals()
                .into_iter()
                .map(|(name, value)| DapVar {
                    name,
                    value: value.to_string(),
                    type_name: value.type_name().to_string(),
                })
                .collect(),
        };
        (self.notify)(DebugMsg::Stopped(Box::new(info)));

        match self.resume_rx.recv() {
            Ok(ResumeCmd::Continue) => Some(DebugAction::Continue),
            Ok(ResumeCmd::Next) => Some(DebugAction::StepOver),
            Ok(ResumeCmd::StepIn) => Some(DebugAction::StepIn),
            Ok(ResumeCmd::StepOut) => Some(DebugAction::StepOut),
            // A dropped channel means the adapter is gone: stop the debuggee rather than hang.
            Ok(ResumeCmd::Terminate) | Err(_) => Some(DebugAction::Terminate),
        }
    }
}

impl DapHook {
    /// The interpreter records a call-site span per frame, so DAP frame `n` shows the name of
    /// the function being executed and the position of the call that led into frame `n - 1`.
    fn build_frames(&mut self, event: &DebugEvent<'_>) -> Vec<DapFrame> {
        let stack = event.interp.debug_call_stack();
        let mut frames = Vec::with_capacity(stack.len() + 1);
        let mut position = self.locate(event.span.start);
        for frame in stack.iter().rev() {
            let (path, line, column) = std::mem::replace(&mut position, self.locate(frame.span.start));
            frames.push(DapFrame { name: frame.name.to_string(), line, column, path });
        }
        let (path, line, column) = position;
        frames.push(DapFrame { name: MODULE_FRAME.to_string(), line, column, path });
        frames
    }

    fn locate(&mut self, offset: usize) -> (Option<String>, usize, usize) {
        if !self.source.contains(offset)
            && let Ok(sources) = self.sources.try_borrow()
            && let Some(file) = sources.lookup(offset)
        {
            let lines = self.module_lines.entry(file.base()).or_insert_with(|| LineIndex::new(file));
            let (line, column) = lines.position(file, offset);
            return (Some(file.name.clone()), line, column);
        }
        let (line, column) = self.lines.position(&self.source, offset);
        (None, line, column)
    }
}

pub struct LaunchConfig {
    pub program: PathBuf,
    pub stop_on_entry: bool,
    pub breakpoints: Arc<Mutex<HashSet<usize>>>,
}

/// Runs the program on its own thread. The interpreter is built inside that thread because it
/// is full of `Rc`s and cannot cross thread boundaries.
pub fn spawn(config: LaunchConfig, notify: Notify) -> DebuggeeHandle {
    let (resume_tx, resume_rx) = std::sync::mpsc::channel();
    let pause_flag = Arc::new(AtomicBool::new(false));
    let hook_pause_flag = Arc::clone(&pause_flag);

    thread::spawn(move || {
        let error = match catch_unwind(AssertUnwindSafe(|| run_program(config, &notify, resume_rx, hook_pause_flag))) {
            Ok(error) => error,
            Err(payload) => Some(format!("Внутренняя ошибка отладчика: {}", panic_message(payload.as_ref()))),
        };
        notify(DebugMsg::Exited { error });
    });

    DebuggeeHandle { resume_tx, pause_flag }
}

fn panic_message(payload: &(dyn Any + Send)) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|message| (*message).to_string())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "неизвестная паника".to_string())
}

fn run_program(
    config: LaunchConfig,
    notify: &Notify,
    resume_rx: Receiver<ResumeCmd>,
    pause_flag: Arc<AtomicBool>,
) -> Option<String> {
    let text = match std::fs::read_to_string(&config.program) {
        Ok(text) => text,
        Err(err) => return Some(format!("Не удалось прочитать '{}': {err}", config.program.display())),
    };
    let sources = Rc::new(RefCell::new(Sources::default()));
    let source = SourceFile::clone(&sources.borrow_mut().add(config.program.display().to_string(), text));
    let (tokens, lex_diags) = Lexer::new(&source).tokenize();
    if let Some(diag) = lex_diags.iter().find(|diag| diag.severity == Severity::Error) {
        return Some(source.describe(diag));
    }
    let (program, parse_diags) = Parser::new(&tokens, &source).parse_program();
    if let Some(diag) = parse_diags.iter().find(|diag| diag.severity == Severity::Error) {
        return Some(source.describe(diag));
    }

    let mut interp = Interpreter::new();
    interp.set_sources(Rc::clone(&sources));
    let main_source = source.clone();
    interp.set_output_sink(Box::new(NotifySink { notify: Arc::clone(notify) }));
    interp.block_stdin("чтение из stdin недоступно под отладчиком: канал занят протоколом DAP");
    if let Some(parent) = config.program.parent() {
        interp.set_base_path(parent.to_path_buf());
    }
    interp.set_debug_hook(Box::new(DapHook {
        lines: LineIndex::new(&source),
        source,
        sources: Rc::clone(&sources),
        module_lines: HashMap::new(),
        breakpoints: config.breakpoints,
        pause_flag,
        notify: Arc::clone(notify),
        resume_rx,
        entry_pending: config.stop_on_entry,
    }));
    if !config.stop_on_entry {
        interp.set_debug_resume(DebugAction::Continue);
    }

    match interp.run(&program) {
        Ok(()) => None,
        Err(err) if err.message == yps_interpreter::DEBUG_TERMINATED => None,
        Err(err) => Some(describe_runtime_error(&err, &sources.borrow(), &main_source)),
    }
}

fn describe_runtime_error(err: &yps_interpreter::RuntimeError, sources: &Sources, main: &SourceFile) -> String {
    let locate = |offset: usize| {
        let file = sources.lookup(offset).map_or(main, AsRef::as_ref);
        let (line, column) = file.position(offset);
        format!("{}:{line}:{column}", file.name)
    };
    std::iter::once(format!("{}: {err}", locate(err.span.start)))
        .chain(err.stack.iter().map(|frame| format!("  в {}:{}", frame.name, locate(frame.span.start))))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;
    use std::time::Duration;

    use super::*;
    use crate::test_support::fixture;

    fn exits_of(name: &str, inspect: impl Fn(&DebugMsg) + Send + Sync + 'static) -> Vec<Option<String>> {
        let (tx, rx) = mpsc::channel();
        let program = PathBuf::from(fixture(name));
        let config = LaunchConfig { program, stop_on_entry: false, breakpoints: Arc::new(Mutex::new(HashSet::new())) };

        let _handle = spawn(
            config,
            Arc::new(move |msg| {
                inspect(&msg);
                let _ = tx.send(msg);
            }),
        );

        let mut exited = Vec::new();
        while let Ok(msg) = rx.recv_timeout(Duration::from_secs(10)) {
            if let DebugMsg::Exited { error } = msg {
                exited.push(error);
            }
        }
        exited
    }

    #[test]
    fn panic_in_the_debuggee_thread_still_delivers_exactly_one_exited() {
        let exited =
            exits_of("print.yopta", |msg| assert!(!matches!(msg, DebugMsg::Output { .. }), "паника при выводе"));

        assert_eq!(exited.len(), 1);
        assert!(exited[0].as_deref().is_some_and(|error| error.contains("паника при выводе")));
    }

    #[test]
    fn normal_run_delivers_exactly_one_exited_without_error() {
        assert_eq!(exits_of("loop.yopta", |_| {}), vec![None]);
    }
}
