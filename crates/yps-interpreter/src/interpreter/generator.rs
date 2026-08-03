use std::cell::RefCell;
use std::rc::Rc;

use yps_lexer::Span;
use yps_parser::ast::{Block, Expr, Stmt};

use crate::environment::Environment;
use crate::error::RuntimeError;
use crate::value::{BindTarget, GenFrame, GenState, IteratorState, LoopPhase, TryState, Value};

use super::{ControlFlow, Interpreter};

pub(super) enum GenStep {
    Yielded(Value),
    Awaited(Value),
    Done(Value),
    Threw(Value),
}

pub(crate) enum GenInput {
    Send(Value),
    Return(Value),
    Throw(Value),
}

pub(crate) enum StepOutcome {
    Yielded(Value),
    Awaited(Value),
    Done(Value),
}

enum Unwind {
    Throw(Value),
    Break(Option<String>),
    Continue(Option<String>),
    Return(Value),
}

fn targets(frame: &Option<String>, want: &Option<String>) -> bool {
    match want {
        None => true,
        Some(name) => frame.as_deref() == Some(name.as_str()),
    }
}

pub(crate) fn build_generator(name: Rc<str>, env: Environment, body: &Rc<Block>, is_async: bool) -> GenState {
    let lowered;
    let stmts: Rc<[Stmt]> = if is_async {
        lowered = super::async_lower::lower_async_body(body);
        Rc::from(lowered.stmts.as_slice())
    } else {
        Rc::from(body.stmts.as_slice())
    };
    GenState {
        name,
        env,
        frames: vec![GenFrame::Block { stmts, idx: 0, owns_scope: false, label: None }],
        completed: false,
        is_async,
        pending_bind: None,
        pending_send: None,
        pending_return: false,
        pending_label: None,
    }
}

pub(crate) fn step_generator(
    interp: &mut Interpreter,
    g: &mut GenState,
    input: GenInput,
    span: Span,
) -> Result<StepOutcome, RuntimeError> {
    if g.completed {
        return match input {
            GenInput::Throw(v) => Err(RuntimeError::thrown(v, span)),
            GenInput::Return(v) => Ok(StepOutcome::Done(v)),
            GenInput::Send(_) => Ok(StepOutcome::Done(Value::Undefined)),
        };
    }

    let saved_env = std::mem::replace(&mut interp.env, g.env.clone());
    let saved_stack = std::mem::take(&mut interp.call_stack);
    interp.push_frame(Rc::clone(&g.name), span);
    let mut result = match input {
        GenInput::Send(v) => {
            if std::mem::take(&mut g.pending_return) {
                pump_with_unwind(interp, g, Unwind::Return(v), span)
            } else {
                if let Some(bind) = g.pending_bind.take() {
                    apply_bind(&mut g.env, bind, v);
                } else {
                    g.pending_send = Some(v);
                }
                pump(interp, g, span)
            }
        }
        GenInput::Return(v) => {
            g.pending_bind = None;
            g.pending_send = None;
            g.pending_return = false;
            pump_with_unwind(interp, g, Unwind::Return(v), span)
        }
        GenInput::Throw(v) => {
            g.pending_bind = None;
            g.pending_send = None;
            g.pending_return = false;
            pump_with_unwind(interp, g, Unwind::Throw(v), span)
        }
    };
    if let Err(e) = &mut result {
        e.attach_stack(interp.snapshot_stack());
    }
    let gen_stack = if matches!(result, Ok(GenStep::Threw(_))) { interp.snapshot_stack() } else { Vec::new() };
    interp.pop_frame();
    interp.call_stack = saved_stack;
    g.env = std::mem::replace(&mut interp.env, saved_env);

    match result? {
        GenStep::Yielded(v) => Ok(StepOutcome::Yielded(v)),
        GenStep::Awaited(v) => Ok(StepOutcome::Awaited(v)),
        GenStep::Done(v) => {
            g.completed = true;
            Ok(StepOutcome::Done(v))
        }
        GenStep::Threw(v) => {
            g.completed = true;
            Err(RuntimeError::thrown_with_stack(v, span, gen_stack))
        }
    }
}

pub(crate) enum SyncStep {
    Yielded(Value),
    Done(Value),
}

pub(crate) fn step_generator_awaiting(
    interp: &mut Interpreter,
    g: &mut GenState,
    input: GenInput,
    span: Span,
) -> Result<SyncStep, RuntimeError> {
    let mut input = input;
    loop {
        match step_generator(interp, g, input, span)? {
            StepOutcome::Awaited(value) => match interp.do_await(value, span) {
                Ok(v) => input = GenInput::Send(v),
                Err(e) => match e.thrown {
                    Some(thrown) => input = GenInput::Throw(*thrown),
                    None => return Err(e),
                },
            },
            StepOutcome::Yielded(v) => return Ok(SyncStep::Yielded(v)),
            StepOutcome::Done(v) => return Ok(SyncStep::Done(v)),
        }
    }
}

