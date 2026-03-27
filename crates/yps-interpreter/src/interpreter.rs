use std::collections::HashMap;

use yps_lexer::Span;
use yps_parser::ast::{BinaryOp, Block, Expr, Literal, Pattern, PostfixOp, Program, Stmt, TemplatePart, UnaryOp};

use crate::builtins::{builtin_names, call_builtin};
use crate::environment::Environment;
use crate::error::RuntimeError;
use crate::value::Value;

enum ControlFlow {
    Break,
    Continue,
    Return(Value),
    Throw(Value),
}

enum AccessSegment {
    Index(Value),
    Member(String),
}

pub struct Interpreter {
    env: Environment,
}

impl Default for Interpreter {
    fn default() -> Self {
        Self::new()
    }
}

impl Interpreter {
    pub fn new() -> Self {
        let mut env = Environment::new();
        for name in builtin_names() {
            env.define(name.to_string(), Value::BuiltinFunction(name.to_string()), true);
        }
        Self { env }
    }

    pub fn get(&self, name: &str) -> Option<Value> {
        self.env.get(name)
    }

    pub fn run(&mut self, program: &Program) -> Result<(), RuntimeError> {
        for stmt in &program.items {
            if let Some(cf) = self.exec_stmt(stmt)? {
                match cf {
                    ControlFlow::Return(_) => return Ok(()),
                    ControlFlow::Break => {
                        return Err(RuntimeError::new("'харэ' вне цикла", Span { start: 0, end: 0 }));
                    }
                    ControlFlow::Continue => {
                        return Err(RuntimeError::new("'двигай' вне цикла", Span { start: 0, end: 0 }));
                    }
                    ControlFlow::Throw(val) => {
                        return Err(RuntimeError::new(
                            format!("Необработанное исключение: {val}"),
                            Span { start: 0, end: 0 },
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    fn exec_stmt(&mut self, stmt: &Stmt) -> Result<Option<ControlFlow>, RuntimeError> {
        match stmt {
            Stmt::VarDecl { pattern, init, is_const, span } => {
                let value = self.eval_expr(init)?;
                self.destructure_pattern(pattern, value, *is_const, *span)?;
                Ok(None)
            }
            Stmt::Expr { expr, .. } => {
                self.eval_expr(expr)?;
                Ok(None)
            }
            Stmt::Block(block) => self.exec_block(block),
            Stmt::Empty { .. } => Ok(None),
            Stmt::If { condition, then_branch, else_branch, .. } => {
                let cond = self.eval_expr(condition)?;
                if cond.is_truthy() {
                    self.exec_stmt(then_branch)
                } else if let Some(else_br) = else_branch {
                    self.exec_stmt(else_br)
                } else {
                    Ok(None)
                }
            }
            Stmt::While { condition, body, .. } => {
                loop {
                    let cond = self.eval_expr(condition)?;
                    if !cond.is_truthy() {
                        break;
                    }
                    if let Some(cf) = self.exec_stmt(body)? {
                        match cf {
                            ControlFlow::Break => break,
                            ControlFlow::Continue => continue,
                            cf @ (ControlFlow::Return(_) | ControlFlow::Throw(_)) => return Ok(Some(cf)),
                        }
                    }
                }
                Ok(None)
            }
            Stmt::For { init, condition, update, body, .. } => {
                self.env.push_scope();
                if let Some(init_stmt) = init {
                    self.exec_stmt(init_stmt)?;
                }
                loop {
                    if let Some(cond) = condition {
                        let val = self.eval_expr(cond)?;
                        if !val.is_truthy() {
                            break;
                        }
                    }
                    if let Some(cf) = self.exec_stmt(body)? {
                        match cf {
                            ControlFlow::Break => break,
                            ControlFlow::Continue => {}
                            cf @ (ControlFlow::Return(_) | ControlFlow::Throw(_)) => {
                                self.env.pop_scope();
                                return Ok(Some(cf));
                            }
                        }
                    }
                    if let Some(upd) = update {
                        self.eval_expr(upd)?;
                    }
                }
                self.env.pop_scope();
                Ok(None)
            }
            Stmt::Break { .. } => Ok(Some(ControlFlow::Break)),
            Stmt::Continue { .. } => Ok(Some(ControlFlow::Continue)),
            Stmt::FunctionDecl { name, params, body, .. } => {
                let func = Value::Function {
                    name: name.name.clone(),
                    params: params.iter().map(|p| p.name.clone()).collect(),
                    body: body.clone(),
                    env: self.env.snapshot(),
                };
                self.env.define(name.name.clone(), func, false);
                Ok(None)
            }
            Stmt::Return { value, .. } => {
                let val = match value {
                    Some(expr) => self.eval_expr(expr)?,
                    None => Value::Undefined,
                };
                Ok(Some(ControlFlow::Return(val)))
            }
            Stmt::Throw { value, .. } => {
                let val = self.eval_expr(value)?;
                Ok(Some(ControlFlow::Throw(val)))
            }
            Stmt::ForIn { variable, iterable, body, span, .. } => {
                let val = self.eval_expr(iterable)?;
                let items: Vec<Value> = match val {
                    Value::Array(elements) => elements,
                    Value::Object(map) => map.keys().map(|k| Value::String(k.clone())).collect(),
                    other => {
                        return Err(RuntimeError::new(
                            format!("Нельзя итерировать по типу '{}'", other.type_name()),
                            *span,
                        ));
                    }
                };
                self.env.push_scope();
                self.env.define(variable.name.clone(), Value::Undefined, false);
                for item in items {
                    self.env.set(&variable.name, item);
                    if let Some(cf) = self.exec_stmt(body)? {
                        match cf {
                            ControlFlow::Break => break,
                            ControlFlow::Continue => continue,
                            cf @ (ControlFlow::Return(_) | ControlFlow::Throw(_)) => {
                                self.env.pop_scope();
                                return Ok(Some(cf));
                            }
                        }
                    }
                }
                self.env.pop_scope();
                Ok(None)
            }
            Stmt::DoWhile { body, condition, .. } => {
                loop {
                    if let Some(cf) = self.exec_stmt(body)? {
                        match cf {
                            ControlFlow::Break => break,
                            ControlFlow::Continue => {}
                            cf @ (ControlFlow::Return(_) | ControlFlow::Throw(_)) => return Ok(Some(cf)),
                        }
                    }
                    let cond = self.eval_expr(condition)?;
                    if !cond.is_truthy() {
                        break;
                    }
                }
                Ok(None)
            }
            Stmt::Switch { expr, cases, default, .. } => {
                let switch_val = self.eval_expr(expr)?;
                for case in cases {
                    let case_val = self.eval_expr(&case.value)?;
                    if switch_val == case_val {
                        return self.exec_block(&case.body);
                    }
                }
                if let Some(default_block) = default {
                    return self.exec_block(default_block);
                }
                Ok(None)
            }
            Stmt::TryCatch { try_block, catch_param, catch_block, finally_block, .. } => {
                let try_result = self.exec_block(try_block);

                let result = match try_result {
                    Err(err) => {
                        if let Some(cb) = catch_block {
                            self.env.push_scope();
                            if let Some(param) = catch_param {
                                self.env.define(param.name.clone(), Value::String(err.message), false);
                            }
                            let r = self.exec_block_stmts(&cb.stmts);
                            self.env.pop_scope();
                            r
                        } else {
                            Ok(None)
                        }
                    }
                    Ok(Some(ControlFlow::Throw(val))) => {
                        if let Some(cb) = catch_block {
                            self.env.push_scope();
                            if let Some(param) = catch_param {
                                self.env.define(param.name.clone(), val, false);
                            }
                            let r = self.exec_block_stmts(&cb.stmts);
                            self.env.pop_scope();
                            r
                        } else {
                            Ok(Some(ControlFlow::Throw(val)))
                        }
                    }
                    other => other,
                };

                if let Some(fb) = finally_block {
                    let finally_result = self.exec_block(fb);
                    finally_result.as_ref().map_err(|e| RuntimeError::new(&e.message, e.span))?;
                    if let Ok(Some(cf)) = &finally_result
                        && matches!(cf, ControlFlow::Return(_) | ControlFlow::Throw(_))
                    {
                        return finally_result;
                    }
                }

                result
            }
        }
    }

    fn exec_block(&mut self, block: &Block) -> Result<Option<ControlFlow>, RuntimeError> {
        self.env.push_scope();
        let result = self.exec_block_stmts(&block.stmts);
        self.env.pop_scope();
        result
    }

    fn exec_block_stmts(&mut self, stmts: &[Stmt]) -> Result<Option<ControlFlow>, RuntimeError> {
        for stmt in stmts {
            if let Some(cf) = self.exec_stmt(stmt)? {
                return Ok(Some(cf));
            }
        }
        Ok(None)
    }

    fn destructure_pattern(
        &mut self,
        pattern: &Pattern,
        value: Value,
        is_const: bool,
        span: Span,
    ) -> Result<(), RuntimeError> {
        match pattern {
            Pattern::Identifier(ident) => {
                self.env.define(ident.name.clone(), value, is_const);
                Ok(())
            }
            Pattern::Array { elements, rest, .. } => {
                let items = match &value {
                    Value::Array(arr) => arr.clone(),
                    _ => {
                        return Err(RuntimeError::new(
                            format!("Невозможно деструктурировать {} как массив", value.type_name()),
                            span,
                        ));
                    }
                };

                for (i, elem) in elements.iter().enumerate() {
                    if let Some(pat) = elem {
                        let val = items.get(i).cloned().unwrap_or(Value::Undefined);
                        self.destructure_pattern(pat, val, is_const, span)?;
                    }
                }

                if let Some(rest_pat) = rest {
                    let start = elements.len();
                    let rest_items = if start < items.len() { items[start..].to_vec() } else { Vec::new() };
                    self.destructure_pattern(rest_pat, Value::Array(rest_items), is_const, span)?;
                }

                Ok(())
            }
            Pattern::Object { properties, rest, .. } => {
                let mut map = match value {
                    Value::Object(map) => map,
                    _ => {
                        return Err(RuntimeError::new(
                            format!("Невозможно деструктурировать {} как объект", value.type_name()),
                            span,
                        ));
                    }
                };

                let mut used_keys = Vec::new();

                for prop in properties {
                    let val = map.get(&prop.key.name).cloned().unwrap_or(Value::Undefined);
                    used_keys.push(prop.key.name.clone());

                    if let Some(ref value_pat) = prop.value {
                        self.destructure_pattern(value_pat, val, is_const, span)?;
                    } else {
                        self.env.define(prop.key.name.clone(), val, is_const);
                    }
                }

                if let Some(rest_pat) = rest {
                    for key in &used_keys {
                        map.remove(key);
                    }
                    self.destructure_pattern(rest_pat, Value::Object(map), is_const, span)?;
                }

                Ok(())
            }
        }
    }

    fn eval_expr(&mut self, expr: &Expr) -> Result<Value, RuntimeError> {
        match expr {
            Expr::Literal(lit) => self.eval_literal(lit),
            Expr::Identifier(ident) => self
                .env
                .get(&ident.name)
                .ok_or_else(|| RuntimeError::new(format!("Переменная '{}' не определена", ident.name), ident.span)),
            Expr::Unary { op, expr, span } => {
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
                    let left = self.eval_expr(lhs)?;
                    if !matches!(left, Value::Null | Value::Undefined) {
                        return Ok(left);
                    }
                    let right = self.eval_expr(rhs)?;
                    return self.assign_to_target(lhs, right, *span);
                }
                if *op == BinaryOp::AndAssign {
                    let left = self.eval_expr(lhs)?;
                    if !left.is_truthy() {
                        return Ok(left);
                    }
                    let right = self.eval_expr(rhs)?;
                    return self.assign_to_target(lhs, right, *span);
                }
                if *op == BinaryOp::OrAssign {
                    let left = self.eval_expr(lhs)?;
                    if left.is_truthy() {
                        return Ok(left);
                    }
                    let right = self.eval_expr(rhs)?;
                    return self.assign_to_target(lhs, right, *span);
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
                let func = self.eval_expr(callee)?;
                let mut arg_values = Vec::with_capacity(args.len());
                for arg in args {
                    arg_values.push(self.eval_expr(arg)?);
                }
                self.call_function(func, arg_values, *span)
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
                    let mut arg_values = Vec::with_capacity(args.len());
                    for arg in args {
                        arg_values.push(self.eval_expr(arg)?);
                    }
                    self.call_function(func, arg_values, *span)
                }
            }
            Expr::Conditional { condition, then_expr, else_expr, .. } => {
                let cond = self.eval_expr(condition)?;
                if cond.is_truthy() { self.eval_expr(then_expr) } else { self.eval_expr(else_expr) }
            }
            Expr::ArrowFunction { params, body, .. } => {
                let func = Value::Function {
                    name: String::new(),
                    params: params.iter().map(|p| p.name.clone()).collect(),
                    body: body.clone(),
                    env: self.env.snapshot(),
                };
                Ok(func)
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
                Ok(Value::String(result))
            }
        }
    }

    fn eval_literal(&mut self, lit: &Literal) -> Result<Value, RuntimeError> {
        match lit {
            Literal::Number { raw, span } => raw
                .parse::<f64>()
                .map(Value::Number)
                .map_err(|_| RuntimeError::new(format!("Невалидное число: '{raw}'"), *span)),
            Literal::String { value, .. } => Ok(Value::String(value.clone())),
            Literal::Boolean { value, .. } => Ok(Value::Boolean(*value)),
            Literal::Null { .. } => Ok(Value::Null),
            Literal::Undefined { .. } => Ok(Value::Undefined),
            Literal::Array { elements, .. } => {
                let mut values = Vec::with_capacity(elements.len());
                for el in elements {
                    values.push(self.eval_expr(el)?);
                }
                Ok(Value::Array(values))
            }
            Literal::Object { properties, .. } => {
                let mut map = HashMap::new();
                for prop in properties {
                    let val = self.eval_expr(&prop.value)?;
                    map.insert(prop.key.name.clone(), val);
                }
                Ok(Value::Object(map))
            }
        }
    }

    fn eval_unary(&self, op: UnaryOp, val: Value, span: Span) -> Result<Value, RuntimeError> {
        match op {
            UnaryOp::Minus => match val {
                Value::Number(n) => Ok(Value::Number(-n)),
                _ => Err(RuntimeError::new(format!("Нельзя применить '-' к типу '{}'", val.type_name()), span)),
            },
            UnaryOp::Plus => match val {
                Value::Number(n) => Ok(Value::Number(n)),
                _ => Err(RuntimeError::new(format!("Нельзя применить '+' к типу '{}'", val.type_name()), span)),
            },
            UnaryOp::Not => Ok(Value::Boolean(!val.is_truthy())),
        }
    }

    fn eval_binary(&self, op: BinaryOp, left: Value, right: Value, span: Span) -> Result<Value, RuntimeError> {
        match op {
            BinaryOp::Add => match (&left, &right) {
                (Value::Number(a), Value::Number(b)) => Ok(Value::Number(a + b)),
                (Value::String(a), Value::String(b)) => Ok(Value::String(format!("{a}{b}"))),
                (Value::String(a), _) => Ok(Value::String(format!("{a}{right}"))),
                (_, Value::String(b)) => Ok(Value::String(format!("{left}{b}"))),
                _ => Err(RuntimeError::new(
                    format!("Нельзя сложить '{}' и '{}'", left.type_name(), right.type_name()),
                    span,
                )),
            },
            BinaryOp::Sub => self.numeric_op(&left, &right, span, |a, b| a - b),
            BinaryOp::Mul => self.numeric_op(&left, &right, span, |a, b| a * b),
            BinaryOp::Div => {
                if let (Value::Number(_), Value::Number(b)) = (&left, &right)
                    && *b == 0.0
                {
                    return Err(RuntimeError::new("Деление на ноль", span));
                }
                self.numeric_op(&left, &right, span, |a, b| a / b)
            }
            BinaryOp::Mod => self.numeric_op(&left, &right, span, |a, b| a % b),
            BinaryOp::Exp => self.numeric_op(&left, &right, span, |a, b| a.powf(b)),
            BinaryOp::Equals | BinaryOp::StrictEquals => Ok(Value::Boolean(left == right)),
            BinaryOp::NotEquals | BinaryOp::StrictNotEquals => Ok(Value::Boolean(left != right)),
            BinaryOp::Less => self.compare_op(&left, &right, span, |a, b| a < b),
            BinaryOp::Greater => self.compare_op(&left, &right, span, |a, b| a > b),
            BinaryOp::LessOrEqual => self.compare_op(&left, &right, span, |a, b| a <= b),
            BinaryOp::GreaterOrEqual => self.compare_op(&left, &right, span, |a, b| a >= b),
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
            | BinaryOp::ExpAssign => unreachable!("handled in eval_expr"),
        }
    }

    fn eval_assignment(&mut self, lhs: &Expr, rhs: &Expr, span: Span) -> Result<Value, RuntimeError> {
        let val = self.eval_expr(rhs)?;
        self.assign_to_target(lhs, val, span)
    }

    fn eval_compound_assignment(
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

    fn assign_to_target(&mut self, target: &Expr, value: Value, span: Span) -> Result<Value, RuntimeError> {
        match target {
            Expr::Identifier(ident) => {
                self.set_variable(&ident.name, value.clone(), span)?;
                Ok(value)
            }
            Expr::Index { .. } | Expr::Member { .. } => {
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

    fn collect_access_path(
        &mut self,
        expr: &Expr,
        path: &mut Vec<AccessSegment>,
        span: Span,
    ) -> Result<String, RuntimeError> {
        match expr {
            Expr::Identifier(ident) => Ok(ident.name.clone()),
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

    fn eval_postfix(&mut self, op: PostfixOp, expr: &Expr, span: Span) -> Result<Value, RuntimeError> {
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

    fn set_variable(&mut self, name: &str, value: Value, span: Span) -> Result<(), RuntimeError> {
        if self.env.is_const(name) {
            return Err(RuntimeError::new(format!("Нельзя изменить константу '{name}'"), span));
        }
        if !self.env.set(name, value) {
            return Err(RuntimeError::new(format!("Переменная '{name}' не определена"), span));
        }
        Ok(())
    }

    fn numeric_op(
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

    fn compare_op(
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

    fn call_function(&mut self, func: Value, args: Vec<Value>, span: Span) -> Result<Value, RuntimeError> {
        match func {
            Value::BuiltinFunction(name) => call_builtin(&name, args, span),
            Value::Function { name, params, body, env } => {
                if args.len() != params.len() {
                    return Err(RuntimeError::new(
                        format!("Функция '{}' ожидает {} аргумент(ов), получено {}", name, params.len(), args.len()),
                        span,
                    ));
                }
                let saved_env = self.env.clone();
                self.env = Environment::from_snapshot(env);
                self.env.push_scope();
                for (param, arg) in params.iter().zip(args) {
                    self.env.define(param.clone(), arg, false);
                }
                let result = self.exec_block_stmts(&body.stmts);
                self.env = saved_env;
                match result? {
                    Some(ControlFlow::Return(val)) => Ok(val),
                    Some(ControlFlow::Break) => Err(RuntimeError::new("'харэ' вне цикла", span)),
                    Some(ControlFlow::Continue) => Err(RuntimeError::new("'двигай' вне цикла", span)),
                    Some(ControlFlow::Throw(val)) => {
                        Err(RuntimeError::new(format!("Необработанное исключение: {val}"), span))
                    }
                    None => Ok(Value::Undefined),
                }
            }
            _ => Err(RuntimeError::new(format!("'{}' не является функцией", func.type_name()), span)),
        }
    }

    fn eval_index(&self, obj: Value, index: Value, span: Span) -> Result<Value, RuntimeError> {
        match (&obj, &index) {
            (Value::Array(arr), Value::Number(n)) => {
                let i = *n as usize;
                Ok(arr.get(i).cloned().unwrap_or(Value::Undefined))
            }
            (Value::Object(map), Value::String(key)) => Ok(map.get(key).cloned().unwrap_or(Value::Undefined)),
            _ => Err(RuntimeError::new(
                format!("Нельзя индексировать '{}' с помощью '{}'", obj.type_name(), index.type_name()),
                span,
            )),
        }
    }

    fn eval_member(&self, obj: Value, property: &str, span: Span) -> Result<Value, RuntimeError> {
        match &obj {
            Value::Object(map) => Ok(map.get(property).cloned().unwrap_or(Value::Undefined)),
            _ => Err(RuntimeError::new(format!("Нельзя получить свойство у типа '{}'", obj.type_name()), span)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yps_lexer::{Lexer, SourceFile};
    use yps_parser::Parser;

    fn run_code(src: &str) -> Interpreter {
        let source = SourceFile::new("test".to_string(), src.to_string());
        let (tokens, lex_diags) = Lexer::new(&source).tokenize();
        assert!(lex_diags.is_empty(), "Ошибки лексера: {lex_diags:?}");
        let (program, parse_diags) = Parser::new(&tokens, &source).parse_program();
        assert!(parse_diags.is_empty(), "Ошибки парсера: {parse_diags:?}");
        let mut interp = Interpreter::new();
        interp.run(&program).expect("Ошибка интерпретатора");
        interp
    }

    fn run_code_err(src: &str) -> RuntimeError {
        let source = SourceFile::new("test".to_string(), src.to_string());
        let (tokens, _) = Lexer::new(&source).tokenize();
        let (program, _) = Parser::new(&tokens, &source).parse_program();
        let mut interp = Interpreter::new();
        interp.run(&program).unwrap_err()
    }

    #[test]
    fn assign_array_index() {
        let interp = run_code(
            r#"
            гыы арр = [1, 2, 3];
            арр[0] = 10;
            гыы результат = арр[0];
            "#,
        );
        assert_eq!(interp.get("результат"), Some(Value::Number(10.0)));
    }

    #[test]
    fn assign_array_index_middle() {
        let interp = run_code(
            r#"
            гыы арр = [1, 2, 3];
            арр[1] = 42;
            гыы результат = арр[1];
            "#,
        );
        assert_eq!(interp.get("результат"), Some(Value::Number(42.0)));
    }

    #[test]
    fn assign_array_index_preserves_other_elements() {
        let interp = run_code(
            r#"
            гыы арр = [10, 20, 30];
            арр[1] = 99;
            гыы а = арр[0];
            гыы б = арр[2];
            "#,
        );
        assert_eq!(interp.get("а"), Some(Value::Number(10.0)));
        assert_eq!(interp.get("б"), Some(Value::Number(30.0)));
    }

    #[test]
    fn assign_array_index_out_of_bounds() {
        let err = run_code_err(
            r#"
            гыы арр = [1, 2];
            арр[5] = 10;
            "#,
        );
        assert!(err.message.contains("вне диапазона") || err.message.contains("Индекс"));
    }

    #[test]
    fn assign_object_member() {
        let interp = run_code(
            r#"
            гыы чел = { имя: "Вася", возраст: 25 };
            чел.имя = "Петя";
            гыы результат = чел.имя;
            "#,
        );
        assert_eq!(interp.get("результат"), Some(Value::String("Петя".to_string())));
    }

    #[test]
    fn assign_object_member_new_property() {
        let interp = run_code(
            r#"
            гыы чел = { имя: "Вася" };
            чел.возраст = 30;
            гыы результат = чел.возраст;
            "#,
        );
        assert_eq!(interp.get("результат"), Some(Value::Number(30.0)));
    }

    #[test]
    fn assign_object_bracket_notation() {
        let interp = run_code(
            r#"
            гыы чел = { имя: "Вася" };
            чел["имя"] = "Коля";
            гыы результат = чел.имя;
            "#,
        );
        assert_eq!(interp.get("результат"), Some(Value::String("Коля".to_string())));
    }

    #[test]
    fn assign_member_on_non_object_fails() {
        let err = run_code_err(
            r#"
            гыы х = 5;
            х.поле = 10;
            "#,
        );
        assert!(err.message.contains("свойство") || err.message.contains("объект"));
    }

    #[test]
    fn compound_assign_array_index() {
        let interp = run_code(
            r#"
            гыы арр = [10, 20, 30];
            арр[0] += 5;
            гыы результат = арр[0];
            "#,
        );
        assert_eq!(interp.get("результат"), Some(Value::Number(15.0)));
    }

    #[test]
    fn compound_assign_object_member() {
        let interp = run_code(
            r#"
            гыы чел = { баланс: 100 };
            чел.баланс -= 30;
            гыы результат = чел.баланс;
            "#,
        );
        assert_eq!(interp.get("результат"), Some(Value::Number(70.0)));
    }

    #[test]
    fn assign_nested_array() {
        let interp = run_code(
            r#"
            гыы матрица = [[1, 2], [3, 4]];
            матрица[0][1] = 99;
            гыы результат = матрица[0][1];
            "#,
        );
        assert_eq!(interp.get("результат"), Some(Value::Number(99.0)));
    }

    #[test]
    fn assign_nested_object() {
        let interp = run_code(
            r#"
            гыы данные = { внутри: { значение: 1 } };
            данные.внутри.значение = 42;
            гыы результат = данные.внутри.значение;
            "#,
        );
        assert_eq!(interp.get("результат"), Some(Value::Number(42.0)));
    }

    #[test]
    fn assign_object_in_array() {
        let interp = run_code(
            r#"
            гыы список = [{ имя: "А" }, { имя: "Б" }];
            список[0].имя = "В";
            гыы результат = список[0].имя;
            "#,
        );
        assert_eq!(interp.get("результат"), Some(Value::String("В".to_string())));
    }

    #[test]
    fn assign_array_in_object() {
        let interp = run_code(
            r#"
            гыы данные = { список: [1, 2, 3] };
            данные.список[2] = 99;
            гыы результат = данные.список[2];
            "#,
        );
        assert_eq!(interp.get("результат"), Some(Value::Number(99.0)));
    }

    #[test]
    fn try_catch_catches_runtime_error() {
        let interp = run_code(
            r#"
            гыы результат = 0;
            хапнуть {
                гыы х = 1 / 0;
            } гоп (е) {
                результат = 1;
            }
            "#,
        );
        assert_eq!(interp.get("результат"), Some(Value::Number(1.0)));
    }

    #[test]
    fn try_catch_catches_throw() {
        let interp = run_code(
            r#"
            гыы результат = "";
            хапнуть {
                кидай "ошибка";
            } гоп (е) {
                результат = е;
            }
            "#,
        );
        assert_eq!(interp.get("результат"), Some(Value::String("ошибка".to_string())));
    }

    #[test]
    fn try_catch_throw_number() {
        let interp = run_code(
            r#"
            гыы результат = 0;
            хапнуть {
                кидай 42;
            } гоп (е) {
                результат = е;
            }
            "#,
        );
        assert_eq!(interp.get("результат"), Some(Value::Number(42.0)));
    }

    #[test]
    fn try_catch_no_error_skips_catch() {
        let interp = run_code(
            r#"
            гыы результат = 1;
            хапнуть {
                результат = 2;
            } гоп (е) {
                результат = 3;
            }
            "#,
        );
        assert_eq!(interp.get("результат"), Some(Value::Number(2.0)));
    }

    #[test]
    fn try_finally_runs_always() {
        let interp = run_code(
            r#"
            гыы результат = 0;
            хапнуть {
                результат = 1;
            } тюряжка {
                результат = результат + 10;
            }
            "#,
        );
        assert_eq!(interp.get("результат"), Some(Value::Number(11.0)));
    }

    #[test]
    fn try_catch_finally_on_error() {
        let interp = run_code(
            r#"
            гыы шаг1 = 0;
            гыы шаг2 = 0;
            хапнуть {
                кидай "бум";
            } гоп (е) {
                шаг1 = 1;
            } тюряжка {
                шаг2 = 1;
            }
            "#,
        );
        assert_eq!(interp.get("шаг1"), Some(Value::Number(1.0)));
        assert_eq!(interp.get("шаг2"), Some(Value::Number(1.0)));
    }

    #[test]
    fn try_catch_finally_no_error() {
        let interp = run_code(
            r#"
            гыы шаг1 = 0;
            гыы шаг2 = 0;
            хапнуть {
                шаг1 = 1;
            } гоп (е) {
                шаг1 = 99;
            } тюряжка {
                шаг2 = 1;
            }
            "#,
        );
        assert_eq!(interp.get("шаг1"), Some(Value::Number(1.0)));
        assert_eq!(interp.get("шаг2"), Some(Value::Number(1.0)));
    }

    #[test]
    fn uncaught_throw_is_error() {
        let err = run_code_err(
            r#"
            кидай "паника";
            "#,
        );
        assert!(err.message.contains("Необработанное исключение"));
    }

    #[test]
    fn try_catch_without_param() {
        let interp = run_code(
            r#"
            гыы результат = 0;
            хапнуть {
                кидай "бум";
            } гоп {
                результат = 1;
            }
            "#,
        );
        assert_eq!(interp.get("результат"), Some(Value::Number(1.0)));
    }

    #[test]
    fn try_catch_runtime_error_message() {
        let interp = run_code(
            r#"
            гыы результат = "";
            хапнуть {
                гыы х = неизвестная;
            } гоп (е) {
                результат = е;
            }
            "#,
        );
        let val = interp.get("результат").unwrap();
        if let Value::String(s) = val {
            assert!(s.contains("не определена"));
        } else {
            panic!("Expected string error message");
        }
    }

    #[test]
    fn nested_try_catch() {
        let interp = run_code(
            r#"
            гыы результат = "";
            хапнуть {
                хапнуть {
                    кидай "внутри";
                } гоп (е) {
                    кидай "снаружи";
                }
            } гоп (е) {
                результат = е;
            }
            "#,
        );
        assert_eq!(interp.get("результат"), Some(Value::String("снаружи".to_string())));
    }

    #[test]
    fn try_catch_with_alias_keywords() {
        let interp = run_code(
            r#"
            гыы результат = 0;
            побратски {
                кидай 1;
            } аченетак (е) {
                результат = е;
            }
            "#,
        );
        assert_eq!(interp.get("результат"), Some(Value::Number(1.0)));
    }

    #[test]
    fn finally_runs_after_throw_without_catch() {
        let interp = run_code(
            r#"
            гыы результат = 0;
            хапнуть {
                хапнуть {
                    кидай "бум";
                } тюряжка {
                    результат = 1;
                }
            } гоп (е) {
                результат = результат + 10;
            }
            "#,
        );
        assert_eq!(interp.get("результат"), Some(Value::Number(11.0)));
    }

    #[test]
    fn switch_matches_first_case() {
        let interp = run_code(
            r#"
            гыы результат = 0;
            базарпо (1) {
                тема 1: {
                    результат = 10;
                }
                тема 2: {
                    результат = 20;
                }
            }
            "#,
        );
        assert_eq!(interp.get("результат"), Some(Value::Number(10.0)));
    }

    #[test]
    fn switch_matches_second_case() {
        let interp = run_code(
            r#"
            гыы результат = 0;
            базарпо (2) {
                тема 1: {
                    результат = 10;
                }
                тема 2: {
                    результат = 20;
                }
            }
            "#,
        );
        assert_eq!(interp.get("результат"), Some(Value::Number(20.0)));
    }

    #[test]
    fn switch_default_when_no_match() {
        let interp = run_code(
            r#"
            гыы результат = 0;
            базарпо (99) {
                тема 1: {
                    результат = 10;
                }
                нуичо {
                    результат = 42;
                }
            }
            "#,
        );
        assert_eq!(interp.get("результат"), Some(Value::Number(42.0)));
    }

    #[test]
    fn switch_no_match_no_default() {
        let interp = run_code(
            r#"
            гыы результат = 0;
            базарпо (99) {
                тема 1: {
                    результат = 10;
                }
            }
            "#,
        );
        assert_eq!(interp.get("результат"), Some(Value::Number(0.0)));
    }

    #[test]
    fn switch_with_string_cases() {
        let interp = run_code(
            r#"
            гыы результат = "";
            базарпо ("привет") {
                тема "пока": {
                    результат = "прощание";
                }
                тема "привет": {
                    результат = "приветствие";
                }
            }
            "#,
        );
        assert_eq!(interp.get("результат"), Some(Value::String("приветствие".to_string())));
    }

    #[test]
    fn switch_with_variable_expr() {
        let interp = run_code(
            r#"
            гыы х = 3;
            гыы результат = 0;
            базарпо (х) {
                тема 1: {
                    результат = 10;
                }
                тема 3: {
                    результат = 30;
                }
                нуичо {
                    результат = 99;
                }
            }
            "#,
        );
        assert_eq!(interp.get("результат"), Some(Value::Number(30.0)));
    }

    #[test]
    fn switch_no_fallthrough() {
        let interp = run_code(
            r#"
            гыы результат = 0;
            базарпо (1) {
                тема 1: {
                    результат = результат + 10;
                }
                тема 2: {
                    результат = результат + 20;
                }
            }
            "#,
        );
        assert_eq!(interp.get("результат"), Some(Value::Number(10.0)));
    }

    #[test]
    fn switch_default_only() {
        let interp = run_code(
            r#"
            гыы результат = 0;
            базарпо (1) {
                нуичо {
                    результат = 42;
                }
            }
            "#,
        );
        assert_eq!(interp.get("результат"), Some(Value::Number(42.0)));
    }

    #[test]
    fn switch_with_return_in_function() {
        let interp = run_code(
            r#"
            йопта проверка(х) {
                базарпо (х) {
                    тема 1: {
                        отвечаю 10;
                    }
                    тема 2: {
                        отвечаю 20;
                    }
                    нуичо {
                        отвечаю 0;
                    }
                }
            }
            гыы а = проверка(1);
            гыы б = проверка(2);
            гыы в = проверка(99);
            "#,
        );
        assert_eq!(interp.get("а"), Some(Value::Number(10.0)));
        assert_eq!(interp.get("б"), Some(Value::Number(20.0)));
        assert_eq!(interp.get("в"), Some(Value::Number(0.0)));
    }

    #[test]
    fn do_while_executes_at_least_once() {
        let interp = run_code(
            r#"
            гыы счётчик = 0;
            крутани {
                счётчик = счётчик + 1;
            } потрещим (лож);
            "#,
        );
        assert_eq!(interp.get("счётчик"), Some(Value::Number(1.0)));
    }

    #[test]
    fn do_while_loops_while_true() {
        let interp = run_code(
            r#"
            гыы счётчик = 0;
            крутани {
                счётчик = счётчик + 1;
            } потрещим (счётчик < 5);
            "#,
        );
        assert_eq!(interp.get("счётчик"), Some(Value::Number(5.0)));
    }

    #[test]
    fn do_while_break() {
        let interp = run_code(
            r#"
            гыы счётчик = 0;
            крутани {
                счётчик = счётчик + 1;
                вилкойвглаз (счётчик == 3) {
                    харэ;
                }
            } потрещим (счётчик < 10);
            "#,
        );
        assert_eq!(interp.get("счётчик"), Some(Value::Number(3.0)));
    }

    #[test]
    fn do_while_continue() {
        let interp = run_code(
            r#"
            гыы счётчик = 0;
            гыы сумма = 0;
            крутани {
                счётчик = счётчик + 1;
                вилкойвглаз (счётчик == 3) {
                    двигай;
                }
                сумма = сумма + счётчик;
            } потрещим (счётчик < 5);
            "#,
        );
        assert_eq!(interp.get("счётчик"), Some(Value::Number(5.0)));
        assert_eq!(interp.get("сумма"), Some(Value::Number(12.0)));
    }

    #[test]
    fn do_while_with_return() {
        let interp = run_code(
            r#"
            йопта сумма() {
                гыы с = 0;
                гыы и = 0;
                крутани {
                    и = и + 1;
                    с = с + и;
                    вилкойвглаз (и == 3) {
                        отвечаю с;
                    }
                } потрещим (правда);
            }
            гыы результат = сумма();
            "#,
        );
        assert_eq!(interp.get("результат"), Some(Value::Number(6.0)));
    }

    #[test]
    fn for_in_array() {
        let interp = run_code(
            r#"
            гыы сумма = 0;
            гыы арр = [1, 2, 3, 4];
            го (х из арр) {
                сумма = сумма + х;
            }
            "#,
        );
        assert_eq!(interp.get("сумма"), Some(Value::Number(10.0)));
    }

    #[test]
    fn for_in_empty_array() {
        let interp = run_code(
            r#"
            гыы сумма = 0;
            го (х из []) {
                сумма = сумма + 1;
            }
            "#,
        );
        assert_eq!(interp.get("сумма"), Some(Value::Number(0.0)));
    }

    #[test]
    fn for_in_object_keys() {
        let interp = run_code(
            r#"
            гыы счётчик = 0;
            гыы чел = { имя: "Вася", возраст: 25 };
            го (к из чел) {
                счётчик = счётчик + 1;
            }
            "#,
        );
        assert_eq!(interp.get("счётчик"), Some(Value::Number(2.0)));
    }

    #[test]
    fn for_in_break() {
        let interp = run_code(
            r#"
            гыы сумма = 0;
            го (х из [10, 20, 30, 40]) {
                сумма = сумма + х;
                вилкойвглаз (х == 20) {
                    харэ;
                }
            }
            "#,
        );
        assert_eq!(interp.get("сумма"), Some(Value::Number(30.0)));
    }

    #[test]
    fn for_in_continue() {
        let interp = run_code(
            r#"
            гыы сумма = 0;
            го (х из [1, 2, 3, 4, 5]) {
                вилкойвглаз (х == 3) {
                    двигай;
                }
                сумма = сумма + х;
            }
            "#,
        );
        assert_eq!(interp.get("сумма"), Some(Value::Number(12.0)));
    }

    #[test]
    fn for_in_with_return() {
        let interp = run_code(
            r#"
            йопта найти(арр) {
                го (х из арр) {
                    вилкойвглаз (х > 3) {
                        отвечаю х;
                    }
                }
                отвечаю 0;
            }
            гыы результат = найти([1, 2, 5, 4]);
            "#,
        );
        assert_eq!(interp.get("результат"), Some(Value::Number(5.0)));
    }

    #[test]
    fn for_in_non_iterable_fails() {
        let err = run_code_err(
            r#"
            го (х из 42) {
                гыы а = 1;
            }
            "#,
        );
        assert!(err.message.contains("итерировать"));
    }

    #[test]
    fn for_in_string_array() {
        let interp = run_code(
            r#"
            гыы результат = "";
            го (с из ["а", "б", "в"]) {
                результат = результат + с;
            }
            "#,
        );
        assert_eq!(interp.get("результат"), Some(Value::String("абв".to_string())));
    }

    #[test]
    fn destructure_array_basic() {
        let interp = run_code(
            r#"
            гыы [а, б, в] = [1, 2, 3];
            "#,
        );
        assert_eq!(interp.get("а"), Some(Value::Number(1.0)));
        assert_eq!(interp.get("б"), Some(Value::Number(2.0)));
        assert_eq!(interp.get("в"), Some(Value::Number(3.0)));
    }

    #[test]
    fn destructure_array_fewer_elements() {
        let interp = run_code(
            r#"
            гыы [а, б] = [1];
            "#,
        );
        assert_eq!(interp.get("а"), Some(Value::Number(1.0)));
        assert_eq!(interp.get("б"), Some(Value::Undefined));
    }

    #[test]
    fn destructure_array_skip_elements() {
        let interp = run_code(
            r#"
            гыы [, , в] = [1, 2, 3];
            "#,
        );
        assert_eq!(interp.get("в"), Some(Value::Number(3.0)));
    }

    #[test]
    fn destructure_array_rest() {
        let interp = run_code(
            r#"
            гыы [а, ...остаток] = [1, 2, 3, 4];
            гыы длинна = длина(остаток);
            гыы б = остаток[0];
            гыы в = остаток[1];
            гыы г = остаток[2];
            "#,
        );
        assert_eq!(interp.get("а"), Some(Value::Number(1.0)));
        assert_eq!(interp.get("длинна"), Some(Value::Number(3.0)));
        assert_eq!(interp.get("б"), Some(Value::Number(2.0)));
        assert_eq!(interp.get("в"), Some(Value::Number(3.0)));
        assert_eq!(interp.get("г"), Some(Value::Number(4.0)));
    }

    #[test]
    fn destructure_array_rest_empty() {
        let interp = run_code(
            r#"
            гыы [а, ...остаток] = [1];
            гыы длинна = длина(остаток);
            "#,
        );
        assert_eq!(interp.get("а"), Some(Value::Number(1.0)));
        assert_eq!(interp.get("длинна"), Some(Value::Number(0.0)));
    }

    #[test]
    fn destructure_array_non_array_fails() {
        let err = run_code_err(
            r#"
            гыы [а, б] = 42;
            "#,
        );
        assert!(err.message.contains("деструктурировать"));
    }

    #[test]
    fn destructure_object_shorthand() {
        let interp = run_code(
            r#"
            гыы {х, у} = { х: 10, у: 20 };
            "#,
        );
        assert_eq!(interp.get("х"), Some(Value::Number(10.0)));
        assert_eq!(interp.get("у"), Some(Value::Number(20.0)));
    }

    #[test]
    fn destructure_object_rename() {
        let interp = run_code(
            r#"
            гыы {х: а, у: б} = { х: 10, у: 20 };
            "#,
        );
        assert_eq!(interp.get("а"), Some(Value::Number(10.0)));
        assert_eq!(interp.get("б"), Some(Value::Number(20.0)));
    }

    #[test]
    fn destructure_object_missing_key() {
        let interp = run_code(
            r#"
            гыы {х, з} = { х: 10, у: 20 };
            "#,
        );
        assert_eq!(interp.get("х"), Some(Value::Number(10.0)));
        assert_eq!(interp.get("з"), Some(Value::Undefined));
    }

    #[test]
    fn destructure_object_rest() {
        let interp = run_code(
            r#"
            гыы {х, ...остаток} = { х: 1, у: 2, з: 3 };
            "#,
        );
        assert_eq!(interp.get("х"), Some(Value::Number(1.0)));
        let rest = interp.get("остаток").unwrap();
        if let Value::Object(map) = rest {
            assert_eq!(map.get("у"), Some(&Value::Number(2.0)));
            assert_eq!(map.get("з"), Some(&Value::Number(3.0)));
            assert_eq!(map.len(), 2);
        } else {
            panic!("Ожидался объект");
        }
    }

    #[test]
    fn destructure_object_non_object_fails() {
        let err = run_code_err(
            r#"
            гыы {х} = 42;
            "#,
        );
        assert!(err.message.contains("деструктурировать"));
    }

    #[test]
    fn destructure_nested_array_in_array() {
        let interp = run_code(
            r#"
            гыы [а, [б, в]] = [1, [2, 3]];
            "#,
        );
        assert_eq!(interp.get("а"), Some(Value::Number(1.0)));
        assert_eq!(interp.get("б"), Some(Value::Number(2.0)));
        assert_eq!(interp.get("в"), Some(Value::Number(3.0)));
    }

    #[test]
    fn destructure_object_in_array() {
        let interp = run_code(
            r#"
            гыы [а, {б}] = [1, { б: 2 }];
            "#,
        );
        assert_eq!(interp.get("а"), Some(Value::Number(1.0)));
        assert_eq!(interp.get("б"), Some(Value::Number(2.0)));
    }

    #[test]
    fn destructure_array_in_object() {
        let interp = run_code(
            r#"
            гыы {данные: [а, б]} = { данные: [10, 20] };
            "#,
        );
        assert_eq!(interp.get("а"), Some(Value::Number(10.0)));
        assert_eq!(interp.get("б"), Some(Value::Number(20.0)));
    }

    #[test]
    fn destructure_const_array() {
        let err = run_code_err(
            r#"
            участковый [а, б] = [1, 2];
            а = 10;
            "#,
        );
        assert!(err.message.contains("константу") || err.message.contains("const"));
    }

    #[test]
    fn destructure_const_object() {
        let err = run_code_err(
            r#"
            участковый {х, у} = { х: 1, у: 2 };
            х = 10;
            "#,
        );
        assert!(err.message.contains("константу") || err.message.contains("const"));
    }

    #[test]
    fn string_escape_newline() {
        let interp = run_code(
            r#"
            гыы с = "привет\nмир";
            "#,
        );
        assert_eq!(interp.get("с"), Some(Value::String("привет\nмир".to_string())));
    }

    #[test]
    fn string_escape_tab() {
        let interp = run_code(
            r#"
            гыы с = "а\tб";
            "#,
        );
        assert_eq!(interp.get("с"), Some(Value::String("а\tб".to_string())));
    }

    #[test]
    fn string_escape_backslash() {
        let interp = run_code(
            r#"
            гыы с = "путь\\файл";
            "#,
        );
        assert_eq!(interp.get("с"), Some(Value::String("путь\\файл".to_string())));
    }

    #[test]
    fn string_escape_quote() {
        let interp = run_code(
            r#"
            гыы с = "он сказал \"да\"";
            "#,
        );
        assert_eq!(interp.get("с"), Some(Value::String("он сказал \"да\"".to_string())));
    }

    #[test]
    fn string_escape_combined() {
        let interp = run_code(
            r#"
            гыы с = "строка1\nстрока2\tтаб";
            "#,
        );
        assert_eq!(interp.get("с"), Some(Value::String("строка1\nстрока2\tтаб".to_string())));
    }

    #[test]
    fn ternary_true_branch() {
        let interp = run_code(
            r#"
            гыы р = правда ? 10 : 20;
            "#,
        );
        assert_eq!(interp.get("р"), Some(Value::Number(10.0)));
    }

    #[test]
    fn ternary_false_branch() {
        let interp = run_code(
            r#"
            гыы р = лож ? 10 : 20;
            "#,
        );
        assert_eq!(interp.get("р"), Some(Value::Number(20.0)));
    }

    #[test]
    fn ternary_with_expression_condition() {
        let interp = run_code(
            r#"
            гыы x = 7;
            гыы р = x > 5 ? "да" : "нет";
            "#,
        );
        assert_eq!(interp.get("р"), Some(Value::String("да".to_string())));
    }

    #[test]
    fn ternary_nested() {
        let interp = run_code(
            r#"
            гыы x = 3;
            гыы р = x > 10 ? "большое" : x > 5 ? "среднее" : "маленькое";
            "#,
        );
        assert_eq!(interp.get("р"), Some(Value::String("маленькое".to_string())));
    }

    #[test]
    fn ternary_with_function_call() {
        let interp = run_code(
            r#"
            гыы arr = [1, 2, 3];
            гыы р = длина(arr) > 0 ? arr[0] : ноль;
            "#,
        );
        assert_eq!(interp.get("р"), Some(Value::Number(1.0)));
    }

    #[test]
    fn arrow_function_expr_body() {
        let interp = run_code(
            r#"
            гыы двойное = (х) => х * 2;
            гыы р = двойное(5);
            "#,
        );
        assert_eq!(interp.get("р"), Some(Value::Number(10.0)));
    }

    #[test]
    fn arrow_function_block_body() {
        let interp = run_code(
            r#"
            гыы сумма = (а, б) => {
                отвечаю а + б;
            };
            гыы р = сумма(3, 4);
            "#,
        );
        assert_eq!(interp.get("р"), Some(Value::Number(7.0)));
    }

    #[test]
    fn arrow_function_no_params() {
        let interp = run_code(
            r#"
            гыы привет = () => "здарова";
            гыы р = привет();
            "#,
        );
        assert_eq!(interp.get("р"), Some(Value::String("здарова".into())));
    }

    #[test]
    fn arrow_function_single_param_no_parens() {
        let interp = run_code(
            r#"
            гыы квадрат = х => х * х;
            гыы р = квадрат(6);
            "#,
        );
        assert_eq!(interp.get("р"), Some(Value::Number(36.0)));
    }

    #[test]
    fn arrow_function_as_callback() {
        let interp = run_code(
            r#"
            йопта применить(ф, знач) {
                отвечаю ф(знач);
            }
            гыы р = применить((х) => х + 10, 5);
            "#,
        );
        assert_eq!(interp.get("р"), Some(Value::Number(15.0)));
    }

    #[test]
    fn arrow_function_iife() {
        let interp = run_code(
            r#"
            гыы р = ((а, б) => а * б)(3, 7);
            "#,
        );
        assert_eq!(interp.get("р"), Some(Value::Number(21.0)));
    }

    #[test]
    fn template_no_substitution() {
        let interp = run_code("гыы р = `привет мир`;");
        assert_eq!(interp.get("р"), Some(Value::String("привет мир".to_string())));
    }

    #[test]
    fn template_empty() {
        let interp = run_code("гыы р = ``;");
        assert_eq!(interp.get("р"), Some(Value::String(String::new())));
    }

    #[test]
    fn template_single_interpolation() {
        let interp = run_code(
            r#"
            гыы имя = "Вася";
            гыы р = `привет, ${имя}!`;
            "#,
        );
        assert_eq!(interp.get("р"), Some(Value::String("привет, Вася!".to_string())));
    }

    #[test]
    fn template_multiple_interpolations() {
        let interp = run_code(
            r#"
            гыы а = 1;
            гыы б = 2;
            гыы р = `${а} + ${б} = ${а + б}`;
            "#,
        );
        assert_eq!(interp.get("р"), Some(Value::String("1 + 2 = 3".to_string())));
    }

    #[test]
    fn template_expression_interpolation() {
        let interp = run_code("гыы р = `результат: ${2 + 3 * 4}`;");
        assert_eq!(interp.get("р"), Some(Value::String("результат: 14".to_string())));
    }

    #[test]
    fn template_with_escape() {
        let interp = run_code("гыы р = `строка1\\nстрока2`;");
        assert_eq!(interp.get("р"), Some(Value::String("строка1\nстрока2".to_string())));
    }

    #[test]
    fn template_multiline() {
        let interp = run_code("гыы р = `строка1\nстрока2`;");
        assert_eq!(interp.get("р"), Some(Value::String("строка1\nстрока2".to_string())));
    }

    #[test]
    fn template_nested() {
        let interp = run_code(
            r#"
            гыы х = 5;
            гыы р = `внешний ${`внутренний ${х}`}`;
            "#,
        );
        assert_eq!(interp.get("р"), Some(Value::String("внешний внутренний 5".to_string())));
    }

    #[test]
    fn template_with_object_in_braces() {
        let interp = run_code(
            r#"
            гыы а = [1, 2, 3];
            гыы р = `длина: ${длина(а)}`;
            "#,
        );
        assert_eq!(interp.get("р"), Some(Value::String("длина: 3".to_string())));
    }

    #[test]
    fn template_only_interpolation() {
        let interp = run_code(
            r#"
            гыы х = 42;
            гыы р = `${х}`;
            "#,
        );
        assert_eq!(interp.get("р"), Some(Value::String("42".to_string())));
    }

    #[test]
    fn template_escaped_dollar() {
        let interp = run_code("гыы р = `цена: \\${100}`;");
        assert_eq!(interp.get("р"), Some(Value::String("цена: ${100}".to_string())));
    }

    #[test]
    fn template_ternary_inside() {
        let interp = run_code(
            r#"
            гыы х = 10;
            гыы р = `число ${х > 5 ? "большое" : "маленькое"}`;
            "#,
        );
        assert_eq!(interp.get("р"), Some(Value::String("число большое".to_string())));
    }

    #[test]
    fn function_without_return_gives_undefined() {
        let interp = run_code(
            r#"
            йопта ф() {}
            гыы р = ф();
            "#,
        );
        assert_eq!(interp.get("р"), Some(Value::Undefined));
    }

    #[test]
    fn return_without_value_gives_undefined() {
        let interp = run_code(
            r#"
            йопта ф() { отвечаю; }
            гыы р = ф();
            "#,
        );
        assert_eq!(interp.get("р"), Some(Value::Undefined));
    }

    #[test]
    fn missing_object_property_gives_undefined() {
        let interp = run_code(
            r#"
            гыы о = { а: 1 };
            гыы р = о.б;
            "#,
        );
        assert_eq!(interp.get("р"), Some(Value::Undefined));
    }

    #[test]
    fn array_index_out_of_bounds_gives_undefined() {
        let interp = run_code(
            r#"
            гыы м = [1, 2, 3];
            гыы р = м[10];
            "#,
        );
        assert_eq!(interp.get("р"), Some(Value::Undefined));
    }

    #[test]
    fn typeof_undefined() {
        let interp = run_code(
            r#"
            йопта ф() {}
            гыы р = тип(ф());
            "#,
        );
        assert_eq!(interp.get("р"), Some(Value::String("неопределено".to_string())));
    }

    #[test]
    fn null_not_equal_undefined() {
        let interp = run_code(
            r#"
            йопта ф() {}
            гыы р = ф() == ноль;
            "#,
        );
        assert_eq!(interp.get("р"), Some(Value::Boolean(false)));
    }

    #[test]
    fn closure_captures_variable() {
        let interp = run_code(
            r#"
            йопта создать() {
                гыы н = 0;
                отвечаю () => {
                    н = н + 1;
                    отвечаю н;
                };
            }
            гыы инкр = создать();
            гыы а = инкр();
            гыы б = инкр();
            гыы в = инкр();
            "#,
        );
        assert_eq!(interp.get("а"), Some(Value::Number(1.0)));
        assert_eq!(interp.get("б"), Some(Value::Number(2.0)));
        assert_eq!(interp.get("в"), Some(Value::Number(3.0)));
    }

    #[test]
    fn closure_independent_instances() {
        let interp = run_code(
            r#"
            йопта счётчик() {
                гыы н = 0;
                отвечаю () => {
                    н = н + 1;
                    отвечаю н;
                };
            }
            гыы а = счётчик();
            гыы б = счётчик();
            а();
            а();
            б();
            гыы ра = а();
            гыы рб = б();
            "#,
        );
        assert_eq!(interp.get("ра"), Some(Value::Number(3.0)));
        assert_eq!(interp.get("рб"), Some(Value::Number(2.0)));
    }

    #[test]
    fn closure_captures_outer_scope() {
        let interp = run_code(
            r#"
            гыы х = 10;
            йопта создать() {
                отвечаю () => х;
            }
            гыы получить = создать();
            гыы р = получить();
            "#,
        );
        assert_eq!(interp.get("р"), Some(Value::Number(10.0)));
    }

    #[test]
    fn closure_sees_mutations_of_shared_variable() {
        let interp = run_code(
            r#"
            йопта создать() {
                гыы н = 0;
                гыы инкр = () => {
                    н = н + 1;
                };
                гыы получить = () => н;
                отвечаю [инкр, получить];
            }
            гыы пара = создать();
            гыы инкр = пара[0];
            гыы получить = пара[1];
            инкр();
            инкр();
            гыы р = получить();
            "#,
        );
        assert_eq!(interp.get("р"), Some(Value::Number(2.0)));
    }

    #[test]
    fn closure_in_loop() {
        let interp = run_code(
            r#"
            гыы функции = [];
            го (гыы и = 0; и < 3; и++) {
                гыы текущий = и;
                функции = втолкнуть(функции, () => текущий);
            }
            гыы а = функции[0]();
            гыы б = функции[1]();
            гыы в = функции[2]();
            "#,
        );
        assert_eq!(interp.get("а"), Some(Value::Number(0.0)));
        assert_eq!(interp.get("б"), Some(Value::Number(1.0)));
        assert_eq!(interp.get("в"), Some(Value::Number(2.0)));
    }

    #[test]
    fn nested_closure() {
        let interp = run_code(
            r#"
            йопта внешняя(х) {
                отвечаю (у) => {
                    отвечаю () => х + у;
                };
            }
            гыы ф = внешняя(10)(20);
            гыы р = ф();
            "#,
        );
        assert_eq!(interp.get("р"), Some(Value::Number(30.0)));
    }

    #[test]
    fn exponent_basic() {
        let interp = run_code("гыы р = 2 ** 3;");
        assert_eq!(interp.get("р"), Some(Value::Number(8.0)));
    }

    #[test]
    fn exponent_right_associative() {
        let interp = run_code("гыы р = 2 ** 3 ** 2;");
        assert_eq!(interp.get("р"), Some(Value::Number(512.0)));
    }

    #[test]
    fn exponent_with_multiply() {
        let interp = run_code("гыы р = 3 * 2 ** 3;");
        assert_eq!(interp.get("р"), Some(Value::Number(24.0)));
    }

    #[test]
    fn exponent_assign() {
        let interp = run_code(
            r#"
            гыы р = 2;
            р **= 10;
            "#,
        );
        assert_eq!(interp.get("р"), Some(Value::Number(1024.0)));
    }

    #[test]
    fn nullish_coalescing_null() {
        let interp = run_code("гыы р = ноль ?? 42;");
        assert_eq!(interp.get("р"), Some(Value::Number(42.0)));
    }

    #[test]
    fn nullish_coalescing_undefined() {
        let interp = run_code("гыы р = неибу ?? 42;");
        assert_eq!(interp.get("р"), Some(Value::Number(42.0)));
    }

    #[test]
    fn nullish_coalescing_non_null() {
        let interp = run_code("гыы р = 0 ?? 42;");
        assert_eq!(interp.get("р"), Some(Value::Number(0.0)));
    }

    #[test]
    fn nullish_coalescing_false_is_not_nullish() {
        let interp = run_code("гыы р = лож ?? 42;");
        assert_eq!(interp.get("р"), Some(Value::Boolean(false)));
    }

    #[test]
    fn nullish_coalescing_chain() {
        let interp = run_code("гыы р = ноль ?? неибу ?? 7;");
        assert_eq!(interp.get("р"), Some(Value::Number(7.0)));
    }

    #[test]
    fn nullish_assign_null() {
        let interp = run_code(
            r#"
            гыы р = ноль;
            р ??= 99;
            "#,
        );
        assert_eq!(interp.get("р"), Some(Value::Number(99.0)));
    }

    #[test]
    fn nullish_assign_non_null() {
        let interp = run_code(
            r#"
            гыы р = 5;
            р ??= 99;
            "#,
        );
        assert_eq!(interp.get("р"), Some(Value::Number(5.0)));
    }

    #[test]
    fn alias_true_trulio() {
        let interp = run_code("гыы р = трулио;");
        assert_eq!(interp.get("р"), Some(Value::Boolean(true)));
    }

    #[test]
    fn alias_true_chotko() {
        let interp = run_code("гыы р = чотко;");
        assert_eq!(interp.get("р"), Some(Value::Boolean(true)));
    }

    #[test]
    fn alias_false_netrulio() {
        let interp = run_code("гыы р = нетрулио;");
        assert_eq!(interp.get("р"), Some(Value::Boolean(false)));
    }

    #[test]
    fn alias_false_pizdish() {
        let interp = run_code("гыы р = пиздишь;");
        assert_eq!(interp.get("р"), Some(Value::Boolean(false)));
    }

    #[test]
    fn alias_null_nullio() {
        let interp = run_code("гыы р = нуллио;");
        assert_eq!(interp.get("р"), Some(Value::Null));
    }

    #[test]
    fn alias_null_porozhnyak() {
        let interp = run_code("гыы р = порожняк;");
        assert_eq!(interp.get("р"), Some(Value::Null));
    }

    #[test]
    fn alias_undefined_neibu() {
        let interp = run_code("гыы р = неибу;");
        assert_eq!(interp.get("р"), Some(Value::Undefined));
    }

    #[test]
    fn alias_throw_pnh() {
        let interp = run_code(
            r#"
            гыы р = 0;
            хапнуть {
                пнх 42;
            } гоп (е) {
                р = е;
            }
            "#,
        );
        assert_eq!(interp.get("р"), Some(Value::Number(42.0)));
    }

    #[test]
    fn alias_switch_estcho() {
        let interp = run_code(
            r#"
            гыы р = 0;
            естьчо (1) {
                лещ 1: {
                    р = 10;
                }
                аеслинайду 2: {
                    р = 20;
                }
                пахану {
                    р = 99;
                }
            }
            "#,
        );
        assert_eq!(interp.get("р"), Some(Value::Number(10.0)));
    }

    #[test]
    fn alias_do_while_krch() {
        let interp = run_code(
            r#"
            гыы р = 0;
            крч {
                р = р + 1;
            } потрещим (р < 3);
            "#,
        );
        assert_eq!(interp.get("р"), Some(Value::Number(3.0)));
    }

    #[test]
    #[allow(clippy::approx_constant)]
    fn alias_const_yasen_huy_capital() {
        let interp = run_code(
            r#"
            ЯсенХуй ПИ = 3.14;
            гыы р = ПИ;
            "#,
        );
        assert_eq!(interp.get("р"), Some(Value::Number(3.14)));
    }

    #[test]
    fn optional_chain_member_on_object() {
        let interp = run_code(
            r#"
            гыы чел = { имя: "Вася" };
            гыы р = чел?.имя;
            "#,
        );
        assert_eq!(interp.get("р"), Some(Value::String("Вася".to_string())));
    }

    #[test]
    fn optional_chain_member_on_null() {
        let interp = run_code(
            r#"
            гыы чел = ноль;
            гыы р = чел?.имя;
            "#,
        );
        assert_eq!(interp.get("р"), Some(Value::Undefined));
    }

    #[test]
    fn optional_chain_member_on_undefined() {
        let interp = run_code(
            r#"
            гыы чел = неибу;
            гыы р = чел?.имя;
            "#,
        );
        assert_eq!(interp.get("р"), Some(Value::Undefined));
    }

    #[test]
    fn optional_chain_nested() {
        let interp = run_code(
            r#"
            гыы данные = { а: { б: 42 } };
            гыы р = данные?.а?.б;
            "#,
        );
        assert_eq!(interp.get("р"), Some(Value::Number(42.0)));
    }

    #[test]
    fn optional_chain_nested_null() {
        let interp = run_code(
            r#"
            гыы данные = { а: ноль };
            гыы р = данные?.а?.б;
            "#,
        );
        assert_eq!(interp.get("р"), Some(Value::Undefined));
    }

    #[test]
    fn optional_chain_index() {
        let interp = run_code(
            r#"
            гыы арр = [10, 20, 30];
            гыы р = арр?.[1];
            "#,
        );
        assert_eq!(interp.get("р"), Some(Value::Number(20.0)));
    }

    #[test]
    fn optional_chain_index_on_null() {
        let interp = run_code(
            r#"
            гыы арр = ноль;
            гыы р = арр?.[0];
            "#,
        );
        assert_eq!(interp.get("р"), Some(Value::Undefined));
    }

    #[test]
    fn optional_chain_call() {
        let interp = run_code(
            r#"
            гыы ф = () => { отвечаю 42; };
            гыы р = ф?.();
            "#,
        );
        assert_eq!(interp.get("р"), Some(Value::Number(42.0)));
    }

    #[test]
    fn optional_chain_call_on_null() {
        let interp = run_code(
            r#"
            гыы ф = ноль;
            гыы р = ф?.();
            "#,
        );
        assert_eq!(interp.get("р"), Some(Value::Undefined));
    }

    #[test]
    fn logical_and_assign_truthy() {
        let interp = run_code(
            r#"
            гыы а = 1;
            а &&= 42;
            "#,
        );
        assert_eq!(interp.get("а"), Some(Value::Number(42.0)));
    }

    #[test]
    fn logical_and_assign_falsy() {
        let interp = run_code(
            r#"
            гыы а = 0;
            а &&= 42;
            "#,
        );
        assert_eq!(interp.get("а"), Some(Value::Number(0.0)));
    }

    #[test]
    fn logical_or_assign_falsy() {
        let interp = run_code(
            r#"
            гыы а = 0;
            а ||= 42;
            "#,
        );
        assert_eq!(interp.get("а"), Some(Value::Number(42.0)));
    }

    #[test]
    fn logical_or_assign_truthy() {
        let interp = run_code(
            r#"
            гыы а = 1;
            а ||= 42;
            "#,
        );
        assert_eq!(interp.get("а"), Some(Value::Number(1.0)));
    }
}
