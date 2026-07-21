use std::rc::Rc;

use yps_lexer::Span;
use yps_parser::ast::{
    BinaryOp, Block, ClassMember, ExportKind, Expr, Identifier, ImportSpec, Literal, ObjectEntry, Param, Pattern,
    PostfixOp, Program, PropKey, Stmt, TemplatePart, UnaryOp,
};

use crate::chunk::{
    Chunk, ClassBlueprint, ClassMemberDesc, Constant, FnProto, ImportBinding, ImportRequest, MemberKind, Op, Slot,
    TemplateStrings, UpvalueDesc,
};
use crate::error::CompileError;
use crate::value::string_to_number;

const THIS_LOCAL: &str = "\0this";

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
    label: Option<String>,
    is_loop: bool,
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum FnKind {
    Script,
    Function,
}

#[derive(Clone, Copy)]
struct CallableKind {
    binds_this: bool,
    is_method: bool,
    is_generator: bool,
    is_async: bool,
}

impl CallableKind {
    const FUNCTION: Self = Self { binds_this: true, is_method: false, is_generator: false, is_async: false };
    const GENERATOR: Self = Self { binds_this: true, is_method: false, is_generator: true, is_async: false };
    const ARROW: Self = Self { binds_this: false, is_method: false, is_generator: false, is_async: false };
    const METHOD: Self = Self { binds_this: true, is_method: true, is_generator: false, is_async: false };

    const fn with_async(mut self, is_async: bool) -> Self {
        self.is_async = is_async;
        self
    }
}

struct FnState {
    kind: FnKind,
    name: String,
    arity: usize,
    has_rest: bool,
    is_method: bool,
    is_generator: bool,
    is_async: bool,
    locals: Vec<Local>,
    upvalues: Vec<UpvalueDesc>,
    scope_depth: i32,
    chunk: Chunk,
    loops: Vec<LoopCtx>,
    try_ctxs: Vec<TryCtx>,
    pending_label: Option<String>,
    using_counts: Vec<u32>,
}

