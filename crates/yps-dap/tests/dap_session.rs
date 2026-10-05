use std::io::{BufReader, PipeWriter};
use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::time::Duration;

use serde_json::{Value, json};

use yps_dap::protocol;

const MESSAGE_TIMEOUT: Duration = Duration::from_secs(10);

struct Client {
    to_server: PipeWriter,
    from_server: Receiver<Value>,
    seq: i64,
}

impl Client {
    fn start() -> Self {
        let (server_in, to_server) = std::io::pipe().expect("канал запроса");
        let (from_server, server_out) = std::io::pipe().expect("канал ответа");
        std::thread::spawn(move || {
            let mut out = server_out;
            yps_dap::serve(server_in, &mut out).expect("сервер должен отработать");
        });
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(from_server);
            while let Some(message) = protocol::read_message(&mut reader).expect("чтение") {
                if tx.send(message).is_err() {
                    return;
                }
            }
        });
        Self { to_server, from_server: rx, seq: 0 }
    }

    fn request(&mut self, command: &str, arguments: Value) -> i64 {
        self.seq += 1;
        let message = json!({ "seq": self.seq, "type": "request", "command": command, "arguments": arguments });
        protocol::write_message(&mut self.to_server, &message).expect("запрос должен уйти");
        self.seq
    }

    fn next_message(&mut self) -> Value {
        self.from_server.recv_timeout(MESSAGE_TIMEOUT).expect("адаптер не ответил вовремя")
    }

    fn answer(&mut self, command: &str, arguments: Value) -> Value {
        let seq = self.request(command, arguments);
        loop {
            let message = self.next_message();
            if message["type"] == "response" && message["request_seq"] == seq {
                return message;
            }
        }
    }

    fn call(&mut self, command: &str, arguments: Value) -> Value {
        let message = self.answer(command, arguments);
        assert_eq!(message["success"], true, "{command} должен успешно выполниться: {message}");
        message
    }

    fn wait_event(&mut self, name: &str) -> Value {
        loop {
            let message = self.next_message();
            if message["type"] == "event" && message["event"] == name {
                return message;
            }
        }
    }

    fn expect_exit_without_stop(&mut self) -> Value {
        loop {
            let message = self.next_message();
            assert!(!(message["type"] == "event" && message["event"] == "stopped"), "лишняя остановка: {message}");
            if message["type"] == "event" && message["event"] == "exited" {
                return message;
            }
        }
    }

    fn set_breakpoints(&mut self, path: &str, lines: &[usize]) -> Value {
        let breakpoints: Vec<Value> = lines.iter().map(|line| json!({ "line": line })).collect();
        self.call("setBreakpoints", json!({ "source": { "path": path }, "breakpoints": breakpoints }))
    }

    fn handshake(&mut self, fixture: &str, stop_on_entry: bool, breakpoint_lines: &[usize]) -> Value {
        self.call("initialize", json!({ "adapterID": "yopta" }));
        let program = fixture_path(fixture);
        self.call("launch", json!({ "program": program, "stopOnEntry": stop_on_entry }));
        let set = self.set_breakpoints(&program, breakpoint_lines);
        self.call("configurationDone", json!({}));
        set
    }

    fn frames(&mut self) -> Vec<Value> {
        let response = self.call("stackTrace", json!({ "threadId": 1 }));
        response["body"]["stackFrames"].as_array().cloned().expect("stackFrames")
    }

    fn locals(&mut self, frame_id: i64) -> Vec<Value> {
        let scopes = self.call("scopes", json!({ "frameId": frame_id }));
        let reference = scopes["body"]["scopes"][0]["variablesReference"].as_i64().expect("ссылка на переменные");
        let response = self.call("variables", json!({ "variablesReference": reference }));
        response["body"]["variables"].as_array().cloned().expect("variables")
    }
}

fn fixture_path(name: &str) -> String {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests/fixtures");
    path.push(name);
    path.display().to_string()
}

fn local_value(variables: &[Value], name: &str) -> Option<String> {
    variables.iter().find(|v| v["name"] == name).map(|v| v["value"].as_str().unwrap_or_default().to_string())
}

