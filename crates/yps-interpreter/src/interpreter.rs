use std::collections::HashMap;

use yps_lexer::Span;
use yps_parser::ast::{BinaryOp, Block, Expr, Literal, PostfixOp, Program, Stmt, UnaryOp};

use crate::builtins::{builtin_names, call_builtin};
use crate::environment::Environment;
use crate::error::RuntimeError;
use crate::value::Value;

enum ControlFlow {
    Break,
    Continue,
    Return(Value),
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
                }
            }
        }
        Ok(())
    }

    fn exec_stmt(&mut self, stmt: &Stmt) -> Result<Option<ControlFlow>, RuntimeError> {
        match stmt {
            Stmt::VarDecl { name, init, is_const, .. } => {
                let value = self.eval_expr(init)?;
                self.env.define(name.name.clone(), value, *is_const);
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
                            ControlFlow::Return(v) => return Ok(Some(ControlFlow::Return(v))),
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
                            ControlFlow::Return(v) => {
                                self.env.pop_scope();
                                return Ok(Some(ControlFlow::Return(v)));
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
                };
                self.env.define(name.name.clone(), func, false);
                Ok(None)
            }
            Stmt::Return { value, .. } => {
                let val = match value {
                    Some(expr) => self.eval_expr(expr)?,
                    None => Value::Null,
                };
                Ok(Some(ControlFlow::Return(val)))
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

    fn eval_expr(&mut self, expr: &Expr) -> Result<Value, RuntimeError> {
        match expr {
            Expr::Literal(lit) => self.eval_literal(lit),
            Expr::Identifier(ident) => {
                self.env.get(&ident.name).cloned().ok_or_else(|| {
                    RuntimeError::new(format!("Переменная '{}' не определена", ident.name), ident.span)
                })
            }
            Expr::Unary { op, expr, span } => {
                let val = self.eval_expr(expr)?;
                self.eval_unary(*op, val, *span)
            }
            Expr::Binary { op, lhs, rhs, span } => {
                // short-circuit for logical operators
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
                if *op == BinaryOp::Assign {
                    return self.eval_assignment(lhs, rhs, *span);
                }
                if matches!(op, BinaryOp::PlusAssign | BinaryOp::MinusAssign | BinaryOp::MulAssign | BinaryOp::DivAssign) {
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
        }
    }

    fn eval_literal(&mut self, lit: &Literal) -> Result<Value, RuntimeError> {
        match lit {
            Literal::Number { raw, span } => {
                raw.parse::<f64>().map(Value::Number).map_err(|_| {
                    RuntimeError::new(format!("Невалидное число: '{raw}'"), *span)
                })
            }
            Literal::String { value, .. } => Ok(Value::String(value.clone())),
            Literal::Boolean { value, .. } => Ok(Value::Boolean(*value)),
            Literal::Null { .. } => Ok(Value::Null),
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
                _ => Err(RuntimeError::new(
                    format!("Нельзя применить '-' к типу '{}'", val.type_name()),
                    span,
                )),
            },
            UnaryOp::Plus => match val {
                Value::Number(n) => Ok(Value::Number(n)),
                _ => Err(RuntimeError::new(
                    format!("Нельзя применить '+' к типу '{}'", val.type_name()),
                    span,
                )),
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
            BinaryOp::Equals | BinaryOp::StrictEquals => Ok(Value::Boolean(left == right)),
            BinaryOp::NotEquals | BinaryOp::StrictNotEquals => Ok(Value::Boolean(left != right)),
            BinaryOp::Less => self.compare_op(&left, &right, span, |a, b| a < b),
            BinaryOp::Greater => self.compare_op(&left, &right, span, |a, b| a > b),
            BinaryOp::LessOrEqual => self.compare_op(&left, &right, span, |a, b| a <= b),
            BinaryOp::GreaterOrEqual => self.compare_op(&left, &right, span, |a, b| a >= b),
            BinaryOp::And | BinaryOp::Or => unreachable!("handled in eval_expr"),
            BinaryOp::Assign | BinaryOp::PlusAssign | BinaryOp::MinusAssign
            | BinaryOp::MulAssign | BinaryOp::DivAssign => unreachable!("handled in eval_expr"),
        }
    }

    fn eval_assignment(&mut self, lhs: &Expr, rhs: &Expr, span: Span) -> Result<Value, RuntimeError> {
        let val = self.eval_expr(rhs)?;
        match lhs {
            Expr::Identifier(ident) => {
                self.set_variable(&ident.name, val.clone(), span)?;
                Ok(val)
            }
            _ => Err(RuntimeError::new("Левая сторона присваивания должна быть переменной", span)),
        }
    }

    fn eval_compound_assignment(&mut self, op: BinaryOp, lhs: &Expr, rhs: &Expr, span: Span) -> Result<Value, RuntimeError> {
        let Expr::Identifier(ident) = lhs else {
            return Err(RuntimeError::new("Левая сторона присваивания должна быть переменной", span));
        };
        let left = self.env.get(&ident.name).cloned().ok_or_else(|| {
            RuntimeError::new(format!("Переменная '{}' не определена", ident.name), span)
        })?;
        let right = self.eval_expr(rhs)?;
        let arith_op = match op {
            BinaryOp::PlusAssign => BinaryOp::Add,
            BinaryOp::MinusAssign => BinaryOp::Sub,
            BinaryOp::MulAssign => BinaryOp::Mul,
            BinaryOp::DivAssign => BinaryOp::Div,
            _ => unreachable!(),
        };
        let result = self.eval_binary(arith_op, left, right, span)?;
        self.set_variable(&ident.name, result.clone(), span)?;
        Ok(result)
    }

    fn eval_postfix(&mut self, op: PostfixOp, expr: &Expr, span: Span) -> Result<Value, RuntimeError> {
        let Expr::Identifier(ident) = expr else {
            return Err(RuntimeError::new("'++' / '--' можно применить только к переменной", span));
        };
        let old = self.env.get(&ident.name).cloned().ok_or_else(|| {
            RuntimeError::new(format!("Переменная '{}' не определена", ident.name), span)
        })?;
        let Value::Number(n) = old else {
            return Err(RuntimeError::new(
                format!("'++' / '--' требует число, получено '{}'", old.type_name()),
                span,
            ));
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
            return Err(RuntimeError::new(
                format!("Нельзя изменить константу '{name}'"),
                span,
            ));
        }
        if !self.env.set(name, value) {
            return Err(RuntimeError::new(
                format!("Переменная '{name}' не определена"),
                span,
            ));
        }
        Ok(())
    }

    fn numeric_op(&self, left: &Value, right: &Value, span: Span, f: fn(f64, f64) -> f64) -> Result<Value, RuntimeError> {
        match (left, right) {
            (Value::Number(a), Value::Number(b)) => Ok(Value::Number(f(*a, *b))),
            _ => Err(RuntimeError::new(
                format!("Операция требует числа, получено '{}' и '{}'", left.type_name(), right.type_name()),
                span,
            )),
        }
    }

    fn compare_op(&self, left: &Value, right: &Value, span: Span, f: fn(f64, f64) -> bool) -> Result<Value, RuntimeError> {
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
            Value::Function { name, params, body } => {
                if args.len() != params.len() {
                    return Err(RuntimeError::new(
                        format!("Функция '{}' ожидает {} аргумент(ов), получено {}", name, params.len(), args.len()),
                        span,
                    ));
                }
                self.env.push_scope();
                for (param, arg) in params.iter().zip(args) {
                    self.env.define(param.clone(), arg, false);
                }
                let result = self.exec_block_stmts(&body.stmts);
                self.env.pop_scope();
                match result? {
                    Some(ControlFlow::Return(val)) => Ok(val),
                    Some(ControlFlow::Break) => Err(RuntimeError::new("'харэ' вне цикла", span)),
                    Some(ControlFlow::Continue) => Err(RuntimeError::new("'двигай' вне цикла", span)),
                    None => Ok(Value::Null),
                }
            }
            _ => Err(RuntimeError::new(
                format!("'{}' не является функцией", func.type_name()),
                span,
            )),
        }
    }

    fn eval_index(&self, obj: Value, index: Value, span: Span) -> Result<Value, RuntimeError> {
        match (&obj, &index) {
            (Value::Array(arr), Value::Number(n)) => {
                let i = *n as usize;
                arr.get(i).cloned().ok_or_else(|| {
                    RuntimeError::new(format!("Индекс {i} вне диапазона (длина {})", arr.len()), span)
                })
            }
            (Value::Object(map), Value::String(key)) => {
                map.get(key).cloned().ok_or_else(|| {
                    RuntimeError::new(format!("Ключ '{key}' не найден в объекте"), span)
                })
            }
            _ => Err(RuntimeError::new(
                format!("Нельзя индексировать '{}' с помощью '{}'", obj.type_name(), index.type_name()),
                span,
            )),
        }
    }

    fn eval_member(&self, obj: Value, property: &str, span: Span) -> Result<Value, RuntimeError> {
        match &obj {
            Value::Object(map) => {
                map.get(property).cloned().ok_or_else(|| {
                    RuntimeError::new(format!("Свойство '{property}' не найдено в объекте"), span)
                })
            }
            _ => Err(RuntimeError::new(
                format!("Нельзя получить свойство у типа '{}'", obj.type_name()),
                span,
            )),
        }
    }
}
