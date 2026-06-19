use std::rc::Rc;

use yps_lexer::Span;

pub type ConstIdx = u32;
pub type Slot = u32;

#[derive(Debug, Clone)]
pub enum Constant {
    Number(f64),
    BigInt(i128),
    Str(Rc<str>),
    Proto(Rc<FnProto>),
    Class(Rc<ClassBlueprint>),
    Template(Rc<TemplateStrings>),
    RegExp { pattern: Rc<str>, flags: Rc<str> },
    Import(Rc<ImportRequest>),
}

#[derive(Debug, Clone)]
pub struct ImportRequest {
    pub source: String,
    pub is_json: bool,
    pub specifiers: Vec<ImportBinding>,
}

#[derive(Debug, Clone)]
pub enum ImportBinding {
    Default { local: String },
    Named { imported: String, local: String },
    Namespace { local: String },
}

#[derive(Debug, Clone)]
pub struct TemplateStrings {
    pub cooked: Vec<String>,
    pub raw: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemberKind {
    Method,
    Getter,
    Setter,
    StaticMethod,
    StaticGetter,
    StaticSetter,
    Field,
    StaticField,
}

#[derive(Debug, Clone)]
pub struct ClassMemberDesc {
    pub kind: MemberKind,
    pub name: String,
    pub has_value: bool,
    pub is_static: bool,
    pub is_private: bool,
    pub decorator_count: u32,
}

#[derive(Debug, Clone)]
pub struct ClassBlueprint {
    pub name: String,
    pub has_parent: bool,
    pub has_constructor: bool,
    pub members: Vec<ClassMemberDesc>,
    pub class_decorator_count: u32,
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
    pub is_method: bool,
    pub is_generator: bool,
    pub is_async: bool,
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
    Dup2,

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
    In,

    DefineGlobal(ConstIdx, bool),
    GetGlobal(ConstIdx),
    SetGlobal(ConstIdx),
    GetLocal(Slot),
    SetLocal(Slot),
    GetUpvalue(Slot),
    SetUpvalue(Slot),
    CloseUpvalue,

    MakeRegex(ConstIdx),

    Jump(usize),
    JumpIfFalse(usize),
    JumpIfFalsePeek(usize),
    JumpIfTruePeek(usize),
    JumpIfNullishPeek(usize),
    JumpIfNotNullishPeek(usize),

    Throw,
    PushHandler(usize, bool),
    PopHandler,

    ForInKeys,
    ForOfValues,
    ForIterInit,
    ForIterNext(usize),
    ForIterClose,
    ArrayLen,

    Call(u16),
    CallSpread,
    Closure(ConstIdx),
    Return,
    Yield,
    YieldDelegate,
    Await,
    DynamicImport,

    NewArray(u32),
    ArrPush,
    AppendSpread,
    ArrayRest(u32),
    ObjectRest(u32),
    NewObject(u32),
    ObjSet,
    SpreadObject,
    DefineGetter,
    DefineSetter,
    GetIndex,
    SetIndex,
    GetProp(ConstIdx),
    SetProp(ConstIdx),
    DeleteProp(ConstIdx),
    DeleteIndex,

    RegisterDisposable,
    DisposeScope(u32),

    Import(ConstIdx),
    RecordExport(ConstIdx),

    BuildClass(ConstIdx),
    New(u16),
    NewSpread,
    Invoke(ConstIdx, u16),
    Instanceof,
    SuperCall(u16),
    SuperCallSpread,
    SuperGet(ConstIdx),
    SuperInvoke(ConstIdx, u16),
    SuperInvokeSpread(ConstIdx),
    TaggedTemplate(ConstIdx),
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
            | Op::JumpIfNullishPeek(t)
            | Op::JumpIfNotNullishPeek(t)
            | Op::PushHandler(t, _)
            | Op::ForIterNext(t) => *t = target,
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
