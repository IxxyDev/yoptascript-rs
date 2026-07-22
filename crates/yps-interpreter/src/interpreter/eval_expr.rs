use std::rc::Rc;

use indexmap::IndexMap;

use yps_lexer::Span;
use yps_parser::ast::{BinaryOp, Expr, Literal, ObjectEntry, Param, PropKey, TemplatePart, UnaryOp};

use crate::environment::Environment;
use crate::error::RuntimeError;
use crate::symbols;
use crate::value::{ClassDef, MethodDef, Value, to_int_n, to_uint_n};

use super::Interpreter;
use super::call::RelOp;
use super::coercion;

impl Interpreter {
    pub(super) fn eval_expr(&mut self, expr: &Expr) -> Result<Value, RuntimeError> {
        stacker::maybe_grow(super::STACK_RED_ZONE, super::STACK_GROW_SIZE, || self.eval_expr_inner(expr))
    }

    fn call_super_method(
        &mut self,
        cls: &Rc<ClassDef>,
        property: &str,
        args: &[Expr],
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if let Some(MethodDef { params, body, env }) = Self::find_method_in_class(cls, property) {
            let params = params.clone();
            let body = Rc::clone(body);
            let env = Rc::clone(env);
            let super_class = Self::find_method_owner_parent(cls, property);
            let this_val = self.env.get(symbols::THIS);
            let arg_values = self.eval_args(args)?;
            return self.call_method_with_this_super(
                Rc::from(property),
                &params,
                &body,
                &env,
                arg_values,
                this_val,
                super_class,
                span,
            );
        }
        Err(RuntimeError::new(format!("'{property}' не является методом родительского класса"), span))
    }

    fn call_object_method(
        &mut self,
        obj: Value,
        func: &Value,
        property: &str,
        arg_values: Vec<Value>,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        let (Value::Object(map), Value::Function { params, body, env, .. }) = (&obj, func) else {
            return self.call_function(func.clone(), arg_values, span);
        };
        let super_class = Self::resolve_class_for_object(map, &self.env)
            .and_then(|cls| Self::find_method_owner_parent(&cls, property));
        self.call_method_with_this_super(
            Rc::from(property),
            params,
            body,
            env,
            arg_values,
            Some(obj.clone()),
            super_class,
            span,
        )
    }

    fn call_class_method(
        &mut self,
        obj: Value,
        func: &Value,
        property: &str,
        arg_values: Vec<Value>,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        let (Value::Class(cls), Value::Function { params, body, env, .. }) = (&obj, func) else {
            return self.call_function(func.clone(), arg_values, span);
        };
        let super_class = Self::find_static_method_owner_parent(cls, property);
        self.call_method_with_this_super(
            Rc::from(property),
            params,
            body,
            env,
            arg_values,
            Some(obj.clone()),
            super_class,
            span,
        )
    }

    pub(super) fn eval_prop_key(&mut self, key: &PropKey) -> Result<String, RuntimeError> {
        Ok(match key {
            PropKey::Identifier(ident) => ident.name.clone(),
            PropKey::Computed(expr) => {
                let k = self.eval_expr(expr)?;
                if let Value::Symbol { id, .. } = &k { crate::symbols::symbol_key(*id) } else { k.to_string() }
            }
        })
    }