struct TryCtx {
    handler_count: usize,
    finally: Option<Block>,
    loops_len: usize,
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
            is_method: false,
            is_generator: false,
            is_async: false,
            locals,
            upvalues: Vec::new(),
            scope_depth: 0,
            chunk: Chunk::new(),
            loops: Vec::new(),
            try_ctxs: Vec::new(),
            pending_label: None,
            using_counts: vec![0],
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
    let global_using = c.cur().using_counts.last().copied().unwrap_or(0);
    if global_using > 0 {
        c.emit(Op::DisposeScope(global_using), span);
    }
    c.emit(Op::Undefined, span);
    c.emit(Op::Return, span);
    let state = c.funcs.pop().expect("script frame");
    Ok(Rc::new(FnProto {
        name: state.name,
        arity: 0,
        has_rest: false,
        is_method: false,
        is_generator: false,
        is_async: false,
        upvalues: state.upvalues,
        chunk: state.chunk,
    }))
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
        self.cur().using_counts.push(0);
    }

    fn end_scope(&mut self, span: Span) {
        let using_count = self.cur().using_counts.pop().unwrap_or(0);
        if using_count > 0 {
            self.emit(Op::DisposeScope(using_count), span);
        }
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
            if i == 0 && local.name != THIS_LOCAL {
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
                self.compile_function_decl(name, params, body, *is_generator, *is_async, *span)?;
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
                Stmt::FunctionDecl { name, span, .. } => {
                    self.emit(Op::Undefined, *span);
                    self.reserve_local(&name.name, false);
                }
                _ => {}
            }
        }
        for stmt in stmts {
            if let Stmt::FunctionDecl { name, params, body, is_generator, is_async, span } = stmt {
                self.compile_named_callable(&name.name, params, body, *is_generator, *is_async, *span)?;
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
                self.compile_function_decl(name, params, body, *is_generator, *is_async, *span)
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
                let name = label.as_ref().map(|l| l.name.clone());
                self.compile_break(name.as_deref(), *span)
            }
            Stmt::Continue { label, span } => {
                let name = label.as_ref().map(|l| l.name.clone());
                self.compile_continue(name.as_deref(), *span)
            }
            Stmt::Labeled { label, body, span } => self.compile_labeled(&label.name, body, *span),
            Stmt::Return { value, span } => {
                match value {
                    Some(v) => self.compile_expr(v)?,
                    None => {
                        self.emit(Op::Undefined, *span);
                    }
                }
                self.emit_try_exits_all(*span)?;
                self.emit(Op::Return, *span);
                Ok(())
            }
            Stmt::Throw { value, span } => {
                self.compile_expr(value)?;
                self.emit(Op::Throw, *span);
                Ok(())
            }
            Stmt::TryCatch { try_block, catch_param, catch_block, finally_block, span } => self.compile_try_catch(
                try_block,
                catch_param.as_ref(),
                catch_block.as_ref(),
                finally_block.as_ref(),
                *span,
            ),
            Stmt::Switch { expr, cases, default, span } => self.compile_switch(expr, cases, default.as_ref(), *span),
            Stmt::ForIn { variable, iterable, body, span } => self.compile_for_in(variable, iterable, body, *span),
            Stmt::ForOf { variable, iterable, body, span } => {
                self.compile_for_of(variable, iterable, body, false, *span)
            }
            Stmt::ForAwaitOf { variable, iterable, body, span } => {
                self.compile_for_of(variable, iterable, body, true, *span)
            }
            Stmt::Debugger { .. } => Ok(()),
            Stmt::ClassDecl { name, super_class, members, decorators, span } => {
                self.compile_class_decl(name, super_class.as_ref(), members, decorators, *span)
            }
            Stmt::Using { name, init, is_await, span } => self.compile_using(name, init, *is_await, *span),
            Stmt::Import { specifiers, source, attributes, span } => {
                self.compile_import(specifiers, source, attributes, *span)
            }
            Stmt::Export { kind, span } => self.compile_export(kind, *span),
        }
    }

    fn compile_try_catch(
        &mut self,
        try_block: &Block,
        catch_param: Option<&Identifier>,
        catch_block: Option<&Block>,
        finally_block: Option<&Block>,
        span: Span,
    ) -> Result<(), CompileError> {
        let has_finally = finally_block.is_some();
        let has_catch = catch_block.is_some();
        let handler_count = usize::from(has_finally) + usize::from(has_catch);
        let loops_len = self.cur().loops.len();
        self.cur().try_ctxs.push(TryCtx { handler_count, finally: finally_block.cloned(), loops_len });

        let outer_push = if has_finally { Some(self.emit(Op::PushHandler(0, true), span)) } else { None };
        let inner_push = if has_catch { Some(self.emit(Op::PushHandler(0, false), span)) } else { None };

        self.compile_block_scoped(try_block)?;
        if has_catch {
            self.emit(Op::PopHandler, span);
        }
        let try_done = self.emit(Op::Jump(0), span);

        if let Some(inner_push) = inner_push {
            let catch_start = self.cur().chunk.code.len();
            self.cur().chunk.patch_jump(inner_push, catch_start);
            self.begin_scope();
            if let Some(param) = catch_param {
                self.add_local(&param.name, false);
            } else {
                self.emit(Op::Pop, span);
            }
            self.compile_stmt_list(&catch_block.unwrap().stmts)?;
            self.end_scope(span);
        }
        let catch_done = self.emit(Op::Jump(0), span);

        let normal_target = self.cur().chunk.code.len();
        self.cur().chunk.patch_jump(try_done, normal_target);
        self.cur().chunk.patch_jump(catch_done, normal_target);

        self.cur().try_ctxs.pop();

        if let Some(outer_push) = outer_push {
            self.emit(Op::PopHandler, span);
            self.compile_block_scoped(finally_block.unwrap())?;
            let end_jump = self.emit(Op::Jump(0), span);

            let finally_throw = self.cur().chunk.code.len();
            self.cur().chunk.patch_jump(outer_push, finally_throw);
            self.compile_block_scoped(finally_block.unwrap())?;
            self.emit(Op::Throw, span);

            let end = self.cur().chunk.code.len();
            self.cur().chunk.patch_jump(end_jump, end);
        }
        Ok(())
    }

    fn compile_switch(
        &mut self,
        expr: &Expr,
        cases: &[yps_parser::ast::SwitchCase],
        default: Option<&Block>,
        span: Span,
    ) -> Result<(), CompileError> {
        self.begin_scope();
        self.compile_expr(expr)?;
        let disc = self.push_temp(span);

        let mut body_jumps: Vec<(usize, usize)> = Vec::new();
        for (i, case) in cases.iter().enumerate() {
            self.emit(Op::GetLocal(disc), span);
            self.compile_expr(&case.value)?;
            self.emit(Op::StrictEq, span);
            let skip = self.emit(Op::JumpIfFalse(0), span);
            let body_jump = self.emit(Op::Jump(0), span);
            body_jumps.push((i, body_jump));
            let next = self.cur().chunk.code.len();
            self.cur().chunk.patch_jump(skip, next);
        }

        let default_jump = self.emit(Op::Jump(0), span);
        let mut end_jumps: Vec<usize> = Vec::new();

        for (i, case) in cases.iter().enumerate() {
            let here = self.cur().chunk.code.len();
            for (ci, bj) in &body_jumps {
                if *ci == i {
                    self.cur().chunk.patch_jump(*bj, here);
                }
            }
            self.compile_block_scoped(&case.body)?;
            end_jumps.push(self.emit(Op::Jump(0), span));
        }

        let default_start = self.cur().chunk.code.len();
        self.cur().chunk.patch_jump(default_jump, default_start);
        if let Some(default_block) = default {
            self.compile_block_scoped(default_block)?;
        }

        let end = self.cur().chunk.code.len();
        for j in end_jumps {
            self.cur().chunk.patch_jump(j, end);
        }
        self.end_scope(span);
        Ok(())
    }

    fn compile_for_in(
        &mut self,
        variable: &Pattern,
        iterable: &Expr,
        body: &Stmt,
        span: Span,
    ) -> Result<(), CompileError> {
        let label = self.take_pending_label();
        self.begin_scope();
        self.compile_expr(iterable)?;
        self.emit(Op::ForInKeys, span);
        let items = self.push_temp(span);

        self.emit(Op::GetLocal(items), span);
        self.emit(Op::ArrayLen, span);
        let len = self.push_temp(span);

        let zero = self.cur().chunk.add_constant(Constant::Number(0.0));
        self.emit(Op::Constant(zero), span);
        let counter = self.push_temp(span);

        if let Pattern::Identifier(id) = variable {
            self.emit(Op::Undefined, span);
            self.add_local(&id.name, false);

            let cond_start = self.cur().chunk.code.len();
            self.emit(Op::GetLocal(counter), span);
            self.emit(Op::GetLocal(len), span);
            self.emit(Op::Lt, span);
            let exit_jump = self.emit(Op::JumpIfFalse(0), span);

            let var_slot = Self::resolve_local(&self.funcs, self.funcs.len() - 1, &id.name)
                .map(|(s, _)| s)
                .expect("loop var slot");
            self.emit(Op::GetLocal(items), span);
            self.emit(Op::GetLocal(counter), span);
            self.emit(Op::GetIndex, span);
            self.emit(Op::SetLocal(var_slot), span);
            self.emit(Op::Pop, span);

            let locals_count = self.cur().locals.len();
            self.push_loop(locals_count, label);
            self.compile_stmt(body)?;

            let continue_target = self.cur().chunk.code.len();
            if self.cur_ref().locals[var_slot as usize].is_captured {
                self.emit(Op::CloseUpvalueTo(var_slot), span);
            }
            self.emit(Op::GetLocal(counter), span);
            let one = self.cur().chunk.add_constant(Constant::Number(1.0));
            self.emit(Op::Constant(one), span);
            self.emit(Op::Add, span);
            self.emit(Op::SetLocal(counter), span);
            self.emit(Op::Pop, span);
            self.emit(Op::Jump(cond_start), span);

            let exit = self.cur().chunk.code.len();
            self.cur().chunk.patch_jump(exit_jump, exit);
            self.finish_loop(exit, continue_target);
            self.end_scope(span);
            return Ok(());
        }

        let cond_start = self.cur().chunk.code.len();
        self.emit(Op::GetLocal(counter), span);
        self.emit(Op::GetLocal(len), span);
        self.emit(Op::Lt, span);
        let exit_jump = self.emit(Op::JumpIfFalse(0), span);

        self.emit(Op::GetLocal(items), span);
        self.emit(Op::GetLocal(counter), span);
        self.emit(Op::GetIndex, span);

        self.begin_scope();
        let locals_count = self.cur().locals.len();
        self.push_loop(locals_count, label);
        self.destructure_pattern(variable, false, false, span)?;
        self.compile_stmt(body)?;
        self.end_scope(span);

        let continue_target = self.cur().chunk.code.len();
        self.emit(Op::GetLocal(counter), span);
        let one = self.cur().chunk.add_constant(Constant::Number(1.0));
        self.emit(Op::Constant(one), span);
        self.emit(Op::Add, span);
        self.emit(Op::SetLocal(counter), span);
        self.emit(Op::Pop, span);
        self.emit(Op::Jump(cond_start), span);

        let exit = self.cur().chunk.code.len();
        self.cur().chunk.patch_jump(exit_jump, exit);
        self.finish_loop(exit, continue_target);
        self.end_scope(span);
        Ok(())
    }

    fn compile_for_of(
        &mut self,
        variable: &Pattern,
        iterable: &Expr,
        body: &Stmt,
        is_await: bool,
        span: Span,
    ) -> Result<(), CompileError> {
        let label = self.take_pending_label();
        self.begin_scope();
        self.compile_expr(iterable)?;
        if is_await {
            self.emit(Op::Await, span);
        }
        self.emit(Op::ForIterInit, span);
        let handle = self.push_temp(span);

        if let Pattern::Identifier(id) = variable {
            self.emit(Op::Undefined, span);
            self.add_local(&id.name, false);
            let var_slot = Self::resolve_local(&self.funcs, self.funcs.len() - 1, &id.name)
                .map(|(s, _)| s)
                .expect("loop var slot");

            let loop_start = self.cur().chunk.code.len();
            self.emit(Op::GetLocal(handle), span);
            let next = self.emit(Op::ForIterNext(0), span);
            if is_await {
                self.emit(Op::Await, span);
            }
            self.emit(Op::SetLocal(var_slot), span);
            self.emit(Op::Pop, span);

            let locals_count = self.cur().locals.len();
            self.push_loop(locals_count, label);
            self.compile_stmt(body)?;

            let continue_target = self.cur().chunk.code.len();
            if self.cur_ref().locals[var_slot as usize].is_captured {
                self.emit(Op::CloseUpvalueTo(var_slot), span);
            }
            self.emit(Op::Jump(loop_start), span);

            let exit = self.cur().chunk.code.len();
            self.cur().chunk.patch_jump(next, exit);
            self.finish_loop(exit, continue_target);

            self.emit(Op::GetLocal(handle), span);
            self.emit(Op::ForIterClose, span);
            self.end_scope(span);
            return Ok(());
        }

        let loop_start = self.cur().chunk.code.len();
        self.emit(Op::GetLocal(handle), span);
        let next = self.emit(Op::ForIterNext(0), span);
        if is_await {
            self.emit(Op::Await, span);
        }

        self.begin_scope();
        let locals_count = self.cur().locals.len();
        self.push_loop(locals_count, label);
        self.destructure_pattern(variable, false, false, span)?;
        self.compile_stmt(body)?;
        self.end_scope(span);

        let continue_target = self.cur().chunk.code.len();
        self.emit(Op::Jump(loop_start), span);

        let exit = self.cur().chunk.code.len();
        self.cur().chunk.patch_jump(next, exit);
        self.finish_loop(exit, continue_target);

        self.emit(Op::GetLocal(handle), span);
        self.emit(Op::ForIterClose, span);
        self.end_scope(span);
        Ok(())
    }

    fn compile_block_scoped(&mut self, block: &Block) -> Result<(), CompileError> {
        self.begin_scope();
        self.compile_stmt_list(&block.stmts)?;
        self.end_scope(block.span);
        Ok(())
    }

    fn compile_var_decl(
        &mut self,
        pattern: &Pattern,
        init: &Expr,
        is_const: bool,
        span: Span,
    ) -> Result<(), CompileError> {
        if let Pattern::Identifier(id) = pattern {
            let name = id.name.clone();
            if self.is_global_scope() {
                self.compile_expr(init)?;
                let idx = self.str_const(&name);
                self.emit(Op::DefineGlobal(idx, is_const), span);
            } else {
                self.compile_expr(init)?;
                self.add_local(&name, is_const);
            }
            return Ok(());
        }
        self.compile_expr(init)?;
        let global = self.is_global_scope();
        self.destructure_pattern(pattern, is_const, global, span)
    }

    fn compile_using(
        &mut self,
        name: &Identifier,
        init: &Expr,
        is_await: bool,
        span: Span,
    ) -> Result<(), CompileError> {
        if is_await {
            return Err(CompileError::new("'юзай сидетьНахуй' пока не поддерживается в этом движке", span));
        }
        self.compile_expr(init)?;
        self.emit(Op::RegisterDisposable, span);
        if self.is_global_scope() {
            let idx = self.str_const(&name.name);
            self.emit(Op::DefineGlobal(idx, true), span);
        } else {
            self.add_local(&name.name, true);
        }
        if let Some(count) = self.cur().using_counts.last_mut() {
            *count += 1;
        }
        Ok(())
    }

    fn compile_import(
        &mut self,
        specifiers: &[ImportSpec],
        source: &str,
        attributes: &[(String, String)],
        span: Span,
    ) -> Result<(), CompileError> {
        if !self.is_global_scope() {
            return Err(CompileError::new("'спиздить' допустим только на верхнем уровне модуля", span));
        }
        let is_json = attributes.iter().any(|(k, v)| k == "type" && v == "json");
        let bindings: Vec<ImportBinding> = specifiers
            .iter()
            .map(|spec| match spec {
                ImportSpec::Default { local } => ImportBinding::Default { local: local.name.clone() },
                ImportSpec::Named { imported, local } => {
                    ImportBinding::Named { imported: imported.name.clone(), local: local.name.clone() }
                }
                ImportSpec::Namespace { local } => ImportBinding::Namespace { local: local.name.clone() },
            })
            .collect();
        let request = ImportRequest { source: source.to_string(), is_json, specifiers: bindings };
        let idx = self.cur().chunk.add_constant(Constant::Import(Rc::new(request)));
        self.emit(Op::Import(idx), span);
        Ok(())
    }

    fn compile_export(&mut self, kind: &ExportKind, span: Span) -> Result<(), CompileError> {
        if !self.is_global_scope() {
            return Err(CompileError::new("'предъява' допустим только на верхнем уровне модуля", span));
        }
        match kind {
            ExportKind::Declaration(decl) => {
                let names = decl_names(decl);
                self.compile_stmt(decl)?;
                for name in names {
                    let idx = self.str_const(&name);
                    self.emit(Op::RecordExport(idx), span);
                }
                Ok(())
            }
            ExportKind::Named(idents) => {
                for ident in idents {
                    let idx = self.str_const(&ident.name);
                    self.emit(Op::RecordExport(idx), span);
                }
                Ok(())
            }
        }
    }

    fn destructure_pattern(
        &mut self,
        pattern: &Pattern,
        is_const: bool,
        global: bool,
        span: Span,
    ) -> Result<(), CompileError> {
        match pattern {
            Pattern::Identifier(id) => {
                self.bind_destructured(&id.name, is_const, global, span);
                Ok(())
            }
            Pattern::Default { pattern: inner, default, .. } => {
                self.emit(Op::Dup, span);
                self.emit(Op::Undefined, span);
                self.emit(Op::StrictEq, span);
                let skip = self.emit(Op::JumpIfFalse(0), span);
                self.emit(Op::Pop, span);
                self.compile_expr(default)?;
                let here = self.cur().chunk.code.len();
                self.cur().chunk.patch_jump(skip, here);
                self.destructure_pattern(inner, is_const, global, span)
            }
            Pattern::Array { elements, rest, .. } => {
                self.emit(Op::NormalizeIterable, span);
                let temp = self.push_temp(span);
                for (i, elem) in elements.iter().enumerate() {
                    if let Some(pat) = elem {
                        self.emit(Op::GetLocal(temp), span);
                        let idx = self.cur().chunk.add_constant(Constant::Number(i as f64));
                        self.emit(Op::Constant(idx), span);
                        self.emit(Op::GetIndex, span);
                        self.destructure_pattern(pat, is_const, global, span)?;
                    }
                }
                if let Some(rest_pat) = rest {
                    self.emit(Op::GetLocal(temp), span);
                    self.emit(Op::ArrayRest(elements.len() as u32), span);
                    self.destructure_pattern(rest_pat, is_const, global, span)?;
                }
                Ok(())
            }
            Pattern::Object { properties, rest, .. } => {
                let temp = self.push_temp(span);
                for prop in properties {
                    self.emit(Op::GetLocal(temp), span);
                    let kidx = self.str_const(&prop.key.name);
                    self.emit(Op::GetProp(kidx), span);
                    match &prop.value {
                        Some(value_pat) => self.destructure_pattern(value_pat, is_const, global, span)?,
                        None => self.bind_destructured(&prop.key.name, is_const, global, span),
                    }
                }
                if let Some(rest_pat) = rest {
                    self.emit(Op::GetLocal(temp), span);
                    for prop in properties {
                        let kidx = self.str_const(&prop.key.name);
                        self.emit(Op::Constant(kidx), span);
                    }
                    self.emit(Op::ObjectRest(properties.len() as u32), span);
                    self.destructure_pattern(rest_pat, is_const, global, span)?;
                }
                Ok(())
            }
        }
    }

    fn push_temp(&mut self, _span: Span) -> Slot {
        let f = self.cur();
        let slot = f.locals.len() as Slot;
        let depth = f.scope_depth;
        f.locals.push(Local {
            name: String::from("\0destr"),
            depth,
            is_const: false,
            is_captured: false,
            initialized: true,
        });
        slot
    }

    fn bind_destructured(&mut self, name: &str, is_const: bool, global: bool, span: Span) {
        if global {
            let idx = self.str_const(name);
            self.emit(Op::DefineGlobal(idx, is_const), span);
        } else {
            self.add_local(name, is_const);
        }
    }

    fn compile_function_decl(
        &mut self,
        name: &Identifier,
        params: &[Param],
        body: &Block,
        is_generator: bool,
        is_async: bool,
        span: Span,
    ) -> Result<(), CompileError> {
        if self.is_global_scope() {
            self.compile_named_callable(&name.name, params, body, is_generator, is_async, span)?;
            let idx = self.str_const(&name.name);
            self.emit(Op::DefineGlobal(idx, false), span);
        } else {
            self.add_local(&name.name, false);
            self.compile_named_callable(&name.name, params, body, is_generator, is_async, span)?;
        }
        Ok(())
    }

    fn compile_named_callable(
        &mut self,
        name: &str,
        params: &[Param],
        body: &Block,
        is_generator: bool,
        is_async: bool,
        span: Span,
    ) -> Result<(), CompileError> {
        if is_generator {
            self.compile_generator(name, params, body, span)
        } else {
            self.compile_function(name, params, body, is_async, span)
        }
    }

    fn compile_function(
        &mut self,
        name: &str,
        params: &[Param],
        body: &Block,
        is_async: bool,
        span: Span,
    ) -> Result<(), CompileError> {
        self.compile_callable(name, params, body, span, CallableKind::FUNCTION.with_async(is_async))
    }

    fn compile_generator(
        &mut self,
        name: &str,
        params: &[Param],
        body: &Block,
        span: Span,
    ) -> Result<(), CompileError> {
        self.compile_callable(name, params, body, span, CallableKind::GENERATOR)
    }

    fn compile_arrow(
        &mut self,
        params: &[Param],
        body: &Block,
        is_async: bool,
        span: Span,
    ) -> Result<(), CompileError> {
        self.compile_callable("", params, body, span, CallableKind::ARROW.with_async(is_async))
    }

    fn compile_method(
        &mut self,
        name: &str,
        params: &[Param],
        body: &Block,
        is_async: bool,
        span: Span,
    ) -> Result<(), CompileError> {
        self.compile_callable(name, params, body, span, CallableKind::METHOD.with_async(is_async))
    }

    fn compile_callable(
        &mut self,
        name: &str,
        params: &[Param],
        body: &Block,
        span: Span,
        kind: CallableKind,
    ) -> Result<(), CompileError> {
        self.funcs.push(FnState::new(FnKind::Function, name.to_string()));
        self.cur().is_method = kind.is_method;
        self.cur().is_generator = kind.is_generator;
        self.cur().is_async = kind.is_async;
        if kind.binds_this {
            self.cur().locals[0].name = THIS_LOCAL.to_string();
        }
        for param in params {
            if param.is_rest {
                self.cur().has_rest = true;
            }
            self.cur().arity += 1;
            let slot_name = if param.pattern.is_some() { "\0param" } else { param.name.name.as_str() };
            self.add_local(slot_name, false);
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
        for (i, param) in params.iter().enumerate() {
            if let Some(pat) = &param.pattern {
                let slot = (i + 1) as Slot;
                self.emit(Op::GetLocal(slot), span);
                self.destructure_pattern(pat, false, false, span)?;
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
            is_method: state.is_method,
            is_generator: state.is_generator,
            is_async: state.is_async,
            upvalues: state.upvalues,
            chunk: state.chunk,
        });
        let idx = self.cur().chunk.add_constant(Constant::Proto(proto));
        self.emit(Op::Closure(idx), span);
        Ok(())
    }

    fn compile_class_decl(
        &mut self,
        name: &Identifier,
        super_class: Option<&Expr>,
        members: &[ClassMember],
        decorators: &[Expr],
        span: Span,
    ) -> Result<(), CompileError> {
        self.compile_class_expr(name, super_class, members, decorators, span)?;
        if self.is_global_scope() {
            let idx = self.str_const(&name.name);
            self.emit(Op::DefineGlobal(idx, false), span);
        } else {
            self.add_local(&name.name, false);
        }
        Ok(())
    }

    fn compile_class_expr(
        &mut self,
        name: &Identifier,
        super_class: Option<&Expr>,
        members: &[ClassMember],
        decorators: &[Expr],
        span: Span,
    ) -> Result<(), CompileError> {
        let has_parent = super_class.is_some();
        if let Some(sc) = super_class {
            self.compile_expr(sc)?;
        }

        let mut has_constructor = false;
        for m in members {
            if let ClassMember::Constructor { params, body, span: cspan } = m {
                self.compile_method("конструктор", params, body, false, *cspan)?;
                has_constructor = true;
            }
        }

        let mut descs: Vec<ClassMemberDesc> = Vec::new();
        for m in members {
            match m {
                ClassMember::Constructor { .. } => {}
                ClassMember::Method { name: mn, params, body, is_static, is_private, decorators, span: mspan } => {
                    self.compile_method(&mn.name, params, body, false, *mspan)?;
                    descs.push(ClassMemberDesc {
                        kind: if *is_static { MemberKind::StaticMethod } else { MemberKind::Method },
                        name: mn.name.clone(),
                        has_value: true,
                        is_static: *is_static,
                        is_private: *is_private,
                        decorator_count: decorators.len() as u32,
                    });
                }
                ClassMember::Getter { name: gn, body, is_static, is_private, decorators, span: gspan } => {
                    self.compile_method(&gn.name, &[], body, false, *gspan)?;
                    descs.push(ClassMemberDesc {
                        kind: if *is_static { MemberKind::StaticGetter } else { MemberKind::Getter },
                        name: gn.name.clone(),
                        has_value: true,
                        is_static: *is_static,
                        is_private: *is_private,
                        decorator_count: decorators.len() as u32,
                    });
                }
                ClassMember::Setter { name: sn, param, body, is_static, is_private, decorators, span: sspan } => {
                    self.compile_method(&sn.name, std::slice::from_ref(param), body, false, *sspan)?;
                    descs.push(ClassMemberDesc {
                        kind: if *is_static { MemberKind::StaticSetter } else { MemberKind::Setter },
                        name: sn.name.clone(),
                        has_value: true,
                        is_static: *is_static,
                        is_private: *is_private,
                        decorator_count: decorators.len() as u32,
                    });
                }
                ClassMember::Field { name: fn_, init, is_static, is_private, decorators, span: fspan } => {
                    let has_value = init.is_some();
                    if let Some(init_expr) = init {
                        let body = Block {
                            stmts: vec![Stmt::Return { value: Some(init_expr.clone()), span: *fspan }],
                            span: *fspan,
                        };
                        self.compile_method(&fn_.name, &[], &body, false, *fspan)?;
                    }
                    descs.push(ClassMemberDesc {
                        kind: if *is_static { MemberKind::StaticField } else { MemberKind::Field },
                        name: fn_.name.clone(),
                        has_value,
                        is_static: *is_static,
                        is_private: *is_private,
                        decorator_count: decorators.len() as u32,
                    });
                }
            }
        }

        for m in members {
            let decs = match m {
                ClassMember::Method { decorators, .. }
                | ClassMember::Getter { decorators, .. }
                | ClassMember::Setter { decorators, .. }
                | ClassMember::Field { decorators, .. } => decorators.as_slice(),
                ClassMember::Constructor { .. } => continue,
            };
            for dec in decs {
                self.compile_expr(dec)?;
            }
        }
        for dec in decorators {
            self.compile_expr(dec)?;
        }

        let blueprint = ClassBlueprint {
            name: name.name.clone(),
            has_parent,
            has_constructor,
            members: descs,
            class_decorator_count: decorators.len() as u32,
        };
        let idx = self.cur().chunk.add_constant(Constant::Class(Rc::new(blueprint)));
        self.emit(Op::BuildClass(idx), span);
        Ok(())
    }

    fn compile_labeled(&mut self, label: &str, body: &Stmt, span: Span) -> Result<(), CompileError> {
        if is_loop_stmt(body) {
            self.cur().pending_label = Some(label.to_string());
            return self.compile_stmt(body);
        }
        let locals_count = self.cur().locals.len();
        self.cur().loops.push(LoopCtx {
            locals_count,
            break_jumps: Vec::new(),
            continue_jumps: Vec::new(),
            label: Some(label.to_string()),
            is_loop: false,
        });
        self.compile_stmt(body)?;
        let ctx = self.cur().loops.pop().expect("labeled context");
        let here = self.cur().chunk.code.len();
        for j in ctx.break_jumps {
            self.cur().chunk.patch_jump(j, here);
        }
        let _ = span;
        Ok(())
    }

    fn take_pending_label(&mut self) -> Option<String> {
        self.cur().pending_label.take()
    }

    fn push_loop(&mut self, locals_count: usize, label: Option<String>) {
        self.cur().loops.push(LoopCtx {
            locals_count,
            break_jumps: Vec::new(),
            continue_jumps: Vec::new(),
            label,
            is_loop: true,
        });
    }

    fn compile_while(&mut self, condition: &Expr, body: &Stmt, span: Span) -> Result<(), CompileError> {
        let label = self.take_pending_label();
        let loop_start = self.cur().chunk.code.len();
        self.compile_expr(condition)?;
        let exit_jump = self.emit(Op::JumpIfFalse(0), span);
        let locals_count = self.cur().locals.len();
        self.push_loop(locals_count, label);
        self.compile_stmt(body)?;
        self.emit(Op::Jump(loop_start), span);
        let exit = self.cur().chunk.code.len();
        self.cur().chunk.patch_jump(exit_jump, exit);
        self.finish_loop(exit, loop_start);
        Ok(())
    }

    fn compile_do_while(&mut self, body: &Stmt, condition: &Expr, span: Span) -> Result<(), CompileError> {
        let label = self.take_pending_label();
        let loop_start = self.cur().chunk.code.len();
        let locals_count = self.cur().locals.len();
        self.push_loop(locals_count, label);
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
        let label = self.take_pending_label();
        self.begin_scope();
        let init_local_start = self.cur_ref().locals.len();
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
        self.push_loop(locals_count, label);
        self.compile_stmt(body)?;
        let locals_len = self.cur_ref().locals.len();
        let captures_loop_var = (init_local_start..locals_len).any(|i| self.cur_ref().locals[i].is_captured);
        let continue_target = if captures_loop_var {
            let target = self.cur().chunk.code.len();
            self.emit(Op::CloseUpvalueTo(init_local_start as Slot), span);
            self.emit(Op::Jump(update_start), span);
            target
        } else {
            self.emit(Op::Jump(update_start), span);
            update_start
        };
        let exit = self.cur().chunk.code.len();
        if let Some(exit_jump) = exit_after_cond {
            self.cur().chunk.patch_jump(exit_jump, exit);
        }
        self.finish_loop(exit, continue_target);
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

    fn find_loop_index(&self, label: Option<&str>, need_loop: bool) -> Option<usize> {
        let loops = &self.cur_ref().loops;
        match label {
            None => loops.iter().rposition(|c| c.is_loop),
            Some(name) => loops.iter().rposition(|c| c.label.as_deref() == Some(name) && (!need_loop || c.is_loop)),
        }
    }

    fn cur_ref(&self) -> &FnState {
        self.funcs.last().expect("function frame")
    }

    fn compile_break(&mut self, label: Option<&str>, span: Span) -> Result<(), CompileError> {
        let idx = match self.find_loop_index(label, false) {
            Some(i) => i,
            None => {
                let msg = match label {
                    Some(l) => format!("метка '{l}' не найдена"),
                    None => "'харэ' вне цикла".to_string(),
                };
                return Err(CompileError::new(msg, span));
            }
        };
        self.emit_try_exits_crossing(idx, span)?;
        let target_count = self.cur().loops[idx].locals_count;
        self.pop_locals_to(target_count, span);
        let jump = self.emit(Op::Jump(0), span);
        self.cur().loops[idx].break_jumps.push(jump);
        Ok(())
    }

    fn emit_try_exits_crossing(&mut self, loop_idx: usize, span: Span) -> Result<(), CompileError> {
        let crossed: Vec<usize> = self
            .cur()
            .try_ctxs
            .iter()
            .enumerate()
            .rev()
            .filter(|(_, t)| t.loops_len > loop_idx)
            .map(|(i, _)| i)
            .collect();
        self.emit_try_exits(&crossed, span)
    }

    fn emit_try_exits_all(&mut self, span: Span) -> Result<(), CompileError> {
        let all: Vec<usize> = (0..self.cur().try_ctxs.len()).rev().collect();
        self.emit_try_exits(&all, span)
    }

    fn emit_try_exits(&mut self, indices: &[usize], span: Span) -> Result<(), CompileError> {
        let saved: Vec<TryCtx> = self.cur().try_ctxs.drain(..).collect();
        let mut result = Ok(());
        for &i in indices {
            for _ in 0..saved[i].handler_count {
                self.emit(Op::PopHandler, span);
            }
            if let Some(finally) = saved[i].finally.clone() {
                self.cur().try_ctxs.truncate(i);
                if let Err(e) = self.compile_block_scoped(&finally) {
                    result = Err(e);
                    break;
                }
            }
        }
        self.cur().try_ctxs = saved;
        result
    }

    fn compile_continue(&mut self, label: Option<&str>, span: Span) -> Result<(), CompileError> {
        let idx = match self.find_loop_index(label, true) {
            Some(i) => i,
            None => {
                let msg = match label {
                    Some(l) => format!("метка цикла '{l}' не найдена"),
                    None => "'двигай' вне цикла".to_string(),
                };
                return Err(CompileError::new(msg, span));
            }
        };
        self.emit_try_exits_crossing(idx, span)?;
        let target_count = self.cur().loops[idx].locals_count;
        self.pop_locals_to(target_count, span);
        let jump = self.emit(Op::Jump(0), span);
        self.cur().loops[idx].continue_jumps.push(jump);
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
                if matches!(object.as_ref(), Expr::Super { .. }) {
                    if !self.in_method() {
                        return Err(CompileError::new("'яга' (super) используется вне класса-наследника", *span));
                    }
                    let idx = self.str_const(&property.name);
                    self.emit(Op::SuperGet(idx), *span);
                    return Ok(());
                }
                self.compile_expr(object)?;
                let idx = self.str_const(&property.name);
                self.emit(Op::GetProp(idx), *span);
                Ok(())
            }
            Expr::ArrowFunction { params, body, is_async, span } => self.compile_arrow(params, body, *is_async, *span),
            Expr::FunctionExpr { name, params, body, is_async, span } => {
                let fname = name.as_ref().map(|n| n.name.as_str()).unwrap_or("");
                self.compile_function(fname, params, body, *is_async, *span)
            }
            Expr::Await { argument, span } => {
                self.compile_expr(argument)?;
                self.emit(Op::Await, *span);
                Ok(())
            }
            Expr::TemplateLiteral { parts, span } => self.compile_template(parts, *span),
            Expr::OptionalMember { object, property, span } => {
                let idx = self.str_const(&property.name);
                self.compile_optional_chain(object, *span, |c| {
                    c.emit(Op::GetProp(idx), *span);
                    Ok(())
                })
            }
            Expr::OptionalIndex { object, index, span } => self.compile_optional_chain(object, *span, |c| {
                c.compile_expr(index)?;
                c.emit(Op::GetIndex, *span);
                Ok(())
            }),
            Expr::OptionalCall { callee, args, span } => {
                self.compile_optional_chain(callee, *span, |c| c.compile_call_args(args, *span))
            }
            Expr::This { span } => {
                if matches!(self.resolve(THIS_LOCAL), VarLoc::Global(_)) {
                    return Err(CompileError::new("'тырыпыры' (this) используется вне контекста объекта", *span));
                }
                self.compile_var_get(THIS_LOCAL, *span)
            }
            Expr::New { callee, args, span } => self.compile_new(callee, args, *span),
            Expr::Super { span } => {
                Err(CompileError::new("'яга' (super) допустим только как 'яга(...)' или 'яга.член'", *span))
            }
            Expr::TaggedTemplate { tag, quasis, expressions, span } => {
                self.compile_tagged_template(tag, quasis, expressions, *span)
            }
            Expr::DynamicImport { source, span } => {
                self.compile_expr(source)?;
                self.emit(Op::DynamicImport, *span);
                Ok(())
            }
            Expr::Yield { argument, delegate, span } => {
                if !self.cur().is_generator {
                    return Err(CompileError::new("'поебалу' допустим только внутри генератора", *span));
                }
                match argument {
                    Some(arg) => self.compile_expr(arg)?,
                    None => {
                        if *delegate {
                            return Err(CompileError::new("'поебалуна' требует аргумент", *span));
                        }
                        self.emit(Op::Undefined, *span);
                    }
                }
                self.emit(if *delegate { Op::YieldDelegate } else { Op::Yield }, *span);
                Ok(())
            }
            other => {
                Err(CompileError::new(format!("выражение не поддерживается VM: {}", expr_kind(other)), other.span()))
            }
        }
    }

    fn in_method(&self) -> bool {
        self.funcs.iter().rev().any(|f| f.is_method)
    }

    fn compile_new(&mut self, callee: &Expr, args: &[Expr], span: Span) -> Result<(), CompileError> {
        self.compile_expr(callee)?;
        if args.iter().any(|a| matches!(a, Expr::Spread { .. })) {
            self.compile_spread_array(args, span)?;
            self.emit(Op::NewSpread, span);
        } else {
            for arg in args {
                self.compile_expr(arg)?;
            }
            self.emit(Op::New(args.len() as u16), span);
        }
        Ok(())
    }

    fn compile_tagged_template(
        &mut self,
        tag: &Expr,
        quasis: &[yps_parser::ast::TemplateQuasi],
        expressions: &[Expr],
        span: Span,
    ) -> Result<(), CompileError> {
        self.compile_expr(tag)?;
        let strings = TemplateStrings {
            cooked: quasis.iter().map(|q| q.cooked.clone()).collect(),
            raw: quasis.iter().map(|q| q.raw.clone()).collect(),
        };
        let idx = self.cur().chunk.add_constant(Constant::Template(Rc::new(strings)));
        self.emit(Op::TaggedTemplate(idx), span);
        for e in expressions {
            self.compile_expr(e)?;
        }
        self.emit(Op::Call((expressions.len() + 1) as u16), span);
        Ok(())
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
                if elements.iter().any(|el| matches!(el, Expr::Spread { .. })) {
                    self.compile_spread_array(elements, *span)?;
                } else {
                    for el in elements {
                        self.compile_expr(el)?;
                    }
                    self.emit(Op::NewArray(elements.len() as u32), *span);
                }
                Ok(())
            }
            Literal::Object { entries, span } => {
                let needs_builder = entries.iter().any(|e| !matches!(e, ObjectEntry::Property { .. }));
                if !needs_builder {
                    for entry in entries {
                        if let ObjectEntry::Property { key, value } = entry {
                            self.compile_prop_key(key, *span)?;
                            self.compile_object_value(value)?;
                        }
                    }
                    self.emit(Op::NewObject(entries.len() as u32), *span);
                    return Ok(());
                }
                self.emit(Op::NewObject(0), *span);
                for entry in entries {
                    match entry {
                        ObjectEntry::Property { key, value } => {
                            self.compile_prop_key(key, *span)?;
                            self.compile_object_value(value)?;
                            self.emit(Op::ObjSet, *span);
                        }
                        ObjectEntry::Spread(expr) => {
                            self.compile_expr(expr)?;
                            self.emit(Op::SpreadObject, *span);
                        }
                        ObjectEntry::Getter { key, body, span: gspan } => {
                            self.compile_prop_key(key, *span)?;
                            self.compile_function("", &[], body, false, *gspan)?;
                            self.emit(Op::DefineGetter, *span);
                        }
                        ObjectEntry::Setter { key, param, body, span: sspan } => {
                            self.compile_prop_key(key, *span)?;
                            self.compile_function("", std::slice::from_ref(param), body, false, *sspan)?;
                            self.emit(Op::DefineSetter, *span);
                        }
                    }
                }
                Ok(())
            }
            Literal::BigInt { value, span } => {
                let idx = self.cur().chunk.add_constant(Constant::BigInt(*value));
                self.emit(Op::Constant(idx), *span);
                Ok(())
            }
            Literal::RegExp { pattern, flags, span } => {
                let idx = self.cur().chunk.add_constant(Constant::RegExp {
                    pattern: Rc::from(pattern.as_str()),
                    flags: Rc::from(flags.as_str()),
                });
                self.emit(Op::MakeRegex(idx), *span);
                Ok(())
            }
        }
    }

    fn compile_object_value(&mut self, value: &Expr) -> Result<(), CompileError> {
        if let Expr::ArrowFunction { params, body, is_async, span } = value {
            return self.compile_function("", params, body, *is_async, *span);
        }
        self.compile_expr(value)
    }

    fn compile_prop_key(&mut self, key: &PropKey, span: Span) -> Result<(), CompileError> {
        match key {
            PropKey::Identifier(id) => {
                let idx = self.str_const(&id.name);
                self.emit(Op::Constant(idx), span);
            }
            PropKey::Computed(expr) => self.compile_expr(expr)?,
        }
        Ok(())
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
            self.emit(Op::ConcatTemplate, span);
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
            UnaryOp::Delete => self.compile_delete(expr, span),
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

    fn compile_delete(&mut self, expr: &Expr, span: Span) -> Result<(), CompileError> {
        match expr {
            Expr::Member { object, property, .. } => {
                self.compile_expr(object)?;
                let idx = self.str_const(&property.name);
                self.emit(Op::DeleteProp(idx), span);
                Ok(())
            }
            Expr::Index { object, index, .. } => {
                self.compile_expr(object)?;
                self.compile_expr(index)?;
                self.emit(Op::DeleteIndex, span);
                Ok(())
            }
            _ => {
                self.compile_expr(expr)?;
                self.emit(Op::Pop, span);
                self.emit(Op::True, span);
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
            BinaryOp::AndAssign | BinaryOp::OrAssign | BinaryOp::NullishAssign => {
                return self.compile_logical_assign(op, lhs, rhs, span);
            }
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
            BinaryOp::Instanceof => Op::Instanceof,
            BinaryOp::In => Op::In,
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
            Expr::Member { object, property, .. } => {
                self.compile_expr(object)?;
                self.emit(Op::Dup, span);
                let idx = self.str_const(&property.name);
                self.emit(Op::GetProp(idx), span);
                self.compile_expr(rhs)?;
                self.emit(base, span);
                self.emit(Op::SetProp(idx), span);
                Ok(())
            }
            Expr::Index { object, index, .. } => {
                self.compile_expr(object)?;
                self.compile_expr(index)?;
                self.emit(Op::Dup2, span);
                self.emit(Op::GetIndex, span);
                self.compile_expr(rhs)?;
                self.emit(base, span);
                self.emit(Op::SetIndex, span);
                Ok(())
            }
            _ => Err(CompileError::new("недопустимая цель составного присваивания в VM", span)),
        }
    }

    fn compile_logical_assign(&mut self, op: BinaryOp, lhs: &Expr, rhs: &Expr, span: Span) -> Result<(), CompileError> {
        self.compile_expr(lhs)?;
        let short_circuit = match op {
            BinaryOp::AndAssign => self.emit(Op::JumpIfFalsePeek(0), span),
            BinaryOp::OrAssign => self.emit(Op::JumpIfTruePeek(0), span),
            BinaryOp::NullishAssign => self.emit(Op::JumpIfNotNullishPeek(0), span),
            _ => unreachable!("compile_logical_assign получил не-логический оператор"),
        };
        self.emit(Op::Pop, span);
        self.compile_assign(lhs, rhs, span)?;
        let end = self.cur().chunk.code.len();
        self.cur().chunk.patch_jump(short_circuit, end);
        Ok(())
    }

    fn compile_postfix(&mut self, op: PostfixOp, expr: &Expr, span: Span) -> Result<(), CompileError> {
        let Expr::Identifier(id) = expr else {
            return Err(CompileError::new("'++' / '--' можно применить только к переменной", span));
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
        match callee {
            Expr::Super { span: sspan } => {
                if !self.in_method() {
                    return Err(CompileError::new("'яга' (super) используется вне класса-наследника", *sspan));
                }
                if args.iter().any(|a| matches!(a, Expr::Spread { .. })) {
                    self.compile_spread_array(args, span)?;
                    self.emit(Op::SuperCallSpread, span);
                    return Ok(());
                }
                for arg in args {
                    self.compile_expr(arg)?;
                }
                self.emit(Op::SuperCall(args.len() as u16), span);
                return Ok(());
            }
            Expr::Member { object, property, .. } if matches!(object.as_ref(), Expr::Super { .. }) => {
                if !self.in_method() {
                    return Err(CompileError::new("'яга' (super) используется вне класса-наследника", span));
                }
                let idx = self.str_const(&property.name);
                if args.iter().any(|a| matches!(a, Expr::Spread { .. })) {
                    self.compile_spread_array(args, span)?;
                    self.emit(Op::SuperInvokeSpread(idx), span);
                    return Ok(());
                }
                for arg in args {
                    self.compile_expr(arg)?;
                }
                self.emit(Op::SuperInvoke(idx, args.len() as u16), span);
                return Ok(());
            }
            Expr::Member { object, property, .. } => {
                self.compile_expr(object)?;
                if args.iter().any(|a| matches!(a, Expr::Spread { .. })) {
                    let kidx = self.str_const(&property.name);
                    self.emit(Op::GetProp(kidx), span);
                    return self.compile_call_args(args, span);
                }
                for arg in args {
                    self.compile_expr(arg)?;
                }
                let idx = self.str_const(&property.name);
                self.emit(Op::Invoke(idx, args.len() as u16), span);
                return Ok(());
            }
            _ => self.compile_expr(callee)?,
        }
        self.compile_call_args(args, span)
    }

    fn compile_optional_chain(
        &mut self,
        base: &Expr,
        span: Span,
        emit_access: impl FnOnce(&mut Self) -> Result<(), CompileError>,
    ) -> Result<(), CompileError> {
        self.compile_expr(base)?;
        let nullish = self.emit(Op::JumpIfNullishPeek(0), span);
        emit_access(self)?;
        let done = self.emit(Op::Jump(0), span);
        let nullish_here = self.cur().chunk.code.len();
        self.cur().chunk.patch_jump(nullish, nullish_here);
        self.emit(Op::Pop, span);
        self.emit(Op::Undefined, span);
        let end = self.cur().chunk.code.len();
        self.cur().chunk.patch_jump(done, end);
        Ok(())
    }

    fn compile_spread_array(&mut self, args: &[Expr], span: Span) -> Result<(), CompileError> {
        self.emit(Op::NewArray(0), span);
        for arg in args {
            if let Expr::Spread { expr, .. } = arg {
                self.compile_expr(expr)?;
                self.emit(Op::AppendSpread, span);
            } else {
                self.compile_expr(arg)?;
                self.emit(Op::ArrPush, span);
            }
        }
        Ok(())
    }

    fn compile_call_args(&mut self, args: &[Expr], span: Span) -> Result<(), CompileError> {
        if args.iter().any(|a| matches!(a, Expr::Spread { .. })) {
            self.compile_spread_array(args, span)?;
            self.emit(Op::CallSpread, span);
        } else {
            for arg in args {
                self.compile_expr(arg)?;
            }
            self.emit(Op::Call(args.len() as u16), span);
        }
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

fn is_loop_stmt(stmt: &Stmt) -> bool {
    matches!(stmt, Stmt::While { .. } | Stmt::For { .. } | Stmt::DoWhile { .. })
}

fn decl_names(stmt: &Stmt) -> Vec<String> {
    match stmt {
        Stmt::VarDecl { pattern, .. } => {
            let mut names = Vec::new();
            pattern_names(pattern, &mut names);
            names
        }
        Stmt::FunctionDecl { name, .. } | Stmt::ClassDecl { name, .. } => vec![name.name.clone()],
        _ => Vec::new(),
    }
}

fn pattern_names(pattern: &Pattern, out: &mut Vec<String>) {
    match pattern {
        Pattern::Identifier(ident) => out.push(ident.name.clone()),
        Pattern::Default { pattern, .. } => pattern_names(pattern, out),
        Pattern::Array { elements, rest, .. } => {
            for el in elements.iter().flatten() {
                pattern_names(el, out);
            }
            if let Some(r) = rest {
                pattern_names(r, out);
            }
        }
        Pattern::Object { properties, rest, .. } => {
            for prop in properties {
                match &prop.value {
                    Some(value) => pattern_names(value, out),
                    None => out.push(prop.key.name.clone()),
                }
            }
            if let Some(r) = rest {
                pattern_names(r, out);
            }
        }
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