#[test]
fn breakpoint_hit_inspection_and_continue_to_completion() {
    let mut client = Client::start();

    let set = client.handshake("loop.yopta", false, &[3]);

    assert_eq!(set["body"]["breakpoints"][0]["verified"], true);
    assert_eq!(set["body"]["breakpoints"][0]["line"], 3);

    let stopped = client.wait_event("stopped");

    assert_eq!(stopped["body"]["reason"], "breakpoint");
    assert_eq!(stopped["body"]["threadId"], 1);

    let threads = client.call("threads", json!({}));

    assert_eq!(threads["body"]["threads"].as_array().map(Vec::len), Some(1));

    let frames = client.frames();

    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0]["line"], 3);
    assert_eq!(frames[0]["name"], "(модуль)");
    assert_eq!(frames[0]["source"]["name"], "loop.yopta");

    let variables = client.locals(frames[0]["id"].as_i64().unwrap());

    assert_eq!(local_value(&variables, "i").as_deref(), Some("0"));
    assert_eq!(local_value(&variables, "сумма").as_deref(), Some("0"));

    client.call("continue", json!({ "threadId": 1 }));
    client.wait_event("stopped");
    let variables = client.locals(1);

    assert_eq!(local_value(&variables, "i").as_deref(), Some("1"));
    assert_eq!(local_value(&variables, "сумма").as_deref(), Some("0"));

    client.call("continue", json!({ "threadId": 1 }));
    client.wait_event("stopped");
    let variables = client.locals(1);

    assert_eq!(local_value(&variables, "i").as_deref(), Some("2"));
    assert_eq!(local_value(&variables, "сумма").as_deref(), Some("1"));

    client.call("continue", json!({ "threadId": 1 }));

    client.wait_event("terminated");
    client.wait_event("exited");
}

#[test]
fn step_over_stays_in_the_caller() {
    let mut client = Client::start();
    client.handshake("call.yopta", true, &[]);

    let stopped = client.wait_event("stopped");

    assert_eq!(stopped["body"]["reason"], "entry");
    assert_eq!(client.frames()[0]["line"], 1);

    client.call("next", json!({ "threadId": 1 }));
    let stopped = client.wait_event("stopped");
    let frames = client.frames();

    assert_eq!(stopped["body"]["reason"], "step");
    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0]["line"], 5);

    client.call("next", json!({ "threadId": 1 }));
    client.wait_event("stopped");
    let frames = client.frames();

    assert_eq!(frames.len(), 1, "шаг через вызов не должен заходить внутрь удвоить");
    assert_eq!(frames[0]["line"], 6);

    client.call("disconnect", json!({}));
}

#[test]
fn step_in_descends_into_the_callee_and_step_out_returns() {
    let mut client = Client::start();
    client.handshake("call.yopta", true, &[]);
    client.wait_event("stopped");

    client.call("next", json!({ "threadId": 1 }));
    client.wait_event("stopped");

    assert_eq!(client.frames()[0]["line"], 5);

    client.call("stepIn", json!({ "threadId": 1 }));
    client.wait_event("stopped");
    let frames = client.frames();

    assert_eq!(frames.len(), 2, "внутри удвоить видно два кадра");
    assert_eq!(frames[0]["name"], "удвоить");
    assert_eq!(frames[0]["line"], 2);
    assert_eq!(frames[1]["name"], "(модуль)");
    assert_eq!(frames[1]["line"], 5, "кадр вызывающего показывает место вызова");

    let variables = client.locals(1);
    let outer = client.locals(2);

    assert_eq!(local_value(&variables, "a").as_deref(), Some("21"));
    assert!(outer.is_empty(), "внешние кадры не хранят окружение");

    client.call("stepOut", json!({ "threadId": 1 }));
    client.wait_event("stopped");
    let frames = client.frames();
    let variables = client.locals(1);

    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0]["line"], 6);
    assert_eq!(local_value(&variables, "x").as_deref(), Some("42"));

    client.call("disconnect", json!({}));
}

#[test]
fn breakpoint_on_a_closing_brace_snaps_to_the_next_statement() {
    let mut client = Client::start();

    let set = client.handshake("loop.yopta", false, &[4]);

    assert_eq!(set["body"]["breakpoints"][0]["verified"], true);
    assert_eq!(set["body"]["breakpoints"][0]["line"], 5, "строка 4 — закрывающая скобка, ближайший оператор на 5");

    client.wait_event("stopped");
    let variables = client.locals(1);

    assert_eq!(local_value(&variables, "сумма").as_deref(), Some("3"));

    client.call("continue", json!({ "threadId": 1 }));

    client.wait_event("terminated");
}