    fn eval_expr_inner(&mut self, expr: &Expr) -> Result<Value, RuntimeError> {
        match expr {
            Expr::Literal(lit) => self.eval_literal(lit),
            Expr::Identifier(ident) => self
                .env
                .get(&ident.name)
                .ok_or_else(|| RuntimeError::new(format!("Переменная '{}' не определена", ident.name), ident.span)),
            Expr::Unary { op, expr, span } => {
                if *op == UnaryOp::Typeof {
                    if let Expr::Identifier(ident) = expr.as_ref() {
                        let val = self.env.get(&ident.name).unwrap_or(Value::Undefined);
                        return Ok(Value::String(val.typeof_str().to_string().into()));
                    }
                    let val = self.eval_expr(expr)?;
                    return Ok(Value::String(val.typeof_str().to_string().into()));
                }
                if *op == UnaryOp::Delete {
                    return self.eval_delete(expr, *span);
                }
                if *op == UnaryOp::Void {
                    self.eval_expr(expr)?;
                    return Ok(Value::Undefined);
                }
                let val = self.eval_expr(expr)?;
                self.eval_unary(*op, val, *span)
            }
            Expr::Binary { op, lhs, rhs, span } => {
                if *op == BinaryOp::And {
                    let left = self.eval_expr(lhs)?;
                    if !left.is_truthy() {
                        return Ok(left);
                    }
                    return self.eval_expr(rhs);
                }
                if *op == BinaryOp::Or {
                    let left = self.eval_expr(lhs)?;
                    if left.is_truthy() {
                        return Ok(left);
                    }
                    return self.eval_expr(rhs);
                }
                if *op == BinaryOp::NullishCoalescing {
                    let left = self.eval_expr(lhs)?;
                    if !matches!(left, Value::Null | Value::Undefined) {
                        return Ok(left);
                    }
                    return self.eval_expr(rhs);
                }
                if *op == BinaryOp::NullishAssign {
                    return self
                        .eval_logical_assign(lhs, rhs, *span, |left| matches!(left, Value::Null | Value::Undefined));
                }
                if *op == BinaryOp::AndAssign {
                    return self.eval_logical_assign(lhs, rhs, *span, |left| left.is_truthy());
                }
                if *op == BinaryOp::OrAssign {
                    return self.eval_logical_assign(lhs, rhs, *span, |left| !left.is_truthy());
                }
                if *op == BinaryOp::Pipeline {
                    let left = self.eval_expr(lhs)?;
                    let func = self.eval_expr(rhs)?;
                    return self.call_function(func, vec![left], *span);
                }
                if *op == BinaryOp::Assign {
                    return self.eval_assignment(lhs, rhs, *span);
                }
                if matches!(
                    op,
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
                        | BinaryOp::UshrAssign
                ) {
                    return self.eval_compound_assignment(*op, lhs, rhs, *span);
                }
                let left = self.eval_expr(lhs)?;
                let right = self.eval_expr(rhs)?;
                self.eval_binary(*op, left, right, *span)
            }
            Expr::Assignment { target, value, span } => {
                let val = self.eval_expr(value)?;
                self.set_variable(&target.name, val.clone(), *span)?;
                Ok(val)
            }
            Expr::Postfix { op, expr, span } => self.eval_postfix(*op, expr, *span),
            Expr::Grouping { expr, .. } => self.eval_expr(expr),
            Expr::Call { callee, args, span } => {
                if let Expr::Member { object, property, .. } = callee.as_ref() {
                    let obj = self.eval_expr(object)?;
                    let obj = match obj.proxy_parts() {
                        Some((target, handler))
                            if crate::stdlib::proxy::trap(&handler, crate::stdlib::proxy::GET).is_none() =>
                        {
                            (*target).clone()
                        }
                        _ => obj,
                    };
                    if let Expr::Super { .. } = object.as_ref()
                        && let Value::Class(cls) = &obj
                    {
                        return self.call_super_method(cls, &property.name, args, *span);
                    }
                    if crate::stdlib::has_builtin_methods(&obj) {
                        let arg_values = self.eval_args(args)?;
                        let (ret, new_receiver) =
                            crate::stdlib::call_method(self, obj, &property.name, arg_values, *span)?;
                        if let Some(new) = new_receiver {
                            self.write_back_object(object, new, *span)?;
                        }
                        return Ok(ret);
                    }
                    let func = self.eval_member(obj.clone(), &property.name, *span)?;
                    let arg_values = self.eval_args(args)?;
                    match &obj {
                        Value::Object(_) => {
                            self.call_object_method(obj.clone(), &func, &property.name, arg_values, *span)
                        }
                        Value::Class(cls) if Self::find_static_method_in_class(cls, &property.name).is_some() => {
                            self.call_class_method(obj.clone(), &func, &property.name, arg_values, *span)
                        }
                        _ => self.call_function(func, arg_values, *span),
                    }
                } else if let Expr::Super { span: super_span } = callee.as_ref() {
                    let super_val = self.env.get(symbols::SUPER).ok_or_else(|| {
                        RuntimeError::new("'яга' (super) используется вне класса-наследника", *super_span)
                    })?;
                    if let Value::Class(cls) = &super_val
                        && let Some(MethodDef { params, body, env }) = &cls.constructor
                    {
                        let arg_values = self.eval_args(args)?;
                        let this_val = self.env.get(symbols::THIS);
                        let grandparent = cls.parent.clone();
                        return self.call_method_with_this_super(
                            Rc::from("<конструктор>"),
                            params,
                            body,
                            env,
                            arg_values,
                            this_val,
                            grandparent,
                            *span,
                        );
                    }
                    Err(RuntimeError::new("Родительский класс не имеет конструктора", *span))
                } else {
                    let func = self.eval_expr(callee)?;
                    let arg_values = self.eval_args(args)?;
                    self.call_function(func, arg_values, *span)
                }
            }
            Expr::Index { object, index, span } => {
                let obj = self.eval_expr(object)?;
                let idx = self.eval_expr(index)?;
                self.eval_index(obj, idx, *span)
            }
            Expr::Member { object, property, span } => {
                let obj = self.eval_expr(object)?;
                self.eval_member(obj, &property.name, *span)
            }
            Expr::OptionalMember { object, property, span } => {
                let obj = self.eval_expr(object)?;
                if matches!(obj, Value::Null | Value::Undefined) {
                    Ok(Value::Undefined)
                } else {
                    self.eval_member(obj, &property.name, *span)
                }
            }
            Expr::OptionalIndex { object, index, span } => {
                let obj = self.eval_expr(object)?;
                if matches!(obj, Value::Null | Value::Undefined) {
                    Ok(Value::Undefined)
                } else {
                    let idx = self.eval_expr(index)?;
                    self.eval_index(obj, idx, *span)
                }
            }
            Expr::OptionalCall { callee, args, span } => {
                let func = self.eval_expr(callee)?;
                if matches!(func, Value::Null | Value::Undefined) {
                    Ok(Value::Undefined)
                } else {
                    let arg_values = self.eval_args(args)?;
                    self.call_function(func, arg_values, *span)
                }
            }
            Expr::Conditional { condition, then_expr, else_expr, .. } => {
                let cond = self.eval_expr(condition)?;
                if cond.is_truthy() { self.eval_expr(then_expr) } else { self.eval_expr(else_expr) }
            }
            Expr::ArrowFunction { params, body, is_async, .. } => {
                let func = Value::Function {
                    name: Rc::from(""),
                    params: params.clone(),
                    body: body.clone(),
                    env: self.env.snapshot(),
                    is_generator: false,
                    is_async: *is_async,
                };
                Ok(func)
            }
            Expr::FunctionExpr { name, params, body, is_generator, is_async, .. } => match name {
                Some(ident) => {
                    let mut fn_env = Environment::from_snapshot(self.env.snapshot(), self.env.registry());
                    fn_env.push_scope();
                    let func = Value::Function {
                        name: Rc::from(ident.name.as_str()),
                        params: params.clone(),
                        body: body.clone(),
                        env: fn_env.snapshot(),
                        is_generator: *is_generator,
                        is_async: *is_async,
                    };
                    fn_env.define(ident.name.clone(), func.clone(), false);
                    Ok(func)
                }
                None => Ok(Value::Function {
                    name: Rc::from(""),
                    params: params.clone(),
                    body: body.clone(),
                    env: self.env.snapshot(),
                    is_generator: *is_generator,
                    is_async: *is_async,
                }),
            },
            Expr::Await { argument, span } => {
                let val = self.eval_expr(argument)?;
                self.do_await(val, *span)
            }
            Expr::TemplateLiteral { parts, .. } => {
                let mut result = String::new();
                for part in parts {
                    match part {
                        TemplatePart::Str(s) => result.push_str(s),
                        TemplatePart::Expr(expr) => {
                            let val = self.eval_expr(expr)?;
                            result.push_str(&val.to_string());
                        }
                    }
                }
                Ok(Value::String(result.into()))
            }
            Expr::TaggedTemplate { tag, quasis, expressions, span } => {
                let mut strings_map = IndexMap::new();
                let mut raw_vec = Vec::with_capacity(quasis.len());
                for (i, q) in quasis.iter().enumerate() {
                    strings_map.insert(i.to_string(), Value::String(q.cooked.clone().into()));
                    raw_vec.push(Value::String(q.raw.clone().into()));
                }
                let len = Value::Number(quasis.len() as f64);
                strings_map.insert("длина".to_string(), len.clone());
                strings_map.insert("length".to_string(), len);
                let raw_arr = Value::array(raw_vec);
                strings_map.insert("сырьё".to_string(), raw_arr.clone());
                strings_map.insert("raw".to_string(), raw_arr);

                let mut args = Vec::with_capacity(expressions.len() + 1);
                args.push(Value::object(strings_map));
                for e in expressions {
                    args.push(self.eval_expr(e)?);
                }

                let func = self.eval_expr(tag)?;
                self.call_function(func, args, *span)
            }
            Expr::Spread { span, .. } => Err(RuntimeError::new(
                "Оператор '...' допустим только в массивах, объектах или аргументах вызова",
                *span,
            )),
            Expr::This { span } => self
                .env
                .get(symbols::THIS)
                .ok_or_else(|| RuntimeError::new("'тырыпыры' (this) используется вне контекста объекта", *span)),
            Expr::New { callee, args, span } => {
                let class_val = self.eval_expr(callee)?;
                let arg_values = self.eval_args(args)?;
                self.construct_instance(class_val, arg_values, *span)
            }
            Expr::Super { span } => self
                .env
                .get(symbols::SUPER)
                .ok_or_else(|| RuntimeError::new("'яга' (super) используется вне класса-наследника", *span)),
            Expr::Yield { span, .. } => Err(RuntimeError::new(
                "'поебалу' разрешён только как самостоятельный оператор или правая часть присваивания/декларации внутри 'пиздюли'",
                *span,
            )),
            Expr::DynamicImport { source, span } => {
                let source_val = self.eval_expr(source)?;
                let path = match source_val {
                    Value::String(s) => s,
                    other => {
                        return Ok(Self::make_rejected_promise(Value::String(
                            format!(
                                "Аргумент динамического импорта должен быть строкой, получено '{}'",
                                other.type_name()
                            )
                            .into(),
                        )));
                    }
                };
                match self.load_module(&path, *span) {
                    Ok(exports) => {
                        let mut map = IndexMap::new();
                        for (k, v) in exports {
                            map.insert(k, v);
                        }
                        Ok(Self::make_fulfilled_promise(Value::object(map)))
                    }
                    Err(err) => Ok(Self::make_rejected_promise(Value::String(err.message.into()))),
                }
            }
        }
    }

