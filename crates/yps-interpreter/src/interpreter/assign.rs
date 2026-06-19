use std::cell::RefCell;
use std::rc::Rc;

use indexmap::IndexMap;
use yps_lexer::Span;
use yps_parser::ast::{BinaryOp, Block, Expr, Literal, ObjectEntry, PostfixOp, PropKey};

use crate::environment::{EnvFrame, Environment};
use crate::error::RuntimeError;
use crate::symbols;
use crate::value::Value;

use super::{AccessSegment, ControlFlow, Interpreter};

impl Interpreter {
    pub(super) fn eval_assignment(&mut self, lhs: &Expr, rhs: &Expr, span: Span) -> Result<Value, RuntimeError> {
        let val = self.eval_expr(rhs)?;
        self.assign_to_target(lhs, val, span)
    }

    pub(super) fn eval_compound_assignment(
        &mut self,
        op: BinaryOp,
        lhs: &Expr,
        rhs: &Expr,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        let old = self.eval_expr(lhs)?;
        let right = self.eval_expr(rhs)?;
        let arith_op = match op {
            BinaryOp::PlusAssign => BinaryOp::Add,
            BinaryOp::MinusAssign => BinaryOp::Sub,
            BinaryOp::MulAssign => BinaryOp::Mul,
            BinaryOp::DivAssign => BinaryOp::Div,
            BinaryOp::ExpAssign => BinaryOp::Exp,
            BinaryOp::ModAssign => BinaryOp::Mod,
            BinaryOp::BitAndAssign => BinaryOp::BitAnd,
            BinaryOp::BitOrAssign => BinaryOp::BitOr,
            BinaryOp::BitXorAssign => BinaryOp::BitXor,
            BinaryOp::ShlAssign => BinaryOp::LeftShift,
            BinaryOp::ShrAssign => BinaryOp::RightShift,
            BinaryOp::UshrAssign => BinaryOp::UnsignedRightShift,
            _ => unreachable!(),
        };
        let result = self.eval_binary(arith_op, old, right, span)?;
        self.assign_to_target(lhs, result, span)
    }

    pub(super) fn assign_to_target(&mut self, target: &Expr, value: Value, span: Span) -> Result<Value, RuntimeError> {
        match target {
            Expr::Identifier(ident) => {
                self.set_variable(&ident.name, value.clone(), span)?;
                Ok(value)
            }
            Expr::Member { object, property, .. } => {
                if let Some(result) = self.try_call_setter(object, &property.name, value.clone(), span)? {
                    return Ok(result);
                }
                if matches!(property.name.as_str(), "последнийИндекс" | "lastIndex") {
                    let obj_val = self.eval_expr(object)?;
                    if let Value::RegExp { last_index, .. } = &obj_val {
                        let n = match &value {
                            Value::Number(n) => *n,
                            other => {
                                return Err(RuntimeError::new(
                                    format!("lastIndex требует число, получено '{}'", other.type_name()),
                                    span,
                                ));
                            }
                        };
                        let idx = if n.is_finite() && n >= 0.0 { n as usize } else { 0 };
                        *last_index.borrow_mut() = idx;
                        return Ok(value);
                    }
                }
                if property.name.starts_with('#') {
                    self.check_private_access_for_set(object, &property.name, span)?;
                }
                let mut path = Vec::new();
                let root_name = self.collect_access_path(target, &mut path, span)?;
                path.reverse();
                if self.env.is_const(&root_name) {
                    return Err(RuntimeError::new(format!("Нельзя изменить константу '{root_name}'"), span));
                }
                let root = self
                    .env
                    .get(&root_name)
                    .ok_or_else(|| RuntimeError::new(format!("Переменная '{root_name}' не определена"), span))?;
                Self::set_at_path(root, &path, value.clone(), span)?;
                Ok(value)
            }
            Expr::Index { object, index, .. } => {
                let obj = self.eval_expr(object)?;
                if let Some((ptarget, handler)) = obj.proxy_parts() {
                    let key = self.eval_expr(index)?.to_string();
                    self.proxy_set(&ptarget, &handler, &key, value.clone(), obj, span)?;
                    return Ok(value);
                }
                let mut path = Vec::new();
                let root_name = self.collect_access_path(target, &mut path, span)?;
                path.reverse();
                if self.env.is_const(&root_name) {
                    return Err(RuntimeError::new(format!("Нельзя изменить константу '{root_name}'"), span));
                }
                let root = self
                    .env
                    .get(&root_name)
                    .ok_or_else(|| RuntimeError::new(format!("Переменная '{root_name}' не определена"), span))?;
                Self::set_at_path(root, &path, value.clone(), span)?;
                Ok(value)
            }
            Expr::Literal(Literal::Array { elements, .. }) => {
                self.destructure_assign_array(elements, value.clone(), span)?;
                Ok(value)
            }
            Expr::Literal(Literal::Object { entries, .. }) => {
                self.destructure_assign_object(entries, value.clone(), span)?;
                Ok(value)
            }
            Expr::Grouping { expr, .. } => self.assign_to_target(expr, value, span),
            _ => Err(RuntimeError::new("Левая сторона присваивания должна быть переменной", span)),
        }
    }

