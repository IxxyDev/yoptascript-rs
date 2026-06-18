use std::rc::Rc;

use yps_lexer::Span;
use yps_parser::ast::{
    BinaryOp, Block, Expr, Identifier, Literal, ObjectEntry, Param, Pattern, PostfixOp, Program, PropKey, Stmt,
    TemplatePart, UnaryOp,
};

use crate::chunk::{Chunk, Constant, FnProto, Op, Slot, UpvalueDesc};
use crate::error::CompileError;
use crate::value::string_to_number;

struct Local {
    name: String,
    depth: i32,
    is_const: bool,
    is_captured: bool,
    initialized: bool,
}

struct LoopCtx {
    locals_count: usize,
    break_jumps: Vec<usize>,
    continue_jumps: Vec<usize>,
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum FnKind {
    Script,
    Function,
}

struct FnState {
    kind: FnKind,
    name: String,
    arity: usize,
    has_rest: bool,
    locals: Vec<Local>,
    upvalues: Vec<UpvalueDesc>,
    scope_depth: i32,
    chunk: Chunk,
    loops: Vec<LoopCtx>,
}

impl FnState {
    fn new(kind: FnKind, name: String) -> Self {
        let locals =
            vec![Local { name: String::new(), depth: 0, is_const: false, is_captured: false, initialized: true }];
        FnState {
            kind,
            name,
            arity: 0,
            has_rest: false,
            locals,
            upvalues: Vec::new(),
            scope_depth: 0,
            chunk: Chunk::new(),
            loops: Vec::new(),
        }
    }
}

enum VarLoc {
    Local(Slot, bool),
    Upvalue(Slot),
    Global(u32),
}

pub struct Compiler {
    funcs: Vec<FnState>,
}

pub fn compile_program(program: &Program) -> Result<Rc<FnProto>, CompileError> {
    let mut c = Compiler { funcs: vec![FnState::new(FnKind::Script, String::from("<скрипт>"))] };
    let span = Span { start: 0, end: 0 };
    c.compile_stmt_list(&program.items)?;
    c.emit(Op::Undefined, span);
    c.emit(Op::Return, span);
    let state = c.funcs.pop().expect("script frame");
    Ok(Rc::new(FnProto { name: state.name, arity: 0, has_rest: false, upvalues: state.upvalues, chunk: state.chunk }))
}

impl Compiler {
    fn cur(&mut self) -> &mut FnState {
        self.funcs.last_mut().expect("function frame")
    }

    fn emit(&mut self, op: Op, span: Span) -> usize {
        self.cur().chunk.push_op(op, span)
    }

    fn str_const(&mut self, s: &str) -> u32 {
        self.cur().chunk.add_constant(Constant::Str(Rc::from(s)))
    }

    fn begin_scope(&mut self) {
        self.cur().scope_depth += 1;
    }

    fn end_scope(&mut self, span: Span) {
        let depth = self.cur().scope_depth;
        while let Some(local) = self.cur().locals.last() {
            if local.depth < depth {
                break;
            }
            let captured = local.is_captured;
            self.cur().locals.pop();
            if captured {
                self.emit(Op::CloseUpvalue, span);
            } else {
                self.emit(Op::Pop, span);
            }
        }
        self.cur().scope_depth -= 1;
    }

    fn add_local(&mut self, name: &str, is_const: bool) {
        let depth = self.cur().scope_depth;
        self.cur().locals.push(Local {
            name: name.to_string(),
            depth,
            is_const,
            is_captured: false,
            initialized: true,
        });
    }

    fn reserve_local(&mut self, name: &str, is_const: bool) {
        let depth = self.cur().scope_depth;
        self.cur().locals.push(Local {
            name: name.to_string(),
            depth,
            is_const,
            is_captured: false,
            initialized: false,
        });
    }

    fn mark_initialized(&mut self, name: &str) {
        if let Some(local) = self.cur().locals.iter_mut().rev().find(|l| l.name == name) {
            local.initialized = true;
        }
    }

    fn is_global_scope(&mut self) -> bool {
        let f = self.cur();
        f.kind == FnKind::Script && f.scope_depth == 0
    }

    fn resolve_local(funcs: &[FnState], func_idx: usize, name: &str) -> Option<(Slot, bool)> {
        let locals = &funcs[func_idx].locals;
        for (i, local) in locals.iter().enumerate().rev() {
            if i == 0 {
                continue;
            }
            if local.name == name {
                return Some((i as Slot, local.is_const));
            }
        }
        None
    }

