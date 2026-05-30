use std::cell::RefCell;
use std::rc::Rc;

use yps_lexer::Span;
use yps_parser::ast::{Block, Expr};

use crate::builtins::call_builtin;
use crate::environment::{EnvFrame, Environment};
use crate::error::RuntimeError;
use crate::symbols;
use crate::value::Value;

use super::{ControlFlow, Interpreter};

impl Interpreter {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn call_method_returning_this(
        &mut self,
        name: Rc<str>,
        params: &[yps_parser::ast::Param],
        body: &Rc<Block>,
        env: &Rc<RefCell<EnvFrame>>,
        args: Vec<Value>,
        this_val: Value,
        span: Span,
    ) -> Result<(Value, Value), RuntimeError> {
        let saved_env = self.env.clone();
        self.env = Environment::from_snapshot(Rc::clone(env));
        self.env.push_scope();

        self.env.define(symbols::THIS.to_string(), this_val.clone(), false);

        if let Some(super_val) = saved_env.get(symbols::SUPER) {
            self.env.define(symbols::SUPER.to_string(), super_val, false);
        }

        for (i, param) in params.iter().enumerate() {
            if param.is_rest {
                let rest_start = i.min(args.len());
                let rest_values: Vec<Value> = args[rest_start..].to_vec();
                self.env.define(param.name.name.clone(), Value::Array(rest_values), false);
                break;
            }
            let value = if i < args.len() {
                args[i].clone()
            } else if let Some(default_expr) = &param.default {
                self.eval_expr(default_expr)?
            } else {
                Value::Undefined
            };
            self.env.define(param.name.name.clone(), value, false);
        }

        self.push_frame(name, span);
        let mut result = self.exec_block_stmts(&body.stmts);
        if let Err(e) = &mut result {
            e.attach_stack(self.snapshot_stack());
        }
        let frame_stack =
            if matches!(result, Ok(Some(ControlFlow::Throw(_)))) { self.snapshot_stack() } else { Vec::new() };
        let updated_this = self.env.get(symbols::THIS).unwrap_or(this_val);
        self.pop_frame();

        self.env = saved_env;

        match result? {
            Some(ControlFlow::Return(val)) => Ok((val, updated_this)),
            Some(ControlFlow::Break(label)) => Err(RuntimeError::new(
                label.map_or_else(|| "'харэ' вне цикла".to_string(), |l| format!("Метка '{l}' не найдена")),
                span,
            )),
            Some(ControlFlow::Continue(label)) => Err(RuntimeError::new(
                label.map_or_else(|| "'двигай' вне цикла".to_string(), |l| format!("Метка '{l}' не найдена")),
                span,
            )),
            Some(ControlFlow::Throw(val)) => Err(RuntimeError::thrown_with_stack(val, span, frame_stack)),
            None => Ok((Value::Undefined, updated_this)),
        }
    }

    pub(super) fn numeric_op(
        &self,
        left: &Value,
        right: &Value,
        span: Span,
        f: fn(f64, f64) -> f64,
    ) -> Result<Value, RuntimeError> {
        match (left, right) {
            (Value::Number(a), Value::Number(b)) => Ok(Value::Number(f(*a, *b))),
            _ => Err(RuntimeError::new(
                format!("Операция требует числа, получено '{}' и '{}'", left.type_name(), right.type_name()),
                span,
            )),
        }
    }

    pub(super) fn compare_op(
        &self,
        left: &Value,
        right: &Value,
        span: Span,
        f: fn(f64, f64) -> bool,
    ) -> Result<Value, RuntimeError> {
        match (left, right) {
            (Value::Number(a), Value::Number(b)) => Ok(Value::Boolean(f(*a, *b))),
            _ => Err(RuntimeError::new(
                format!("Сравнение требует числа, получено '{}' и '{}'", left.type_name(), right.type_name()),
                span,
            )),
        }
    }

