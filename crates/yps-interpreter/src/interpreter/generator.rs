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
    Done(Value),
}

enum Unwind {
    Throw(Value),
    Break,
    Continue,
    Return(Value),
}

pub(crate) fn build_generator(name: Rc<str>, env: Environment, body: &Rc<Block>) -> GenState {
    let stmts: Rc<[Stmt]> = Rc::from(body.stmts.as_slice());
    GenState { name, env, frames: vec![GenFrame::Block { stmts, idx: 0 }], completed: false, pending_bind: None }
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
            if let Some(bind) = g.pending_bind.take() {
                apply_bind(&mut g.env, bind, v);
            }
            pump(interp, g, span)
        }
        GenInput::Return(v) => {
            g.pending_bind = None;
            pump_with_unwind(interp, g, Unwind::Return(v), span)
        }
        GenInput::Throw(v) => {
            g.pending_bind = None;
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

fn pump(interp: &mut Interpreter, g: &mut GenState, span: Span) -> Result<GenStep, RuntimeError> {
    loop {
        let Some(frame) = g.frames.last_mut() else {
            return Ok(GenStep::Done(Value::Undefined));
        };

        match frame {
            GenFrame::Block { stmts, idx } => {
                if *idx >= stmts.len() {
                    g.frames.pop();
                    continue;
                }
                let stmts_rc = Rc::clone(stmts);
                let i = *idx;
                *idx += 1;
                if let Some(step) = step_block_stmt(interp, g, &stmts_rc[i], span)? {
                    return Ok(step);
                }
            }
            GenFrame::While { condition, body, phase } => match *phase {
                LoopPhase::CheckCond => {
                    let cond_rc = Rc::clone(condition);
                    let body_rc = Rc::clone(body);
                    let cond = interp.eval_expr(&cond_rc)?;
                    if cond.is_truthy() {
                        if let Some(GenFrame::While { phase, .. }) = g.frames.last_mut() {
                            *phase = LoopPhase::AfterBody;
                        }
                        push_body(g, &body_rc);
                    } else {
                        g.frames.pop();
                    }
                }
                LoopPhase::AfterBody => {
                    *phase = LoopPhase::CheckCond;
                }
            },
            GenFrame::DoWhile { condition, body, phase } => match *phase {
                LoopPhase::AfterBody => {
                    let cond_rc = Rc::clone(condition);
                    let body_rc = Rc::clone(body);
                    let cond = interp.eval_expr(&cond_rc)?;
                    if cond.is_truthy() {
                        push_body(g, &body_rc);
                    } else {
                        g.frames.pop();
                    }
                }
                LoopPhase::CheckCond => {
                    let body_rc = Rc::clone(body);
                    *phase = LoopPhase::AfterBody;
                    push_body(g, &body_rc);
                }
            },
            GenFrame::For { condition, update, body, phase } => match *phase {
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
                        push_body(g, &body_rc);
                    } else {
                        g.frames.pop();
                        interp.env.pop_scope();
                    }
                }
                LoopPhase::AfterBody => {
                    if let Some(u) = update.as_ref().map(Rc::clone) {
                        interp.eval_expr(&u)?;
                    }
                    if let Some(GenFrame::For { phase, .. }) = g.frames.last_mut() {
                        *phase = LoopPhase::CheckCond;
                    }
                }
            },
            GenFrame::ForIter { var_name, iter, body } => {
                let var_name = var_name.clone();
                let iter_rc = iter.clone();
                let body_rc = body.clone();
                let next_val = {
                    let mut state = iter_rc.borrow_mut();
                    crate::stdlib::iterator::next(interp, &mut state, span)?
                };
                match next_val {
                    Some(v) => {
                        interp.env.set(&var_name, v);
                        push_body(g, &body_rc);
                    }
                    None => {
                        g.frames.pop();
                        interp.env.pop_scope();
                    }
                }
            }
            GenFrame::Delegate { inner } => {
                let inner_rc = inner.clone();
                let next_val = {
                    let mut state = inner_rc.borrow_mut();
                    crate::stdlib::iterator::next(interp, &mut state, span)?
                };
                match next_val {
                    Some(v) => return Ok(GenStep::Yielded(v)),
                    None => {
                        g.frames.pop();
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
                        if let Some(fb) = fb_clone {
                            if let GenFrame::TryCatch { state, .. } = &mut g.frames[top_idx] {
                                *state = TryState::FinallyNormal;
                            }
                            g.frames.push(GenFrame::Block { stmts: fb, idx: 0 });
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
                            g.frames.push(GenFrame::Block { stmts: fb, idx: 0 });
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
                    TryState::FinallyAfterBreak => {
                        g.frames.pop();
                        if let Some(step) = unwind(interp, g, Unwind::Break, span)? {
                            return Ok(step);
                        }
                    }
                    TryState::FinallyAfterContinue => {
                        g.frames.pop();
                        if let Some(step) = unwind(interp, g, Unwind::Continue, span)? {
                            return Ok(step);
                        }
                    }
                }
            }
        }
    }
}

fn push_body(g: &mut GenState, body: &Rc<[Stmt]>) {
    g.frames.push(GenFrame::Block { stmts: Rc::clone(body), idx: 0 });
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
        Stmt::Expr { expr: Expr::Yield { argument, delegate, span: ys }, .. } => {
            if *delegate {
                let arg = argument.as_deref().ok_or_else(|| RuntimeError::new("'поебалуна' требует аргумент", *ys))?;
                let val = interp.eval_expr(arg)?;
                let iter_rc = value_to_iterator(val, *ys)?;
                g.frames.push(GenFrame::Delegate { inner: iter_rc });
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
            if let Expr::Yield { argument, delegate, span: ys } = init {
                if *delegate {
                    return Err(RuntimeError::new(
                        "'поебалуна' не допускается в декларации; используйте отдельный оператор",
                        *ys,
                    ));
                }
                let name = match pattern {
                    yps_parser::ast::Pattern::Identifier(ident) => ident.name.clone(),
                    _ => {
                        return Err(RuntimeError::new(
                            "'поебалу' в декларации поддерживается только для простого имени",
                            *vs,
                        ));
                    }
                };
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
                && let Expr::Yield { argument, delegate, span: ys } = rhs.as_ref()
                && let Expr::Identifier(ident) = lhs.as_ref()
            {
                if *delegate {
                    return Err(RuntimeError::new(
                        "'поебалуна' не допускается в присваивании; используйте отдельный оператор",
                        *ys,
                    ));
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
            let stmts: Rc<[Stmt]> = Rc::from(block.stmts.as_slice());
            g.frames.push(GenFrame::Block { stmts, idx: 0 });
            Ok(None)
        }
        Stmt::If { condition, then_branch, else_branch, .. } => {
            let cond = interp.eval_expr(condition)?;
            if cond.is_truthy() {
                push_body(g, &body_stmts(then_branch));
            } else if let Some(eb) = else_branch {
                push_body(g, &body_stmts(eb));
            }
            Ok(None)
        }
        Stmt::While { condition, body, .. } => {
            g.frames.push(GenFrame::While {
                condition: Rc::new(condition.clone()),
                body: body_stmts(body),
                phase: LoopPhase::CheckCond,
            });
            Ok(None)
        }
        Stmt::DoWhile { body, condition, .. } => {
            g.frames.push(GenFrame::DoWhile {
                condition: Rc::new(condition.clone()),
                body: body_stmts(body),
                phase: LoopPhase::CheckCond,
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
            });
            Ok(None)
        }
        Stmt::ForOf { variable, iterable, body, span: fs } => {
            let val = interp.eval_expr(iterable)?;
            let iter_rc = value_to_iterator(val, *fs)?;
            interp.env.push_scope();
            interp.env.define(variable.name.clone(), Value::Undefined, false);
            g.frames.push(GenFrame::ForIter { var_name: variable.name.clone(), iter: iter_rc, body: body_stmts(body) });
            Ok(None)
        }
        Stmt::ForIn { variable, iterable, body, span: fs } => {
            let val = interp.eval_expr(iterable)?;
            let keys: Vec<Value> = match val {
                Value::Array(arr) => (0..arr.borrow().len()).map(|i| Value::Number(i as f64)).collect(),
                Value::TypedArray { length, .. } => (0..length).map(|i| Value::Number(i as f64)).collect(),
                Value::Object(map) => map.borrow().keys().map(|k| Value::String(k.clone())).collect(),
                other => {
                    return Err(RuntimeError::new(format!("Нельзя итерировать по типу '{}'", other.type_name()), *fs));
                }
            };
            let iter_rc = Rc::new(RefCell::new(IteratorState::Array { values: keys, index: 0 }));
            interp.env.push_scope();
            interp.env.define(variable.name.clone(), Value::Undefined, false);
            g.frames.push(GenFrame::ForIter { var_name: variable.name.clone(), iter: iter_rc, body: body_stmts(body) });
            Ok(None)
        }
        Stmt::Return { value, .. } => {
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
        Stmt::Labeled { body, .. } => {
            push_body(g, &body_stmts(body));
            Ok(None)
        }
        Stmt::Break { label, .. } => {
            if label.is_some() {
                return Err(RuntimeError::new("Маркированный 'харэ' не поддерживается внутри генераторов", span));
            }
            if let Some(step) = unwind(interp, g, Unwind::Break, span)? {
                return Ok(Some(step));
            }
            Ok(None)
        }
        Stmt::Continue { label, .. } => {
            if label.is_some() {
                return Err(RuntimeError::new("Маркированный 'двигай' не поддерживается внутри генераторов", span));
            }
            if let Some(step) = unwind(interp, g, Unwind::Continue, span)? {
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
            let try_stmts: Rc<[Stmt]> = Rc::from(try_block.stmts.as_slice());
            g.frames.push(GenFrame::Block { stmts: try_stmts, idx: 0 });
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
                    if let Some(step) = unwind(interp, g, Unwind::Break, span)? {
                        return Ok(Some(step));
                    }
                    Ok(None)
                }
                Some(ControlFlow::Continue(_)) => {
                    if let Some(step) = unwind(interp, g, Unwind::Continue, span)? {
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
    kind: Unwind,
    span: Span,
) -> Result<Option<GenStep>, RuntimeError> {
    loop {
        let Some(top) = g.frames.last_mut() else {
            return match kind {
                Unwind::Throw(v) => Ok(Some(GenStep::Threw(v))),
                Unwind::Return(v) => Ok(Some(GenStep::Done(v))),
                Unwind::Break => Err(RuntimeError::new("'харэ' вне цикла", span)),
                Unwind::Continue => Err(RuntimeError::new("'двигай' вне цикла", span)),
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
                            g.frames.push(GenFrame::Block { stmts: cb, idx: 0 });
                            return Ok(None);
                        } else if let Some(fb) = finally_body.clone() {
                            *state = TryState::FinallyAfterThrow(v.clone());
                            interp.env.pop_scope();
                            g.frames.push(GenFrame::Block { stmts: fb, idx: 0 });
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
                            g.frames.push(GenFrame::Block { stmts: fb, idx: 0 });
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
                    if let Some(fb) = finally_body.clone() {
                        match state {
                            TryState::Trying => interp.env.pop_scope(),
                            TryState::InCatch => interp.env.pop_scope(),
                            _ => {}
                        }
                        *state = TryState::FinallyAfterReturn(v.clone());
                        g.frames.push(GenFrame::Block { stmts: fb, idx: 0 });
                        return Ok(None);
                    } else {
                        match state {
                            TryState::Trying => interp.env.pop_scope(),
                            TryState::InCatch => interp.env.pop_scope(),
                            _ => {}
                        }
                        g.frames.pop();
                        continue;
                    }
                }
                Unwind::Break => {
                    if let Some(fb) = finally_body.clone() {
                        match state {
                            TryState::Trying => interp.env.pop_scope(),
                            TryState::InCatch => interp.env.pop_scope(),
                            _ => {}
                        }
                        *state = TryState::FinallyAfterBreak;
                        g.frames.push(GenFrame::Block { stmts: fb, idx: 0 });
                        return Ok(None);
                    } else {
                        match state {
                            TryState::Trying => interp.env.pop_scope(),
                            TryState::InCatch => interp.env.pop_scope(),
                            _ => {}
                        }
                        g.frames.pop();
                        continue;
                    }
                }
                Unwind::Continue => {
                    if let Some(fb) = finally_body.clone() {
                        match state {
                            TryState::Trying => interp.env.pop_scope(),
                            TryState::InCatch => interp.env.pop_scope(),
                            _ => {}
                        }
                        *state = TryState::FinallyAfterContinue;
                        g.frames.push(GenFrame::Block { stmts: fb, idx: 0 });
                        return Ok(None);
                    } else {
                        match state {
                            TryState::Trying => interp.env.pop_scope(),
                            TryState::InCatch => interp.env.pop_scope(),
                            _ => {}
                        }
                        g.frames.pop();
                        continue;
                    }
                }
            },
            GenFrame::While { phase, .. } | GenFrame::DoWhile { phase, .. } => match &kind {
                Unwind::Break => {
                    g.frames.pop();
                    return Ok(None);
                }
                Unwind::Continue => {
                    *phase = LoopPhase::CheckCond;
                    return Ok(None);
                }
                _ => {
                    g.frames.pop();
                    continue;
                }
            },
            GenFrame::For { phase, .. } => match &kind {
                Unwind::Break => {
                    g.frames.pop();
                    interp.env.pop_scope();
                    return Ok(None);
                }
                Unwind::Continue => {
                    *phase = LoopPhase::AfterBody;
                    return Ok(None);
                }
                _ => {
                    g.frames.pop();
                    interp.env.pop_scope();
                    continue;
                }
            },
            GenFrame::ForIter { .. } => match &kind {
                Unwind::Break => {
                    g.frames.pop();
                    interp.env.pop_scope();
                    return Ok(None);
                }
                Unwind::Continue => {
                    return Ok(None);
                }
                _ => {
                    g.frames.pop();
                    interp.env.pop_scope();
                    continue;
                }
            },
            GenFrame::Block { .. } | GenFrame::Delegate { .. } => {
                g.frames.pop();
                continue;
            }
        }
    }
}

fn value_to_iterator(val: Value, span: Span) -> Result<Rc<RefCell<IteratorState>>, RuntimeError> {
    let state = match val {
        Value::Iterator(rc) => return Ok(rc),
        Value::Array(values) => IteratorState::Array { values: values.borrow().clone(), index: 0 },
        Value::String(s) => IteratorState::Chars { chars: s.chars().collect(), index: 0 },
        Value::Set(items) => {
            IteratorState::Array { values: items.borrow().iter().map(|k| k.as_value().clone()).collect(), index: 0 }
        }
        Value::Map(entries) => IteratorState::MapEntries {
            entries: entries.borrow().iter().map(|(k, v)| (k.as_value().clone(), v.clone())).collect(),
            index: 0,
        },
        Value::TypedArray { buffer, offset, length, kind } => IteratorState::Array {
            values: crate::stdlib::typed_array::ta_elements(&buffer, offset, length, kind),
            index: 0,
        },
        other => {
            return Err(RuntimeError::new(format!("Нельзя итерировать по типу '{}'", other.type_name()), span));
        }
    };
    Ok(Rc::new(RefCell::new(state)))
}