    fn add_upvalue(funcs: &mut [FnState], func_idx: usize, index: usize, from_parent_local: bool) -> Slot {
        let existing =
            funcs[func_idx].upvalues.iter().position(|u| u.index == index && u.from_parent_local == from_parent_local);
        if let Some(pos) = existing {
            return pos as Slot;
        }
        funcs[func_idx].upvalues.push(UpvalueDesc { from_parent_local, index });
        (funcs[func_idx].upvalues.len() - 1) as Slot
    }

    fn resolve_upvalue(funcs: &mut [FnState], func_idx: usize, name: &str) -> Option<Slot> {
        if func_idx == 0 {
            return None;
        }
        let enclosing = func_idx - 1;
        if let Some((slot, _)) = Self::resolve_local(funcs, enclosing, name) {
            funcs[enclosing].locals[slot as usize].is_captured = true;
            return Some(Self::add_upvalue(funcs, func_idx, slot as usize, true));
        }
        if let Some(up) = Self::resolve_upvalue(funcs, enclosing, name) {
            return Some(Self::add_upvalue(funcs, func_idx, up as usize, false));
        }
        None
    }

    fn resolve(&mut self, name: &str) -> VarLoc {
        let top = self.funcs.len() - 1;
        if let Some((slot, is_const)) = Self::resolve_local(&self.funcs, top, name) {
            return VarLoc::Local(slot, is_const);
        }
        if let Some(slot) = Self::resolve_upvalue(&mut self.funcs, top, name) {
            return VarLoc::Upvalue(slot);
        }
        VarLoc::Global(self.str_const(name))
    }

    fn compile_stmt_list(&mut self, stmts: &[Stmt]) -> Result<(), CompileError> {
        if self.is_global_scope() {
            return self.compile_global_list(stmts);
        }
        self.compile_local_list(stmts)
    }

    fn compile_global_list(&mut self, stmts: &[Stmt]) -> Result<(), CompileError> {
        for stmt in stmts {
            if let Stmt::FunctionDecl { name, params, body, is_generator, is_async, span } = stmt {
                if *is_generator || *is_async {
                    return Err(CompileError::new("генераторы и async-функции не поддерживаются VM", *span));
                }
                self.compile_function_decl(name, params, body, *span)?;
            }
        }
        for stmt in stmts {
            if matches!(stmt, Stmt::FunctionDecl { .. }) {
                continue;
            }
            self.compile_stmt(stmt)?;
        }
        Ok(())
    }

    fn compile_local_list(&mut self, stmts: &[Stmt]) -> Result<(), CompileError> {
        for stmt in stmts {
            match stmt {
                Stmt::VarDecl { pattern: Pattern::Identifier(id), is_const, span, init: _ } => {
                    self.emit(Op::Undefined, *span);
                    self.reserve_local(&id.name, *is_const);
                }
                Stmt::FunctionDecl { name, is_generator, is_async, span, .. } => {
                    if *is_generator || *is_async {
                        return Err(CompileError::new("генераторы и async-функции не поддерживаются VM", *span));
                    }
                    self.emit(Op::Undefined, *span);
                    self.reserve_local(&name.name, false);
                }
                _ => {}
            }
        }
        for stmt in stmts {
            if let Stmt::FunctionDecl { name, params, body, span, .. } = stmt {
                self.compile_function(&name.name, params, body, *span)?;
                self.store_reserved_local(&name.name, *span);
                self.mark_initialized(&name.name);
            }
        }
        for stmt in stmts {
            match stmt {
                Stmt::FunctionDecl { .. } => {}
                Stmt::VarDecl { pattern: Pattern::Identifier(id), init, span, .. } => {
                    self.compile_expr(init)?;
                    self.store_reserved_local(&id.name, *span);
                    self.mark_initialized(&id.name);
                }
                other => self.compile_stmt(other)?,
            }
        }
        Ok(())
    }

    fn store_reserved_local(&mut self, name: &str, span: Span) {
        let top = self.funcs.len() - 1;
        let slot = Self::resolve_local(&self.funcs, top, name).map(|(s, _)| s).expect("reserved local");
        self.emit(Op::SetLocal(slot), span);
        self.emit(Op::Pop, span);
    }