    fn destructure_assign_array(&mut self, elements: &[Expr], value: Value, span: Span) -> Result<(), RuntimeError> {
        let items: Vec<Value> = match &value {
            Value::Array(arr) => arr.borrow().0.clone(),
            _ => {
                return Err(RuntimeError::new(
                    format!("Невозможно деструктурировать {} как массив", value.type_name()),
                    span,
                ));
            }
        };
        for (i, elem) in elements.iter().enumerate() {
            if let Expr::Spread { expr, .. } = elem {
                let rest_items = if i < items.len() { items[i..].to_vec() } else { Vec::new() };
                self.assign_to_target(expr, Value::array(rest_items), span)?;
                break;
            }
            let val = items.get(i).cloned().unwrap_or(Value::Undefined);
            self.assign_destructure_element(elem, val, span)?;
        }
        Ok(())
    }

    fn destructure_assign_object(
        &mut self,
        entries: &[ObjectEntry],
        value: Value,
        span: Span,
    ) -> Result<(), RuntimeError> {
        let map: IndexMap<String, Value> = match &value {
            Value::Object(obj) => obj.borrow().map.clone(),
            _ => {
                return Err(RuntimeError::new(
                    format!("Невозможно деструктурировать {} как объект", value.type_name()),
                    span,
                ));
            }
        };
        let mut used_keys = Vec::new();
        for entry in entries {
            match entry {
                ObjectEntry::Property { key, value: target } => {
                    let key_str = match key {
                        PropKey::Identifier(ident) => ident.name.clone(),
                        PropKey::Computed(expr) => {
                            let k = self.eval_expr(expr)?;
                            if let Value::Symbol { id, .. } = &k {
                                crate::symbols::symbol_key(*id)
                            } else {
                                k.to_string()
                            }
                        }
                    };
                    let val = map.get(&key_str).cloned().unwrap_or(Value::Undefined);
                    used_keys.push(key_str);
                    self.assign_destructure_element(target, val, span)?;
                }
                ObjectEntry::Spread(target) => {
                    let mut rest_map = map.clone();
                    for key in &used_keys {
                        rest_map.shift_remove(key);
                    }
                    self.assign_to_target(target, Value::object(rest_map), span)?;
                }
                _ => {
                    return Err(RuntimeError::new(
                        "Геттеры и сеттеры недопустимы в цели деструктурирующего присваивания",
                        span,
                    ));
                }
            }
        }
        Ok(())
    }

    fn assign_destructure_element(&mut self, target: &Expr, val: Value, span: Span) -> Result<(), RuntimeError> {
        if let Expr::Binary { op: BinaryOp::Assign, lhs, rhs, .. } = target {
            let val = if matches!(val, Value::Undefined) { self.eval_expr(rhs)? } else { val };
            self.assign_to_target(lhs, val, span)?;
        } else {
            self.assign_to_target(target, val, span)?;
        }
        Ok(())
    }

