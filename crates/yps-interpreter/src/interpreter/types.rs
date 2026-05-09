use crate::value::Value;

pub(crate) enum ControlFlow {
    Break,
    Continue,
    Return(Value),
    Throw(Value),
}

pub(crate) enum AccessSegment {
    Index(Value),
    Member(String),
}