fn pump_with_unwind(
    interp: &mut Interpreter,
    g: &mut GenState,
    u: Unwind,
    span: Span,
) -> Result<GenStep, RuntimeError> {
    if let Some(step) = unwind(interp, g, u, span)? {
        return Ok(step);
    }
    pump(interp, g, span)
}

fn apply_bind(env: &mut Environment, target: BindTarget, sent: Value) {
    match target {
        BindTarget::Variable { name, is_const } => env.define(name, sent, is_const),
        BindTarget::Reassign(name) => {
            env.set(&name, sent);
        }
    }
}

fn route_throw(
    interp: &mut Interpreter,
    g: &mut GenState,
    e: RuntimeError,
    span: Span,
) -> Result<Option<GenStep>, RuntimeError> {
    if let Some(thrown) = e.thrown.clone() {
        return unwind(interp, g, Unwind::Throw(*thrown), span);
    }
    if !g.frames.iter().any(|f| matches!(f, GenFrame::TryCatch { .. })) {
        return Err(e);
    }
    let mut map = indexmap::IndexMap::new();
    map.insert(
        crate::symbols::ERROR_NAME_FIELD.to_string(),
        Value::String(crate::symbols::ERROR_NAME.to_string().into()),
    );
    map.insert(crate::symbols::ERROR_MESSAGE_FIELD.to_string(), Value::String(e.message.clone().into()));
    unwind(interp, g, Unwind::Throw(Value::object(map)), span)
}

