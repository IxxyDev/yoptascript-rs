use std::io::{BufReader, PipeWriter};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

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

    fn next_or_end(&mut self) -> Option<Value> {
        match self.from_server.recv_timeout(MESSAGE_TIMEOUT) {
            Ok(message) => Some(message),
            Err(RecvTimeoutError::Disconnected) => None,
            Err(RecvTimeoutError::Timeout) => panic!("адаптер не ответил вовремя"),
        }
    }

    fn next_message(&mut self) -> Value {
        self.next_or_end().expect("поток не должен закрыться")
    }

    fn next_message_before(&mut self, deadline: Instant) -> Value {
        let left = deadline.saturating_duration_since(Instant::now());
        self.from_server.recv_timeout(left).expect("адаптер не ответил вовремя")
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
        self.handshake_with(json!({ "adapterID": "yopta" }), fixture, stop_on_entry, breakpoint_lines)
    }

    fn handshake_with(
        &mut self,
        initialize: Value,
        fixture: &str,
        stop_on_entry: bool,
        breakpoint_lines: &[usize],
    ) -> Value {
        self.call("initialize", initialize);
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

    client.wait_event("exited");
    client.wait_event("terminated");
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
fn stack_trace_honours_start_frame_and_levels() {
    let mut client = Client::start();
    client.handshake("call.yopta", false, &[2]);
    client.wait_event("stopped");

    let all = client.call("stackTrace", json!({ "threadId": 1 }));
    assert_eq!(all["body"]["stackFrames"].as_array().map(Vec::len), Some(2));
    assert_eq!(all["body"]["totalFrames"], 2);

    let first = client.call("stackTrace", json!({ "threadId": 1, "startFrame": 0, "levels": 1 }));
    assert_eq!(first["body"]["stackFrames"].as_array().map(Vec::len), Some(1));
    assert_eq!(first["body"]["stackFrames"][0]["id"], 1);
    assert_eq!(first["body"]["totalFrames"], 2);

    let second = client.call("stackTrace", json!({ "threadId": 1, "startFrame": 1, "levels": 20 }));
    assert_eq!(second["body"]["stackFrames"].as_array().map(Vec::len), Some(1));
    assert_eq!(second["body"]["stackFrames"][0]["id"], 2);
    assert_eq!(second["body"]["stackFrames"][0]["name"], "(модуль)");

    let past_end = client.call("stackTrace", json!({ "threadId": 1, "startFrame": 5 }));
    assert_eq!(past_end["body"]["stackFrames"].as_array().map(Vec::len), Some(0));
    assert_eq!(past_end["body"]["totalFrames"], 2);

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
    assert_eq!(message["body"]["supportsDelayedStackTraceLoading"], true);

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

#[test]
fn runtime_error_inside_a_module_names_the_module_file_and_line() {
    let mut client = Client::start();
    client.handshake("module_throw_main.yopta", false, &[]);

    let output = client.wait_event("output");
    let exited = client.wait_event("exited");

    assert_eq!(output["body"]["category"], "stderr");
    let text = output["body"]["output"].as_str().unwrap_or_default();
    assert!(text.contains("module_throw_lib.yopta:4:"), "ошибка должна указывать на модуль и строку: {text}");
    assert!(text.contains("module_throw_main.yopta:2:"), "кадр вызова должен указывать на главный файл: {text}");
    assert_eq!(exited["body"]["exitCode"], 1);
}

#[test]
fn malformed_json_frame_is_reported_and_the_session_keeps_answering() {
    use std::io::Write;

    let mut client = Client::start();
    client.call("initialize", json!({}));

    client.to_server.write_all(b"Content-Length: 5\r\n\r\n{nope").expect("кадр должен уйти");
    let report = client.wait_event("output");
    let threads = client.call("threads", json!({}));

    assert_eq!(report["body"]["category"], "console");
    let text = report["body"]["output"].as_str().unwrap_or_default();
    assert!(text.contains("Некорректное сообщение протокола"), "{text}");
    assert_eq!(threads["body"]["threads"].as_array().map(Vec::len), Some(1));
}

#[test]
fn unrecoverable_framing_error_ends_the_session() {
    use std::io::Write;

    let mut client = Client::start();
    client.call("initialize", json!({}));

    client.to_server.write_all(b"Content-Length: abc\r\n\r\n").expect("кадр должен уйти");
    let report = client.wait_event("output");

    let text = report["body"]["output"].as_str().unwrap_or_default();
    assert!(text.contains("некорректное значение Content-Length"), "{text}");
    assert!(client.next_or_end().is_none(), "сессия должна закрыться");
}

#[test]
fn breakpoint_ids_are_unique_across_requests() {
    let mut client = Client::start();
    let program = fixture_path("loop.yopta");
    client.call("initialize", json!({}));
    client.call("launch", json!({ "program": program, "stopOnEntry": false }));

    let first = client.set_breakpoints(&program, &[3, 5]);
    let second = client.set_breakpoints(&program, &[3, 99]);
    let foreign = client.set_breakpoints(&fixture_path("blank_line.yopta"), &[1]);

    let mut ids: Vec<i64> = [&first, &second, &foreign]
        .iter()
        .flat_map(|response| response["body"]["breakpoints"].as_array().cloned().unwrap_or_default())
        .filter_map(|breakpoint| breakpoint["id"].as_i64())
        .collect();
    let total = ids.len();
    ids.sort_unstable();
    ids.dedup();

    assert_eq!(total, 5);
    assert_eq!(ids.len(), 5, "идентификаторы должны быть уникальны: {ids:?}");
}

#[test]
fn exited_event_precedes_terminated() {
    let mut client = Client::start();
    client.handshake("print.yopta", false, &[]);

    let mut order = Vec::new();
    while order.len() < 2 {
        let message = client.next_message();
        if message["type"] == "event" && matches!(message["event"].as_str(), Some("exited" | "terminated")) {
            order.push(message["event"].as_str().unwrap_or_default().to_string());
        }
    }

    assert_eq!(order, ["exited", "terminated"]);
}

#[test]
fn terminate_stops_the_debuggee_but_keeps_the_adapter_alive_until_disconnect() {
    let mut client = Client::start();
    client.handshake("spin.yopta", false, &[]);

    let response = client.call("terminate", json!({}));
    let stack_seq = client.request("stackTrace", json!({ "threadId": 1 }));
    let threads_seq = client.request("threads", json!({}));
    let disconnect_seq = client.request("disconnect", json!({}));
    let mut events = Vec::new();
    let mut responses = std::collections::HashMap::new();
    while let Some(message) = client.next_or_end() {
        if message["type"] == "event" {
            events.push(message["event"].as_str().unwrap_or_default().to_string());
        } else if let Some(seq) = message["request_seq"].as_i64() {
            responses.insert(seq, message);
        }
    }

    assert_eq!(response["success"], true);
    assert_eq!(events.first().map(String::as_str), Some("terminated"), "{events:?}");
    assert!(
        events[1..].iter().all(|name| name == "exited" || name == "output"),
        "после terminate не должно быть остановки или повторного terminated: {events:?}"
    );
    assert!(events.iter().filter(|name| *name == "exited").count() <= 1, "{events:?}");
    assert_error(&responses[&stack_seq], "notStopped");
    assert_eq!(responses[&threads_seq]["body"]["threads"].as_array().map(Vec::len), Some(1));
    assert_eq!(responses[&disconnect_seq]["success"], true);
}

#[test]
fn terminate_answers_promptly_even_while_an_interval_keeps_the_program_alive() {
    let mut client = Client::start();
    client.handshake("ticker.yopta", false, &[]);
    client.wait_event("output");
    let terminate_seq = client.request("terminate", json!({}));
    let deadline = Instant::now() + Duration::from_secs(5);

    let mut terminate_answered = false;
    loop {
        let message = client.next_message_before(deadline);
        assert!(!(message["type"] == "event" && message["event"] == "stopped"), "{message}");
        if message["type"] == "response" && message["request_seq"] == terminate_seq {
            assert_eq!(message["success"], true, "{message}");
            terminate_answered = true;
        }
        if message["type"] == "event" && message["event"] == "terminated" {
            break;
        }
    }
    assert!(terminate_answered, "ответ на terminate должен прийти до terminated");

    let disconnect_seq = client.request("disconnect", json!({}));
    loop {
        let message = client.next_message_before(deadline);
        assert!(!(message["type"] == "event" && message["event"] == "terminated"), "terminated дважды: {message}");
        if message["type"] == "response" && message["request_seq"] == disconnect_seq {
            assert_eq!(message["success"], true, "{message}");
            break;
        }
    }
}

#[test]
fn configuration_done_after_terminate_does_not_start_the_program() {
    let mut client = Client::start();
    client.call("initialize", json!({ "adapterID": "yopta" }));
    client.call("launch", json!({ "program": fixture_path("loop.yopta"), "stopOnEntry": true }));
    client.call("terminate", json!({}));
    client.wait_event("terminated");

    let response = client.answer("configurationDone", json!({}));
    let seq = client.request("disconnect", json!({}));
    let mut events = Vec::new();
    while let Some(message) = client.next_or_end() {
        if message["type"] == "event" {
            events.push(message["event"].as_str().unwrap_or_default().to_string());
        }
        if message["type"] == "response" && message["request_seq"] == seq {
            break;
        }
    }

    assert_error(&response, "sessionEnded");
    assert!(events.is_empty(), "программа не должна была стартовать: {events:?}");
}

#[test]
fn messages_that_are_not_requests_are_ignored() {
    let mut client = Client::start();
    client.call("initialize", json!({ "adapterID": "yopta" }));

    let junk = [
        json!({ "seq": 90, "type": "response", "request_seq": 1, "command": "runInTerminal", "success": true }),
        json!({ "seq": 91, "type": "event", "event": "output" }),
        json!({ "seq": 92, "command": "threads" }),
        json!([1, 2, 3]),
        json!(42),
    ];
    for message in &junk {
        protocol::write_message(&mut client.to_server, message).expect("сообщение должно уйти");
    }
    let seq = client.request("threads", json!({}));
    let next = client.next_message();

    assert_eq!(next["type"], "response", "{next}");
    assert_eq!(next["request_seq"], seq, "на не-запросы не должно быть ответа: {next}");
    assert_eq!(next["success"], true, "{next}");
}

#[test]
fn breakpoint_on_the_last_main_line_does_not_fire_inside_module_code() {
    let mut client = Client::start();
    client.handshake("module_step_main.yopta", false, &[3]);

    let stopped = client.wait_event("stopped");
    let frames = client.frames();
    let variables = client.locals(1);

    assert_eq!(stopped["body"]["reason"], "breakpoint");
    assert_eq!(frames.len(), 1, "остановка должна быть в главном файле, а не в модуле: {frames:?}");
    assert_eq!(frames[0]["line"], 3);
    assert_eq!(local_value(&variables, "итог").as_deref(), Some("13"));
    client.call("disconnect", json!({}));
}

#[test]
fn stepping_into_a_module_function_reports_the_module_file_and_line() {
    let mut client = Client::start();
    client.handshake("module_step_main.yopta", true, &[]);
    client.wait_event("stopped");
    client.call("next", json!({ "threadId": 1 }));
    client.wait_event("stopped");

    assert_eq!(client.frames()[0]["line"], 2);

    client.call("stepIn", json!({ "threadId": 1 }));
    client.wait_event("stopped");
    let frames = client.frames();

    assert_eq!(frames.len(), 2, "{frames:?}");
    assert_eq!(frames[0]["name"], "посчитать");
    assert_eq!(frames[0]["source"]["name"], "module_step_lib.yopta", "{frames:?}");
    assert_eq!(frames[0]["line"], 2, "{frames:?}");
    assert_eq!(frames[0]["column"], 3, "{frames:?}");
    assert_eq!(frames[1]["source"]["name"], "module_step_main.yopta", "{frames:?}");
    assert_eq!(frames[1]["line"], 2, "{frames:?}");

    client.call("next", json!({ "threadId": 1 }));
    client.wait_event("stopped");

    assert_eq!(client.frames()[0]["line"], 3);
    client.call("disconnect", json!({}));
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

#[test]
fn step_requests_while_running_are_rejected_instead_of_queued() {
    let mut client = Client::start();
    client.handshake("spin.yopta", false, &[]);

    let response = client.answer("continue", json!({ "threadId": 1 }));
    client.call("pause", json!({ "threadId": 1 }));
    let stopped = client.wait_event("stopped");

    assert_error(&response, "notStopped");
    assert_eq!(stopped["body"]["reason"], "pause", "программа должна была продолжать работать");
    client.call("disconnect", json!({}));
}

#[test]
fn zero_based_client_lines_and_columns_are_converted() {
    let mut client = Client::start();
    let initialize = json!({ "adapterID": "yopta", "linesStartAt1": false, "columnsStartAt1": false });
    let set = client.handshake_with(initialize, "loop.yopta", false, &[2]);

    assert_eq!(set["body"]["breakpoints"][0]["verified"], true);
    assert_eq!(set["body"]["breakpoints"][0]["line"], 2);

    client.wait_event("stopped");
    let frames = client.frames();

    assert_eq!(frames[0]["line"], 2);
    assert_eq!(frames[0]["column"], 4);
    client.call("disconnect", json!({}));
}

#[test]
fn stopped_event_names_the_breakpoint_that_was_hit() {
    let mut client = Client::start();

    let set = client.handshake("loop.yopta", false, &[3]);
    let stopped = client.wait_event("stopped");

    assert_eq!(stopped["body"]["hitBreakpointIds"], json!([set["body"]["breakpoints"][0]["id"]]));
    client.call("disconnect", json!({}));
}

#[test]
fn unverified_breakpoints_explain_why_with_a_reason() {
    let mut client = Client::start();
    let program = fixture_path("loop.yopta");
    client.call("initialize", json!({ "adapterID": "yopta" }));

    let early = client.set_breakpoints(&program, &[3]);
    client.call("launch", json!({ "program": program }));
    let pathless =
        client.call("setBreakpoints", json!({ "source": { "sourceReference": 7 }, "breakpoints": [{ "line": 3 }] }));
    let past_end = client.set_breakpoints(&program, &[99]);

    assert_eq!(early["body"]["breakpoints"][0]["reason"], "pending");
    assert_eq!(pathless["body"]["breakpoints"][0]["verified"], false);
    assert_eq!(pathless["body"]["breakpoints"][0]["reason"], "failed");
    assert_eq!(past_end["body"]["breakpoints"][0]["reason"], "failed");
    client.call("disconnect", json!({}));
}

#[test]
fn variable_type_is_sent_only_to_clients_that_support_it() {
    let mut plain = Client::start();
    plain.handshake("loop.yopta", false, &[3]);
    plain.wait_event("stopped");
    let variables = plain.locals(1);

    assert!(!variables.is_empty());
    assert!(variables.iter().all(|variable| variable.get("type").is_none()), "{variables:?}");
    plain.call("disconnect", json!({}));

    let mut typed = Client::start();
    typed.handshake_with(json!({ "adapterID": "yopta", "supportsVariableType": true }), "loop.yopta", false, &[3]);
    typed.wait_event("stopped");
    let variables = typed.locals(1);

    assert!(variables.iter().all(|variable| variable["type"].is_string()), "{variables:?}");
    typed.call("disconnect", json!({}));
}

#[test]
fn variables_honour_filter_start_and_count() {
    let mut client = Client::start();
    client.handshake("loop.yopta", false, &[3]);
    client.wait_event("stopped");
    let reference = client.call("scopes", json!({ "frameId": 1 }))["body"]["scopes"][0]["variablesReference"].clone();

    let all = client.call("variables", json!({ "variablesReference": reference }))["body"]["variables"].clone();
    let indexed = client.call("variables", json!({ "variablesReference": reference, "filter": "indexed" }));
    let page = client.call("variables", json!({ "variablesReference": reference, "start": 1, "count": 1 }));
    let beyond = client.call("variables", json!({ "variablesReference": reference, "start": 999 }));

    assert!(all.as_array().map_or(0, Vec::len) >= 2, "{all}");
    assert_eq!(indexed["body"]["variables"], json!([]));
    assert_eq!(page["body"]["variables"], json!([all[1]]));
    assert_eq!(beyond["body"]["variables"], json!([]));
    client.call("disconnect", json!({}));
}