#[test]
fn breakpoint_on_a_blank_line_snaps_to_the_next_statement() {
    let mut client = Client::start();

    let set = client.handshake("blank_line.yopta", false, &[2]);

    assert_eq!(set["body"]["breakpoints"][0]["verified"], true);
    assert_eq!(set["body"]["breakpoints"][0]["line"], 3, "строка 2 пустая, ближайший оператор на 3");

    client.wait_event("stopped");
    let variables = client.locals(1);

    assert_eq!(local_value(&variables, "а").as_deref(), Some("1"));

    client.call("continue", json!({ "threadId": 1 }));

    client.wait_event("terminated");
}

#[test]
fn debuggee_output_arrives_as_output_events() {
    let mut client = Client::start();
    client.handshake("print.yopta", false, &[]);

    let first = client.wait_event("output");
    let second = client.wait_event("output");
    let third = client.wait_event("output");
    let exited = client.wait_event("exited");

    assert_eq!(first["body"]["category"], "stdout");
    assert_eq!(first["body"]["output"], "привет\n");
    assert_eq!(second["body"]["category"], "stderr");
    assert_eq!(second["body"]["output"], "боль\n");
    assert_eq!(third["body"]["category"], "stdout");
    assert_eq!(third["body"]["output"], "пока\n");
    assert_eq!(exited["body"]["exitCode"], 0);
}

#[test]
fn stdin_read_raises_a_catchable_error() {
    let mut client = Client::start();
    client.handshake("stdin.yopta", false, &[]);

    let event = client.wait_event("output");
    let exited = client.wait_event("exited");

    let text = event["body"]["output"].as_str().unwrap_or_default();
    assert_eq!(event["body"]["category"], "stdout");
    assert!(text.starts_with("поймали: "), "вывод должен прийти из гоп: {text}");
    assert!(text.contains("stdin"), "сообщение должно объяснять причину: {text}");
    assert_eq!(exited["body"]["exitCode"], 0);
}

#[test]
fn module_output_is_captured_and_does_not_corrupt_the_protocol() {
    let mut client = Client::start();
    client.handshake("module_main.yopta", false, &[]);

    let first = client.wait_event("output");
    let second = client.wait_event("output");
    let exited = client.wait_event("exited");

    assert_eq!(first["body"]["category"], "stdout");
    assert_eq!(first["body"]["output"], "из модуля\n");
    assert_eq!(second["body"]["output"], "из главного 5\n");
    assert_eq!(exited["body"]["exitCode"], 0);
}

#[test]
fn breakpoints_in_a_foreign_file_are_rejected_and_do_not_clobber_real_ones() {
    let mut client = Client::start();
    let program = fixture_path("loop.yopta");
    client.call("initialize", json!({ "adapterID": "yopta" }));
    client.call("launch", json!({ "program": program, "stopOnEntry": false }));

    let set = client.set_breakpoints(&program, &[3]);
    let foreign = client.set_breakpoints(&fixture_path("blank_line.yopta"), &[3]);

    assert_eq!(set["body"]["breakpoints"][0]["verified"], true);
    assert_eq!(foreign["body"]["breakpoints"][0]["verified"], false);

    client.call("configurationDone", json!({}));
    let stopped = client.wait_event("stopped");
    let frames = client.frames();

    assert_eq!(stopped["body"]["reason"], "breakpoint");
    assert_eq!(frames[0]["line"], 3);
    assert_eq!(frames[0]["source"]["name"], "loop.yopta");

    client.call("disconnect", json!({}));
}

#[test]
fn breakpoints_sent_before_launch_are_rejected_and_relaunch_works() {
    let mut client = Client::start();
    let program = fixture_path("loop.yopta");
    client.call("initialize", json!({ "adapterID": "yopta" }));

    let early_foreign = client.set_breakpoints(&fixture_path("blank_line.yopta"), &[3]);
    let early_own = client.set_breakpoints(&program, &[3]);

    assert_eq!(early_foreign["body"]["breakpoints"][0]["verified"], false);
    assert_eq!(early_own["body"]["breakpoints"][0]["verified"], false);

    client.call("launch", json!({ "program": program, "stopOnEntry": false }));
    let set = client.set_breakpoints(&program, &[3]);

    assert_eq!(set["body"]["breakpoints"][0]["verified"], true);

    client.call("configurationDone", json!({}));
    let stopped = client.wait_event("stopped");
    let frames = client.frames();

    assert_eq!(stopped["body"]["reason"], "breakpoint");
    assert_eq!(frames[0]["line"], 3);
    assert_eq!(frames[0]["source"]["name"], "loop.yopta");

    client.call("disconnect", json!({}));
}