    fn compile_stmt(&mut self, stmt: &Stmt) -> Result<(), CompileError> {
        match stmt {
            Stmt::VarDecl { pattern, init, is_const, span } => self.compile_var_decl(pattern, init, *is_const, *span),
            Stmt::FunctionDecl { name, params, body, is_generator, is_async, span } => {
                if *is_generator || *is_async {
                    return Err(CompileError::new("генераторы и async-функции не поддерживаются VM", *span));
                }
                self.compile_function_decl(name, params, body, *span)
            }
            Stmt::Expr { expr, span } => {
                self.compile_expr(expr)?;
                self.emit(Op::Pop, *span);
                Ok(())
            }
            Stmt::Block(block) => {
                self.begin_scope();
                self.compile_stmt_list(&block.stmts)?;
                self.end_scope(block.span);
                Ok(())
            }
            Stmt::Empty { .. } => Ok(()),
            Stmt::If { condition, then_branch, else_branch, span } => {
                self.compile_expr(condition)?;
                let else_jump = self.emit(Op::JumpIfFalse(0), *span);
                self.compile_stmt(then_branch)?;
                if let Some(else_branch) = else_branch {
                    let end_jump = self.emit(Op::Jump(0), *span);
                    let here = self.cur().chunk.code.len();
                    self.cur().chunk.patch_jump(else_jump, here);
                    self.compile_stmt(else_branch)?;
                    let end = self.cur().chunk.code.len();
                    self.cur().chunk.patch_jump(end_jump, end);
                } else {
                    let here = self.cur().chunk.code.len();
                    self.cur().chunk.patch_jump(else_jump, here);
                }
                Ok(())
            }
            Stmt::While { condition, body, span } => self.compile_while(condition, body, *span),
            Stmt::For { init, condition, update, body, span } => {
                self.compile_for(init.as_deref(), condition.as_ref(), update.as_ref(), body, *span)
            }
            Stmt::DoWhile { body, condition, span } => self.compile_do_while(body, condition, *span),
            Stmt::Break { label, span } => {
                if label.is_some() {
                    return Err(CompileError::new("метки циклов не поддерживаются VM", *span));
                }
                self.compile_break(*span)
            }
            Stmt::Continue { label, span } => {
                if label.is_some() {
                    return Err(CompileError::new("метки циклов не поддерживаются VM", *span));
                }
                self.compile_continue(*span)
            }
            Stmt::Return { value, span } => {
                match value {
                    Some(v) => self.compile_expr(v)?,
                    None => {
                        self.emit(Op::Undefined, *span);
                    }
                }
                self.emit(Op::Return, *span);
                Ok(())
            }
            other => {
                Err(CompileError::new(format!("оператор не поддерживается VM: {}", stmt_kind(other)), other.span()))
            }
        }
    }

    fn compile_var_decl(
        &mut self,
        pattern: &Pattern,
        init: &Expr,
        is_const: bool,
        span: Span,
    ) -> Result<(), CompileError> {
        let name = match pattern {
            Pattern::Identifier(id) => id.name.clone(),
            _ => return Err(CompileError::new("деструктуризация не поддерживается VM", span)),
        };
        if self.is_global_scope() {
            self.compile_expr(init)?;
            let idx = self.str_const(&name);
            self.emit(Op::DefineGlobal(idx, is_const), span);
        } else {
            self.compile_expr(init)?;
            self.add_local(&name, is_const);
        }
        Ok(())
    }

    fn compile_function_decl(
        &mut self,
        name: &Identifier,
        params: &[Param],
        body: &Block,
        span: Span,
    ) -> Result<(), CompileError> {
        if self.is_global_scope() {
            self.compile_function(&name.name, params, body, span)?;
            let idx = self.str_const(&name.name);
            self.emit(Op::DefineGlobal(idx, false), span);
        } else {
            self.add_local(&name.name, false);
            self.compile_function(&name.name, params, body, span)?;
        }
        Ok(())
    }

