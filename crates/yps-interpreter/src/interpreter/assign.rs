use std::cell::RefCell;
use std::rc::Rc;

use yps_lexer::Span;
use yps_parser::ast::{BinaryOp, Block, Expr, PostfixOp};

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
                let mut root = self
                    .env
                    .get(&root_name)
                    .ok_or_else(|| RuntimeError::new(format!("Переменная '{root_name}' не определена"), span))?;
                Self::set_at_path(&mut root, &path, value.clone(), span)?;
                self.env.set(&root_name, root);
                Ok(value)
            }
            Expr::Index { .. } => {
                let mut path = Vec::new();
                let root_name = self.collect_access_path(target, &mut path, span)?;
                path.reverse();
                if self.env.is_const(&root_name) {
                    return Err(RuntimeError::new(format!("Нельзя изменить константу '{root_name}'"), span));
                }
                let mut root = self
                    .env
                    .get(&root_name)
                    .ok_or_else(|| RuntimeError::new(format!("Переменная '{root_name}' не определена"), span))?;
                Self::set_at_path(&mut root, &path, value.clone(), span)?;
                self.env.set(&root_name, root);
                Ok(value)
            }
            _ => Err(RuntimeError::new("Левая сторона присваивания должна быть переменной", span)),
        }
    }

    fn try_call_setter(
        &mut self,
        object_expr: &Expr,
        property: &str,
        value: Value,
        span: Span,
    ) -> Result<Option<Value>, RuntimeError> {
        let obj = self.eval_expr(object_expr)?;
        match &obj {
            Value::Object(map) => {
                let setter_key = symbols::setter_key(property);
                if let Some(Value::Function { params, body, env, .. }) = map.get(&setter_key) {
                    let params = params.clone();
                    let body = Rc::clone(body);
                    let env = Rc::clone(env);
                    let updated = self.call_setter_returning_this(&params, &body, &env, value.clone(), obj, span)?;
                    self.write_back_object(object_expr, updated, span)?;
                    return Ok(Some(value));
                }
                if let Some(cls) = Self::resolve_class_for_object(map, &self.env)
                    && let Some((params, body, env)) = Self::find_setter_in_class(&cls, property)
                {
                    let params = params.clone();
                    let body = Rc::clone(body);
                    let env = Rc::clone(env);
                    let updated =
                        self.call_setter_returning_this(&params, &body, &env, value.clone(), obj.clone(), span)?;
                    self.write_back_object(object_expr, updated, span)?;
                    return Ok(Some(value));
                }
                Ok(None)
            }
            Value::Class(cls) => {
                if let Some((params, body, env)) = cls.static_setters.get(property) {
                    self.call_method_with_this(params, body, env, vec![value.clone()], None, span)?;
                    return Ok(Some(value));
                }
                Ok(None)
            }
            _ => Ok(None),
        }
    }

    fn call_setter_returning_this(
        &mut self,
        params: &[yps_parser::ast::Param],
        body: &Rc<Block>,
        env: &Rc<RefCell<EnvFrame>>,
        value: Value,
        this_val: Value,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        let saved_env = self.env.clone();
        self.env = Environment::from_snapshot(Rc::clone(env));
        self.env.push_scope();
        self.env.define(symbols::THIS.to_string(), this_val.clone(), false);
        if let Some(param) = params.first() {
            self.env.define(param.name.name.clone(), value, false);
        }
        let result = self.exec_block_stmts(&body.stmts);
        let updated_this = self.env.get(symbols::THIS).unwrap_or(this_val);
        self.env = saved_env;
        match result? {
            Some(ControlFlow::Throw(val)) => Err(RuntimeError::thrown(val, span)),
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

    fn set_at_path(target: &mut Value, path: &[AccessSegment], value: Value, span: Span) -> Result<(), RuntimeError> {
        if path.is_empty() {
            *target = value;
            return Ok(());
        }
        match (&path[0], target) {
            (AccessSegment::Index(Value::Number(n)), Value::Array(arr)) => {
                let i = *n as usize;
                let len = arr.len();
                let elem = arr
                    .get_mut(i)
                    .ok_or_else(|| RuntimeError::new(format!("Индекс {i} вне диапазона (длина {len})"), span))?;
                Self::set_at_path(elem, &path[1..], value, span)
            }
            (AccessSegment::Index(Value::String(key)), Value::Object(map)) => {
                if path.len() == 1 {
                    map.insert(key.clone(), value);
                    Ok(())
                } else {
                    let entry = map
                        .get_mut(key)
                        .ok_or_else(|| RuntimeError::new(format!("Ключ '{key}' не найден в объекте"), span))?;
                    Self::set_at_path(entry, &path[1..], value, span)
                }
            }
            (AccessSegment::Member(prop), Value::Object(map)) => {
                if path.len() == 1 {
                    map.insert(prop.clone(), value);
                    Ok(())
                } else {
                    let entry = map
                        .get_mut(prop)
                        .ok_or_else(|| RuntimeError::new(format!("Свойство '{prop}' не найдено в объекте"), span))?;
                    Self::set_at_path(entry, &path[1..], value, span)
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