fn pump(interp: &mut Interpreter, g: &mut GenState, span: Span) -> Result<GenStep, RuntimeError> {
    loop {
        let Some(frame) = g.frames.last_mut() else {
            return Ok(GenStep::Done(Value::Undefined));
        };

        match frame {
            GenFrame::Block { stmts, idx, owns_scope, .. } => {
                if *idx >= stmts.len() {
                    let owns = *owns_scope;
                    g.frames.pop();
                    if owns {
                        interp.env.pop_scope();
                    }
                    continue;
                }
                let stmts_rc = Rc::clone(stmts);
                let i = *idx;
                *idx += 1;
                match step_block_stmt(interp, g, &stmts_rc[i], span) {
                    Ok(Some(step)) => return Ok(step),
                    Ok(None) => {}
                    Err(e) => {
                        if let Some(step) = route_throw(interp, g, e, span)? {
                            return Ok(step);
                        }
                    }
                }
            }
            GenFrame::While { condition, body, phase, .. } => match *phase {
                LoopPhase::CheckCond => {
                    let cond_rc = Rc::clone(condition);
                    let body_rc = Rc::clone(body);
                    let cond = interp.eval_expr(&cond_rc)?;
                    if cond.is_truthy() {
                        if let Some(GenFrame::While { phase, .. }) = g.frames.last_mut() {
                            *phase = LoopPhase::AfterBody;
                        }
                        push_body(interp, g, &body_rc);
                    } else {
                        g.frames.pop();
                    }
                }
                LoopPhase::AfterBody => {
                    *phase = LoopPhase::CheckCond;
                }
            },
            GenFrame::DoWhile { condition, body, phase, .. } => match *phase {
                LoopPhase::AfterBody => {
                    let cond_rc = Rc::clone(condition);
                    let body_rc = Rc::clone(body);
                    let cond = interp.eval_expr(&cond_rc)?;
                    if cond.is_truthy() {
                        push_body(interp, g, &body_rc);
                    } else {
                        g.frames.pop();
                    }
                }
                LoopPhase::CheckCond => {
                    let body_rc = Rc::clone(body);
                    *phase = LoopPhase::AfterBody;
                    push_body(interp, g, &body_rc);
                }
            },
            GenFrame::For { condition, update, body, phase, .. } => match *phase {
                LoopPhase::CheckCond => {
                    let cond_rc = condition.as_ref().map(Rc::clone);
                    let body_rc = Rc::clone(body);
                    let truthy = match cond_rc {
                        Some(c) => interp.eval_expr(&c)?.is_truthy(),
                        None => true,
                    };
                    if truthy {
                        if let Some(GenFrame::For { phase, .. }) = g.frames.last_mut() {
                            *phase = LoopPhase::AfterBody;
                        }
                        push_body(interp, g, &body_rc);
                    } else {
                        g.frames.pop();
                        interp.env.pop_scope();
                    }
                }
                LoopPhase::AfterBody => {
                    interp.env.fork_current();
                    if let Some(u) = update.as_ref().map(Rc::clone) {
                        interp.eval_expr(&u)?;
                    }
                    if let Some(GenFrame::For { phase, .. }) = g.frames.last_mut() {
                        *phase = LoopPhase::CheckCond;
                    }
                }
            },
            GenFrame::ForIter { variable, iter, body, .. } => {
                let variable = variable.clone();
                let iter_rc = iter.clone();
                let body_rc = body.clone();
                let next_val = {
                    let mut state = iter_rc.borrow_mut();
                    crate::stdlib::iterator::next(interp, &mut state, span)?
                };
                match next_val {
                    Some(v) => {
                        interp.env.fork_current();
                        interp.destructure_pattern(&variable, v, false, span)?;
                        push_body(interp, g, &body_rc);
                    }
                    None => {
                        g.frames.pop();
                        interp.env.pop_scope();
                    }
                }
            }
            GenFrame::ForAwait { aiter, variable, body, phase, .. } => match *phase {
                LoopPhase::CheckCond => {
                    let aiter = aiter.clone();
                    let pending = interp.async_iter_next_pending(&aiter, span)?;
                    if let Some(GenFrame::ForAwait { phase, .. }) = g.frames.last_mut() {
                        *phase = LoopPhase::AfterBody;
                    }
                    return Ok(GenStep::Awaited(pending));
                }
                LoopPhase::AfterBody => {
                    let variable = variable.clone();
                    let body_rc = Rc::clone(body);
                    let result = g.pending_send.take().unwrap_or(Value::Undefined);
                    let (done, item) = Interpreter::async_iter_unpack(&result);
                    if done {
                        g.frames.pop();
                        interp.env.pop_scope();
                    } else {
                        interp.env.fork_current();
                        interp.destructure_pattern(&variable, item, false, span)?;
                        if let Some(GenFrame::ForAwait { phase, .. }) = g.frames.last_mut() {
                            *phase = LoopPhase::CheckCond;
                        }
                        push_body(interp, g, &body_rc);
                    }
                }
            },
            GenFrame::ForAwaitSync { iter, variable, body, phase, .. } => match *phase {
                LoopPhase::CheckCond => {
                    let iter_rc = Rc::clone(iter);
                    let next_val = {
                        let mut state = iter_rc.borrow_mut();
                        crate::stdlib::iterator::next(interp, &mut state, span)?
                    };
                    match next_val {
                        Some(v) => {
                            if let Some(GenFrame::ForAwaitSync { phase, .. }) = g.frames.last_mut() {
                                *phase = LoopPhase::AfterBody;
                            }
                            return Ok(GenStep::Awaited(v));
                        }
                        None => {
                            g.frames.pop();
                            interp.env.pop_scope();
                        }
                    }
                }
                LoopPhase::AfterBody => {
                    let variable = variable.clone();
                    let body_rc = Rc::clone(body);
                    let item = g.pending_send.take().unwrap_or(Value::Undefined);
                    interp.env.fork_current();
                    interp.destructure_pattern(&variable, item, false, span)?;
                    if let Some(GenFrame::ForAwaitSync { phase, .. }) = g.frames.last_mut() {
                        *phase = LoopPhase::CheckCond;
                    }
                    push_body(interp, g, &body_rc);
                }
            },
            GenFrame::Delegate { inner, bind } => {
                let inner_rc = inner.clone();
                let bind = bind.take();
                let sent = g.pending_send.take().unwrap_or(Value::Undefined);
                let outcome = delegate_step(interp, &inner_rc, GenInput::Send(sent), span)?;
                match outcome {
                    DelegateOutcome::Yielded(v) => {
                        if let Some(GenFrame::Delegate { bind: slot, .. }) = g.frames.last_mut() {
                            *slot = bind;
                        }
                        return Ok(GenStep::Yielded(v));
                    }
                    DelegateOutcome::Done(ret) => {
                        g.frames.pop();
                        if let Some(target) = bind {
                            apply_bind(&mut g.env, target, ret);
                        }
                    }
                }
            }
            GenFrame::TryCatch { .. } => {
                let top_idx = g.frames.len() - 1;
                let (snapshot, fb_clone) = match &g.frames[top_idx] {
                    GenFrame::TryCatch { state, finally_body, .. } => (state.clone(), finally_body.clone()),
                    _ => unreachable!(),
                };
                match snapshot {
                    TryState::Trying => {
                        interp.env.pop_scope();
                        if let Some(fb) = fb_clone {
                            if let GenFrame::TryCatch { state, .. } = &mut g.frames[top_idx] {
                                *state = TryState::FinallyNormal;
                            }
                            interp.env.mark_tdz(crate::resolver::lexical_declarations(&fb));
                            g.frames.push(GenFrame::Block { stmts: fb, idx: 0, owns_scope: false, label: None });
                        } else {
                            g.frames.pop();
                        }
                    }
                    TryState::InCatch => {
                        interp.env.pop_scope();
                        if let Some(fb) = fb_clone {
                            if let GenFrame::TryCatch { state, .. } = &mut g.frames[top_idx] {
                                *state = TryState::FinallyNormal;
                            }
                            interp.env.mark_tdz(crate::resolver::lexical_declarations(&fb));
                            g.frames.push(GenFrame::Block { stmts: fb, idx: 0, owns_scope: false, label: None });
                        } else {
                            g.frames.pop();
                        }
                    }
                    TryState::FinallyNormal => {
                        g.frames.pop();
                    }
                    TryState::FinallyAfterThrow(v) => {
                        g.frames.pop();
                        if let Some(step) = unwind(interp, g, Unwind::Throw(v), span)? {
                            return Ok(step);
                        }
                    }
                    TryState::FinallyAfterReturn(v) => {
                        g.frames.clear();
                        return Ok(GenStep::Done(v));
                    }
                    TryState::FinallyAfterBreak(label) => {
                        g.frames.pop();
                        if let Some(step) = unwind(interp, g, Unwind::Break(label), span)? {
                            return Ok(step);
                        }
                    }
                    TryState::FinallyAfterContinue(label) => {
                        g.frames.pop();
                        if let Some(step) = unwind(interp, g, Unwind::Continue(label), span)? {
                            return Ok(step);
                        }
                    }
                }
            }
        }
    }
}