    fn compile_function(&mut self, name: &str, params: &[Param], body: &Block, span: Span) -> Result<(), CompileError> {
        self.funcs.push(FnState::new(FnKind::Function, name.to_string()));
        for param in params {
            if param.pattern.is_some() {
                return Err(CompileError::new("деструктуризация параметров не поддерживается VM", span));
            }
            if param.is_rest {
                self.cur().has_rest = true;
            }
            self.cur().arity += 1;
            self.add_local(&param.name.name, false);
        }
        for (i, param) in params.iter().enumerate() {
            if let Some(default) = &param.default {
                let slot = (i + 1) as Slot;
                self.emit(Op::GetLocal(slot), span);
                self.emit(Op::Undefined, span);
                self.emit(Op::StrictEq, span);
                let skip = self.emit(Op::JumpIfFalse(0), span);
                self.compile_expr(default)?;
                self.emit(Op::SetLocal(slot), span);
                self.emit(Op::Pop, span);
                let here = self.cur().chunk.code.len();
                self.cur().chunk.patch_jump(skip, here);
            }
        }
        self.compile_stmt_list(&body.stmts)?;
        self.emit(Op::Undefined, span);
        self.emit(Op::Return, span);

        let state = self.funcs.pop().expect("function frame");
        let proto = Rc::new(FnProto {
            name: state.name,
            arity: state.arity,
            has_rest: state.has_rest,
            upvalues: state.upvalues,
            chunk: state.chunk,
        });
        let idx = self.cur().chunk.add_constant(Constant::Proto(proto));
        self.emit(Op::Closure(idx), span);
        Ok(())
    }

    fn compile_while(&mut self, condition: &Expr, body: &Stmt, span: Span) -> Result<(), CompileError> {
        let loop_start = self.cur().chunk.code.len();
        self.compile_expr(condition)?;
        let exit_jump = self.emit(Op::JumpIfFalse(0), span);
        let locals_count = self.cur().locals.len();
        self.cur().loops.push(LoopCtx { locals_count, break_jumps: Vec::new(), continue_jumps: Vec::new() });
        self.compile_stmt(body)?;
        self.emit(Op::Jump(loop_start), span);
        let exit = self.cur().chunk.code.len();
        self.cur().chunk.patch_jump(exit_jump, exit);
        self.finish_loop(exit, loop_start);
        Ok(())
    }

    fn compile_do_while(&mut self, body: &Stmt, condition: &Expr, span: Span) -> Result<(), CompileError> {
        let loop_start = self.cur().chunk.code.len();
        let locals_count = self.cur().locals.len();
        self.cur().loops.push(LoopCtx { locals_count, break_jumps: Vec::new(), continue_jumps: Vec::new() });
        self.compile_stmt(body)?;
        let continue_target = self.cur().chunk.code.len();
        self.compile_expr(condition)?;
        let exit_jump = self.emit(Op::JumpIfFalse(0), span);
        self.emit(Op::Jump(loop_start), span);
        let exit = self.cur().chunk.code.len();
        self.cur().chunk.patch_jump(exit_jump, exit);
        self.finish_loop(exit, continue_target);
        Ok(())
    }

    fn compile_for(
        &mut self,
        init: Option<&Stmt>,
        condition: Option<&Expr>,
        update: Option<&Expr>,
        body: &Stmt,
        span: Span,
    ) -> Result<(), CompileError> {
        self.begin_scope();
        if let Some(init) = init {
            self.compile_stmt(init)?;
        }
        let cond_start = self.cur().chunk.code.len();
        let exit_after_cond = if let Some(cond) = condition {
            self.compile_expr(cond)?;
            Some(self.emit(Op::JumpIfFalse(0), span))
        } else {
            None
        };
        let body_jump = self.emit(Op::Jump(0), span);
        let update_start = self.cur().chunk.code.len();
        if let Some(update) = update {
            self.compile_expr(update)?;
            self.emit(Op::Pop, span);
        }
        self.emit(Op::Jump(cond_start), span);
        let body_start = self.cur().chunk.code.len();
        self.cur().chunk.patch_jump(body_jump, body_start);

        let locals_count = self.cur().locals.len();
        self.cur().loops.push(LoopCtx { locals_count, break_jumps: Vec::new(), continue_jumps: Vec::new() });
        self.compile_stmt(body)?;
        self.emit(Op::Jump(update_start), span);
        let exit = self.cur().chunk.code.len();
        if let Some(exit_jump) = exit_after_cond {
            self.cur().chunk.patch_jump(exit_jump, exit);
        }
        self.finish_loop(exit, update_start);
        self.end_scope(span);
        Ok(())
    }

    fn finish_loop(&mut self, break_target: usize, continue_target: usize) {
        let ctx = self.cur().loops.pop().expect("loop context");
        for j in ctx.break_jumps {
            self.cur().chunk.patch_jump(j, break_target);
        }
        for j in ctx.continue_jumps {
            self.cur().chunk.patch_jump(j, continue_target);
        }
    }

