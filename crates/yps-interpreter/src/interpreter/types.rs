use crate::value::Value;

pub(super) enum ControlFlow {
    Break,
    Continue,
    Return(Value),
    Throw(Value),
}

pub(super) enum AccessSegment {
    Index(Value),
    Member(String),
}