enum DelegateOutcome {
    Yielded(Value),
    Done(Value),
}

fn delegate_step(
    interp: &mut Interpreter,
    inner_rc: &Rc<RefCell<IteratorState>>,
    input: GenInput,
    span: Span,
) -> Result<DelegateOutcome, RuntimeError> {
    let mut state = inner_rc.borrow_mut();
    if let IteratorState::Generator(gen_state) = &mut *state {
        let outcome = step_generator_awaiting(interp, gen_state, input, span)?;
        return Ok(match outcome {
            SyncStep::Yielded(v) => DelegateOutcome::Yielded(v),
            SyncStep::Done(v) => DelegateOutcome::Done(v),
        });
    }
    match input {
        GenInput::Send(_) => match crate::stdlib::iterator::next(interp, &mut state, span)? {
            Some(v) => Ok(DelegateOutcome::Yielded(v)),
            None => Ok(DelegateOutcome::Done(Value::Undefined)),
        },
        GenInput::Return(v) => {
            *state = IteratorState::Done;
            Ok(DelegateOutcome::Done(v))
        }
        GenInput::Throw(v) => Err(RuntimeError::thrown(v, span)),
    }
}

fn push_body(interp: &mut Interpreter, g: &mut GenState, body: &Rc<[Stmt]>) {
    interp.env.mark_tdz(crate::resolver::lexical_declarations(body));
    g.frames.push(GenFrame::Block { stmts: Rc::clone(body), idx: 0, owns_scope: false, label: None });
}

fn body_stmts(body: &Stmt) -> Rc<[Stmt]> {
    match body {
        Stmt::Block(b) => Rc::from(b.stmts.as_slice()),
        other => Rc::from(vec![other.clone()].as_slice()),
    }
}