    fn compile_break(&mut self, span: Span) -> Result<(), CompileError> {
        let target_count = match self.cur().loops.last() {
            Some(ctx) => ctx.locals_count,
            None => return Err(CompileError::new("'харэ' вне цикла", span)),
        };
        self.pop_locals_to(target_count, span);
        let jump = self.emit(Op::Jump(0), span);
        self.cur().loops.last_mut().unwrap().break_jumps.push(jump);
        Ok(())
    }

    fn compile_continue(&mut self, span: Span) -> Result<(), CompileError> {
        let target_count = match self.cur().loops.last() {
            Some(ctx) => ctx.locals_count,
            None => return Err(CompileError::new("'двигай' вне цикла", span)),
        };
        self.pop_locals_to(target_count, span);
        let jump = self.emit(Op::Jump(0), span);
        self.cur().loops.last_mut().unwrap().continue_jumps.push(jump);
        Ok(())
    }

    fn pop_locals_to(&mut self, target_count: usize, span: Span) {
        let mut i = self.cur().locals.len();
        while i > target_count {
            i -= 1;
            if self.cur().locals[i].is_captured {
                self.emit(Op::CloseUpvalue, span);
            } else {
                self.emit(Op::Pop, span);
            }
        }
    }

    fn compile_expr(&mut self, expr: &Expr) -> Result<(), CompileError> {
        match expr {
            Expr::Literal(lit) => self.compile_literal(lit),
            Expr::Identifier(id) => self.compile_var_get(&id.name, id.span),
            Expr::Grouping { expr, .. } => self.compile_expr(expr),
            Expr::Unary { op, expr, span } => self.compile_unary(*op, expr, *span),
            Expr::Binary { op, lhs, rhs, span } => self.compile_binary(*op, lhs, rhs, *span),
            Expr::Assignment { target, value, span } => {
                self.compile_expr(value)?;
                self.compile_var_set(&target.name, *span)?;
                Ok(())
            }
            Expr::Postfix { op, expr, span } => self.compile_postfix(*op, expr, *span),
            Expr::Conditional { condition, then_expr, else_expr, span } => {
                self.compile_expr(condition)?;
                let else_jump = self.emit(Op::JumpIfFalse(0), *span);
                self.compile_expr(then_expr)?;
                let end_jump = self.emit(Op::Jump(0), *span);
                let else_start = self.cur().chunk.code.len();
                self.cur().chunk.patch_jump(else_jump, else_start);
                self.compile_expr(else_expr)?;
                let end = self.cur().chunk.code.len();
                self.cur().chunk.patch_jump(end_jump, end);
                Ok(())
            }
            Expr::Call { callee, args, span } => self.compile_call(callee, args, *span),
            Expr::Index { object, index, span } => {
                self.compile_expr(object)?;
                self.compile_expr(index)?;
                self.emit(Op::GetIndex, *span);
                Ok(())
            }
            Expr::Member { object, property, span } => {
                self.compile_expr(object)?;
                let idx = self.str_const(&property.name);
                self.emit(Op::GetProp(idx), *span);
                Ok(())
            }
            Expr::ArrowFunction { params, body, is_async, span } => {
                if *is_async {
                    return Err(CompileError::new("async-стрелки не поддерживаются VM", *span));
                }
                self.compile_function("", params, body, *span)
            }
            Expr::FunctionExpr { name, params, body, is_async, span } => {
                if *is_async {
                    return Err(CompileError::new("async-функции не поддерживаются VM", *span));
                }
                let fname = name.as_ref().map(|n| n.name.as_str()).unwrap_or("");
                self.compile_function(fname, params, body, *span)
            }
            Expr::TemplateLiteral { parts, span } => self.compile_template(parts, *span),
            other => {
                Err(CompileError::new(format!("выражение не поддерживается VM: {}", expr_kind(other)), other.span()))
            }
        }
    }