#[test]
fn initialized_event_arrives_only_after_launch() {
    let mut client = Client::start();

    let seq = client.request("initialize", json!({ "adapterID": "yopta" }));
    let message = client.next_message();

    assert_eq!(message["type"], "response");
    assert_eq!(message["request_seq"], seq);

    let launch_seq = client.request("launch", json!({ "program": fixture_path("loop.yopta"), "stopOnEntry": false }));
    let response = client.next_message();
    let initialized = client.next_message();

    assert_eq!(response["type"], "response");
    assert_eq!(response["request_seq"], launch_seq);
    assert_eq!(initialized["type"], "event");
    assert_eq!(initialized["event"], "initialized");

    client.call("disconnect", json!({}));
}

#[test]
fn set_exception_breakpoints_and_loaded_sources_succeed() {
    let mut client = Client::start();
    client.call("initialize", json!({ "adapterID": "yopta" }));

    client.call("setExceptionBreakpoints", json!({ "filters": [] }));
    let sources = client.call("loadedSources", json!({}));

    assert_eq!(sources["body"]["sources"].as_array().map(Vec::len), Some(0));
}

#[test]
fn pause_while_stopped_does_not_stop_again_after_continue() {
    let mut client = Client::start();
    client.handshake("call.yopta", false, &[5]);
    let stopped = client.wait_event("stopped");
    assert_eq!(stopped["body"]["reason"], "breakpoint");

    client.call("pause", json!({ "threadId": 1 }));
    client.call("continue", json!({ "threadId": 1 }));

    assert_eq!(client.expect_exit_without_stop()["body"]["exitCode"], 0);
}

#[test]
fn pause_while_running_stops_with_reason_pause() {
    let mut client = Client::start();
    client.handshake("spin.yopta", false, &[]);

    client.call("pause", json!({ "threadId": 1 }));
    let stopped = client.wait_event("stopped");

    assert_eq!(stopped["body"]["reason"], "pause");
    client.call("disconnect", json!({}));
}

#[test]
fn syntax_error_reports_position_and_exit_code_one() {
    let mut client = Client::start();
    client.handshake("syntax_error.yopta", false, &[]);

    let output = client.wait_event("output");
    let exited = client.wait_event("exited");

    assert_eq!(output["body"]["category"], "stderr");
    let text = output["body"]["output"].as_str().unwrap_or_default();
    assert!(text.contains("syntax_error.yopta:2:"), "нет позиции в сообщении: {text}");
    assert_eq!(exited["body"]["exitCode"], 1);
}

fn assert_error(response: &Value, code: &str) {
    assert_eq!(response["success"], false, "{response}");
    assert_eq!(response["message"], code, "{response}");
    assert!(response["body"]["error"]["id"].is_i64(), "{response}");
    assert!(response["body"]["error"]["format"].as_str().is_some_and(|text| !text.is_empty()), "{response}");
}

#[test]
fn launch_of_an_unreadable_program_fails_with_no_program() {
    let mut client = Client::start();
    client.call("initialize", json!({ "adapterID": "yopta" }));
    let program = fixture_path("нет_такого.yopta");

    let response = client.answer("launch", json!({ "program": program }));

    assert_error(&response, "noProgram");
    assert!(response["body"]["error"]["format"].as_str().unwrap_or_default().contains(&program), "{response}");
    client.call("disconnect", json!({}));
}

#[test]
fn error_responses_carry_a_short_code_and_a_structured_message() {
    let mut client = Client::start();
    client.call("initialize", json!({ "adapterID": "yopta" }));

    let unknown = client.answer("рулетка", json!({}));
    let not_stopped = client.answer("stackTrace", json!({ "threadId": 1 }));
    let step = client.answer("next", json!({ "threadId": 1 }));
    let no_program = client.answer("launch", json!({}));

    assert_error(&unknown, "unsupported");
    assert!(unknown["body"]["error"]["format"].as_str().unwrap_or_default().contains("рулетка"));
    assert_error(&not_stopped, "notStopped");
    assert_error(&step, "notStopped");
    assert_error(&no_program, "noProgram");
}
