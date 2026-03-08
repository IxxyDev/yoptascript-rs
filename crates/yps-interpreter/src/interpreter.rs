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

    pub fn get(&self, name: &str) -> Option<&Value> {
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
            Stmt::Throw { value, .. } => {
                let val = self.eval_expr(value)?;
                Ok(Some(ControlFlow::Throw(val)))
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

    fn eval_expr(&mut self, expr: &Expr) -> Result<Value, RuntimeError> {
        match expr {
            Expr::Literal(lit) => self.eval_literal(lit),
            Expr::Identifier(ident) => self
                .env
                .get(&ident.name)
                .cloned()
                .ok_or_else(|| RuntimeError::new(format!("Переменная '{}' не определена", ident.name), ident.span)),
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
                if matches!(
                    op,
                    BinaryOp::PlusAssign | BinaryOp::MinusAssign | BinaryOp::MulAssign | BinaryOp::DivAssign
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
            BinaryOp::Equals | BinaryOp::StrictEquals => Ok(Value::Boolean(left == right)),
            BinaryOp::NotEquals | BinaryOp::StrictNotEquals => Ok(Value::Boolean(left != right)),
            BinaryOp::Less => self.compare_op(&left, &right, span, |a, b| a < b),
            BinaryOp::Greater => self.compare_op(&left, &right, span, |a, b| a > b),
            BinaryOp::LessOrEqual => self.compare_op(&left, &right, span, |a, b| a <= b),
            BinaryOp::GreaterOrEqual => self.compare_op(&left, &right, span, |a, b| a >= b),
            BinaryOp::And | BinaryOp::Or => unreachable!("handled in eval_expr"),
            BinaryOp::Assign
            | BinaryOp::PlusAssign
            | BinaryOp::MinusAssign
            | BinaryOp::MulAssign
            | BinaryOp::DivAssign => unreachable!("handled in eval_expr"),
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
                let root = self
                    .env
                    .get_mut(&root_name)
                    .ok_or_else(|| RuntimeError::new(format!("Переменная '{root_name}' не определена"), span))?;
                Self::set_at_path(root, &path, value.clone(), span)?;
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
            .cloned()
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
                    Some(ControlFlow::Throw(val)) => {
                        Err(RuntimeError::new(format!("Необработанное исключение: {val}"), span))
                    }
                    None => Ok(Value::Null),
                }
            }
            _ => Err(RuntimeError::new(format!("'{}' не является функцией", func.type_name()), span)),
        }
    }

    fn eval_index(&self, obj: Value, index: Value, span: Span) -> Result<Value, RuntimeError> {
        match (&obj, &index) {
            (Value::Array(arr), Value::Number(n)) => {
                let i = *n as usize;
                arr.get(i)
                    .cloned()
                    .ok_or_else(|| RuntimeError::new(format!("Индекс {i} вне диапазона (длина {})", arr.len()), span))
            }
            (Value::Object(map), Value::String(key)) => map
                .get(key)
                .cloned()
                .ok_or_else(|| RuntimeError::new(format!("Ключ '{key}' не найден в объекте"), span)),
            _ => Err(RuntimeError::new(
                format!("Нельзя индексировать '{}' с помощью '{}'", obj.type_name(), index.type_name()),
                span,
            )),
        }
    }

    fn eval_member(&self, obj: Value, property: &str, span: Span) -> Result<Value, RuntimeError> {
        match &obj {
            Value::Object(map) => map
                .get(property)
                .cloned()
                .ok_or_else(|| RuntimeError::new(format!("Свойство '{property}' не найдено в объекте"), span)),
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

    // ── Присваивание по индексу массива ──

    #[test]
    fn assign_array_index() {
        let interp = run_code(
            r#"
            гыы арр = [1, 2, 3];
            арр[0] = 10;
            гыы результат = арр[0];
            "#,
        );
        assert_eq!(interp.get("результат"), Some(&Value::Number(10.0)));
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
        assert_eq!(interp.get("результат"), Some(&Value::Number(42.0)));
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
        assert_eq!(interp.get("а"), Some(&Value::Number(10.0)));
        assert_eq!(interp.get("б"), Some(&Value::Number(30.0)));
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

    // ── Присваивание по свойству объекта ──

    #[test]
    fn assign_object_member() {
        let interp = run_code(
            r#"
            гыы чел = { имя: "Вася", возраст: 25 };
            чел.имя = "Петя";
            гыы результат = чел.имя;
            "#,
        );
        assert_eq!(interp.get("результат"), Some(&Value::String("Петя".to_string())));
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
        assert_eq!(interp.get("результат"), Some(&Value::Number(30.0)));
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
        assert_eq!(interp.get("результат"), Some(&Value::String("Коля".to_string())));
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

    // ── Составное присваивание по индексу/свойству ──

    #[test]
    fn compound_assign_array_index() {
        let interp = run_code(
            r#"
            гыы арр = [10, 20, 30];
            арр[0] += 5;
            гыы результат = арр[0];
            "#,
        );
        assert_eq!(interp.get("результат"), Some(&Value::Number(15.0)));
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
        assert_eq!(interp.get("результат"), Some(&Value::Number(70.0)));
    }

    // ── Вложенные присваивания ──

    #[test]
    fn assign_nested_array() {
        let interp = run_code(
            r#"
            гыы матрица = [[1, 2], [3, 4]];
            матрица[0][1] = 99;
            гыы результат = матрица[0][1];
            "#,
        );
        assert_eq!(interp.get("результат"), Some(&Value::Number(99.0)));
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
        assert_eq!(interp.get("результат"), Some(&Value::Number(42.0)));
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
        assert_eq!(interp.get("результат"), Some(&Value::String("В".to_string())));
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
        assert_eq!(interp.get("результат"), Some(&Value::Number(99.0)));
    }

    // ── try/catch/finally ──

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
        assert_eq!(interp.get("результат"), Some(&Value::Number(1.0)));
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
        assert_eq!(interp.get("результат"), Some(&Value::String("ошибка".to_string())));
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
        assert_eq!(interp.get("результат"), Some(&Value::Number(42.0)));
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
        assert_eq!(interp.get("результат"), Some(&Value::Number(2.0)));
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
        assert_eq!(interp.get("результат"), Some(&Value::Number(11.0)));
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
        assert_eq!(interp.get("шаг1"), Some(&Value::Number(1.0)));
        assert_eq!(interp.get("шаг2"), Some(&Value::Number(1.0)));
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
        assert_eq!(interp.get("шаг1"), Some(&Value::Number(1.0)));
        assert_eq!(interp.get("шаг2"), Some(&Value::Number(1.0)));
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
        assert_eq!(interp.get("результат"), Some(&Value::Number(1.0)));
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
        assert_eq!(interp.get("результат"), Some(&Value::String("снаружи".to_string())));
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
        assert_eq!(interp.get("результат"), Some(&Value::Number(1.0)));
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
        assert_eq!(interp.get("результат"), Some(&Value::Number(11.0)));
    }

    // ── switch/case (базарпо/тема/нуичо) ──

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
        assert_eq!(interp.get("результат"), Some(&Value::Number(10.0)));
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
        assert_eq!(interp.get("результат"), Some(&Value::Number(20.0)));
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
        assert_eq!(interp.get("результат"), Some(&Value::Number(42.0)));
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
        assert_eq!(interp.get("результат"), Some(&Value::Number(0.0)));
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
        assert_eq!(interp.get("результат"), Some(&Value::String("приветствие".to_string())));
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
        assert_eq!(interp.get("результат"), Some(&Value::Number(30.0)));
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
        assert_eq!(interp.get("результат"), Some(&Value::Number(10.0)));
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
        assert_eq!(interp.get("результат"), Some(&Value::Number(42.0)));
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
        assert_eq!(interp.get("а"), Some(&Value::Number(10.0)));
        assert_eq!(interp.get("б"), Some(&Value::Number(20.0)));
        assert_eq!(interp.get("в"), Some(&Value::Number(0.0)));
    }
}