fn step_block_stmt(
    interp: &mut Interpreter,
    g: &mut GenState,
    stmt: &Stmt,
    span: Span,
) -> Result<Option<GenStep>, RuntimeError> {
    match stmt {
        Stmt::Expr { expr: Expr::Await { argument, .. }, .. } if g.is_async => {
            let val = interp.eval_expr(argument)?;
            Ok(Some(GenStep::Awaited(val)))
        }
        Stmt::Expr { expr: Expr::Yield { argument, delegate, span: ys }, .. } => {
            if *delegate {
                let arg = argument.as_deref().ok_or_else(|| RuntimeError::new("'поебалуна' требует аргумент", *ys))?;
                let val = interp.eval_expr(arg)?;
                let iter_rc = value_to_iterator(val, *ys)?;
                g.frames.push(GenFrame::Delegate { inner: iter_rc, bind: None });
                Ok(None)
            } else {
                let val = match argument.as_deref() {
                    Some(a) => interp.eval_expr(a)?,
                    None => Value::Undefined,
                };
                Ok(Some(GenStep::Yielded(val)))
            }
        }
        Stmt::VarDecl { pattern, init, is_const, span: vs } => {
            if let Expr::Await { argument, .. } = init
                && g.is_async
            {
                let yps_parser::ast::Pattern::Identifier(ident) = pattern else {
                    interp.exec_stmt(stmt)?;
                    return Ok(None);
                };
                let val = interp.eval_expr(argument)?;
                g.pending_bind = Some(BindTarget::Variable { name: ident.name.clone(), is_const: *is_const });
                return Ok(Some(GenStep::Awaited(val)));
            }
            if let Expr::Yield { argument, delegate, span: ys } = init {
                let name = match pattern {
                    yps_parser::ast::Pattern::Identifier(ident) => ident.name.clone(),
                    _ => {
                        return Err(RuntimeError::new(
                            "'поебалу' в декларации поддерживается только для простого имени",
                            *vs,
                        ));
                    }
                };
                if *delegate {
                    let arg =
                        argument.as_deref().ok_or_else(|| RuntimeError::new("'поебалуна' требует аргумент", *ys))?;
                    let val = interp.eval_expr(arg)?;
                    let iter_rc = value_to_iterator(val, *ys)?;
                    g.frames.push(GenFrame::Delegate {
                        inner: iter_rc,
                        bind: Some(BindTarget::Variable { name, is_const: *is_const }),
                    });
                    return Ok(None);
                }
                let val = match argument.as_deref() {
                    Some(a) => interp.eval_expr(a)?,
                    None => Value::Undefined,
                };
                g.pending_bind = Some(BindTarget::Variable { name, is_const: *is_const });
                return Ok(Some(GenStep::Yielded(val)));
            }
            interp.exec_stmt(stmt)?;
            Ok(None)
        }
        Stmt::Expr { expr: Expr::Binary { op, lhs, rhs, span: bs }, .. } => {
            if matches!(op, yps_parser::ast::BinaryOp::Assign)
                && g.is_async
                && let Expr::Await { argument, .. } = rhs.as_ref()
                && let Expr::Identifier(ident) = lhs.as_ref()
            {
                let val = interp.eval_expr(argument)?;
                g.pending_bind = Some(BindTarget::Reassign(ident.name.clone()));
                return Ok(Some(GenStep::Awaited(val)));
            }
            if matches!(op, yps_parser::ast::BinaryOp::Assign)
                && let Expr::Yield { argument, delegate, span: ys } = rhs.as_ref()
                && let Expr::Identifier(ident) = lhs.as_ref()
            {
                if *delegate {
                    let arg =
                        argument.as_deref().ok_or_else(|| RuntimeError::new("'поебалуна' требует аргумент", *ys))?;
                    let val = interp.eval_expr(arg)?;
                    let iter_rc = value_to_iterator(val, *ys)?;
                    g.frames.push(GenFrame::Delegate {
                        inner: iter_rc,
                        bind: Some(BindTarget::Reassign(ident.name.clone())),
                    });
                    return Ok(None);
                }
                let val = match argument.as_deref() {
                    Some(a) => interp.eval_expr(a)?,
                    None => Value::Undefined,
                };
                g.pending_bind = Some(BindTarget::Reassign(ident.name.clone()));
                return Ok(Some(GenStep::Yielded(val)));
            }
            let _ = bs;
            interp.exec_stmt(stmt)?;
            Ok(None)
        }
        Stmt::Block(block) => {
            interp.env.push_scope();
            interp.env.mark_tdz(crate::resolver::lexical_declarations(&block.stmts));
            let stmts: Rc<[Stmt]> = Rc::from(block.stmts.as_slice());
            g.frames.push(GenFrame::Block { stmts, idx: 0, owns_scope: true, label: g.pending_label.take() });
            Ok(None)
        }
        Stmt::If { condition, then_branch, else_branch, .. } => {
            let cond = interp.eval_expr(condition)?;
            if cond.is_truthy() {
                push_body(interp, g, &body_stmts(then_branch));
            } else if let Some(eb) = else_branch {
                push_body(interp, g, &body_stmts(eb));
            }
            Ok(None)
        }
        Stmt::While { condition, body, .. } => {
            g.frames.push(GenFrame::While {
                condition: Rc::new(condition.clone()),
                body: body_stmts(body),
                phase: LoopPhase::CheckCond,
                label: g.pending_label.take(),
            });
            Ok(None)
        }
        Stmt::DoWhile { body, condition, .. } => {
            g.frames.push(GenFrame::DoWhile {
                condition: Rc::new(condition.clone()),
                body: body_stmts(body),
                phase: LoopPhase::CheckCond,
                label: g.pending_label.take(),
            });
            Ok(None)
        }
        Stmt::For { init, condition, update, body, .. } => {
            interp.env.push_scope();
            if let Some(init_stmt) = init {
                interp.exec_stmt(init_stmt)?;
            }
            g.frames.push(GenFrame::For {
                condition: condition.clone().map(Rc::new),
                update: update.clone().map(Rc::new),
                body: body_stmts(body),
                phase: LoopPhase::CheckCond,
                label: g.pending_label.take(),
            });
            Ok(None)
        }
        Stmt::ForOf { variable, iterable, body, span: fs } => {
            let val = interp.eval_expr(iterable)?;
            let iter_rc = value_to_iterator(val, *fs)?;
            interp.env.push_scope();
            g.frames.push(GenFrame::ForIter {
                variable: variable.clone(),
                iter: iter_rc,
                body: body_stmts(body),
                label: g.pending_label.take(),
            });
            Ok(None)
        }
        Stmt::ForAwaitOf { variable, iterable, body, span: fs } if g.is_async => {
            let val = interp.eval_expr(iterable)?;
            let val = interp.do_await(val, *fs)?;
            match interp.get_async_iterator(&val, *fs)? {
                Some(aiter) => {
                    interp.env.push_scope();
                    g.frames.push(GenFrame::ForAwait {
                        aiter,
                        variable: variable.clone(),
                        body: body_stmts(body),
                        phase: LoopPhase::CheckCond,
                        label: g.pending_label.take(),
                    });
                }
                None => {
                    let iter_rc = value_to_iterator(val, *fs)?;
                    interp.env.push_scope();
                    g.frames.push(GenFrame::ForAwaitSync {
                        iter: iter_rc,
                        variable: variable.clone(),
                        body: body_stmts(body),
                        phase: LoopPhase::CheckCond,
                        label: g.pending_label.take(),
                    });
                }
            }
            Ok(None)
        }
        Stmt::ForIn { variable, iterable, body, span: fs } => {
            let val = interp.eval_expr(iterable)?;
            let keys: Vec<Value> = match val {
                Value::Array(arr) => (0..arr.borrow().len()).map(|i| Value::Number(i as f64)).collect(),
                Value::TypedArray(ta) => (0..ta.length).map(|i| Value::Number(i as f64)).collect(),
                Value::Proxy { target, handler } => interp.proxy_own_keys(&target, &handler, *fs)?,
                Value::Object(map) => map.borrow().keys().map(|k| Value::String(k.clone().into())).collect(),
                other => {
                    return Err(RuntimeError::new(format!("Нельзя итерировать по типу '{}'", other.type_name()), *fs));
                }
            };
            let iter_rc = Rc::new(RefCell::new(IteratorState::Array { values: keys, index: 0 }));
            interp.env.push_scope();
            g.frames.push(GenFrame::ForIter {
                variable: variable.clone(),
                iter: iter_rc,
                body: body_stmts(body),
                label: g.pending_label.take(),
            });
            Ok(None)
        }
        Stmt::Return { value, .. } => {
            if let Some(Expr::Await { argument, .. }) = value
                && g.is_async
            {
                let val = interp.eval_expr(argument)?;
                g.pending_return = true;
                return Ok(Some(GenStep::Awaited(val)));
            }
            let val = match value {
                Some(e) => {
                    if let Expr::Yield { span: ys, .. } = e {
                        return Err(RuntimeError::new("'поебалу' не допускается в 'отвечаю'", *ys));
                    }
                    interp.eval_expr(e)?
                }
                None => Value::Undefined,
            };
            if let Some(step) = unwind(interp, g, Unwind::Return(val), span)? {
                return Ok(Some(step));
            }
            Ok(None)
        }
        Stmt::Throw { value, span: ts } => {
            let val = interp.eval_expr(value)?;
            let _ = ts;
            if let Some(step) = unwind(interp, g, Unwind::Throw(val), span)? {
                return Ok(Some(step));
            }
            Ok(None)
        }
        Stmt::Labeled { label, body, .. } => {
            g.pending_label = Some(label.name.clone());
            let inner = step_block_stmt(interp, g, body, span);
            g.pending_label = None;
            inner
        }
        Stmt::Break { label, .. } => {
            let want = label.as_ref().map(|l| l.name.clone());
            if let Some(step) = unwind(interp, g, Unwind::Break(want), span)? {
                return Ok(Some(step));
            }
            Ok(None)
        }
        Stmt::Continue { label, .. } => {
            let want = label.as_ref().map(|l| l.name.clone());
            if let Some(step) = unwind(interp, g, Unwind::Continue(want), span)? {
                return Ok(Some(step));
            }
            Ok(None)
        }
        Stmt::TryCatch { try_block, catch_param, catch_block, finally_block, .. } => {
            let catch_body = catch_block.as_ref().map(|b| Rc::from(b.stmts.as_slice()));
            let finally_body = finally_block.as_ref().map(|b| Rc::from(b.stmts.as_slice()));
            g.frames.push(GenFrame::TryCatch {
                catch_param: catch_param.as_ref().map(|p| p.name.clone()),
                catch_body,
                finally_body,
                state: TryState::Trying,
            });
            interp.env.push_scope();
            interp.env.mark_tdz(crate::resolver::lexical_declarations(&try_block.stmts));
            let try_stmts: Rc<[Stmt]> = Rc::from(try_block.stmts.as_slice());
            g.frames.push(GenFrame::Block { stmts: try_stmts, idx: 0, owns_scope: false, label: None });
            Ok(None)
        }
        other => {
            let cf = interp.exec_stmt(other)?;
            match cf {
                None => Ok(None),
                Some(ControlFlow::Return(v)) => {
                    if let Some(step) = unwind(interp, g, Unwind::Return(v), span)? {
                        return Ok(Some(step));
                    }
                    Ok(None)
                }
                Some(ControlFlow::Throw(v)) => {
                    if let Some(step) = unwind(interp, g, Unwind::Throw(v), span)? {
                        return Ok(Some(step));
                    }
                    Ok(None)
                }
                Some(ControlFlow::Break(_)) => {
                    if let Some(step) = unwind(interp, g, Unwind::Break(None), span)? {
                        return Ok(Some(step));
                    }
                    Ok(None)
                }
                Some(ControlFlow::Continue(_)) => {
                    if let Some(step) = unwind(interp, g, Unwind::Continue(None), span)? {
                        return Ok(Some(step));
                    }
                    Ok(None)
                }
            }
        }
    }
}

