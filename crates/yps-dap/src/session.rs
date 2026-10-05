use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

use yps_lexer::{Lexer, SourceFile};
use yps_parser::Parser;

use crate::breakpoints;
use crate::debuggee::{self, DebugMsg, DebuggeeHandle, LaunchConfig, ResumeCmd, StopInfo, StopReason};

pub const THREAD_ID: i64 = 1;
const LOCALS_SCOPE_BASE: i64 = 1000;
const NOT_PAUSED: &str = "Программа не находится на паузе";

#[derive(Clone, Copy)]
enum ErrorCode {
    NotStopped = 1,
    Cancelled = 2,
    Unsupported = 3,
    NoProgram = 4,
    UnknownFrame = 5,
    DebuggeeGone = 6,
    SessionEnded = 8,
}

impl ErrorCode {
    const fn as_str(self) -> &'static str {
        match self {
            Self::NotStopped => "notStopped",
            Self::Cancelled => "cancelled",
            Self::Unsupported => "unsupported",
            Self::NoProgram => "noProgram",
            Self::UnknownFrame => "unknownFrame",
            Self::DebuggeeGone => "debuggeeGone",
            Self::SessionEnded => "sessionEnded",
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

fn arg_index(value: &Value) -> Option<usize> {
    value.as_u64().and_then(|n| usize::try_from(n).ok())
}

fn unverified(id: i64, line: usize, message: &str, reason: &str) -> Value {
    json!({ "id": id, "verified": false, "line": line, "message": message, "reason": reason })
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

struct ClientCaps {
    lines_start_at1: bool,
    columns_start_at1: bool,
    variable_type: bool,
}

impl ClientCaps {
    fn from_initialize(arguments: &Value) -> Self {
        Self {
            lines_start_at1: arguments["linesStartAt1"].as_bool().unwrap_or(true),
            columns_start_at1: arguments["columnsStartAt1"].as_bool().unwrap_or(true),
            variable_type: arguments["supportsVariableType"].as_bool().unwrap_or(false),
        }
    }
}

impl Default for ClientCaps {
    fn default() -> Self {
        Self::from_initialize(&Value::Null)
    }
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
    terminated_sent: bool,
    next_breakpoint_id: i64,
    program: Option<Source>,
    stop_on_entry: bool,
    caps: ClientCaps,
    breakpoints: Arc<Mutex<HashSet<usize>>>,
    breakpoint_ids: HashMap<usize, Vec<i64>>,
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
            terminated_sent: false,
            next_breakpoint_id: 0,
            program: None,
            stop_on_entry: false,
            caps: ClientCaps::default(),
            breakpoints: Arc::new(Mutex::new(HashSet::new())),
            breakpoint_ids: HashMap::new(),
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

    fn next_breakpoint_id(&mut self) -> i64 {
        self.next_breakpoint_id += 1;
        self.next_breakpoint_id
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

    const fn line_to_client(&self, line: usize) -> usize {
        if self.caps.lines_start_at1 { line } else { line.saturating_sub(1) }
    }

    const fn column_to_client(&self, column: usize) -> usize {
        if self.caps.columns_start_at1 { column } else { column.saturating_sub(1) }
    }

    fn event(&mut self, name: &str, body: Value) -> Value {
        let seq = self.next_seq();
        json!({ "seq": seq, "type": "event", "event": name, "body": body })
    }

    pub fn handle(&mut self, incoming: Incoming) -> Vec<Value> {
        match incoming {
            Incoming::Client(request) if request["type"] == "request" => self.handle_client(&request),
            Incoming::Client(_) => Vec::new(),
            Incoming::ClientEof => {
                let _ = self.terminate_debuggee();
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
            DebugMsg::Stopped(_) if self.state == State::Exited => {}
            DebugMsg::Stopped(info) => {
                let reason = info.reason.as_dap();
                let mut body = json!({ "reason": reason, "threadId": THREAD_ID, "allThreadsStopped": true });
                if info.reason == StopReason::Breakpoint
                    && let Some(ids) = info.frames.first().and_then(|frame| self.breakpoint_ids.get(&frame.line))
                {
                    body["hitBreakpointIds"] = json!(ids);
                }
                self.stopped = Some(*info);
                self.state = State::Stopped;
                out.push(self.event("stopped", body));
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
                out.push(self.event("exited", json!({ "exitCode": exit_code })));
                if !std::mem::replace(&mut self.terminated_sent, true) {
                    out.push(self.event("terminated", json!({})));
                }
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
            "initialize" => self.handle_initialize(request),
            "launch" => self.handle_launch(request),
            "setBreakpoints" => self.handle_set_breakpoints(request),
            "setExceptionBreakpoints" => {
                vec![self.response(request, json!({ "breakpoints": [] }))]
            }
            "loadedSources" => vec![self.response(request, json!({ "sources": [] }))],
            "configurationDone" => self.handle_configuration_done(request),
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
            "disconnect" | "terminate" => self.handle_disconnect(request, command == "disconnect"),
            other => {
                let message = format!("Команда '{other}' не поддерживается");
                vec![self.failure(request, ErrorCode::Unsupported, message)]
            }
        }
    }

    fn handle_initialize(&mut self, request: &Value) -> Vec<Value> {
        self.caps = ClientCaps::from_initialize(&request["arguments"]);
        let body = json!({
            "supportsConfigurationDoneRequest": true,
            "supportsTerminateRequest": true,
            "supportsStepInTargetsRequest": false,
            "supportsEvaluateForHovers": false,
            "supportsFunctionBreakpoints": false,
            "supportsConditionalBreakpoints": false,
        });
        vec![self.response(request, body)]
    }

    fn handle_configuration_done(&mut self, request: &Value) -> Vec<Value> {
        if self.program.is_none() {
            return vec![self.failure(request, ErrorCode::NoProgram, "Программа не задана: сначала пришлите 'launch'")];
        }
        if self.terminated_sent || self.state == State::Exited {
            return vec![self.failure(request, ErrorCode::SessionEnded, "Сеанс отладки уже завершён")];
        }
        let response = self.response(request, json!({}));
        self.start_debuggee();
        vec![response]
    }

    fn handle_disconnect(&mut self, request: &Value, end_session: bool) -> Vec<Value> {
        let mut out = self.terminate_debuggee();
        self.should_exit |= end_session;
        out.push(self.response(request, json!({})));
        if !std::mem::replace(&mut self.terminated_sent, true) {
            out.push(self.event("terminated", json!({})));
        }
        out
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
            items.iter().filter_map(|item| arg_index(&item["line"])).collect()
        } else {
            arguments["lines"].as_array().into_iter().flatten().filter_map(arg_index).collect()
        };

        let ids: Vec<i64> = requested.iter().map(|_| self.next_breakpoint_id()).collect();

        let path = arguments["source"]["path"].as_str().map(Path::new);
        let launched = self.program.as_ref().filter(|program| path.is_some_and(|path| same_file(&program.path, path)));
        let Some(program) = launched else {
            let (reason, message) = match (path, &self.program) {
                (None, _) => ("failed", "Поддерживаются только источники с путём к файлу"),
                (Some(_), None) => ("pending", "Сначала пришлите 'launch'"),
                (Some(_), Some(_)) => ("failed", "Отлаживается только запущенный файл"),
            };
            let rejected: Vec<Value> =
                requested.into_iter().zip(ids).map(|(line, id)| unverified(id, line, message, reason)).collect();
            return vec![self.response(request, json!({ "breakpoints": rejected }))];
        };

        let offset = usize::from(!self.caps.lines_start_at1);
        let mut verified = Vec::new();
        let mut resolved: HashMap<usize, Vec<i64>> = HashMap::new();
        for (line, id) in requested.into_iter().zip(ids) {
            match line.checked_add(offset).and_then(|line| breakpoints::resolve_line(line, &program.statement_lines)) {
                Some(actual) => {
                    resolved.entry(actual).or_default().push(id);
                    verified.push(json!({ "id": id, "verified": true, "line": self.line_to_client(actual) }));
                }
                None => verified.push(unverified(id, line, "На этой строке нет оператора", "failed")),
            }
        }
        if let Ok(mut set) = self.breakpoints.lock() {
            *set = resolved.keys().copied().collect();
        }
        self.breakpoint_ids = resolved;
        vec![self.response(request, json!({ "breakpoints": verified }))]
    }

    fn start_debuggee(&mut self) {
        if self.debuggee.is_some() {
            return;
        }
        let Some(program) = self.program.as_ref().map(|source| source.path.clone()) else {
            return;
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

    fn terminate_debuggee(&mut self) -> Vec<Value> {
        if let Some(handle) = &self.debuggee {
            // Force the hook to stop at the next statement so it drains resume_rx and
            // observes Terminate; while merely `Continue`-ing, the hook never reads that
            // channel at all, so a queued Terminate would sit unread until the script ends.
            handle.pause_flag.store(true, Ordering::SeqCst);
            let _ = handle.resume_tx.send(ResumeCmd::Terminate);
        }
        self.stopped = None;
        self.state = State::Exited;
        let abandoned: Vec<Value> = self.deferred.drain(..).collect();
        abandoned.iter().map(|request| self.failure(request, ErrorCode::Cancelled, "Сеанс отладки завершён")).collect()
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
                    "line": self.line_to_client(frame.line),
                    "column": self.column_to_client(frame.column),
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
                    let mut variable = json!({
                        "name": var.name,
                        "value": var.value,
                        "variablesReference": 0,
                    });
                    if self.caps.variable_type {
                        variable["type"] = json!(var.type_name);
                    }
                    variable
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
    use crate::test_support::{Lcg, fixture};
    use std::sync::mpsc::{Receiver, channel};
    use std::time::{Duration, Instant};

    fn random_number(rng: &mut Lcg) -> Value {
        let options = [
            json!(-1),
            json!(0),
            json!(1),
            json!(2),
            json!(3),
            json!(5),
            json!(999),
            json!(1000),
            json!(1001),
            json!(1002),
            json!(i64::MAX),
            json!(i64::MIN),
            json!(u64::MAX),
            json!(1e300),
            json!(-1e300),
            json!(0.5),
            json!("7"),
            json!(null),
            json!(true),
            json!([]),
            json!({}),
        ];
        rng.pick(&options)
    }

    fn random_path(rng: &mut Lcg) -> Value {
        let options = [
            json!(fixture("loop.yopta")),
            json!(fixture("loop.yopta")),
            json!(fixture("call.yopta")),
            json!(fixture("print.yopta")),
            json!(fixture("stdin.yopta")),
            json!(fixture("syntax_error.yopta")),
            json!(fixture("module_main.yopta")),
            json!(fixture("module_throw_main.yopta")),
            json!(fixture("blank_line.yopta")),
            json!(env!("CARGO_MANIFEST_DIR")),
            json!(""),
            json!("\0"),
            json!("a\0b.yopta"),
            json!("/nonexistent/dir/x.yopta"),
            json!("relative.yopta"),
            json!("/"),
            json!("../../.."),
            json!("a".repeat(5000)),
            json!(123),
            json!(null),
            json!([]),
            json!({ "path": "x" }),
        ];
        rng.pick(&options)
    }

    fn random_string(rng: &mut Lcg) -> String {
        let alphabet: Vec<char> = "abcЖюя \0\n\"\\{}[]0123456789-.".chars().collect();
        let len = rng.below(24);
        (0..len).map(|_| alphabet[rng.below(alphabet.len())]).collect()
    }

    fn random_value(rng: &mut Lcg, depth: usize) -> Value {
        match rng.below(if depth == 0 { 4 } else { 6 }) {
            0 => random_number(rng),
            1 => random_path(rng),
            2 => json!(random_string(rng)),
            3 => json!(rng.below(2) == 0),
            4 => Value::Array((0..rng.below(4)).map(|_| random_value(rng, depth - 1)).collect()),
            _ => {
                let mut map = serde_json::Map::new();
                for _ in 0..rng.below(4) {
                    let key = rng.pick(&["line", "path", "source", "breakpoints", "lines", "frameId", "program"]);
                    map.insert(key.to_string(), random_value(rng, depth - 1));
                }
                Value::Object(map)
            }
        }
    }

    fn random_breakpoint(rng: &mut Lcg) -> Value {
        match rng.below(4) {
            0 => json!({ "line": random_number(rng) }),
            1 => json!({ "line": rng.below(8) }),
            2 => random_value(rng, 1),
            _ => json!({}),
        }
    }

    fn targeted_arguments(rng: &mut Lcg, command: &str) -> Value {
        match command {
            "launch" => json!({ "program": random_path(rng), "stopOnEntry": random_value(rng, 1) }),
            "setBreakpoints" => {
                let source = match rng.below(4) {
                    0 => json!({ "path": random_path(rng) }),
                    1 => json!({ "path": fixture("loop.yopta") }),
                    2 => random_value(rng, 1),
                    _ => json!({}),
                };
                let mut arguments = json!({ "source": source });
                let key = if rng.below(5) == 0 { "lines" } else { "breakpoints" };
                arguments[key] = Value::Array(
                    (0..rng.below(5))
                        .map(|_| if key == "lines" { random_number(rng) } else { random_breakpoint(rng) })
                        .collect(),
                );
                arguments
            }
            "scopes" => json!({ "frameId": random_number(rng) }),
            "variables" => {
                json!({ "variablesReference": random_number(rng), "start": random_number(rng), "count": random_number(rng) })
            }
            "stackTrace" => json!({
                "threadId": random_number(rng),
                "startFrame": random_number(rng),
                "levels": random_number(rng),
            }),
            "continue" | "next" | "stepIn" | "stepOut" | "pause" => json!({ "threadId": random_number(rng) }),
            _ => random_value(rng, 2),
        }
    }

    const COMMANDS: [&str; 18] = [
        "initialize",
        "launch",
        "setBreakpoints",
        "setExceptionBreakpoints",
        "loadedSources",
        "configurationDone",
        "threads",
        "stackTrace",
        "scopes",
        "variables",
        "evaluate",
        "continue",
        "next",
        "stepIn",
        "stepOut",
        "pause",
        "terminate",
        "disconnect",
    ];

    fn random_request(rng: &mut Lcg, seq: i64) -> Value {
        let command = match rng.below(12) {
            0 => json!(random_string(rng)),
            1 => random_number(rng),
            _ => {
                let mut name = rng.pick(&COMMANDS);
                if matches!(name, "terminate" | "disconnect") && rng.below(8) != 0 {
                    name = "threads";
                }
                json!(name)
            }
        };
        let name = command.as_str().unwrap_or_default().to_string();
        let mut request = json!({ "seq": seq, "type": "request", "command": command });
        match rng.below(8) {
            0 => {}
            1 => request["arguments"] = random_value(rng, 2),
            _ => request["arguments"] = targeted_arguments(rng, &name),
        }
        if rng.below(12) == 0 {
            request["seq"] = rng.pick(&[json!("x"), json!(null), json!(1.5), json!(u64::MAX)]);
        }
        request
    }

    struct Harness {
        session: Session,
        rx: Receiver<Incoming>,
        log: Vec<Value>,
        responses: Vec<Value>,
    }

    impl Harness {
        fn new() -> Self {
            let (tx, rx) = channel();
            Self { session: Session::new(tx), rx, log: Vec::new(), responses: Vec::new() }
        }

        fn absorb(&mut self, out: Vec<Value>) {
            self.responses.extend(out.into_iter().filter(|message| message["type"] == "response"));
        }

        fn send(&mut self, request: Value) {
            self.log.push(request.clone());
            let out = self.session.handle(Incoming::Client(request));
            self.absorb(out);
        }

        fn drain(&mut self) {
            while let Ok(incoming) = self.rx.try_recv() {
                let out = self.session.handle(incoming);
                self.absorb(out);
            }
        }

        fn settle(&mut self, case: usize) {
            let deadline = Instant::now() + Duration::from_secs(20);
            while self.session.state == State::Running {
                let left = deadline.saturating_duration_since(Instant::now());
                let incoming = self
                    .rx
                    .recv_timeout(left)
                    .unwrap_or_else(|_| panic!("case {case}: сессия зависла в Running, запросы: {:?}", self.log));
                let out = self.session.handle(incoming);
                self.absorb(out);
            }
            self.drain();
        }
    }

    #[test]
    fn session_survives_pseudo_random_requests_and_answers_each_exactly_once() {
        let mut rng = Lcg(0x0123_4567_89ab_cdef);
        let mut seq = 0;
        let mut inspected_while_paused = 0;
        for case in 0..1000 {
            let mut harness = Harness::new();
            let prefix = rng.below(4);
            if prefix >= 1 {
                seq += 1;
                harness.send(json!({ "seq": seq, "type": "request", "command": "initialize", "arguments": {} }));
                seq += 1;
                let program = fixture(rng.pick(&["loop.yopta", "call.yopta", "print.yopta", "module_main.yopta"]));
                harness.send(json!({
                    "seq": seq,
                    "type": "request",
                    "command": "launch",
                    "arguments": { "program": program, "stopOnEntry": rng.below(2) == 0 },
                }));
            }
            if prefix >= 2 {
                seq += 1;
                harness.send(json!({
                    "seq": seq,
                    "type": "request",
                    "command": "setBreakpoints",
                    "arguments": { "source": { "path": fixture("loop.yopta") }, "breakpoints": [{ "line": 3 }] },
                }));
            }
            if prefix >= 3 {
                seq += 1;
                harness.send(json!({ "seq": seq, "type": "request", "command": "configurationDone" }));
            }
            for _ in 0..(3 + rng.below(12)) {
                seq += 1;
                let request = random_request(&mut rng, seq);
                harness.send(request);
                if rng.below(4) != 0 {
                    harness.settle(case);
                } else {
                    harness.drain();
                }
            }
            harness.settle(case);
            seq += 1;
            harness.send(json!({ "seq": seq, "type": "request", "command": "disconnect" }));
            harness.drain();

            assert_eq!(
                harness.responses.len(),
                harness.log.len(),
                "case {case}: число ответов не совпало с числом запросов\nзапросы: {}\nответы: {}",
                harness.log.iter().map(Value::to_string).collect::<Vec<_>>().join("\n"),
                harness.responses.iter().map(Value::to_string).collect::<Vec<_>>().join("\n"),
            );
            inspected_while_paused += harness
                .responses
                .iter()
                .filter(|response| response["command"] == "stackTrace" && response["success"] == true)
                .count();
            for response in &harness.responses {
                assert!(response["success"].is_boolean(), "case {case}: ответ без success: {response}");
                assert!(response["request_seq"].is_i64(), "case {case}: ответ без request_seq: {response}");
            }
            for request in &harness.log {
                if let Some(request_seq) = request["seq"].as_i64() {
                    let answers =
                        harness.responses.iter().filter(|response| response["request_seq"] == request_seq).count();
                    assert_eq!(answers, 1, "case {case}: запрос {request} получил {answers} ответов");
                }
            }
        }
        assert!(inspected_while_paused > 20, "fuzz never reached a paused debuggee");
    }

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
    fn terminate_while_running_sends_terminated_at_once_swallows_the_wakeup_stop_and_exited_follows_alone() {
        let (mut session, resume_rx, pause_flag) = running_session();

        let out = session.handle(request(1, "terminate"));

        assert_eq!(kinds(&out), ["response:terminate:true", "event:terminated"]);
        assert!(pause_flag.load(Ordering::SeqCst));
        assert_eq!(resume_rx.try_recv().ok(), Some(ResumeCmd::Terminate));

        let out = session.handle(stopped(StopReason::Pause));

        assert!(out.is_empty(), "{out:?}");
        assert_eq!(session.state, State::Exited);
        assert!(session.stopped.is_none());

        let out = session.handle(Incoming::Debug(DebugMsg::Exited { error: None }));

        assert_eq!(kinds(&out), ["event:exited"]);

        let out = session.handle(request(2, "stackTrace"));

        assert_eq!(out[0]["message"], "notStopped", "{out:?}");

        let out = session.handle(request(3, "terminate"));

        assert_eq!(kinds(&out), ["response:terminate:true"]);

        let out = session.handle(request(4, "disconnect"));

        assert_eq!(kinds(&out), ["response:disconnect:true"]);
    }

    #[test]
    fn terminate_before_configuration_done_emits_terminated_at_once() {
        let (events_tx, _) = channel();
        let mut session = Session::new(events_tx);

        let out = session.handle(request(1, "terminate"));

        assert_eq!(kinds(&out), ["response:terminate:true", "event:terminated"]);
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
            (ErrorCode::Cancelled, "cancelled"),
            (ErrorCode::Unsupported, "unsupported"),
            (ErrorCode::NoProgram, "noProgram"),
            (ErrorCode::UnknownFrame, "unknownFrame"),
            (ErrorCode::DebuggeeGone, "debuggeeGone"),
            (ErrorCode::SessionEnded, "sessionEnded"),
        ];
        for (code, text) in expected {
            assert_eq!(code.as_str(), text);
        }
    }
}
