use std::cell::RefCell;
use std::rc::Rc;

use yps_lexer::Span;
use yps_parser::ast::{Block, Expr, Param};

use crate::builtins::call_builtin;
use crate::environment::{EnvFrame, Environment};
use crate::error::{Frame, RuntimeError};
use crate::symbols;
use crate::value::{ThenHandlerData, Value};

use super::{ControlFlow, GcRoot, Interpreter, coercion};

#[derive(Clone, Copy)]
pub(super) enum RelOp {
    Less,
    Greater,
    LessOrEqual,
    GreaterOrEqual,
}

impl Interpreter {
    pub(super) fn numeric_op(
        &mut self,
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
        &mut self,
        left: &Value,
        right: &Value,
        span: Span,
        op: RelOp,
    ) -> Result<Value, RuntimeError> {
        let lp = self.to_primitive(left, span)?;
        let rp = self.to_primitive(right, span)?;
        if let (Value::String(a), Value::String(b)) = (&lp, &rp) {
            let result = match op {
                RelOp::Less => a.as_bytes() < b.as_bytes(),
                RelOp::Greater => a.as_bytes() > b.as_bytes(),
                RelOp::LessOrEqual => a.as_bytes() <= b.as_bytes(),
                RelOp::GreaterOrEqual => a.as_bytes() >= b.as_bytes(),
            };
            return Ok(Value::Boolean(result));
        }
        let a = coercion::to_number(&lp);
        let b = coercion::to_number(&rp);
        if a.is_nan() || b.is_nan() {
            return Ok(Value::Boolean(false));
        }
        let result = match op {
            RelOp::Less => a < b,
            RelOp::Greater => a > b,
            RelOp::LessOrEqual => a <= b,
            RelOp::GreaterOrEqual => a >= b,
        };
        Ok(Value::Boolean(result))
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
                if let Some(res) = crate::host_callback::invoke(&name, args.clone(), span) {
                    return res;
                }
                if let Some(res) = self.try_call_timer_builtin(&name, args.clone(), span) {
                    return res;
                }
                if let Some(res) = crate::stdlib::call_static_namespaced(self, &name, args.clone(), span) {
                    return res;
                }
                call_builtin(&name, args, span)
            }
            Value::Function(func) => {
                let name = Rc::clone(&func.name);
                let params = Rc::clone(&func.params);
                let body = Rc::clone(&func.body);
                let env = Rc::clone(&func.env);
                let is_generator = func.is_generator;
                let is_async = func.is_async;
                drop(func);
                if self.call_stack.len() >= super::MAX_CALL_DEPTH {
                    return Err(RuntimeError::new("Превышена максимальная глубина рекурсии", span));
                }
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
                self.env = Environment::from_snapshot(env, self.env.registry());
                self.env.push_scope();

                self.bind_params(&params, &args, true, span)?;

                if is_generator {
                    let gen_env = std::mem::replace(&mut self.env, saved_env);
                    let gen_state = super::generator::build_generator(name, gen_env, &body, is_async);
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
                    let roots = vec![GcRoot::Frame(Rc::clone(&async_env)), GcRoot::Value(outer.clone())];
                    self.enqueue_microtask(
                        roots,
                        Box::new(move |interp, sp| {
                            let caller_env = interp.env.clone();
                            let saved_stack = std::mem::take(&mut interp.call_stack);
                            interp.env = Environment::from_snapshot(async_env, interp.env.registry());
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
                        }),
                    );
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
                    self.finish_call(result?, frame_stack, span)
                }
            }
            Value::PromiseCapability { state, kind } => {
                let val = args.into_iter().next().unwrap_or(Value::Undefined);
                Self::settle_promise(&state, kind, val, self, span)?;
                Ok(Value::Undefined)
            }
            Value::PromiseThenHandler(data) => {
                let val = args.into_iter().next().unwrap_or(Value::Undefined);
                let ThenHandlerData { handler, resolve, reject, is_fulfill } = *data;
                crate::stdlib::promise::invoke_handler(self, handler, val, resolve, reject, is_fulfill, span)?;
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
                if let Some(target) = target.upgrade() {
                    let reason = target.borrow().reason.clone();
                    crate::stdlib::abort::abort_state(&target, reason, self, span)?;
                }
                Ok(Value::Undefined)
            }
            Value::Proxy { target, handler } => self.proxy_apply(&target, &handler, args, span),
            Value::BoundMethod { receiver, method } => {
                crate::stdlib::call_method(self, *receiver, &method, args, span).map(|(ret, _)| ret)
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
        self.call_method_with_this_super(name, params, body, env, args, this_val, None, span)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn call_method_with_this_super(
        &mut self,
        name: Rc<str>,
        params: &[yps_parser::ast::Param],
        body: &Rc<Block>,
        env: &Rc<RefCell<EnvFrame>>,
        args: Vec<Value>,
        this_val: Option<Value>,
        super_class: Option<Rc<crate::value::ClassDef>>,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if self.call_stack.len() >= super::MAX_CALL_DEPTH {
            return Err(RuntimeError::new("Превышена максимальная глубина рекурсии", span));
        }
        let saved_env = self.env.clone();
        self.env = Environment::from_snapshot(Rc::clone(env), self.env.registry());
        self.env.push_scope();

        if let Some(this) = &this_val {
            self.env.define(symbols::THIS.to_string(), this.clone(), false);
        }

        if let Some(parent) = super_class {
            self.env.define(symbols::SUPER.to_string(), Value::Class(parent), false);
        } else if let Some(super_val) = saved_env.get(symbols::SUPER) {
            self.env.define(symbols::SUPER.to_string(), super_val, false);
        }

        self.bind_params(params, &args, true, span)?;

        self.push_frame(name, span);
        let mut result = self.exec_block_stmts(&body.stmts);
        if let Err(e) = &mut result {
            e.attach_stack(self.snapshot_stack());
        }
        let frame_stack =
            if matches!(result, Ok(Some(ControlFlow::Throw(_)))) { self.snapshot_stack() } else { Vec::new() };
        self.pop_frame();

        self.env = saved_env;

        self.finish_call(result?, frame_stack, span)
    }

    pub(super) fn eval_index(&mut self, obj: Value, index: Value, span: Span) -> Result<Value, RuntimeError> {
        match (&obj, &index) {
            (Value::Proxy { target, handler }, _) => {
                let target = Rc::clone(target);
                let handler = Rc::clone(handler);
                let key = index.to_string();
                self.proxy_get(&target, &handler, &key, obj.clone(), span)
            }
            (Value::Array(arr), Value::Number(n)) => {
                let i = *n as usize;
                Ok(arr.borrow().get(i).cloned().unwrap_or(Value::Undefined))
            }
            (Value::Object(map), Value::String(key)) => {
                Ok(map.borrow().get(key.as_ref()).cloned().unwrap_or(Value::Undefined))
            }
            (Value::Object(map), Value::Number(n)) => {
                Ok(map.borrow().get(&(*n as usize).to_string()).cloned().unwrap_or(Value::Undefined))
            }
            (Value::Object(map), Value::Symbol { id, .. }) => {
                let key = crate::symbols::symbol_key(*id);
                Ok(map.borrow().get(&key).cloned().unwrap_or(Value::Undefined))
            }
            (Value::String(s), Value::Number(n)) => {
                if !n.is_finite() || *n < 0.0 || n.fract() != 0.0 {
                    return Ok(Value::Undefined);
                }
                let i = *n as usize;
                let units: Vec<u16> = s.encode_utf16().collect();
                match units.get(i) {
                    Some(unit) => Ok(Value::String(String::from_utf16_lossy(&[*unit]).into())),
                    None => Ok(Value::Undefined),
                }
            }
            (Value::TypedArray(ta), Value::Number(n)) => {
                if !n.is_finite() || *n < 0.0 || n.fract() != 0.0 {
                    return Ok(Value::Undefined);
                }
                let i = *n as usize;
                if i >= ta.length {
                    return Ok(Value::Undefined);
                }
                let bytes = ta.buffer.borrow();
                Ok(Value::Number(ta.kind.read_le(&bytes, ta.offset + i * ta.kind.element_size())))
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
                let val = match val.proxy_parts() {
                    Some((target, _)) => (*target).clone(),
                    None => val,
                };
                match val {
                    Value::Array(arr) => values.extend(arr.borrow().iter().cloned()),
                    Value::Set(s) => values.extend(s.borrow().iter().map(|k| k.as_value().clone())),
                    Value::String(s) => values.extend(s.chars().map(|c| Value::String(c.to_string().into()))),
                    Value::TypedArray(ta) => {
                        values
                            .extend(crate::stdlib::typed_array::ta_elements(&ta.buffer, ta.offset, ta.length, ta.kind));
                    }
                    Value::Iterator(rc) => {
                        values.extend(crate::stdlib::iterator::drain(self, &rc, *span)?);
                    }
                    other => {
                        let iterator_obj = self.get_user_iterator(&other, *span)?;
                        match iterator_obj {
                            Some(iterator_obj) => {
                                values.extend(self.collect_user_iterable(iterator_obj, *span)?);
                            }
                            None => {
                                return Err(RuntimeError::new(
                                    format!("Нельзя развернуть тип '{}' в аргументы", other.type_name()),
                                    *span,
                                ));
                            }
                        }
                    }
                }
            } else {
                values.push(self.eval_expr(arg)?);
            }
        }
        Ok(values)
    }

    pub(super) fn bind_params(
        &mut self,
        params: &[Param],
        args: &[Value],
        destructure: bool,
        span: Span,
    ) -> Result<(), RuntimeError> {
        for (i, param) in params.iter().enumerate() {
            if param.is_rest {
                let rest_start = i.min(args.len());
                let rest_values: Vec<Value> = args[rest_start..].to_vec();
                self.env.define(param.name.name.clone(), Value::array(rest_values), false);
                break;
            }
            let value = if i < args.len() {
                args[i].clone()
            } else if let Some(default_expr) = &param.default {
                self.eval_expr(default_expr)?
            } else {
                Value::Undefined
            };
            if destructure && let Some(pat) = &param.pattern {
                self.destructure_pattern(pat, value, false, span)?;
            } else {
                self.env.define(param.name.name.clone(), value, false);
            }
        }
        Ok(())
    }

    pub(super) fn finish_call(
        &self,
        flow: Option<ControlFlow>,
        frame_stack: Vec<Frame>,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        match flow {
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
}
