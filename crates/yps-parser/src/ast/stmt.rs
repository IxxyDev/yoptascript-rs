use std::rc::Rc;

use crate::ast::{Expr, Identifier, Param, Pattern};
use yps_lexer::Span;

#[derive(Debug, Clone)]
pub enum ClassMember {
    Constructor {
        params: Rc<[Param]>,
        body: Rc<Block>,
        span: Span,
    },
    Method {
        name: Identifier,
        params: Rc<[Param]>,
        body: Rc<Block>,
        is_static: bool,
        is_private: bool,
        decorators: Vec<Expr>,
        span: Span,
    },
    Field {
        name: Identifier,
        init: Option<Expr>,
        is_static: bool,
        is_private: bool,
        decorators: Vec<Expr>,
        span: Span,
    },
    Getter {
        name: Identifier,
        body: Rc<Block>,
        is_static: bool,
        is_private: bool,
        decorators: Vec<Expr>,
        span: Span,
    },
    Setter {
        name: Identifier,
        param: Param,
        body: Rc<Block>,
        is_static: bool,
        is_private: bool,
        decorators: Vec<Expr>,
        span: Span,
    },
}

#[derive(Debug, Clone)]
pub enum Stmt {
    VarDecl {
        pattern: Pattern,
        init: Expr,
        is_const: bool,
        span: Span,
    },
    Expr {
        expr: Expr,
        span: Span,
    },
    Block(Block),
    Empty {
        span: Span,
    },
    If {
        condition: Expr,
        then_branch: Box<Stmt>,
        else_branch: Option<Box<Stmt>>,
        span: Span,
    },
    While {
        condition: Expr,
        body: Box<Stmt>,
        span: Span,
    },
    For {
        init: Option<Box<Stmt>>,
        condition: Option<Expr>,
        update: Option<Expr>,
        body: Box<Stmt>,
        span: Span,
    },
    Break {
        label: Option<Identifier>,
        span: Span,
    },
    Continue {
        label: Option<Identifier>,
        span: Span,
    },
    Labeled {
        label: Identifier,
        body: Box<Stmt>,
        span: Span,
    },
    FunctionDecl {
        name: Identifier,
        params: Rc<[Param]>,
        body: Rc<Block>,
        is_generator: bool,
        is_async: bool,
        span: Span,
    },
    Return {
        value: Option<Expr>,
        span: Span,
    },
    TryCatch {
        try_block: Block,
        catch_param: Option<Identifier>,
        catch_block: Option<Block>,
        finally_block: Option<Block>,
        span: Span,
    },
    Throw {
        value: Expr,
        span: Span,
    },
    Switch {
        expr: Expr,
        cases: Vec<SwitchCase>,
        default: Option<Block>,
        span: Span,
    },
    DoWhile {
        body: Box<Stmt>,
        condition: Expr,
        span: Span,
    },
    ForIn {
        variable: Identifier,
        iterable: Expr,
        body: Box<Stmt>,
        span: Span,
    },
    ForOf {
        variable: Identifier,
        iterable: Expr,
        body: Box<Stmt>,
        span: Span,
    },
    ForAwaitOf {
        variable: Identifier,
        iterable: Expr,
        body: Box<Stmt>,
        span: Span,
    },
    ClassDecl {
        name: Identifier,
        super_class: Option<Expr>,
        members: Vec<ClassMember>,
        decorators: Vec<Expr>,
        span: Span,
    },
    Using {
        name: Identifier,
        init: Expr,
        is_await: bool,
        span: Span,
    },
    Debugger {
        span: Span,
    },
    Import {
        specifiers: Vec<ImportSpec>,
        source: String,
        attributes: Vec<(String, String)>,
        span: Span,
    },
    Export {
        kind: ExportKind,
        span: Span,
    },
}

impl Stmt {
    #[must_use]
    pub const fn span(&self) -> Span {
        match self {
            Self::Block(Block { span, .. })
            | Self::VarDecl { span, .. }
            | Self::Expr { span, .. }
            | Self::Empty { span }
            | Self::If { span, .. }
            | Self::While { span, .. }
            | Self::For { span, .. }
            | Self::Break { span, .. }
            | Self::Continue { span, .. }
            | Self::Labeled { span, .. }
            | Self::FunctionDecl { span, .. }
            | Self::Return { span, .. }
            | Self::TryCatch { span, .. }
            | Self::Throw { span, .. }
            | Self::Switch { span, .. }
            | Self::DoWhile { span, .. }
            | Self::ForIn { span, .. }
            | Self::ForOf { span, .. }
            | Self::ForAwaitOf { span, .. }
            | Self::ClassDecl { span, .. }
            | Self::Using { span, .. }
            | Self::Debugger { span }
            | Self::Import { span, .. }
            | Self::Export { span, .. } => *span,
        }
    }
}

#[derive(Debug, Clone)]
pub enum ImportSpec {
    Default { local: Identifier },
    Named { imported: Identifier, local: Identifier },
    Namespace { local: Identifier },
}

#[derive(Debug, Clone)]
pub enum ExportKind {
    Declaration(Box<Stmt>),
    Named(Vec<Identifier>),
}

#[derive(Debug, Clone)]
pub struct SwitchCase {
    pub value: Expr,
    pub body: Block,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub span: Span,
}