    fn eval_literal(&mut self, lit: &Literal) -> Result<Value, RuntimeError> {
        match lit {
            Literal::Number { raw, span } => {
                let n = coercion::string_to_number(&raw.replace('_', ""));
                if n.is_nan() {
                    Err(RuntimeError::new(format!("Невалидное число: '{raw}'"), *span))
                } else {
                    Ok(Value::Number(n))
                }
            }
            Literal::BigInt { value, .. } => Ok(Value::BigInt(*value)),
            Literal::String { value, .. } => Ok(Value::String(value.clone().into())),
            Literal::Boolean { value, .. } => Ok(Value::Boolean(*value)),
            Literal::Null { .. } => Ok(Value::Null),
            Literal::Undefined { .. } => Ok(Value::Undefined),
            Literal::Array { elements, .. } => {
                let mut values = Vec::with_capacity(elements.len());
                for el in elements {
                    if let Expr::Spread { expr, span } = el {
                        let val = self.eval_expr(expr)?;
                        let val = match val.proxy_parts() {
                            Some((target, _)) => (*target).clone(),
                            None => val,
                        };
                        match val {
                            Value::Array(arr) => values.extend(arr.borrow().iter().cloned()),
                            Value::Set(s) => values.extend(s.borrow().iter().map(|k| k.as_value().clone())),
                            Value::Map(entries) => {
                                values.extend(
                                    entries
                                        .borrow()
                                        .iter()
                                        .map(|(k, v)| Value::array(vec![k.as_value().clone(), v.clone()])),
                                );
                            }
                            Value::String(s) => values.extend(s.chars().map(|c| Value::String(c.to_string().into()))),
                            Value::TypedArray { buffer, offset, length, kind } => {
                                values.extend(crate::stdlib::typed_array::ta_elements(&buffer, offset, length, kind));
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
                                            format!("Нельзя развернуть тип '{}' в массив", other.type_name()),
                                            *span,
                                        ));
                                    }
                                }
                            }
                        }
                    } else {
                        values.push(self.eval_expr(el)?);
                    }
                }
                Ok(Value::array(values))
            }
            Literal::Object { entries, span } => {
                let mut map = IndexMap::new();
                for entry in entries {
                    match entry {
                        ObjectEntry::Property { key, value } => {
                            let key_str = self.eval_prop_key(key)?;
                            let val = self.eval_expr(value)?;
                            map.insert(key_str, val);
                        }
                        ObjectEntry::Spread(expr) => {
                            let val = self.eval_expr(expr)?;
                            if let Value::Proxy { target, handler } = &val {
                                let (t, h) = ((**target).clone(), (**handler).clone());
                                let keys = self.proxy_own_keys(&t, &h, *span)?;
                                for k in keys {
                                    let ks = k.to_string();
                                    let v = self.proxy_get(&t, &h, &ks, val.clone(), *span)?;
                                    map.insert(ks, v);
                                }
                                continue;
                            }
                            match val {
                                Value::Object(src) => {
                                    for (k, v) in
                                        src.borrow().iter().filter(|(k, _)| !crate::symbols::is_internal_key(k))
                                    {
                                        map.insert(k.clone(), v.clone());
                                    }
                                }
                                other => {
                                    return Err(RuntimeError::new(
                                        format!("Нельзя развернуть тип '{}' в объект", other.type_name()),
                                        *span,
                                    ));
                                }
                            }
                        }
                        ObjectEntry::Getter { key, body, .. } => {
                            let key_str = self.eval_prop_key(key)?;
                            let getter_fn = Value::Function {
                                name: Rc::from(format!("get {key_str}").as_str()),
                                params: Rc::from([] as [Param; 0]),
                                body: Rc::new(body.clone()),
                                env: self.env.snapshot(),
                                is_generator: false,
                                is_async: false,
                            };
                            map.insert(symbols::getter_key(&key_str), getter_fn);
                        }
                        ObjectEntry::Setter { key, param, body, .. } => {
                            let key_str = self.eval_prop_key(key)?;
                            let setter_fn = Value::Function {
                                name: Rc::from(format!("set {key_str}").as_str()),
                                params: Rc::from([param.clone()]),
                                body: Rc::new(body.clone()),
                                env: self.env.snapshot(),
                                is_generator: false,
                                is_async: false,
                            };
                            map.insert(symbols::setter_key(&key_str), setter_fn);
                        }
                    }
                }
                Ok(Value::object(map))
            }
            Literal::RegExp { pattern, flags, span } => {
                let compiled = crate::stdlib::regexp::compile(pattern, flags, *span)?;
                Ok(Value::RegExp {
                    pattern: pattern.clone(),
                    flags: flags.clone(),
                    compiled,
                    last_index: std::rc::Rc::new(std::cell::RefCell::new(0)),
                })
            }
        }
    }

    fn eval_unary(&self, op: UnaryOp, val: Value, span: Span) -> Result<Value, RuntimeError> {
        match op {
            UnaryOp::Minus => match val {
                Value::BigInt(n) => {
                    n.checked_neg().map(Value::BigInt).ok_or_else(|| RuntimeError::new("Переполнение бигцелого", span))
                }
                _ => Ok(Value::Number(-coercion::to_number(&val))),
            },
            UnaryOp::Plus => match val {
                Value::BigInt(_) => Err(RuntimeError::new("Нельзя применить унарный '+' к бигцелому", span)),
                _ => Ok(Value::Number(coercion::to_number(&val))),
            },
            UnaryOp::Not => Ok(Value::Boolean(!val.is_truthy())),
            UnaryOp::BitwiseNot => {
                let n = match &val {
                    Value::Number(n) => *n,
                    _ => {
                        return Err(RuntimeError::new(
                            format!("Нельзя применить '~' к типу '{}'", val.type_name()),
                            span,
                        ));
                    }
                };
                Ok(Value::Number(!(to_int_n(n, 32) as i32) as f64))
            }
            UnaryOp::Typeof => Ok(Value::String(val.typeof_str().to_string().into())),
            UnaryOp::Delete => Ok(Value::Boolean(true)),
            UnaryOp::Void => Ok(Value::Undefined),
        }
    }

    pub(super) fn eval_binary(
        &mut self,
        op: BinaryOp,
        left: Value,
        right: Value,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if matches!(
            op,
            BinaryOp::Add
                | BinaryOp::Sub
                | BinaryOp::Mul
                | BinaryOp::Div
                | BinaryOp::Mod
                | BinaryOp::Exp
                | BinaryOp::Less
                | BinaryOp::Greater
                | BinaryOp::LessOrEqual
                | BinaryOp::GreaterOrEqual
        ) {
            if let (Value::BigInt(a), Value::BigInt(b)) = (&left, &right) {
                return Self::bigint_op(op, *a, *b, span);
            }
            if matches!(left, Value::BigInt(_)) ^ matches!(right, Value::BigInt(_)) {
                return Err(RuntimeError::new(
                    format!("Нельзя смешивать '{}' и '{}' в одной операции", left.type_name(), right.type_name()),
                    span,
                ));
            }
        }

        match op {
            BinaryOp::Add => self.add_values(&left, &right, span),
            BinaryOp::Sub => self.numeric_op(&left, &right, span, |a, b| a - b),
            BinaryOp::Mul => self.numeric_op(&left, &right, span, |a, b| a * b),
            BinaryOp::Div => self.numeric_op(&left, &right, span, |a, b| a / b),
            BinaryOp::Mod => self.numeric_op(&left, &right, span, |a, b| a % b),
            BinaryOp::Exp => self.numeric_op(&left, &right, span, |a, b| a.powf(b)),
            BinaryOp::StrictEquals => Ok(Value::Boolean(left == right)),
            BinaryOp::StrictNotEquals => Ok(Value::Boolean(left != right)),
            BinaryOp::Equals => Ok(Value::Boolean(self.abstract_equals(&left, &right, span)?)),
            BinaryOp::NotEquals => Ok(Value::Boolean(!self.abstract_equals(&left, &right, span)?)),
            BinaryOp::Less => self.compare_op(&left, &right, span, RelOp::Less),
            BinaryOp::Greater => self.compare_op(&left, &right, span, RelOp::Greater),
            BinaryOp::LessOrEqual => self.compare_op(&left, &right, span, RelOp::LessOrEqual),
            BinaryOp::GreaterOrEqual => self.compare_op(&left, &right, span, RelOp::GreaterOrEqual),
            BinaryOp::Pipeline => unreachable!("handled in eval_expr"),
            BinaryOp::Instanceof => {
                let right_class = match &right {
                    Value::Class(cls) => Rc::clone(cls),
                    _ => {
                        return Err(RuntimeError::new(
                            format!("Правая сторона 'шкура' должна быть классом, получено '{}'", right.type_name()),
                            span,
                        ));
                    }
                };
                if let Value::Proxy { target, handler } = &left {
                    let (t, h) = ((**target).clone(), (**handler).clone());
                    if crate::stdlib::proxy::trap(&h, crate::stdlib::proxy::GET_PROTOTYPE_OF).is_some() {
                        let proto = self.proxy_get_prototype_of(&t, &h, span)?;
                        return Ok(Value::Boolean(self.instance_of_check(&proto, &right_class)));
                    }
                    return Ok(Value::Boolean(self.instance_of_check(&t, &right_class)));
                }
                Ok(Value::Boolean(self.instance_of_check(&left, &right_class)))
            }
            BinaryOp::In => match right {
                Value::Proxy { target, handler } => {
                    let key = coercion::to_ecma_string(&left);
                    Ok(Value::Boolean(self.proxy_has(&target, &handler, &key, span)?))
                }
                Value::Object(map) => {
                    let key = coercion::to_ecma_string(&left);
                    Ok(Value::Boolean(map.borrow().contains_key(&key)))
                }
                Value::Array(arr) => {
                    let len = arr.borrow().len();
                    let contains = match &left {
                        Value::Number(n) => n.fract() == 0.0 && *n >= 0.0 && (*n as usize) < len,
                        other => {
                            let key = coercion::to_ecma_string(other);
                            match key.parse::<usize>() {
                                Ok(idx) => idx < len,
                                Err(_) => false,
                            }
                        }
                    };
                    Ok(Value::Boolean(contains))
                }
                _ => Err(RuntimeError::new(
                    format!("Правая сторона 'из' должна быть объектом или массивом, получено '{}'", right.type_name()),
                    span,
                )),
            },
            BinaryOp::BitAnd => {
                let a = to_int_n(coercion::to_number(&left), 32) as i32;
                let b = to_int_n(coercion::to_number(&right), 32) as i32;
                Ok(Value::Number((a & b) as f64))
            }
            BinaryOp::BitOr => {
                let a = to_int_n(coercion::to_number(&left), 32) as i32;
                let b = to_int_n(coercion::to_number(&right), 32) as i32;
                Ok(Value::Number((a | b) as f64))
            }
            BinaryOp::BitXor => {
                let a = to_int_n(coercion::to_number(&left), 32) as i32;
                let b = to_int_n(coercion::to_number(&right), 32) as i32;
                Ok(Value::Number((a ^ b) as f64))
            }
            BinaryOp::LeftShift => {
                let a = to_int_n(coercion::to_number(&left), 32) as i32;
                let b = (to_uint_n(coercion::to_number(&right), 32) & 0x1f) as u32;
                Ok(Value::Number((a << b) as f64))
            }
            BinaryOp::RightShift => {
                let a = to_int_n(coercion::to_number(&left), 32) as i32;
                let b = (to_uint_n(coercion::to_number(&right), 32) & 0x1f) as u32;
                Ok(Value::Number((a >> b) as f64))
            }
            BinaryOp::UnsignedRightShift => {
                let a = to_uint_n(coercion::to_number(&left), 32) as u32;
                let b = (to_uint_n(coercion::to_number(&right), 32) & 0x1f) as u32;
                Ok(Value::Number((a >> b) as f64))
            }
            BinaryOp::And
            | BinaryOp::Or
            | BinaryOp::NullishCoalescing
            | BinaryOp::NullishAssign
            | BinaryOp::AndAssign
            | BinaryOp::OrAssign => {
                unreachable!("handled in eval_expr")
            }
            BinaryOp::Assign
            | BinaryOp::PlusAssign
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
            | BinaryOp::UshrAssign => unreachable!("handled in eval_expr"),
        }
    }

    fn abstract_equals(&mut self, left: &Value, right: &Value, span: Span) -> Result<bool, RuntimeError> {
        match (left, right) {
            (Value::Null | Value::Undefined, Value::Null | Value::Undefined) => Ok(true),

            (Value::Number(a), Value::Number(b)) => Ok(a == b),
            (Value::BigInt(a), Value::BigInt(b)) => Ok(a == b),
            (Value::String(a), Value::String(b)) => Ok(a == b),
            (Value::Boolean(a), Value::Boolean(b)) => Ok(a == b),
            (Value::Symbol { id: a, .. }, Value::Symbol { id: b, .. }) => Ok(a == b),

            (Value::Number(a), Value::String(b)) => Ok(*a == coercion::string_to_number(b)),
            (Value::String(a), Value::Number(b)) => Ok(coercion::string_to_number(a) == *b),

            (Value::BigInt(a), Value::String(b)) => Ok(bigint_eq_str(*a, b)),
            (Value::String(a), Value::BigInt(b)) => Ok(bigint_eq_str(*b, a)),

            (Value::BigInt(a), Value::Number(b)) => Ok(bigint_eq_number(*a, *b)),
            (Value::Number(a), Value::BigInt(b)) => Ok(bigint_eq_number(*b, *a)),

            (Value::Boolean(_), _) => {
                let coerced = Value::Number(coercion::to_number(left));
                self.abstract_equals(&coerced, right, span)
            }
            (_, Value::Boolean(_)) => {
                let coerced = Value::Number(coercion::to_number(right));
                self.abstract_equals(left, &coerced, span)
            }

            (Value::Number(_) | Value::String(_) | Value::BigInt(_) | Value::Symbol { .. }, _)
                if is_object_like(right) =>
            {
                let prim = self.to_primitive(right, span)?;
                self.abstract_equals(left, &prim, span)
            }
            (_, Value::Number(_) | Value::String(_) | Value::BigInt(_) | Value::Symbol { .. })
                if is_object_like(left) =>
            {
                let prim = self.to_primitive(left, span)?;
                self.abstract_equals(&prim, right, span)
            }

            _ => Ok(left == right),
        }
    }

    #[allow(clippy::wrong_self_convention)]
    pub(super) fn to_primitive(&mut self, value: &Value, span: Span) -> Result<Value, RuntimeError> {
        if !matches!(value, Value::Object(_)) {
            return Ok(coercion::to_primitive_builtin(value));
        }

        if self.coercion_depth >= super::MAX_COERCION_DEPTH {
            return Err(RuntimeError::new("Превышена глубина коэрции в примитив", span));
        }
        self.coercion_depth += 1;
        let result = self.object_to_primitive(value, span);
        self.coercion_depth -= 1;
        result
    }

    fn object_to_primitive(&mut self, value: &Value, span: Span) -> Result<Value, RuntimeError> {
        let to_primitive_arg = vec![Value::String("умолчание".into())];
        let to_primitive_sym = symbols::symbol_key(crate::stdlib::symbol::TO_PRIMITIVE_ID);
        for hook in [to_primitive_sym.as_str(), symbols::TO_PRIMITIVE_METHOD] {
            if let Some(res) = self.try_call_object_method(value, hook, to_primitive_arg.clone(), span)? {
                if coercion::is_primitive(&res) {
                    return Ok(res);
                }
                return Err(RuntimeError::new("'вПримитив' вернул не примитив", span));
            }
        }

        let mut had_method = false;
        for method in [symbols::VALUE_OF_METHOD, symbols::TO_STRING_METHOD] {
            if let Some(res) = self.try_call_object_method(value, method, Vec::new(), span)? {
                had_method = true;
                if coercion::is_primitive(&res) {
                    return Ok(res);
                }
            }
        }

        if had_method {
            return Err(RuntimeError::new("Не удалось привести объект к примитиву", span));
        }

        if let Some(tag) = self.to_string_tag(value) {
            return Ok(Value::String(format!("[object {tag}]").into()));
        }

        Ok(coercion::to_primitive_builtin(value))
    }

    fn to_string_tag(&self, value: &Value) -> Option<String> {
        let Value::Object(map) = value else {
            return None;
        };
        let key = symbols::symbol_key(crate::stdlib::symbol::TO_STRING_TAG_ID);
        match map.borrow().get(&key) {
            Some(Value::String(tag)) => Some(tag.to_string()),
            _ => None,
        }
    }

    fn try_call_object_method(
        &mut self,
        receiver: &Value,
        method: &str,
        args: Vec<Value>,
        span: Span,
    ) -> Result<Option<Value>, RuntimeError> {
        let Value::Object(map) = receiver else {
            return Ok(None);
        };
        let func = map.borrow().get(method).cloned();
        let Some(Value::Function { name, params, body, env, .. }) = func else {
            return Ok(None);
        };
        let res = self.call_method_with_this(name, &params, &body, &env, args, Some(receiver.clone()), span)?;
        Ok(Some(res))
    }

    fn add_values(&mut self, left: &Value, right: &Value, span: Span) -> Result<Value, RuntimeError> {
        let lp = self.to_primitive(left, span)?;
        let rp = self.to_primitive(right, span)?;
        if matches!(lp, Value::String(_)) || matches!(rp, Value::String(_)) {
            let mut s = coercion::to_ecma_string(&lp);
            s.push_str(&coercion::to_ecma_string(&rp));
            return Ok(Value::String(s.into()));
        }
        Ok(Value::Number(coercion::to_number(&lp) + coercion::to_number(&rp)))
    }

    fn bigint_op(op: BinaryOp, a: i128, b: i128, span: Span) -> Result<Value, RuntimeError> {
        let checked = match op {
            BinaryOp::Add => a.checked_add(b),
            BinaryOp::Sub => a.checked_sub(b),
            BinaryOp::Mul => a.checked_mul(b),
            BinaryOp::Div => {
                if b == 0 {
                    return Err(RuntimeError::new("Деление на ноль", span));
                }
                a.checked_div(b)
            }
            BinaryOp::Mod => {
                if b == 0 {
                    return Err(RuntimeError::new("Деление на ноль", span));
                }
                a.checked_rem(b)
            }
            BinaryOp::Exp => {
                if b < 0 {
                    return Err(RuntimeError::new("Отрицательный показатель степени у бигцелого", span));
                }
                if b > u32::MAX as i128 { None } else { a.checked_pow(b as u32) }
            }
            BinaryOp::Less => return Ok(Value::Boolean(a < b)),
            BinaryOp::Greater => return Ok(Value::Boolean(a > b)),
            BinaryOp::LessOrEqual => return Ok(Value::Boolean(a <= b)),
            BinaryOp::GreaterOrEqual => return Ok(Value::Boolean(a >= b)),
            _ => unreachable!("bigint_op: неподдерживаемая операция"),
        };
        checked.map(Value::BigInt).ok_or_else(|| RuntimeError::new("Переполнение бигцелого", span))
    }
}

fn is_object_like(value: &Value) -> bool {
    !coercion::is_primitive(value) && !matches!(value, Value::Null | Value::Undefined)
}

fn bigint_eq_number(a: i128, b: f64) -> bool {
    if !b.is_finite() || b.fract() != 0.0 {
        return false;
    }
    (a as f64) == b && (b as i128) == a
}

fn bigint_eq_str(a: i128, s: &str) -> bool {
    match s.trim().parse::<i128>() {
        Ok(n) => n == a,
        Err(_) => false,
    }
}