    pub(crate) fn call_function(&mut self, func: Value, args: Vec<Value>, span: Span) -> Result<Value, RuntimeError> {
        if let Value::BuiltinFunction(ref bname) = func
            && bname == symbols::ADD_INITIALIZER_BUILTIN
        {
            if let Some(init_fn) = args.into_iter().next() {
                self.pending_initializers.push(init_fn);
                return Ok(Value::Undefined);
            }
            return Err(RuntimeError::new("добавитьИнициализатор ожидает функцию", span));
        }
        match func {
            Value::BuiltinFunction(name) => {
                if let Some(res) = self.try_call_timer_builtin(&name, args.clone(), span) {
                    return res;
                }
                if let Some(res) = crate::stdlib::call_static_namespaced(self, &name, args.clone(), span) {
                    return res;
                }
                call_builtin(&name, args, span)
            }
            Value::Function { name, params, body, env, is_generator, is_async } => {
                let required_count = params.iter().filter(|p| !p.is_rest && p.default.is_none()).count();

                if args.len() < required_count {
                    return Err(RuntimeError::new(
                        format!(
                            "Функция '{}' ожидает минимум {} аргумент(ов), получено {}",
                            name,
                            required_count,
                            args.len()
                        ),
                        span,
                    ));
                }

                let saved_env = self.env.clone();
                self.env = Environment::from_snapshot(env);
                self.env.push_scope();

                for (i, param) in params.iter().enumerate() {
                    if param.is_rest {
                        let rest_start = i.min(args.len());
                        let rest_values: Vec<Value> = args[rest_start..].to_vec();
                        self.env.define(param.name.name.clone(), Value::Array(rest_values), false);
                        break;
                    }

                    let value = if i < args.len() {
                        args[i].clone()
                    } else if let Some(default_expr) = &param.default {
                        self.eval_expr(default_expr)?
                    } else {
                        Value::Undefined
                    };
                    self.env.define(param.name.name.clone(), value, false);
                }

                if is_generator {
                    let gen_env = std::mem::replace(&mut self.env, saved_env);
                    let gen_state = super::generator::build_generator(name, gen_env, &body);
                    Ok(Value::Iterator(Rc::new(RefCell::new(crate::value::IteratorState::Generator(Box::new(
                        gen_state,
                    ))))))
                } else if is_async {
                    let async_env = self.env.snapshot();
                    self.env = saved_env;
                    let (outer, _resolve, _reject) = Self::make_pending_promise();
                    let outer_state = match &outer {
                        Value::Promise { state } => Rc::clone(state),
                        _ => unreachable!(),
                    };
                    let body_for_task = Rc::clone(&body);
                    let name_for_async = Rc::clone(&name);
                    self.enqueue_microtask(Box::new(move |interp, sp| {
                        let caller_env = interp.env.clone();
                        let saved_stack = std::mem::take(&mut interp.call_stack);
                        interp.env = Environment::from_snapshot(async_env);
                        interp.push_frame(name_for_async, sp);
                        let mut result = interp.exec_block_stmts(&body_for_task.stmts);
                        if let Err(e) = &mut result {
                            e.attach_stack(interp.snapshot_stack());
                        }
                        interp.pop_frame();
                        interp.call_stack = saved_stack;
                        interp.env = caller_env;
                        let (kind, value) = match result {
                            Ok(Some(ControlFlow::Return(val))) => (crate::value::CapKind::Resolve, val),
                            Ok(None) => (crate::value::CapKind::Resolve, Value::Undefined),
                            Ok(Some(ControlFlow::Throw(val))) => (crate::value::CapKind::Reject, val),
                            Ok(Some(ControlFlow::Break(label))) => {
                                return Err(RuntimeError::new(
                                    label.map_or_else(
                                        || "'харэ' вне цикла".to_string(),
                                        |l| format!("Метка '{l}' не найдена"),
                                    ),
                                    sp,
                                ));
                            }
                            Ok(Some(ControlFlow::Continue(label))) => {
                                return Err(RuntimeError::new(
                                    label.map_or_else(
                                        || "'двигай' вне цикла".to_string(),
                                        |l| format!("Метка '{l}' не найдена"),
                                    ),
                                    sp,
                                ));
                            }
                            Err(e) => match e.thrown {
                                Some(val) => (crate::value::CapKind::Reject, *val),
                                None => return Err(e),
                            },
                        };
                        Interpreter::settle_promise(&outer_state, kind, value, interp, sp)
                    }));
                    Ok(outer)
                } else {
                    self.push_frame(name, span);
                    let mut result = self.exec_block_stmts(&body.stmts);
                    if let Err(e) = &mut result {
                        e.attach_stack(self.snapshot_stack());
                    }
                    let frame_stack = if matches!(result, Ok(Some(ControlFlow::Throw(_)))) {
                        self.snapshot_stack()
                    } else {
                        Vec::new()
                    };
                    self.pop_frame();
                    self.env = saved_env;
                    match result? {
                        Some(ControlFlow::Return(val)) => Ok(val),
                        Some(ControlFlow::Break(label)) => Err(RuntimeError::new(
                            label.map_or_else(|| "'харэ' вне цикла".to_string(), |l| format!("Метка '{l}' не найдена")),
                            span,
                        )),
                        Some(ControlFlow::Continue(label)) => Err(RuntimeError::new(
                            label.map_or_else(
                                || "'двигай' вне цикла".to_string(),
                                |l| format!("Метка '{l}' не найдена"),
                            ),
                            span,
                        )),
                        Some(ControlFlow::Throw(val)) => Err(RuntimeError::thrown_with_stack(val, span, frame_stack)),
                        None => Ok(Value::Undefined),
                    }
                }
            }
            Value::PromiseCapability { state, kind } => {
                let val = args.into_iter().next().unwrap_or(Value::Undefined);
                Self::settle_promise(&state, kind, val, self, span)?;
                Ok(Value::Undefined)
            }
            Value::PromiseThenHandler { handler, resolve, reject, is_fulfill } => {
                let val = args.into_iter().next().unwrap_or(Value::Undefined);
                crate::stdlib::promise::invoke_handler(self, *handler, val, *resolve, *reject, is_fulfill, span)?;
                Ok(Value::Undefined)
            }
            Value::PromiseFinallyHandler { cb, cap } => {
                let val = args.into_iter().next().unwrap_or(Value::Undefined);
                self.call_function(*cb, vec![], span)?;
                self.call_function(*cap, vec![val], span)?;
                Ok(Value::Undefined)
            }
            Value::PromiseAggregateHandler { state, index, role } => {
                let val = args.into_iter().next().unwrap_or(Value::Undefined);
                crate::stdlib::promise::apply_aggregate(self, state, index, role, val, span)?;
                Ok(Value::Undefined)
            }
            Value::AbortUnsubscribe { state, token } => {
                if token != u64::MAX {
                    state.borrow_mut().listeners.retain(|(id, _)| *id != token);
                }
                Ok(Value::Undefined)
            }
            Value::AbortListener { target } => {
                let reason = target.borrow().reason.clone();
                crate::stdlib::abort::abort_state(&target, reason, self, span)?;
                Ok(Value::Undefined)
            }
            _ => Err(RuntimeError::new(format!("'{}' не является функцией", func.type_name()), span)),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn call_method_with_this(
        &mut self,
        name: Rc<str>,
        params: &[yps_parser::ast::Param],
        body: &Rc<Block>,
        env: &Rc<RefCell<EnvFrame>>,
        args: Vec<Value>,
        this_val: Option<Value>,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        let saved_env = self.env.clone();
        self.env = Environment::from_snapshot(Rc::clone(env));
        self.env.push_scope();

        if let Some(this) = &this_val {
            self.env.define(symbols::THIS.to_string(), this.clone(), false);
        }

        if let Some(super_val) = saved_env.get(symbols::SUPER) {
            self.env.define(symbols::SUPER.to_string(), super_val, false);
        }

        for (i, param) in params.iter().enumerate() {
            if param.is_rest {
                let rest_start = i.min(args.len());
                let rest_values: Vec<Value> = args[rest_start..].to_vec();
                self.env.define(param.name.name.clone(), Value::Array(rest_values), false);
                break;
            }
            let value = if i < args.len() {
                args[i].clone()
            } else if let Some(default_expr) = &param.default {
                self.eval_expr(default_expr)?
            } else {
                Value::Undefined
            };
            self.env.define(param.name.name.clone(), value, false);
        }

        self.push_frame(name, span);
        let mut result = self.exec_block_stmts(&body.stmts);
        if let Err(e) = &mut result {
            e.attach_stack(self.snapshot_stack());
        }
        let frame_stack =
            if matches!(result, Ok(Some(ControlFlow::Throw(_)))) { self.snapshot_stack() } else { Vec::new() };
        self.pop_frame();

        self.env = saved_env;

        match result? {
            Some(ControlFlow::Return(val)) => Ok(val),
            Some(ControlFlow::Break(label)) => Err(RuntimeError::new(
                label.map_or_else(|| "'харэ' вне цикла".to_string(), |l| format!("Метка '{l}' не найдена")),
                span,
            )),
            Some(ControlFlow::Continue(label)) => Err(RuntimeError::new(
                label.map_or_else(|| "'двигай' вне цикла".to_string(), |l| format!("Метка '{l}' не найдена")),
                span,
            )),
            Some(ControlFlow::Throw(val)) => Err(RuntimeError::thrown_with_stack(val, span, frame_stack)),
            None => Ok(Value::Undefined),
        }
    }

    pub(super) fn eval_index(&self, obj: Value, index: Value, span: Span) -> Result<Value, RuntimeError> {
        match (&obj, &index) {
            (Value::Array(arr), Value::Number(n)) => {
                let i = *n as usize;
                Ok(arr.get(i).cloned().unwrap_or(Value::Undefined))
            }
            (Value::Object(map), Value::String(key)) => Ok(map.get(key).cloned().unwrap_or(Value::Undefined)),
            (Value::Object(map), Value::Number(n)) => {
                Ok(map.get(&(*n as usize).to_string()).cloned().unwrap_or(Value::Undefined))
            }
            _ => Err(RuntimeError::new(
                format!("Нельзя индексировать '{}' с помощью '{}'", obj.type_name(), index.type_name()),
                span,
            )),
        }
    }

    pub(super) fn eval_args(&mut self, args: &[Expr]) -> Result<Vec<Value>, RuntimeError> {
        let mut values = Vec::with_capacity(args.len());
        for arg in args {
            if let Expr::Spread { expr, span } = arg {
                let val = self.eval_expr(expr)?;
                match val {
                    Value::Array(arr) => values.extend(arr),
                    Value::Set(s) => values.extend(s),
                    Value::String(s) => values.extend(s.chars().map(|c| Value::String(c.to_string()))),
                    Value::Iterator(rc) => {
                        values.extend(crate::stdlib::iterator::drain(self, &rc, *span)?);
                    }
                    _ => {
                        return Err(RuntimeError::new(
                            format!("Нельзя развернуть тип '{}' в аргументы", val.type_name()),
                            *span,
                        ));
                    }
                }
            } else {
                values.push(self.eval_expr(arg)?);
            }
        }
        Ok(values)
    }
}
