use std::rc::Rc;

use yps_lexer::Span;

pub type ConstIdx = u32;
pub type Slot = u32;

#[derive(Debug, Clone)]
pub enum Constant {
    Number(f64),
    Str(Rc<str>),
    Proto(Rc<FnProto>),
}

#[derive(Debug, Clone, Copy)]
pub struct UpvalueDesc {
    pub from_parent_local: bool,
    pub index: usize,
}

#[derive(Debug)]
pub struct FnProto {
    pub name: String,
    pub arity: usize,
    pub has_rest: bool,
    pub upvalues: Vec<UpvalueDesc>,
    pub chunk: Chunk,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Constant(ConstIdx),
    Null,
    Undefined,
    True,
    False,
    Pop,
    Dup,

    Neg,
    Pos,
    Not,
    BitNot,
    Typeof,

    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
    UShr,

    Eq,
    Ne,
    StrictEq,
    StrictNe,
    Lt,
    Gt,
    Le,
    Ge,

    DefineGlobal(ConstIdx, bool),
    GetGlobal(ConstIdx),
    SetGlobal(ConstIdx),
    GetLocal(Slot),
    SetLocal(Slot),
    GetUpvalue(Slot),
    SetUpvalue(Slot),
    CloseUpvalue,

    Jump(usize),
    JumpIfFalse(usize),
    JumpIfFalsePeek(usize),
    JumpIfTruePeek(usize),
    JumpIfNullishPeek(usize),

    Call(u16),
    Closure(ConstIdx),
    Return,

    NewArray(u32),
    NewObject(u32),
    GetIndex,
    SetIndex,
    GetProp(ConstIdx),
    SetProp(ConstIdx),
}

#[derive(Debug, Default)]
pub struct Chunk {
    pub code: Vec<Op>,
    pub spans: Vec<Span>,
    pub constants: Vec<Constant>,
}

impl Chunk {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push_op(&mut self, op: Op, span: Span) -> usize {
        self.code.push(op);
        self.spans.push(span);
        self.code.len() - 1
    }

    pub fn add_constant(&mut self, value: Constant) -> ConstIdx {
        self.constants.push(value);
        (self.constants.len() - 1) as ConstIdx
    }

    pub fn patch_jump(&mut self, at: usize, target: usize) {
        match &mut self.code[at] {
            Op::Jump(t)
            | Op::JumpIfFalse(t)
            | Op::JumpIfFalsePeek(t)
            | Op::JumpIfTruePeek(t)
            | Op::JumpIfNullishPeek(t) => *t = target,
            other => panic!("patch_jump on non-jump op: {other:?}"),
        }
    }
}

pub fn disassemble(proto: &FnProto) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    let _ = writeln!(out, "== proto {} (arity {}) ==", proto.name, proto.arity);
    for (i, op) in proto.chunk.code.iter().enumerate() {
        let _ = writeln!(out, "{i:04} {op:?}");
    }
    for c in &proto.chunk.constants {
        if let Constant::Proto(p) = c {
            out.push('\n');
            out.push_str(&disassemble(p));
        }
    }
    out
}
