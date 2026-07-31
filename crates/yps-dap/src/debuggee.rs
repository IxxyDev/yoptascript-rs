use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;

use yps_interpreter::{DebugAction, DebugEvent, DebugHook, Interpreter};
use yps_lexer::{Lexer, SourceFile};
use yps_parser::Parser;

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
    Exited { error: Option<String> },
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
    breakpoints: Arc<Mutex<HashSet<usize>>>,
    pause_flag: Arc<AtomicBool>,
    notify: Box<dyn Fn(DebugMsg) + Send>,
    resume_rx: Receiver<ResumeCmd>,
    entry_pending: bool,
}

impl DebugHook for DapHook {
    fn on_statement(&mut self, event: DebugEvent<'_>) -> Option<DebugAction> {
        let (line, column) = self.source.position(event.span.start);
        let paused = self.pause_flag.swap(false, Ordering::SeqCst);
        let hit_breakpoint = self.breakpoints.lock().is_ok_and(|set| set.contains(&line));
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
            frames: self.build_frames(&event, line, column),
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
    fn build_frames(&self, event: &DebugEvent<'_>, line: usize, column: usize) -> Vec<DapFrame> {
        let stack = event.interp.debug_call_stack();
        let mut frames = Vec::with_capacity(stack.len() + 1);
        let mut position = (line, column);
        for frame in stack.iter().rev() {
            frames.push(DapFrame { name: frame.name.to_string(), line: position.0, column: position.1 });
            position = self.source.position(frame.span.start);
        }
        frames.push(DapFrame { name: MODULE_FRAME.to_string(), line: position.0, column: position.1 });
        frames
    }
}

pub struct LaunchConfig {
    pub program: PathBuf,
    pub stop_on_entry: bool,
    pub breakpoints: Arc<Mutex<HashSet<usize>>>,
}

/// Runs the program on its own thread. The interpreter is built inside that thread because it
/// is full of `Rc`s and cannot cross thread boundaries.
pub fn spawn<N: Fn(DebugMsg) + Clone + Send + 'static>(config: LaunchConfig, notify: N) -> DebuggeeHandle {
    let (resume_tx, resume_rx) = std::sync::mpsc::channel();
    let pause_flag = Arc::new(AtomicBool::new(false));
    let hook_pause_flag = Arc::clone(&pause_flag);

    thread::spawn(move || {
        let hook_notify = notify.clone();
        let text = match std::fs::read_to_string(&config.program) {
            Ok(text) => text,
            Err(err) => {
                notify(DebugMsg::Exited {
                    error: Some(format!("Не удалось прочитать '{}': {err}", config.program.display())),
                });
                return;
            }
        };
        let source = SourceFile::new(config.program.display().to_string(), text);
        let (tokens, lex_diags) = Lexer::new(&source).tokenize();
        if let Some(diag) = lex_diags.first() {
            notify(DebugMsg::Exited { error: Some(diag.message.clone()) });
            return;
        }
        let (program, parse_diags) = Parser::new(&tokens, &source).parse_program();
        if let Some(diag) = parse_diags.first() {
            notify(DebugMsg::Exited { error: Some(diag.message.clone()) });
            return;
        }

        let mut interp = Interpreter::new();
        if let Some(parent) = config.program.parent() {
            interp.set_base_path(parent.to_path_buf());
        }
        interp.set_debug_hook(Box::new(DapHook {
            source,
            breakpoints: config.breakpoints,
            pause_flag: hook_pause_flag,
            notify: Box::new(hook_notify),
            resume_rx,
            entry_pending: config.stop_on_entry,
        }));
        if !config.stop_on_entry {
            interp.set_debug_resume(DebugAction::Continue);
        }

        let error = match interp.run(&program) {
            Ok(()) => None,
            Err(err) if err.message == yps_interpreter::DEBUG_TERMINATED => None,
            Err(err) => Some(err.to_string()),
        };
        notify(DebugMsg::Exited { error });
    });

    DebuggeeHandle { resume_tx, pause_flag }
}