    fn compile_literal(&mut self, lit: &Literal) -> Result<(), CompileError> {
        match lit {
            Literal::Number { raw, span } => {
                let n = parse_number_literal(raw);
                let idx = self.cur().chunk.add_constant(Constant::Number(n));
                self.emit(Op::Constant(idx), *span);
                Ok(())
            }
            Literal::String { value, span } => {
                let idx = self.str_const(value);
                self.emit(Op::Constant(idx), *span);
                Ok(())
            }
            Literal::Boolean { value, span } => {
                self.emit(if *value { Op::True } else { Op::False }, *span);
                Ok(())
            }
            Literal::Null { span } => {
                self.emit(Op::Null, *span);
                Ok(())
            }
            Literal::Undefined { span } => {
                self.emit(Op::Undefined, *span);
                Ok(())
            }
            Literal::Array { elements, span } => {
                for el in elements {
                    if matches!(el, Expr::Spread { .. }) {
                        return Err(CompileError::new("spread в массиве не поддерживается VM", *span));
                    }
                    self.compile_expr(el)?;
                }
                self.emit(Op::NewArray(elements.len() as u32), *span);
                Ok(())
            }
            Literal::Object { entries, span } => {
                for entry in entries {
                    match entry {
                        ObjectEntry::Property { key, value } => {
                            match key {
                                PropKey::Identifier(id) => {
                                    let idx = self.str_const(&id.name);
                                    self.emit(Op::Constant(idx), *span);
                                }
                                PropKey::Computed(expr) => {
                                    self.compile_expr(expr)?;
                                }
                            }
                            self.compile_expr(value)?;
                        }
                        _ => {
                            return Err(CompileError::new(
                                "геттеры/сеттеры/spread объекта не поддерживаются VM",
                                *span,
                            ));
                        }
                    }
                }
                self.emit(Op::NewObject(entries.len() as u32), *span);
                Ok(())
            }
            Literal::BigInt { span, .. } => Err(CompileError::new("BigInt не поддерживается VM", *span)),
            Literal::RegExp { span, .. } => Err(CompileError::new("регулярные выражения не поддерживаются VM", *span)),
        }
    }

    fn compile_template(&mut self, parts: &[TemplatePart], span: Span) -> Result<(), CompileError> {
        let empty = self.str_const("");
        self.emit(Op::Constant(empty), span);
        for part in parts {
            match part {
                TemplatePart::Str(s) => {
                    let idx = self.str_const(s);
                    self.emit(Op::Constant(idx), span);
                }
                TemplatePart::Expr(e) => self.compile_expr(e)?,
            }
            self.emit(Op::Add, span);
        }
        Ok(())
    }

    fn compile_unary(&mut self, op: UnaryOp, expr: &Expr, span: Span) -> Result<(), CompileError> {
        match op {
            UnaryOp::Void => {
                self.compile_expr(expr)?;
                self.emit(Op::Pop, span);
                self.emit(Op::Undefined, span);
                Ok(())
            }
            UnaryOp::Delete => Err(CompileError::new("'delete' не поддерживается VM", span)),
            _ => {
                self.compile_expr(expr)?;
                let vop = match op {
                    UnaryOp::Plus => Op::Pos,
                    UnaryOp::Minus => Op::Neg,
                    UnaryOp::Not => Op::Not,
                    UnaryOp::BitwiseNot => Op::BitNot,
                    UnaryOp::Typeof => Op::Typeof,
                    UnaryOp::Void | UnaryOp::Delete => unreachable!(),
                };
                self.emit(vop, span);
                Ok(())
            }
        }
    }

