use crate::ast::{Expr, Identifier, Param, Pattern};
use yps_lexer::Span;

#[derive(Debug, Clone)]
pub enum ClassMember {
    Constructor { params: Vec<Param>, body: Block, span: Span },
    Method { name: Identifier, params: Vec<Param>, body: Block, is_static: bool, is_private: bool, span: Span },
    Field { name: Identifier, init: Option<Expr>, is_static: bool, is_private: bool, span: Span },
    Getter { name: Identifier, body: Block, is_static: bool, is_private: bool, span: Span },
    Setter { name: Identifier, param: Param, body: Block, is_static: bool, is_private: bool, span: Span },
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
        span: Span,
    },
    Continue {
        span: Span,
    },
    FunctionDecl {
        name: Identifier,
        params: Vec<Param>,
        body: Block,
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
    ClassDecl {
        name: Identifier,
        super_class: Option<Expr>,
        members: Vec<ClassMember>,
        span: Span,
    },
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