fn unwind(
    interp: &mut Interpreter,
    g: &mut GenState,
    mut kind: Unwind,
    span: Span,
) -> Result<Option<GenStep>, RuntimeError> {
    loop {
        let Some(top) = g.frames.last_mut() else {
            return match kind {
                Unwind::Throw(v) => Ok(Some(GenStep::Threw(v))),
                Unwind::Return(v) => Ok(Some(GenStep::Done(v))),
                Unwind::Break(label) => Err(RuntimeError::new(
                    label.map_or_else(|| "'харэ' вне цикла".to_string(), |l| format!("Метка '{l}' не найдена")),
                    span,
                )),
                Unwind::Continue(label) => Err(RuntimeError::new(
                    label.map_or_else(|| "'двигай' вне цикла".to_string(), |l| format!("Метка '{l}' не найдена")),
                    span,
                )),
            };
        };

        match top {
            GenFrame::TryCatch { state, catch_param, catch_body, finally_body } => match &kind {
                Unwind::Throw(v) => match state {
                    TryState::Trying => {
                        if let Some(cb) = catch_body.clone() {
                            *state = TryState::InCatch;
                            interp.env.pop_scope();
                            interp.env.push_scope();
                            if let Some(name) = catch_param {
                                interp.env.define(name.clone(), v.clone(), false);
                            }
                            interp.env.mark_tdz(crate::resolver::lexical_declarations(&cb));
                            g.frames.push(GenFrame::Block { stmts: cb, idx: 0, owns_scope: false, label: None });
                            return Ok(None);
                        } else if let Some(fb) = finally_body.clone() {
                            *state = TryState::FinallyAfterThrow(v.clone());
                            interp.env.pop_scope();
                            interp.env.mark_tdz(crate::resolver::lexical_declarations(&fb));
                            g.frames.push(GenFrame::Block { stmts: fb, idx: 0, owns_scope: false, label: None });
                            return Ok(None);
                        } else {
                            g.frames.pop();
                            interp.env.pop_scope();
                            continue;
                        }
                    }
                    TryState::InCatch => {
                        if let Some(fb) = finally_body.clone() {
                            *state = TryState::FinallyAfterThrow(v.clone());
                            interp.env.pop_scope();
                            interp.env.mark_tdz(crate::resolver::lexical_declarations(&fb));
                            g.frames.push(GenFrame::Block { stmts: fb, idx: 0, owns_scope: false, label: None });
                            return Ok(None);
                        } else {
                            g.frames.pop();
                            interp.env.pop_scope();
                            continue;
                        }
                    }
                    _ => {
                        g.frames.pop();
                        continue;
                    }
                },
                Unwind::Return(v) => {
                    let pending = matches!(state, TryState::Trying | TryState::InCatch);
                    match state {
                        TryState::Trying => interp.env.pop_scope(),
                        TryState::InCatch => interp.env.pop_scope(),
                        _ => {}
                    }
                    if pending && let Some(fb) = finally_body.clone() {
                        *state = TryState::FinallyAfterReturn(v.clone());
                        interp.env.mark_tdz(crate::resolver::lexical_declarations(&fb));
                        g.frames.push(GenFrame::Block { stmts: fb, idx: 0, owns_scope: false, label: None });
                        return Ok(None);
                    } else {
                        g.frames.pop();
                        continue;
                    }
                }
                Unwind::Break(label) => {
                    let pending = matches!(state, TryState::Trying | TryState::InCatch);
                    match state {
                        TryState::Trying => interp.env.pop_scope(),
                        TryState::InCatch => interp.env.pop_scope(),
                        _ => {}
                    }
                    if pending && let Some(fb) = finally_body.clone() {
                        *state = TryState::FinallyAfterBreak(label.clone());
                        interp.env.mark_tdz(crate::resolver::lexical_declarations(&fb));
                        g.frames.push(GenFrame::Block { stmts: fb, idx: 0, owns_scope: false, label: None });
                        return Ok(None);
                    } else {
                        g.frames.pop();
                        continue;
                    }
                }
                Unwind::Continue(label) => {
                    let pending = matches!(state, TryState::Trying | TryState::InCatch);
                    match state {
                        TryState::Trying => interp.env.pop_scope(),
                        TryState::InCatch => interp.env.pop_scope(),
                        _ => {}
                    }
                    if pending && let Some(fb) = finally_body.clone() {
                        *state = TryState::FinallyAfterContinue(label.clone());
                        interp.env.mark_tdz(crate::resolver::lexical_declarations(&fb));
                        g.frames.push(GenFrame::Block { stmts: fb, idx: 0, owns_scope: false, label: None });
                        return Ok(None);
                    } else {
                        g.frames.pop();
                        continue;
                    }
                }
            },
            GenFrame::While { phase, label, .. } | GenFrame::DoWhile { phase, label, .. } => match &kind {
                Unwind::Break(want) if targets(label, want) => {
                    g.frames.pop();
                    return Ok(None);
                }
                Unwind::Continue(want) if targets(label, want) => {
                    *phase = LoopPhase::CheckCond;
                    return Ok(None);
                }
                _ => {
                    g.frames.pop();
                    continue;
                }
            },
            GenFrame::For { phase, label, .. } => match &kind {
                Unwind::Break(want) if targets(label, want) => {
                    g.frames.pop();
                    interp.env.pop_scope();
                    return Ok(None);
                }
                Unwind::Continue(want) if targets(label, want) => {
                    *phase = LoopPhase::AfterBody;
                    return Ok(None);
                }
                _ => {
                    g.frames.pop();
                    interp.env.pop_scope();
                    continue;
                }
            },
            GenFrame::ForIter { label, .. } => match &kind {
                Unwind::Break(want) if targets(label, want) => {
                    g.frames.pop();
                    interp.env.pop_scope();
                    return Ok(None);
                }
                Unwind::Continue(want) if targets(label, want) => {
                    return Ok(None);
                }
                _ => {
                    g.frames.pop();
                    interp.env.pop_scope();
                    continue;
                }
            },
            GenFrame::ForAwait { aiter, phase, label, .. } => {
                if matches!(&kind, Unwind::Continue(want) if targets(label, want)) {
                    *phase = LoopPhase::CheckCond;
                    return Ok(None);
                }
                let stop = matches!(&kind, Unwind::Break(want) if targets(label, want));
                let aiter = aiter.clone();
                g.frames.pop();
                interp.async_iter_close(&aiter, span)?;
                interp.env.pop_scope();
                if stop {
                    return Ok(None);
                }
                continue;
            }
            GenFrame::ForAwaitSync { iter, phase, label, .. } => {
                if matches!(&kind, Unwind::Continue(want) if targets(label, want)) {
                    *phase = LoopPhase::CheckCond;
                    return Ok(None);
                }
                let stop = matches!(&kind, Unwind::Break(want) if targets(label, want));
                let iter_rc = Rc::clone(iter);
                g.frames.pop();
                {
                    let mut state = iter_rc.borrow_mut();
                    let _ = crate::stdlib::iterator::close(interp, &mut state, span);
                }
                interp.env.pop_scope();
                if stop {
                    return Ok(None);
                }
                continue;
            }
            GenFrame::Block { owns_scope, label, .. } => {
                let owns = *owns_scope;
                let stop = matches!(&kind, Unwind::Break(Some(want)) if label.as_deref() == Some(want.as_str()));
                g.frames.pop();
                if owns {
                    interp.env.pop_scope();
                }
                if stop {
                    return Ok(None);
                }
                continue;
            }
            GenFrame::Delegate { inner, .. } => {
                let inner_rc = inner.clone();
                match &kind {
                    Unwind::Return(v) => {
                        let outcome = delegate_step(interp, &inner_rc, GenInput::Return(v.clone()), span)?;
                        match outcome {
                            DelegateOutcome::Yielded(y) => return Ok(Some(GenStep::Yielded(y))),
                            DelegateOutcome::Done(_) => {
                                g.frames.pop();
                                continue;
                            }
                        }
                    }
                    Unwind::Throw(v) => {
                        let outcome = match delegate_step(interp, &inner_rc, GenInput::Throw(v.clone()), span) {
                            Ok(o) => o,
                            Err(e) => {
                                if let Some(thrown) = e.thrown.as_deref() {
                                    g.frames.pop();
                                    kind = Unwind::Throw(thrown.clone());
                                    continue;
                                }
                                return Err(e);
                            }
                        };
                        match outcome {
                            DelegateOutcome::Yielded(y) => return Ok(Some(GenStep::Yielded(y))),
                            DelegateOutcome::Done(_) => {
                                g.frames.pop();
                                continue;
                            }
                        }
                    }
                    Unwind::Break(_) | Unwind::Continue(_) => {
                        g.frames.pop();
                        continue;
                    }
                }
            }
        }
    }
}

fn value_to_iterator(val: Value, span: Span) -> Result<Rc<RefCell<IteratorState>>, RuntimeError> {
    let state = match val {
        Value::Iterator(rc) => return Ok(rc),
        Value::Array(values) => IteratorState::Array { values: values.borrow().0.clone(), index: 0 },
        Value::String(s) => IteratorState::Chars { chars: s.chars().collect(), index: 0 },
        Value::Set(items) => {
            IteratorState::Array { values: items.borrow().iter().map(|k| k.as_value().clone()).collect(), index: 0 }
        }
        Value::Map(entries) => IteratorState::MapEntries {
            entries: entries.borrow().iter().map(|(k, v)| (k.as_value().clone(), v.clone())).collect(),
            index: 0,
        },
        Value::TypedArray(ta) => IteratorState::Array {
            values: crate::stdlib::typed_array::ta_elements(&ta.buffer, ta.offset, ta.length, ta.kind),
            index: 0,
        },
        other => {
            return Err(RuntimeError::new(format!("Нельзя итерировать по типу '{}'", other.type_name()), span));
        }
    };
    Ok(Rc::new(RefCell::new(state)))
}