    fn compile_binary(&mut self, op: BinaryOp, lhs: &Expr, rhs: &Expr, span: Span) -> Result<(), CompileError> {
        match op {
            BinaryOp::Assign => return self.compile_assign(lhs, rhs, span),
            BinaryOp::PlusAssign
            | BinaryOp::MinusAssign
            | BinaryOp::MulAssign
            | BinaryOp::DivAssign
            | BinaryOp::ExpAssign
            | BinaryOp::ModAssign
            | BinaryOp::BitAndAssign
            | BinaryOp::BitOrAssign
            | BinaryOp::BitXorAssign
            | BinaryOp::ShlAssign
            | BinaryOp::ShrAssign
            | BinaryOp::UshrAssign => return self.compile_compound_assign(op, lhs, rhs, span),
            BinaryOp::And => {
                self.compile_expr(lhs)?;
                let jump = self.emit(Op::JumpIfFalsePeek(0), span);
                self.emit(Op::Pop, span);
                self.compile_expr(rhs)?;
                let end = self.cur().chunk.code.len();
                self.cur().chunk.patch_jump(jump, end);
                return Ok(());
            }
            BinaryOp::Or => {
                self.compile_expr(lhs)?;
                let jump = self.emit(Op::JumpIfTruePeek(0), span);
                self.emit(Op::Pop, span);
                self.compile_expr(rhs)?;
                let end = self.cur().chunk.code.len();
                self.cur().chunk.patch_jump(jump, end);
                return Ok(());
            }
            BinaryOp::NullishCoalescing => {
                self.compile_expr(lhs)?;
                let jump = self.emit(Op::JumpIfNullishPeek(0), span);
                let end_jump = self.emit(Op::Jump(0), span);
                let rhs_start = self.cur().chunk.code.len();
                self.cur().chunk.patch_jump(jump, rhs_start);
                self.emit(Op::Pop, span);
                self.compile_expr(rhs)?;
                let end = self.cur().chunk.code.len();
                self.cur().chunk.patch_jump(end_jump, end);
                return Ok(());
            }
            BinaryOp::Pipeline => {
                self.compile_expr(rhs)?;
                self.compile_expr(lhs)?;
                self.emit(Op::Call(1), span);
                return Ok(());
            }
            _ => {}
        }
        self.compile_expr(lhs)?;
        self.compile_expr(rhs)?;
        let vop = match op {
            BinaryOp::Add => Op::Add,
            BinaryOp::Sub => Op::Sub,
            BinaryOp::Mul => Op::Mul,
            BinaryOp::Div => Op::Div,
            BinaryOp::Mod => Op::Mod,
            BinaryOp::Exp => Op::Pow,
            BinaryOp::Equals => Op::Eq,
            BinaryOp::NotEquals => Op::Ne,
            BinaryOp::StrictEquals => Op::StrictEq,
            BinaryOp::StrictNotEquals => Op::StrictNe,
            BinaryOp::Less => Op::Lt,
            BinaryOp::Greater => Op::Gt,
            BinaryOp::LessOrEqual => Op::Le,
            BinaryOp::GreaterOrEqual => Op::Ge,
            BinaryOp::BitAnd => Op::BitAnd,
            BinaryOp::BitOr => Op::BitOr,
            BinaryOp::BitXor => Op::BitXor,
            BinaryOp::LeftShift => Op::Shl,
            BinaryOp::RightShift => Op::Shr,
            BinaryOp::UnsignedRightShift => Op::UShr,
            other => {
                return Err(CompileError::new(format!("бинарный оператор не поддерживается VM: {other:?}"), span));
            }
        };
        self.emit(vop, span);
        Ok(())
    }

    fn compile_assign(&mut self, lhs: &Expr, rhs: &Expr, span: Span) -> Result<(), CompileError> {
        match lhs {
            Expr::Identifier(id) => {
                self.compile_expr(rhs)?;
                self.compile_var_set(&id.name, span)?;
                Ok(())
            }
            Expr::Index { object, index, .. } => {
                self.compile_expr(object)?;
                self.compile_expr(index)?;
                self.compile_expr(rhs)?;
                self.emit(Op::SetIndex, span);
                Ok(())
            }
            Expr::Member { object, property, .. } => {
                self.compile_expr(object)?;
                self.compile_expr(rhs)?;
                let idx = self.str_const(&property.name);
                self.emit(Op::SetProp(idx), span);
                Ok(())
            }
            _ => Err(CompileError::new("недопустимая цель присваивания в VM", span)),
        }
    }

    fn compile_compound_assign(
        &mut self,
        op: BinaryOp,
        lhs: &Expr,
        rhs: &Expr,
        span: Span,
    ) -> Result<(), CompileError> {
        let base = compound_base_op(op);
        match lhs {
            Expr::Identifier(id) => {
                self.compile_var_get(&id.name, span)?;
                self.compile_expr(rhs)?;
                self.emit(base, span);
                self.compile_var_set(&id.name, span)?;
                Ok(())
            }
            _ => Err(CompileError::new("составное присваивание по индексу/полю не поддерживается VM", span)),
        }
    }

    fn compile_postfix(&mut self, op: PostfixOp, expr: &Expr, span: Span) -> Result<(), CompileError> {
        let Expr::Identifier(id) = expr else {
            return Err(CompileError::new("'++'/'--' поддерживаются VM только для переменных", span));
        };
        self.compile_var_get(&id.name, span)?;
        self.emit(Op::Pos, span);
        self.emit(Op::Dup, span);
        let one = self.cur().chunk.add_constant(Constant::Number(1.0));
        self.emit(Op::Constant(one), span);
        self.emit(if matches!(op, PostfixOp::Increment) { Op::Add } else { Op::Sub }, span);
        self.compile_var_set(&id.name, span)?;
        self.emit(Op::Pop, span);
        Ok(())
    }

