use yps_interpreter::{Interpreter, Value, set_script_args};

fn process_arguments(interpreter: &Interpreter) -> Vec<String> {
    let Some(Value::Object(process)) = interpreter.get("Процесс") else {
        panic!("нет объекта Процесс")
    };
    let Some(Value::Array(items)) = process.borrow().get("аргументы").cloned() else {
        panic!("нет Процесс.аргументы")
    };
    items.borrow().iter().map(ToString::to_string).collect()
}

#[test]
fn script_args_replace_the_process_argv_for_every_interpreter() {
    set_script_args(vec!["прог.yopta".to_string(), "а".to_string(), "--флаг".to_string()]);

    assert_eq!(process_arguments(&Interpreter::new()), ["прог.yopta", "а", "--флаг"]);
    assert_eq!(process_arguments(&Interpreter::new()), ["прог.yopta", "а", "--флаг"]);
}
