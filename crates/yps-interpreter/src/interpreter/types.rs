use crate::value::Value;

pub(crate) enum ControlFlow {
    Break(Option<String>),
    Continue(Option<String>),
    Return(Value),
    Throw(Value),
}

pub(crate) enum LoopOp {
    Break,
    Continue,
    Exit(ControlFlow),
}

impl ControlFlow {
    pub(crate) fn for_loop(self, my_label: Option<&str>) -> LoopOp {
        match self {
            Self::Break(None) => LoopOp::Break,
            Self::Break(Some(ref l)) if my_label == Some(l.as_str()) => LoopOp::Break,
            Self::Continue(None) => LoopOp::Continue,
            Self::Continue(Some(ref l)) if my_label == Some(l.as_str()) => LoopOp::Continue,
            other => LoopOp::Exit(other),
        }
    }
}

pub(crate) enum AccessSegment {
    Index(Value),
    Member(String),
}
