use indexmap::IndexMap;
use yps_lexer::Span;

use crate::error::RuntimeError;
use crate::interpreter::Interpreter;
use crate::stdlib::require_args;
use crate::symbols;
use crate::value::Value;

pub fn construct(args: Vec<Value>, span: Span) -> Result<Value, RuntimeError> {
    if args.is_empty() {
        return Err(RuntimeError::new("'Косяк' ожидает минимум 1 аргумент (сообщение)", span));
    }
    let mut iter = args.into_iter();
    let message = iter.next().unwrap();
    let opts = iter.next();
    let mut map = IndexMap::new();
    map.insert(symbols::ERROR_NAME_FIELD.to_string(), Value::String(symbols::ERROR_NAME.to_string()));
    map.insert(symbols::ERROR_MESSAGE_FIELD.to_string(), Value::String(message.to_string()));
    if let Some(Value::Object(o)) = opts
        && let Some(cause) = o.borrow().get(symbols::ERROR_CAUSE_FIELD)
    {
        map.insert(symbols::ERROR_CAUSE_FIELD.to_string(), cause.clone());
    }
    Ok(Value::object(map))
}

pub fn is_error(args: &[Value]) -> bool {
    if let Some(Value::Object(map)) = args.first()
        && let Some(Value::String(name)) = map.borrow().get(symbols::ERROR_NAME_FIELD)
        && name == symbols::ERROR_NAME
    {
        return true;
    }
    false
}

pub fn call_static(
    _interp: &mut Interpreter,
    method: &str,
    args: Vec<Value>,
    span: Span,
) -> Result<Value, RuntimeError> {
    match method {
        "этоКосяк" | "isError" => {
            require_args(&args, 1, span, "Косяк.этоКосяк")?;
            Ok(Value::Boolean(is_error(&args)))
        }
        _ => Err(RuntimeError::new(format!("У 'Косяк' нет статического метода '{method}'"), span)),
    }
}
