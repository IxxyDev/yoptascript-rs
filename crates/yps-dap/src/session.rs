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

/// What arrives on the adapter's single event queue.
pub enum Incoming {
    Client(Value),
    ClientEof,
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
    program: Option<PathBuf>,
    stop_on_entry: bool,
    source_path: Option<PathBuf>,
    statement_lines: BTreeSet<usize>,
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
            source_path: None,
            statement_lines: BTreeSet::new(),
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

    fn response(&mut self, request: &Value, body: Value) -> Value {
        let seq = self.next_seq();
        json!({
            "seq": seq,
            "type": "response",
            "request_seq": request["seq"].as_i64().unwrap_or(0),
            "success": true,
            "command": request["command"].as_str().unwrap_or_default(),
            "body": body,
        })
    }

    fn failure(&mut self, request: &Value, message: impl Into<String>) -> Value {
        let seq = self.next_seq();
        json!({
            "seq": seq,
            "type": "response",
            "request_seq": request["seq"].as_i64().unwrap_or(0),
            "success": false,
            "command": request["command"].as_str().unwrap_or_default(),
            "message": message.into(),
        })
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
                let initialized = self.event("initialized", json!({}));
                vec![response, initialized]
            }
            "launch" => self.handle_launch(request),
            "setBreakpoints" => self.handle_set_breakpoints(request),
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
                if let Some(handle) = &self.debuggee {
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
                vec![self.failure(request, message)]
            }
        }
    }

    fn handle_launch(&mut self, request: &Value) -> Vec<Value> {
        let Some(program) = request["arguments"]["program"].as_str() else {
            return vec![self.failure(request, "В 'launch' не указан аргумент 'program'")];
        };
        self.stop_on_entry = request["arguments"]["stopOnEntry"].as_bool().unwrap_or(false);
        if self.source_path.is_none() {
            self.load_statement_lines(Path::new(program));
        }
        self.program = Some(PathBuf::from(program));
        vec![self.response(request, json!({}))]
    }

    fn load_statement_lines(&mut self, path: &Path) {
        let Ok(text) = std::fs::read_to_string(path) else {
            return;
        };
        let source = SourceFile::new(path.display().to_string(), text);
        let (tokens, _) = Lexer::new(&source).tokenize();
        let (program, _) = Parser::new(&tokens, &source).parse_program();
        self.statement_lines = breakpoints::statement_lines(&program, &source);
        self.source_path = Some(path.to_path_buf());
    }

    fn handle_set_breakpoints(&mut self, request: &Value) -> Vec<Value> {
        if let Some(path) = request["arguments"]["source"]["path"].as_str() {
            let path = Path::new(path);
            if self.source_path.as_deref() != Some(path) {
                self.load_statement_lines(path);
            }
        }

        let arguments = &request["arguments"];
        let requested: Vec<usize> = if let Some(items) = arguments["breakpoints"].as_array() {
            items.iter().filter_map(|item| item["line"].as_u64()).map(|line| line as usize).collect()
        } else if let Some(items) = arguments["lines"].as_array() {
            items.iter().filter_map(Value::as_u64).map(|line| line as usize).collect()
        } else {
            Vec::new()
        };

        let mut verified = Vec::new();
        let mut resolved = HashSet::new();
        for (index, line) in requested.into_iter().enumerate() {
            match breakpoints::resolve_line(line, &self.statement_lines) {
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
        let Some(program) = self.program.clone() else {
            return vec![self.failure(request, "Программа не задана: сначала пришлите 'launch'")];
        };
        let tx = self.events_tx.clone();
        let handle = debuggee::spawn(
            LaunchConfig { program, stop_on_entry: self.stop_on_entry, breakpoints: Arc::clone(&self.breakpoints) },
            move |msg| {
                let _ = tx.send(Incoming::Debug(msg));
            },
        );
        self.debuggee = Some(handle);
        self.state = State::Running;
        Vec::new()
    }

    fn resume(&mut self, request: &Value, cmd: ResumeCmd, body: Value) -> Vec<Value> {
        if self.state != State::Stopped {
            return vec![self.failure(request, NOT_PAUSED)];
        }
        let sent = self.debuggee.as_ref().is_some_and(|handle| handle.resume_tx.send(cmd).is_ok());
        if !sent {
            return vec![self.failure(request, "Отлаживаемая программа недоступна")];
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

    fn source_object(&self) -> Value {
        match &self.program {
            Some(path) => json!({
                "name": path.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned()),
                "path": path.display().to_string(),
            }),
            None => Value::Null,
        }
    }

    fn handle_stack_trace(&mut self, request: &Value) -> Vec<Value> {
        let Some(info) = self.stopped.as_ref() else {
            return vec![self.failure(request, NOT_PAUSED)];
        };
        let source = self.source_object();
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
                    "source": source,
                })
            })
            .collect();
        let total = frames.len();
        vec![self.response(request, json!({ "stackFrames": frames, "totalFrames": total }))]
    }

    fn handle_scopes(&mut self, request: &Value) -> Vec<Value> {
        let Some(frame_count) = self.stopped.as_ref().map(|info| info.frames.len()) else {
            return vec![self.failure(request, NOT_PAUSED)];
        };
        let frame_id = request["arguments"]["frameId"].as_i64().unwrap_or(1);
        if frame_id < 1 || frame_id > frame_count as i64 {
            return vec![self.failure(request, "Неизвестный кадр стека")];
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
            return vec![self.failure(request, NOT_PAUSED)];
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