    fn try_call_setter(
        &mut self,
        object_expr: &Expr,
        property: &str,
        value: Value,
        span: Span,
    ) -> Result<Option<Value>, RuntimeError> {
        let obj = self.eval_expr(object_expr)?;
        if let Some((target, handler)) = obj.proxy_parts() {
            self.proxy_set(&target, &handler, property, value.clone(), obj, span)?;
            return Ok(Some(value));
        }
        match &obj {
            Value::Object(map) => {
                let setter_key = symbols::setter_key(property);
                let setter = match map.borrow().get(&setter_key) {
                    Some(Value::Function { params, body, env, .. }) => {
                        Some((params.clone(), Rc::clone(body), Rc::clone(env)))
                    }
                    _ => None,
                };
                if let Some((params, body, env)) = setter {
                    self.call_setter_returning_this(
                        Rc::from(property),
                        &params,
                        &body,
                        &env,
                        value.clone(),
                        obj,
                        span,
                    )?;
                    return Ok(Some(value));
                }
                let class_setter = Self::resolve_class_for_object(map, &self.env).and_then(|cls| {
                    Self::find_setter_in_class(&cls, property).map(|(p, b, e)| (p.clone(), Rc::clone(b), Rc::clone(e)))
                });
                if let Some((params, body, env)) = class_setter {
                    self.call_setter_returning_this(
                        Rc::from(property),
                        &params,
                        &body,
                        &env,
                        value.clone(),
                        obj.clone(),
                        span,
                    )?;
                    return Ok(Some(value));
                }
                Ok(None)
            }
            Value::Class(cls) => {
                if let Some((params, body, env)) = Self::find_static_setter_in_class(cls, property) {
                    self.call_method_with_this(Rc::from(property), params, body, env, vec![value.clone()], None, span)?;
                    return Ok(Some(value));
                }
                Ok(None)
            }
            _ => Ok(None),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn call_setter_returning_this(
        &mut self,
        name: Rc<str>,
        params: &[yps_parser::ast::Param],
        body: &Rc<Block>,
        env: &Rc<RefCell<EnvFrame>>,
        value: Value,
        this_val: Value,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        let saved_env = self.env.clone();
        self.env = Environment::from_snapshot(Rc::clone(env), self.env.registry());
        self.env.push_scope();
        self.env.define(symbols::THIS.to_string(), this_val.clone(), false);
        if let Some(param) = params.first() {
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
            Some(ControlFlow::Throw(val)) => Err(RuntimeError::thrown_with_stack(val, span, frame_stack)),
            _ => Ok(updated_this),
        }
    }

    pub(super) fn write_back_object(
        &mut self,
        object_expr: &Expr,
        updated: Value,
        span: Span,
    ) -> Result<(), RuntimeError> {
        match object_expr {
            Expr::Identifier(ident) => {
                if self.env.is_const(&ident.name) {
                    return Err(RuntimeError::new(format!("Нельзя изменить константу '{}'", ident.name), span));
                }
                self.env.set(&ident.name, updated);
                Ok(())
            }
            Expr::This { .. } => {
                self.env.set(symbols::THIS, updated);
                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn check_private_access_for_set(
        &self,
        _object_expr: &Expr,
        property: &str,
        span: Span,
    ) -> Result<(), RuntimeError> {
        if !property.starts_with('#') {
            return Ok(());
        }
        let in_method = self.env.get(symbols::THIS).is_some();
        if !in_method {
            return Err(RuntimeError::new(
                format!("Нельзя обращаться к приватному полю '{property}' за пределами класса"),
                span,
            ));
        }
        Ok(())
    }

    pub(super) fn collect_access_path(
        &mut self,
        expr: &Expr,
        path: &mut Vec<AccessSegment>,
        span: Span,
    ) -> Result<String, RuntimeError> {
        match expr {
            Expr::Identifier(ident) => Ok(ident.name.clone()),
            Expr::This { .. } => Ok(symbols::THIS.to_string()),
            Expr::Index { object, index, .. } => {
                let idx_val = self.eval_expr(index)?;
                path.push(AccessSegment::Index(idx_val));
                self.collect_access_path(object, path, span)
            }
            Expr::Member { object, property, .. } => {
                path.push(AccessSegment::Member(property.name.clone()));
                self.collect_access_path(object, path, span)
            }
            _ => Err(RuntimeError::new("Левая сторона присваивания должна быть переменной", span)),
        }
    }

    fn set_at_path(target: Value, path: &[AccessSegment], value: Value, span: Span) -> Result<(), RuntimeError> {
        if path.is_empty() {
            return Ok(());
        }
        let seg = &path[0];
        let is_last = path.len() == 1;
        match (seg, &target) {
            (AccessSegment::Index(Value::Number(n)), Value::TypedArray { buffer, offset, length, kind }) => {
                if !is_last {
                    return Err(RuntimeError::new("Нельзя индексировать элемент типизированного массива далее", span));
                }
                if n.is_finite() && *n >= 0.0 && n.fract() == 0.0 {
                    let i = *n as usize;
                    if i < *length {
                        crate::stdlib::typed_array::write_element(
                            buffer,
                            *kind,
                            *offset + i * kind.element_size(),
                            &value,
                            span,
                        )?;
                    }
                }
                Ok(())
            }
            (AccessSegment::Index(Value::Number(n)), Value::Array(arr)) => {
                let i = *n as usize;
                if is_last {
                    let mut guard = arr
                        .try_borrow_mut()
                        .map_err(|_| RuntimeError::new("Внутренняя реентрантная мутация массива", span))?;
                    let len = guard.len();
                    let slot = guard
                        .get_mut(i)
                        .ok_or_else(|| RuntimeError::new(format!("Индекс {i} вне диапазона (длина {len})"), span))?;
                    *slot = value;
                    Ok(())
                } else {
                    let child = {
                        let guard = arr.borrow();
                        let len = guard.len();
                        guard
                            .get(i)
                            .cloned()
                            .ok_or_else(|| RuntimeError::new(format!("Индекс {i} вне диапазона (длина {len})"), span))?
                    };
                    Self::descend_set(child, &path[1..], value, span)
                }
            }
            (AccessSegment::Index(Value::String(key)), Value::Object(map)) => {
                if is_last {
                    if map.borrow().frozen {
                        return Ok(());
                    }
                    map.try_borrow_mut()
                        .map_err(|_| RuntimeError::new("Внутренняя реентрантная мутация объекта", span))?
                        .insert(key.clone(), value);
                    Ok(())
                } else {
                    let child = map
                        .borrow()
                        .get(key)
                        .cloned()
                        .ok_or_else(|| RuntimeError::new(format!("Ключ '{key}' не найден в объекте"), span))?;
                    Self::descend_set(child, &path[1..], value, span)
                }
            }
            (AccessSegment::Member(prop), Value::Object(map)) => {
                if is_last {
                    if map.borrow().frozen {
                        return Ok(());
                    }
                    map.try_borrow_mut()
                        .map_err(|_| RuntimeError::new("Внутренняя реентрантная мутация объекта", span))?
                        .insert(prop.clone(), value);
                    Ok(())
                } else {
                    let child =
                        map.borrow().get(prop).cloned().ok_or_else(|| {
                            RuntimeError::new(format!("Свойство '{prop}' не найдено в объекте"), span)
                        })?;
                    Self::descend_set(child, &path[1..], value, span)
                }
            }
            (AccessSegment::Index(Value::Symbol { id, .. }), Value::Object(map)) => {
                let key = crate::symbols::symbol_key(*id);
                if is_last {
                    if map.borrow().frozen {
                        return Ok(());
                    }
                    map.try_borrow_mut()
                        .map_err(|_| RuntimeError::new("Внутренняя реентрантная мутация объекта", span))?
                        .insert(key, value);
                    Ok(())
                } else {
                    let child = map.borrow().get(&key).cloned().unwrap_or(Value::Undefined);
                    Self::descend_set(child, &path[1..], value, span)
                }
            }
            (AccessSegment::Member(prop), Value::Class(cls)) => {
                if is_last {
                    let owner = Self::find_static_field_owner(cls, prop).unwrap_or_else(|| Rc::clone(cls));
                    owner.static_fields.borrow_mut().insert(prop.clone(), value);
                    Ok(())
                } else {
                    let child = Self::find_static_field_in_class(cls, prop).unwrap_or(Value::Undefined);
                    Self::descend_set(child, &path[1..], value, span)
                }
            }
            (AccessSegment::Index(idx), val) => Err(RuntimeError::new(
                format!("Нельзя индексировать '{}' с помощью '{}'", val.type_name(), idx.type_name()),
                span,
            )),
            (AccessSegment::Member(_), val) => {
                Err(RuntimeError::new(format!("Нельзя установить свойство у типа '{}'", val.type_name()), span))
            }
        }
    }

    fn descend_set(child: Value, path: &[AccessSegment], value: Value, span: Span) -> Result<(), RuntimeError> {
        match &child {
            Value::Array(_) | Value::Object(_) | Value::TypedArray { .. } => {
                Self::set_at_path(child, path, value, span)
            }
            other => Err(RuntimeError::new(format!("Нельзя индексировать '{}' далее", other.type_name()), span)),
        }
    }

    pub(super) fn eval_postfix(&mut self, op: PostfixOp, expr: &Expr, span: Span) -> Result<Value, RuntimeError> {
        let Expr::Identifier(ident) = expr else {
            return Err(RuntimeError::new("'++' / '--' можно применить только к переменной", span));
        };
        let old = self
            .env
            .get(&ident.name)
            .ok_or_else(|| RuntimeError::new(format!("Переменная '{}' не определена", ident.name), span))?;
        let Value::Number(n) = old else {
            return Err(RuntimeError::new(format!("'++' / '--' требует число, получено '{}'", old.type_name()), span));
        };
        let new_val = match op {
            PostfixOp::Increment => Value::Number(n + 1.0),
            PostfixOp::Decrement => Value::Number(n - 1.0),
        };
        self.set_variable(&ident.name, new_val, span)?;
        Ok(Value::Number(n))
    }

    pub(super) fn set_variable(&mut self, name: &str, value: Value, span: Span) -> Result<(), RuntimeError> {
        if self.env.is_const(name) {
            return Err(RuntimeError::new(format!("Нельзя изменить константу '{name}'"), span));
        }
        if !self.env.set(name, value) {
            return Err(RuntimeError::new(format!("Переменная '{name}' не определена"), span));
        }
        Ok(())
    }
}