    fn compile_call(&mut self, callee: &Expr, args: &[Expr], span: Span) -> Result<(), CompileError> {
        if args.iter().any(|a| matches!(a, Expr::Spread { .. })) {
            return Err(CompileError::new("spread в аргументах не поддерживается VM", span));
        }
        match callee {
            Expr::Member { object, property, .. } => {
                self.compile_expr(object)?;
                let idx = self.str_const(&property.name);
                self.emit(Op::GetProp(idx), span);
            }
            _ => self.compile_expr(callee)?,
        }
        for arg in args {
            self.compile_expr(arg)?;
        }
        self.emit(Op::Call(args.len() as u16), span);
        Ok(())
    }

    fn compile_var_get(&mut self, name: &str, span: Span) -> Result<(), CompileError> {
        match self.resolve(name) {
            VarLoc::Local(slot, _) => {
                if !self.cur().locals[slot as usize].initialized {
                    return Err(CompileError::new(format!("Переменная '{name}' не определена"), span));
                }
                self.emit(Op::GetLocal(slot), span);
            }
            VarLoc::Upvalue(slot) => {
                self.emit(Op::GetUpvalue(slot), span);
            }
            VarLoc::Global(idx) => {
                self.emit(Op::GetGlobal(idx), span);
            }
        }
        Ok(())
    }

    fn compile_var_set(&mut self, name: &str, span: Span) -> Result<(), CompileError> {
        match self.resolve(name) {
            VarLoc::Local(slot, is_const) => {
                if is_const {
                    return Err(CompileError::new(format!("нельзя менять константу '{name}'"), span));
                }
                self.emit(Op::SetLocal(slot), span);
            }
            VarLoc::Upvalue(slot) => {
                self.emit(Op::SetUpvalue(slot), span);
            }
            VarLoc::Global(idx) => {
                self.emit(Op::SetGlobal(idx), span);
            }
        }
        Ok(())
    }
}

fn compound_base_op(op: BinaryOp) -> Op {
    match op {
        BinaryOp::PlusAssign => Op::Add,
        BinaryOp::MinusAssign => Op::Sub,
        BinaryOp::MulAssign => Op::Mul,
        BinaryOp::DivAssign => Op::Div,
        BinaryOp::ExpAssign => Op::Pow,
        BinaryOp::ModAssign => Op::Mod,
        BinaryOp::BitAndAssign => Op::BitAnd,
        BinaryOp::BitOrAssign => Op::BitOr,
        BinaryOp::BitXorAssign => Op::BitXor,
        BinaryOp::ShlAssign => Op::Shl,
        BinaryOp::ShrAssign => Op::Shr,
        BinaryOp::UshrAssign => Op::UShr,
        _ => unreachable!("compound_base_op on non-compound op"),
    }
}

fn parse_number_literal(raw: &str) -> f64 {
    let cleaned = raw.replace('_', "");
    string_to_number(&cleaned)
}

fn stmt_kind(stmt: &Stmt) -> &'static str {
    match stmt {
        Stmt::TryCatch { .. } => "try/catch",
        Stmt::Throw { .. } => "throw",
        Stmt::Switch { .. } => "switch",
        Stmt::ForIn { .. } => "for-in",
        Stmt::ForOf { .. } => "for-of",
        Stmt::ForAwaitOf { .. } => "for-await-of",
        Stmt::ClassDecl { .. } => "class",
        Stmt::Labeled { .. } => "labeled",
        Stmt::Using { .. } => "using",
        Stmt::Import { .. } => "import",
        Stmt::Export { .. } => "export",
        Stmt::Debugger { .. } => "debugger",
        _ => "?",
    }
}

fn expr_kind(expr: &Expr) -> &'static str {
    match expr {
        Expr::New { .. } => "new",
        Expr::This { .. } => "this",
        Expr::Super { .. } => "super",
        Expr::Yield { .. } => "yield",
        Expr::Await { .. } => "await",
        Expr::Spread { .. } => "spread",
        Expr::TaggedTemplate { .. } => "tagged template",
        Expr::DynamicImport { .. } => "dynamic import",
        Expr::OptionalMember { .. } | Expr::OptionalIndex { .. } | Expr::OptionalCall { .. } => "optional chaining",
        _ => "?",
    }
}
