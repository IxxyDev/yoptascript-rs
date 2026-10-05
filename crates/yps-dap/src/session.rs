use std::collections::{BTreeSet, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

use yps_lexer::{Lexer, SourceFile};
use yps_parser::Parser;

use crate::breakpoints;
use crate::debuggee::{self, DebugMsg, DebuggeeHandle, LaunchConfig, ResumeCmd, StopInfo};

pub const THREAD_ID: i64 = 1;
const LOCALS_SCOPE_BASE: i64 = 1000;
const NOT_PAUSED: &str = "Программа не находится на паузе";

#[derive(Clone, Copy)]
enum ErrorCode {
    NotStopped = 1,
    Unsupported = 3,
    NoProgram = 4,
    UnknownFrame = 5,
    DebuggeeGone = 6,
}

impl ErrorCode {
    const fn as_str(self) -> &'static str {
        match self {
            Self::NotStopped => "notStopped",
            Self::Unsupported => "unsupported",
            Self::NoProgram => "noProgram",
            Self::UnknownFrame => "unknownFrame",
            Self::DebuggeeGone => "debuggeeGone",
        }
    }

    const fn show_user(self) -> bool {
        matches!(self, Self::NoProgram | Self::DebuggeeGone)
    }
}

fn same_file(a: &Path, b: &Path) -> bool {
    if a == b {
        return true;
    }
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

fn load_statement_lines(path: &Path) -> std::io::Result<BTreeSet<usize>> {
    let text = std::fs::read_to_string(path)?;
    let source = SourceFile::new(path.display().to_string(), text);
    let (tokens, _) = Lexer::new(&source).tokenize();
    let (program, _) = Parser::new(&tokens, &source).parse_program();
    Ok(breakpoints::statement_lines(&program, &source))
}

struct Source {
    path: PathBuf,
    statement_lines: BTreeSet<usize>,
}

/// What arrives on the adapter's single event queue.
pub enum Incoming {
    Client(Value),
    ClientEof,
    ClientError(String),
    Debug(DebugMsg),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Configuring,
    Running,
    Stopped,
    Exited,
}

pub struct Session {
    seq: i64,
    state: State,
    should_exit: bool,
    program: Option<Source>,
    stop_on_entry: bool,
    breakpoints: Arc<Mutex<HashSet<usize>>>,
    debuggee: Option<DebuggeeHandle>,
    stopped: Option<StopInfo>,
    deferred: VecDeque<Value>,
    events_tx: Sender<Incoming>,
}

impl Session {
    #[must_use]
    pub fn new(events_tx: Sender<Incoming>) -> Self {
        Self {
            seq: 0,
            state: State::Configuring,
            should_exit: false,
            program: None,
            stop_on_entry: false,
            breakpoints: Arc::new(Mutex::new(HashSet::new())),
            debuggee: None,
            stopped: None,
            deferred: VecDeque::new(),
            events_tx,
        }
    }

    #[must_use]
    pub const fn should_exit(&self) -> bool {
        self.should_exit
    }

    fn next_seq(&mut self) -> i64 {
        self.seq += 1;
        self.seq
    }

    fn reply(&mut self, request: &Value, result: Result<Value, (ErrorCode, String)>) -> Value {
        let mut message = json!({
            "seq": self.next_seq(),
            "type": "response",
            "request_seq": request["seq"].as_i64().unwrap_or(0),
            "command": request["command"].as_str().unwrap_or_default(),
        });
        match result {
            Ok(body) => {
                message["success"] = json!(true);
                message["body"] = body;
            }
            Err((code, text)) => {
                message["success"] = json!(false);
                message["message"] = json!(code.as_str());
                message["body"] =
                    json!({ "error": { "id": code as i64, "format": text, "showUser": code.show_user() } });
            }
        }
        message
    }

    fn response(&mut self, request: &Value, body: Value) -> Value {
        self.reply(request, Ok(body))
    }

    fn failure(&mut self, request: &Value, code: ErrorCode, text: impl Into<String>) -> Value {
        self.reply(request, Err((code, text.into())))
    }

    fn event(&mut self, name: &str, body: Value) -> Value {
        let seq = self.next_seq();
        json!({ "seq": seq, "type": "event", "event": name, "body": body })
    }

    pub fn handle(&mut self, incoming: Incoming) -> Vec<Value> {
        match incoming {
            Incoming::Client(request) => self.handle_client(&request),
            Incoming::ClientEof => {
                self.terminate_debuggee();
                self.should_exit = true;
                Vec::new()
            }
            Incoming::ClientError(text) => {
                vec![self.event("output", json!({ "category": "console", "output": format!("{text}\n") }))]
            }
            Incoming::Debug(msg) => self.handle_debuggee(msg),
        }
    }

    fn handle_debuggee(&mut self, msg: DebugMsg) -> Vec<Value> {
        let mut out = Vec::new();
        match msg {
            DebugMsg::Stopped(info) => {
                let reason = info.reason.as_dap();
                self.stopped = Some(*info);
                self.state = State::Stopped;
                out.push(self.event(
                    "stopped",
                    json!({
                        "reason": reason,
                        "threadId": THREAD_ID,
                        "allThreadsStopped": true,
                    }),
                ));
            }
            DebugMsg::Output { category, text } => {
                out.push(self.event("output", json!({ "category": category, "output": text })));
            }
            DebugMsg::Exited { error } => {
                self.state = State::Exited;
                self.stopped = None;
                let exit_code = if let Some(error) = error {
                    out.push(self.event("output", json!({ "category": "stderr", "output": format!("{error}\n") })));
                    1
                } else {
                    0
                };
                out.push(self.event("terminated", json!({})));
                out.push(self.event("exited", json!({ "exitCode": exit_code })));
            }
        }
        while self.state != State::Running
            && let Some(request) = self.deferred.pop_front()
        {
            out.extend(self.handle_client(&request));
        }
        out
    }

    fn handle_client(&mut self, request: &Value) -> Vec<Value> {
        let command = request["command"].as_str().unwrap_or_default();
        // While the debuggee runs, only control requests may interleave; everything else waits
        // for the next `stopped` so the client always sees state from a real pause point.
        if self.state == State::Running
            && !matches!(command, "pause" | "disconnect" | "terminate" | "setBreakpoints" | "threads")
        {
            self.deferred.push_back(request.clone());
            return Vec::new();
        }

        match command {
            "initialize" => {
                let response = self.response(
                    request,
                    json!({
                        "supportsConfigurationDoneRequest": true,
                        "supportsTerminateRequest": true,
                        "supportsStepInTargetsRequest": false,
                        "supportsEvaluateForHovers": false,
                        "supportsFunctionBreakpoints": false,
                        "supportsConditionalBreakpoints": false,
                    }),
                );
                vec![response]
            }
            "launch" => self.handle_launch(request),
            "setBreakpoints" => self.handle_set_breakpoints(request),
            "setExceptionBreakpoints" => {
                vec![self.response(request, json!({ "breakpoints": [] }))]
            }
            "loadedSources" => vec![self.response(request, json!({ "sources": [] }))],
            "configurationDone" => {
                let response = self.response(request, json!({}));
                let mut out = vec![response];
                out.extend(self.start_debuggee(request));
                out
            }
            "threads" => {
                let body = json!({ "threads": [{ "id": THREAD_ID, "name": "главный поток" }] });
                vec![self.response(request, body)]
            }
            "stackTrace" => self.handle_stack_trace(request),
            "scopes" => self.handle_scopes(request),
            "variables" => self.handle_variables(request),
            "continue" => {
                let body = json!({ "allThreadsContinued": true });
                self.resume(request, ResumeCmd::Continue, body)
            }
            "next" => self.resume(request, ResumeCmd::Next, json!({})),
            "stepIn" => self.resume(request, ResumeCmd::StepIn, json!({})),
            "stepOut" => self.resume(request, ResumeCmd::StepOut, json!({})),
            "pause" => {
                if self.state == State::Running
                    && let Some(handle) = &self.debuggee
                {
                    handle.pause_flag.store(true, Ordering::SeqCst);
                }
                vec![self.response(request, json!({}))]
            }
            "disconnect" | "terminate" => {
                self.terminate_debuggee();
                self.should_exit = true;
                let response = self.response(request, json!({}));
                let terminated = self.event("terminated", json!({}));
                vec![response, terminated]
            }
            other => {
                let message = format!("Команда '{other}' не поддерживается");
                vec![self.failure(request, ErrorCode::Unsupported, message)]
            }
        }
    }

    fn handle_launch(&mut self, request: &Value) -> Vec<Value> {
        let arguments = &request["arguments"];
        let Some(program) = arguments["program"].as_str() else {
            return vec![self.failure(request, ErrorCode::NoProgram, "В 'launch' не указан аргумент 'program'")];
        };
        let mut program_path = PathBuf::from(program);
        if program_path.is_relative()
            && let Ok(cwd) = std::env::current_dir()
        {
            program_path = cwd.join(program_path);
        }
        let statement_lines = match load_statement_lines(&program_path) {
            Ok(lines) => lines,
            Err(err) => {
                let text = format!("Не удалось прочитать '{}': {err}", program_path.display());
                return vec![self.failure(request, ErrorCode::NoProgram, text)];
            }
        };
        self.stop_on_entry = arguments["stopOnEntry"].as_bool().unwrap_or(false);
        self.program = Some(Source { path: program_path, statement_lines });
        let response = self.response(request, json!({}));
        let initialized = self.event("initialized", json!({}));
        vec![response, initialized]
    }

    fn handle_set_breakpoints(&mut self, request: &Value) -> Vec<Value> {
        let arguments = &request["arguments"];
        let requested: Vec<usize> = if let Some(items) = arguments["breakpoints"].as_array() {
            items.iter().filter_map(|item| item["line"].as_u64()).map(|line| line as usize).collect()
        } else if let Some(items) = arguments["lines"].as_array() {
            items.iter().filter_map(Value::as_u64).map(|line| line as usize).collect()
        } else {
            Vec::new()
        };

        if let Some(path) = request["arguments"]["source"]["path"].as_str() {
            let path = Path::new(path);
            let (foreign, message) = match &self.program {
                Some(program) => (!same_file(&program.path, path), "Отлаживается только запущенный файл"),
                None => (true, "Сначала пришлите 'launch'"),
            };
            if foreign {
                let rejected: Vec<Value> = requested
                    .into_iter()
                    .enumerate()
                    .map(|(index, line)| {
                        json!({
                            "id": index + 1,
                            "verified": false,
                            "line": line,
                            "message": message,
                        })
                    })
                    .collect();
                return vec![self.response(request, json!({ "breakpoints": rejected }))];
            }
        }

        let empty = BTreeSet::new();
        let statement_lines = self.program.as_ref().map_or(&empty, |program| &program.statement_lines);
        let mut verified = Vec::new();
        let mut resolved = HashSet::new();
        for (index, line) in requested.into_iter().enumerate() {
            match breakpoints::resolve_line(line, statement_lines) {
                Some(actual) => {
                    resolved.insert(actual);
                    verified.push(json!({ "id": index + 1, "verified": true, "line": actual }));
                }
                None => verified.push(json!({
                    "id": index + 1,
                    "verified": false,
                    "line": line,
                    "message": "На этой строке нет оператора",
                })),
            }
        }
        if let Ok(mut set) = self.breakpoints.lock() {
            *set = resolved;
        }
        vec![self.response(request, json!({ "breakpoints": verified }))]
    }

    fn start_debuggee(&mut self, request: &Value) -> Vec<Value> {
        if self.debuggee.is_some() {
            return Vec::new();
        }
        let Some(program) = self.program.as_ref().map(|source| source.path.clone()) else {
            return vec![self.failure(request, ErrorCode::NoProgram, "Программа не задана: сначала пришлите 'launch'")];
        };
        let tx = self.events_tx.clone();
        let handle = debuggee::spawn(
            LaunchConfig { program, stop_on_entry: self.stop_on_entry, breakpoints: Arc::clone(&self.breakpoints) },
            Arc::new(move |msg| {
                let _ = tx.send(Incoming::Debug(msg));
            }),
        );
        self.debuggee = Some(handle);
        self.state = State::Running;
        Vec::new()
    }

    fn resume(&mut self, request: &Value, cmd: ResumeCmd, body: Value) -> Vec<Value> {
        if self.state != State::Stopped {
            return vec![self.failure(request, ErrorCode::NotStopped, NOT_PAUSED)];
        }
        let sent = self.debuggee.as_ref().is_some_and(|handle| {
            handle.pause_flag.store(false, Ordering::SeqCst);
            handle.resume_tx.send(cmd).is_ok()
        });
        if !sent {
            return vec![self.failure(request, ErrorCode::DebuggeeGone, "Отлаживаемая программа недоступна")];
        }
        self.stopped = None;
        self.state = State::Running;
        vec![self.response(request, body)]
    }

    fn terminate_debuggee(&mut self) {
        if let Some(handle) = &self.debuggee {
            // Force the hook to stop at the next statement so it drains resume_rx and
            // observes Terminate; while merely `Continue`-ing, the hook never reads that
            // channel at all, so a queued Terminate would sit unread until the script ends.
            handle.pause_flag.store(true, Ordering::SeqCst);
            let _ = handle.resume_tx.send(ResumeCmd::Terminate);
        }
        self.stopped = None;
        self.state = State::Exited;
        self.deferred.clear();
    }

    fn source_object(&self, module: Option<&str>) -> Option<Value> {
        let path = module.map(Path::new).or_else(|| self.program.as_ref().map(|source| source.path.as_path()))?;
        Some(json!({
            "name": path.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned()),
            "path": path.display().to_string(),
        }))
    }

    fn handle_stack_trace(&mut self, request: &Value) -> Vec<Value> {
        let Some(info) = self.stopped.as_ref() else {
            return vec![self.failure(request, ErrorCode::NotStopped, NOT_PAUSED)];
        };
        let frames: Vec<Value> = info
            .frames
            .iter()
            .enumerate()
            .map(|(index, frame)| {
                json!({
                    "id": index as i64 + 1,
                    "name": frame.name,
                    "line": frame.line,
                    "column": frame.column,
                    "source": self.source_object(frame.path.as_deref()),
                })
            })
            .collect();
        let total = frames.len();
        vec![self.response(request, json!({ "stackFrames": frames, "totalFrames": total }))]
    }

    fn handle_scopes(&mut self, request: &Value) -> Vec<Value> {
        let Some(frame_count) = self.stopped.as_ref().map(|info| info.frames.len()) else {
            return vec![self.failure(request, ErrorCode::NotStopped, NOT_PAUSED)];
        };
        let frame_id = request["arguments"]["frameId"].as_i64().unwrap_or(1);
        if frame_id < 1 || frame_id > frame_count as i64 {
            return vec![self.failure(request, ErrorCode::UnknownFrame, "Неизвестный кадр стека")];
        }
        let body = json!({
            "scopes": [{
                "name": "Локальные",
                "presentationHint": "locals",
                "variablesReference": LOCALS_SCOPE_BASE + frame_id,
                "expensive": false,
            }],
        });
        vec![self.response(request, body)]
    }

    fn handle_variables(&mut self, request: &Value) -> Vec<Value> {
        let Some(info) = self.stopped.as_ref() else {
            return vec![self.failure(request, ErrorCode::NotStopped, NOT_PAUSED)];
        };
        let reference = request["arguments"]["variablesReference"].as_i64().unwrap_or(0);
        // Only the innermost frame has a live environment: the interpreter keeps no
        // per-call-frame scope snapshots, so outer frames report an empty Locals scope.
        let variables: Vec<Value> = if reference == LOCALS_SCOPE_BASE + 1 {
            info.locals
                .iter()
                .map(|var| {
                    json!({
                        "name": var.name,
                        "value": var.value,
                        "type": var.type_name,
                        "variablesReference": 0,
                    })
                })
                .collect()
        } else {
            Vec::new()
        };
        vec![self.response(request, json!({ "variables": variables }))]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::debuggee::StopReason;
    use std::sync::mpsc::{Receiver, channel};

    fn running_session() -> (Session, Receiver<ResumeCmd>, Arc<std::sync::atomic::AtomicBool>) {
        let (events_tx, _) = channel();
        let mut session = Session::new(events_tx);
        let (resume_tx, resume_rx) = channel();
        let pause_flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
        session.debuggee = Some(DebuggeeHandle { resume_tx, pause_flag: Arc::clone(&pause_flag) });
        session.state = State::Running;
        (session, resume_rx, pause_flag)
    }

    fn stopped(reason: StopReason) -> Incoming {
        let frame = debuggee::DapFrame { name: "(модуль)".to_string(), line: 1, column: 1, path: None };
        Incoming::Debug(DebugMsg::Stopped(Box::new(StopInfo { reason, frames: vec![frame], locals: Vec::new() })))
    }

    fn request(seq: i64, command: &str) -> Incoming {
        Incoming::Client(json!({ "seq": seq, "type": "request", "command": command, "arguments": { "threadId": 1 } }))
    }

    fn kinds(out: &[Value]) -> Vec<String> {
        out.iter()
            .map(|message| match message["type"].as_str() {
                Some("event") => format!("event:{}", message["event"].as_str().unwrap_or_default()),
                _ => format!("response:{}:{}", message["command"].as_str().unwrap_or_default(), message["success"]),
            })
            .collect()
    }

    #[test]
    fn pause_raced_with_a_stop_does_not_survive_continue() {
        let (mut session, resume_rx, pause_flag) = running_session();

        session.handle(request(1, "pause"));
        session.handle(stopped(StopReason::Breakpoint));

        assert!(pause_flag.load(Ordering::SeqCst));

        let out = session.handle(request(2, "continue"));

        assert_eq!(kinds(&out), ["response:continue:true"]);
        assert!(!pause_flag.load(Ordering::SeqCst), "флаг паузы должен сброситься перед продолжением");
        assert_eq!(resume_rx.try_recv().ok(), Some(ResumeCmd::Continue));
    }

    #[test]
    fn error_codes_keep_their_wire_strings() {
        let expected = [
            (ErrorCode::NotStopped, "notStopped"),
            (ErrorCode::Unsupported, "unsupported"),
            (ErrorCode::NoProgram, "noProgram"),
            (ErrorCode::UnknownFrame, "unknownFrame"),
            (ErrorCode::DebuggeeGone, "debuggeeGone"),
        ];
        for (code, text) in expected {
            assert_eq!(code.as_str(), text);
        }
    }
}
