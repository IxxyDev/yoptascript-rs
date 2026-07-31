use std::io::{BufReader, PipeReader, PipeWriter};
use std::path::PathBuf;

use serde_json::{Value, json};

use yps_dap::protocol;

struct Client {
    to_server: PipeWriter,
    from_server: BufReader<PipeReader>,
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
        Self { to_server, from_server: BufReader::new(from_server), seq: 0 }
    }

    fn request(&mut self, command: &str, arguments: Value) -> i64 {
        self.seq += 1;
        let message = json!({ "seq": self.seq, "type": "request", "command": command, "arguments": arguments });
        protocol::write_message(&mut self.to_server, &message).expect("запрос должен уйти");
        self.seq
    }

    fn next_message(&mut self) -> Value {
        protocol::read_message(&mut self.from_server).expect("чтение").expect("поток не должен закрыться")
    }

    fn call(&mut self, command: &str, arguments: Value) -> Value {
        let seq = self.request(command, arguments);
        loop {
            let message = self.next_message();
            if message["type"] == "response" && message["request_seq"] == seq {
                assert_eq!(message["success"], true, "{command} должен успешно выполниться: {message}");
                return message;
            }
        }
    }

    fn wait_event(&mut self, name: &str) -> Value {
        loop {
            let message = self.next_message();
            if message["type"] == "event" && message["event"] == name {
                return message;
            }
        }
    }

    fn handshake(&mut self, fixture: &str, stop_on_entry: bool, breakpoint_lines: &[usize]) -> Value {
        self.call("initialize", json!({ "adapterID": "yopta" }));
        let program = fixture_path(fixture);
        self.call("launch", json!({ "program": program, "stopOnEntry": stop_on_entry }));
        let set = self.call(
            "setBreakpoints",
            json!({
                "source": { "path": program },
                "breakpoints": breakpoint_lines.iter().map(|l| json!({ "line": l })).collect::<Vec<_>>(),
            }),
        );
        self.call("configurationDone", json!({}));
        set
    }

    fn frames(&mut self) -> Vec<Value> {
        let response = self.call("stackTrace", json!({ "threadId": 1 }));
        response["body"]["stackFrames"].as_array().cloned().unwrap_or_default()
    }

    fn locals(&mut self, frame_id: i64) -> Vec<Value> {
        let scopes = self.call("scopes", json!({ "frameId": frame_id }));
        let reference = scopes["body"]["scopes"][0]["variablesReference"].as_i64().expect("ссылка на переменные");
        let response = self.call("variables", json!({ "variablesReference": reference }));
        response["body"]["variables"].as_array().cloned().unwrap_or_default()
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
    assert_eq!(stopped["body"]["reason"], "step");
    let frames = client.frames();
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
    assert_eq!(local_value(&variables, "a").as_deref(), Some("21"));
    let outer = client.locals(2);
    assert!(outer.is_empty(), "внешние кадры не хранят окружение");

    client.call("stepOut", json!({ "threadId": 1 }));
    client.wait_event("stopped");
    let frames = client.frames();
    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0]["line"], 6);
    let variables = client.locals(1);
    assert_eq!(local_value(&variables, "x").as_deref(), Some("42"));

    client.call("disconnect", json!({}));
}

#[test]
fn breakpoint_on_a_blank_line_snaps_to_the_next_statement() {
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
fn unknown_command_is_rejected() {
    let mut client = Client::start();
    client.call("initialize", json!({}));
    let seq = client.request("рулетка", json!({}));
    loop {
        let message = client.next_message();
        if message["type"] == "response" && message["request_seq"] == seq {
            assert_eq!(message["success"], false);
            break;
        }
    }
}
