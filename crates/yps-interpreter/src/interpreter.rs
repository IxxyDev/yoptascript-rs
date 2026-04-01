use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use yps_lexer::Span;
use yps_parser::ast::{
    BinaryOp, Block, ClassMember, Expr, Literal, ObjectEntry, Pattern, PostfixOp, Program, PropKey, Stmt, TemplatePart,
    UnaryOp,
};

use crate::builtins::{builtin_names, call_builtin};
use crate::environment::Environment;
use crate::error::RuntimeError;
use crate::value::{ClassDef, Value};

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
                    params: params.clone(),
                    body: Rc::new(body.clone()),
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
            Stmt::ForOf { variable, iterable, body, span, .. } => {
                let val = self.eval_expr(iterable)?;
                let items: Vec<Value> = match val {
                    Value::Array(elements) => elements,
                    Value::String(s) => s.chars().map(|c| Value::String(c.to_string())).collect(),
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
            Stmt::ClassDecl { name, super_class, members, span } => {
                self.exec_class_decl(name, super_class.as_ref(), members, *span)
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
                if *op == UnaryOp::Typeof {
                    if let Expr::Identifier(ident) = expr.as_ref() {
                        let val = self.env.get(&ident.name).unwrap_or(Value::Undefined);
                        return Ok(Value::String(val.typeof_str().to_string()));
                    }
                    let val = self.eval_expr(expr)?;
                    return Ok(Value::String(val.typeof_str().to_string()));
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
                    if let Expr::Super { .. } = object.as_ref()
                        && let Value::Class(cls) = &obj
                        && let Some((ref params, ref body, ref env)) = cls.constructor
                    {
                        let arg_values = self.eval_args(args)?;
                        return self.call_method_with_this(params, body, env, arg_values, None, *span);
                    }
                    let func = self.eval_member(obj.clone(), &property.name, *span)?;
                    let arg_values = self.eval_args(args)?;
                    if matches!(obj, Value::Object(_))
                        && let Value::Function { params, body, env, .. } = &func
                    {
                        return self.call_method_with_this(params, body, env, arg_values, Some(obj), *span);
                    }
                    self.call_function(func, arg_values, *span)
                } else if let Expr::Super { span: super_span } = callee.as_ref() {
                    let super_val = self.env.get("__super__").ok_or_else(|| {
                        RuntimeError::new("'яга' (super) используется вне класса-наследника", *super_span)
                    })?;
                    if let Value::Class(cls) = &super_val
                        && let Some((ref params, ref body, ref env)) = cls.constructor
                    {
                        let arg_values = self.eval_args(args)?;
                        let this_val = self.env.get("тырыпыры");
                        return self.call_method_with_this(params, body, env, arg_values, this_val, *span);
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
            Expr::ArrowFunction { params, body, .. } => {
                let func = Value::Function {
                    name: String::new(),
                    params: params.clone(),
                    body: Rc::new(body.clone()),
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
            Expr::Spread { span, .. } => Err(RuntimeError::new(
                "Оператор '...' допустим только в массивах, объектах или аргументах вызова",
                *span,
            )),
            Expr::This { span } => self
                .env
                .get("тырыпыры")
                .ok_or_else(|| RuntimeError::new("'тырыпыры' (this) используется вне контекста объекта", *span)),
            Expr::New { callee, args, span } => {
                let class_val = self.eval_expr(callee)?;
                let arg_values = self.eval_args(args)?;
                self.construct_instance(class_val, arg_values, *span)
            }
            Expr::Super { span } => self
                .env
                .get("__super__")
                .ok_or_else(|| RuntimeError::new("'яга' (super) используется вне класса-наследника", *span)),
        }
    }

    fn eval_literal(&mut self, lit: &Literal) -> Result<Value, RuntimeError> {
        match lit {
            Literal::Number { raw, span } => raw
                .replace('_', "")
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
                    if let Expr::Spread { expr, span } = el {
                        let val = self.eval_expr(expr)?;
                        match val {
                            Value::Array(arr) => values.extend(arr),
                            _ => {
                                return Err(RuntimeError::new(
                                    format!("Нельзя развернуть тип '{}' в массив", val.type_name()),
                                    *span,
                                ));
                            }
                        }
                    } else {
                        values.push(self.eval_expr(el)?);
                    }
                }
                Ok(Value::Array(values))
            }
            Literal::Object { entries, span } => {
                let mut map = HashMap::new();
                for entry in entries {
                    match entry {
                        ObjectEntry::Property { key, value } => {
                            let key_str = match key {
                                PropKey::Identifier(ident) => ident.name.clone(),
                                PropKey::Computed(expr) => {
                                    let k = self.eval_expr(expr)?;
                                    k.to_string()
                                }
                            };
                            let val = self.eval_expr(value)?;
                            map.insert(key_str, val);
                        }
                        ObjectEntry::Spread(expr) => {
                            let val = self.eval_expr(expr)?;
                            match val {
                                Value::Object(src) => {
                                    for (k, v) in src {
                                        map.insert(k, v);
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
                    }
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
            UnaryOp::Typeof => Ok(Value::String(val.typeof_str().to_string())),
            UnaryOp::Delete => Ok(Value::Boolean(true)),
            UnaryOp::Void => Ok(Value::Undefined),
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
            BinaryOp::Pipeline => unreachable!("handled in eval_expr"),
            BinaryOp::Instanceof => Ok(Value::Boolean(left.type_name() == right.type_name())),
            BinaryOp::In => match right {
                Value::Object(map) => {
                    let key = left.to_string();
                    Ok(Value::Boolean(map.contains_key(&key)))
                }
                Value::Array(arr) => {
                    let key = match &left {
                        Value::Number(n) => *n as usize,
                        _ => {
                            return Err(RuntimeError::new("Индекс массива должен быть числом", span));
                        }
                    };
                    Ok(Value::Boolean(key < arr.len()))
                }
                _ => Err(RuntimeError::new(
                    format!("Правая сторона 'из' должна быть объектом или массивом, получено '{}'", right.type_name()),
                    span,
                )),
            },
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
            Expr::This { .. } => Ok("тырыпыры".to_string()),
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
                let rest_count = params.iter().filter(|p| p.is_rest).count();
                let required_count = params.iter().filter(|p| !p.is_rest && p.default.is_none()).count();
                let positional_count = params.len() - rest_count;

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
                if rest_count == 0 && args.len() > positional_count {
                    return Err(RuntimeError::new(
                        format!(
                            "Функция '{}' ожидает максимум {} аргумент(ов), получено {}",
                            name,
                            positional_count,
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

    fn call_method_with_this(
        &mut self,
        params: &[yps_parser::ast::Param],
        body: &Rc<Block>,
        env: &Rc<RefCell<crate::environment::EnvFrame>>,
        args: Vec<Value>,
        this_val: Option<Value>,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        let saved_env = self.env.clone();
        self.env = Environment::from_snapshot(Rc::clone(env));
        self.env.push_scope();

        if let Some(this) = &this_val {
            self.env.define("тырыпыры".to_string(), this.clone(), false);
        }

        if let Some(super_val) = saved_env.get("__super__") {
            self.env.define("__super__".to_string(), super_val, false);
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

        let result = self.exec_block_stmts(&body.stmts);

        self.env = saved_env;

        match result? {
            Some(ControlFlow::Return(val)) => Ok(val),
            Some(ControlFlow::Break) => Err(RuntimeError::new("'харэ' вне цикла", span)),
            Some(ControlFlow::Continue) => Err(RuntimeError::new("'двигай' вне цикла", span)),
            Some(ControlFlow::Throw(val)) => Err(RuntimeError::new(format!("Необработанное исключение: {val}"), span)),
            None => Ok(Value::Undefined),
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

    fn eval_args(&mut self, args: &[Expr]) -> Result<Vec<Value>, RuntimeError> {
        let mut values = Vec::with_capacity(args.len());
        for arg in args {
            if let Expr::Spread { expr, span } = arg {
                let val = self.eval_expr(expr)?;
                match val {
                    Value::Array(arr) => values.extend(arr),
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

    fn eval_delete(&mut self, expr: &Expr, span: Span) -> Result<Value, RuntimeError> {
        match expr {
            Expr::Member { object, property, .. } => {
                let mut path = Vec::new();
                let root_name = self.collect_access_path(
                    &Expr::Member { object: object.clone(), property: property.clone(), span },
                    &mut path,
                    span,
                )?;
                path.reverse();
                if path.len() == 1
                    && let Some(Value::Object(map)) = self.env.get(&root_name).as_mut()
                {
                    let mut map = map.clone();
                    map.remove(&property.name);
                    self.env.set(&root_name, Value::Object(map));
                }
                Ok(Value::Boolean(true))
            }
            Expr::Index { object, index, .. } => {
                let idx = self.eval_expr(index)?;
                let mut path = Vec::new();
                let root_name = self.collect_access_path(object, &mut path, span)?;
                path.reverse();
                if path.is_empty()
                    && let Some(Value::Object(map)) = self.env.get(&root_name).as_mut()
                {
                    let key = idx.to_string();
                    let mut map = map.clone();
                    map.remove(&key);
                    self.env.set(&root_name, Value::Object(map));
                }
                Ok(Value::Boolean(true))
            }
            _ => Ok(Value::Boolean(true)),
        }
    }

    fn exec_class_decl(
        &mut self,
        name: &yps_parser::ast::Identifier,
        super_class: Option<&Expr>,
        members: &[ClassMember],
        span: Span,
    ) -> Result<Option<ControlFlow>, RuntimeError> {
        let parent = if let Some(sc_expr) = super_class {
            let sc_val = self.eval_expr(sc_expr)?;
            match sc_val {
                Value::Class(cls) => Some(cls),
                _ => return Err(RuntimeError::new("Родительский класс должен быть классом", span)),
            }
        } else {
            None
        };

        let mut constructor = None;
        let mut methods = HashMap::new();
        let mut static_methods = HashMap::new();
        let mut static_fields = HashMap::new();
        let mut field_inits = Vec::new();

        for member in members {
            match member {
                ClassMember::Constructor { params, body, .. } => {
                    constructor = Some((params.clone(), Rc::new(body.clone()), self.env.snapshot()));
                }
                ClassMember::Method { name: m_name, params, body, is_static, .. } => {
                    let entry = (params.clone(), Rc::new(body.clone()), self.env.snapshot());
                    if *is_static {
                        static_methods.insert(m_name.name.clone(), entry);
                    } else {
                        methods.insert(m_name.name.clone(), entry);
                    }
                }
                ClassMember::Field { name: f_name, init, is_static, .. } => {
                    if *is_static {
                        let val =
                            if let Some(init_expr) = init { self.eval_expr(init_expr)? } else { Value::Undefined };
                        static_fields.insert(f_name.name.clone(), val);
                    } else {
                        let body = init.as_ref().map(|expr| {
                            Rc::new(Block {
                                stmts: vec![yps_parser::ast::Stmt::Return { value: Some(expr.clone()), span }],
                                span,
                            })
                        });
                        field_inits.push((f_name.name.clone(), body));
                    }
                }
            }
        }

        let class_def = ClassDef {
            name: name.name.clone(),
            constructor,
            methods,
            static_methods,
            static_fields,
            field_inits,
            parent: parent.map(|p| Box::new((*p).clone())),
        };

        let class_val = Value::Class(Rc::new(class_def));
        self.env.define(name.name.clone(), class_val, false);
        Ok(None)
    }

    fn construct_instance(&mut self, class_val: Value, args: Vec<Value>, span: Span) -> Result<Value, RuntimeError> {
        let class_def = match &class_val {
            Value::Class(cls) => cls.clone(),
            _ => return Err(RuntimeError::new(format!("'{}' не является классом", class_val.type_name()), span)),
        };

        let mut instance = HashMap::new();
        instance.insert("__class__".to_string(), Value::String(class_def.name.clone()));

        self.init_fields(&class_def, &mut instance, span)?;

        let instance_val = Value::Object(instance);

        if let Some((ref params, ref body, ref env)) = class_def.constructor {
            let saved_env = self.env.clone();
            self.env = Environment::from_snapshot(Rc::clone(env));
            self.env.push_scope();

            self.env.define("тырыпыры".to_string(), instance_val.clone(), false);

            if let Some(ref parent) = class_def.parent {
                self.env.define("__super__".to_string(), Value::Class(Rc::new(*parent.clone())), false);
            }

            let required_count = params.iter().filter(|p| !p.is_rest && p.default.is_none()).count();

            if args.len() < required_count {
                self.env = saved_env;
                return Err(RuntimeError::new(
                    format!(
                        "Конструктор '{}' ожидает минимум {} аргумент(ов), получено {}",
                        class_def.name,
                        required_count,
                        args.len()
                    ),
                    span,
                ));
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

            let result = self.exec_block_stmts(&body.stmts);
            let this_after = self.env.get("тырыпыры").unwrap_or(instance_val);
            self.env = saved_env;

            match result? {
                Some(ControlFlow::Return(_)) | None => Ok(this_after),
                Some(ControlFlow::Throw(val)) => {
                    Err(RuntimeError::new(format!("Необработанное исключение: {val}"), span))
                }
                Some(ControlFlow::Break) => Err(RuntimeError::new("'харэ' вне цикла", span)),
                Some(ControlFlow::Continue) => Err(RuntimeError::new("'двигай' вне цикла", span)),
            }
        } else if let Some(ref parent) = class_def.parent {
            if let Some((ref params, ..)) = parent.constructor
                && !params.is_empty()
                && params.iter().filter(|p| p.default.is_none() && !p.is_rest).count() > 0
            {
                let parent_class_val = Value::Class(Rc::new(*parent.clone()));
                return self.construct_with_parent(parent_class_val, args, instance_val, span);
            }
            Ok(instance_val)
        } else {
            Ok(instance_val)
        }
    }

    fn construct_with_parent(
        &mut self,
        parent_class_val: Value,
        args: Vec<Value>,
        child_instance: Value,
        _span: Span,
    ) -> Result<Value, RuntimeError> {
        let _ = (parent_class_val, args, child_instance);
        Ok(Value::Object(HashMap::new()))
    }

    fn init_fields(
        &mut self,
        class_def: &ClassDef,
        instance: &mut HashMap<String, Value>,
        _span: Span,
    ) -> Result<(), RuntimeError> {
        if let Some(ref parent) = class_def.parent {
            self.init_fields(parent, instance, _span)?;
        }
        for (name, init_body) in &class_def.field_inits {
            let val = if let Some(body) = init_body {
                let saved_env = self.env.clone();
                self.env.push_scope();
                self.env.define("тырыпыры".to_string(), Value::Object(instance.clone()), false);
                let result = self.exec_block_stmts(&body.stmts);
                self.env = saved_env;
                match result? {
                    Some(ControlFlow::Return(v)) => v,
                    _ => Value::Undefined,
                }
            } else {
                Value::Undefined
            };
            instance.insert(name.clone(), val);
        }
        Ok(())
    }

    fn find_method_in_class<'a>(class_def: &'a ClassDef, method_name: &str) -> Option<&'a crate::value::MethodDef> {
        if let Some(m) = class_def.methods.get(method_name) {
            return Some(m);
        }
        if let Some(ref parent) = class_def.parent {
            return Self::find_method_in_class(parent, method_name);
        }
        None
    }

    fn eval_member(&self, obj: Value, property: &str, span: Span) -> Result<Value, RuntimeError> {
        match &obj {
            Value::Object(map) => {
                if let Some(val) = map.get(property) {
                    return Ok(val.clone());
                }
                if let Some(Value::String(class_name)) = map.get("__class__")
                    && let Some(Value::Class(cls)) = self.env.get(class_name).as_ref()
                    && let Some((params, body, env)) = Self::find_method_in_class(cls, property)
                {
                    return Ok(Value::Function {
                        name: property.to_string(),
                        params: params.clone(),
                        body: Rc::clone(body),
                        env: Rc::clone(env),
                    });
                }
                Ok(Value::Undefined)
            }
            Value::Class(cls) => {
                if let Some(val) = cls.static_fields.get(property) {
                    return Ok(val.clone());
                }
                if let Some((params, body, env)) = cls.static_methods.get(property) {
                    return Ok(Value::Function {
                        name: property.to_string(),
                        params: params.clone(),
                        body: Rc::clone(body),
                        env: Rc::clone(env),
                    });
                }
                Ok(Value::Undefined)
            }
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

    #[test]
    fn numeric_separator() {
        let interp = run_code(
            r#"
            гыы а = 1_000_000;
            гыы б = 1.23_45;
            "#,
        );
        assert_eq!(interp.get("а"), Some(Value::Number(1_000_000.0)));
        assert_eq!(interp.get("б"), Some(Value::Number(1.2345)));
    }

    #[test]
    fn typeof_basic() {
        let interp = run_code(
            r#"
            гыы а = чезажижан 42;
            гыы б = чезажижан "привет";
            гыы в = чезажижан правда;
            гыы г = чезажижан ноль;
            гыы д = чезажижан неибу;
            "#,
        );
        assert_eq!(interp.get("а"), Some(Value::String("число".to_string())));
        assert_eq!(interp.get("б"), Some(Value::String("строка".to_string())));
        assert_eq!(interp.get("в"), Some(Value::String("булево".to_string())));
        assert_eq!(interp.get("г"), Some(Value::String("объект".to_string())));
        assert_eq!(interp.get("д"), Some(Value::String("неопределено".to_string())));
    }

    #[test]
    fn typeof_undefined_variable() {
        let interp = run_code(
            r#"
            гыы р = чезажижан несуществует;
            "#,
        );
        assert_eq!(interp.get("р"), Some(Value::String("неопределено".to_string())));
    }

    #[test]
    fn typeof_function() {
        let interp = run_code(
            r#"
            йопта ф() {}
            гыы р = чезажижан ф;
            "#,
        );
        assert_eq!(interp.get("р"), Some(Value::String("функция".to_string())));
    }

    #[test]
    fn default_param_used_when_no_arg() {
        let interp = run_code(
            r#"
            йопта приветствие(имя = "мир") {
                отвечаю имя;
            }
            гыы р = приветствие();
            "#,
        );
        assert_eq!(interp.get("р"), Some(Value::String("мир".to_string())));
    }

    #[test]
    fn default_param_overridden_by_arg() {
        let interp = run_code(
            r#"
            йопта приветствие(имя = "мир") {
                отвечаю имя;
            }
            гыы р = приветствие("братан");
            "#,
        );
        assert_eq!(interp.get("р"), Some(Value::String("братан".to_string())));
    }

    #[test]
    fn default_param_multiple() {
        let interp = run_code(
            r#"
            йопта сумма(а, б = 10, в = 20) {
                отвечаю а + б + в;
            }
            гыы р1 = сумма(1);
            гыы р2 = сумма(1, 2);
            гыы р3 = сумма(1, 2, 3);
            "#,
        );
        assert_eq!(interp.get("р1"), Some(Value::Number(31.0)));
        assert_eq!(interp.get("р2"), Some(Value::Number(23.0)));
        assert_eq!(interp.get("р3"), Some(Value::Number(6.0)));
    }

    #[test]
    fn default_param_expression() {
        let interp = run_code(
            r#"
            йопта фн(а = 2 + 3) {
                отвечаю а;
            }
            гыы р = фн();
            "#,
        );
        assert_eq!(interp.get("р"), Some(Value::Number(5.0)));
    }

    #[test]
    fn default_param_arrow_function() {
        let interp = run_code(
            r#"
            гыы фн = (а = 42) => { отвечаю а; };
            гыы р = фн();
            "#,
        );
        assert_eq!(interp.get("р"), Some(Value::Number(42.0)));
    }

    #[test]
    fn rest_param_collects_extra_args() {
        let interp = run_code(
            r#"
            йопта фн(а, ...остальное) {
                отвечаю остальное;
            }
            гыы р = фн(1, 2, 3, 4);
            "#,
        );
        assert_eq!(
            interp.get("р"),
            Some(Value::Array(vec![Value::Number(2.0), Value::Number(3.0), Value::Number(4.0),]))
        );
    }

    #[test]
    fn rest_param_empty_when_no_extra_args() {
        let interp = run_code(
            r#"
            йопта фн(а, ...остальное) {
                отвечаю остальное;
            }
            гыы р = фн(1);
            "#,
        );
        assert_eq!(interp.get("р"), Some(Value::Array(vec![])));
    }

    #[test]
    fn rest_param_only() {
        let interp = run_code(
            r#"
            йопта фн(...все) {
                отвечаю все;
            }
            гыы р = фн(1, 2, 3);
            "#,
        );
        assert_eq!(
            interp.get("р"),
            Some(Value::Array(vec![Value::Number(1.0), Value::Number(2.0), Value::Number(3.0),]))
        );
    }

    #[test]
    fn rest_param_arrow_function() {
        let interp = run_code(
            r#"
            гыы фн = (...арг) => { отвечаю арг; };
            гыы р = фн(10, 20);
            "#,
        );
        assert_eq!(interp.get("р"), Some(Value::Array(vec![Value::Number(10.0), Value::Number(20.0)])));
    }

    #[test]
    fn default_and_rest_params_combined() {
        let interp = run_code(
            r#"
            йопта фн(а, б = 99, ...ост) {
                отвечаю а + б;
            }
            гыы р1 = фн(1);
            гыы р2 = фн(1, 2);
            гыы р3 = фн(1, 2, 3, 4);
            "#,
        );
        assert_eq!(interp.get("р1"), Some(Value::Number(100.0)));
        assert_eq!(interp.get("р2"), Some(Value::Number(3.0)));
        assert_eq!(interp.get("р3"), Some(Value::Number(3.0)));
    }

    #[test]
    fn too_few_args_without_defaults_error() {
        let err = run_code_err(
            r#"
            йопта фн(а, б) {
                отвечаю а + б;
            }
            фн(1);
            "#,
        );
        assert!(err.message.contains("минимум 2"));
    }

    #[test]
    fn too_many_args_without_rest_error() {
        let err = run_code_err(
            r#"
            йопта фн(а) {
                отвечаю а;
            }
            фн(1, 2);
            "#,
        );
        assert!(err.message.contains("максимум 1"));
    }

    #[test]
    fn spread_in_array() {
        let i = run_code(
            r#"
            гыы а = [1, 2, 3];
            гыы б = [0, ...а, 4];
            гыы длн = 0;
            го (гыы и = 0; и < 5; и++) {
                длн = длн + 1;
            }
            гыы рез = б[0] + б[1] + б[2] + б[3] + б[4];
            "#,
        );
        assert_eq!(i.get("рез"), Some(Value::Number(10.0)));
    }

    #[test]
    fn spread_in_call() {
        let i = run_code(
            r#"
            йопта сумма(а, б, в) {
                отвечаю а + б + в;
            }
            гыы арг = [1, 2, 3];
            гыы рез = сумма(...арг);
            "#,
        );
        assert_eq!(i.get("рез"), Some(Value::Number(6.0)));
    }

    #[test]
    fn spread_in_object() {
        let i = run_code(
            r#"
            гыы а = {x: 1, y: 2};
            гыы б = {...а, z: 3};
            гыы рез = б["x"] + б["y"] + б["z"];
            "#,
        );
        assert_eq!(i.get("рез"), Some(Value::Number(6.0)));
    }

    #[test]
    fn computed_property_name() {
        let i = run_code(
            r#"
            гыы ключ = "привет";
            гыы о = {[ключ]: 42};
            гыы рез = о["привет"];
            "#,
        );
        assert_eq!(i.get("рез"), Some(Value::Number(42.0)));
    }

    #[test]
    fn shorthand_property() {
        let i = run_code(
            r#"
            гыы х = 10;
            гыы у = 20;
            гыы о = {х, у};
            гыы рез = о["х"] + о["у"];
            "#,
        );
        assert_eq!(i.get("рез"), Some(Value::Number(30.0)));
    }

    #[test]
    fn method_shorthand_in_object() {
        let i = run_code(
            r#"
            гыы о = {
                удвоить(н) {
                    отвечаю н * 2;
                }
            };
            гыы рез = о.удвоить(5);
            "#,
        );
        assert_eq!(i.get("рез"), Some(Value::Number(10.0)));
    }

    #[test]
    fn for_of_array() {
        let i = run_code(
            r#"
            гыы сумма = 0;
            гыы массив = [1, 2, 3, 4, 5];
            го (элем сашаГрей массив) {
                сумма = сумма + элем;
            }
            "#,
        );
        assert_eq!(i.get("сумма"), Some(Value::Number(15.0)));
    }

    #[test]
    fn for_of_string() {
        let i = run_code(
            r#"
            гыы рез = "";
            го (ч сашаГрей "abc") {
                рез = рез + ч + "-";
            }
            "#,
        );
        assert_eq!(i.get("рез"), Some(Value::String("a-b-c-".to_string())));
    }

    #[test]
    fn delete_object_property() {
        let i = run_code(
            r#"
            гыы о = {а: 1, б: 2};
            ёбнуть о.а;
            гыы рез = чезажижан о["а"];
            "#,
        );
        assert_eq!(i.get("рез"), Some(Value::String("неопределено".to_string())));
    }

    #[test]
    fn instanceof_operator() {
        let i = run_code(
            r#"
            гыы рез = 42 шкура 10;
            "#,
        );
        assert_eq!(i.get("рез"), Some(Value::Boolean(true)));
    }

    #[test]
    fn in_operator() {
        let i = run_code(
            r#"
            гыы о = {х: 1, у: 2};
            гыы р1 = "х" из о;
            гыы р2 = "з" из о;
            "#,
        );
        assert_eq!(i.get("р1"), Some(Value::Boolean(true)));
        assert_eq!(i.get("р2"), Some(Value::Boolean(false)));
    }

    #[test]
    fn pipeline_operator() {
        let i = run_code(
            r#"
            йопта удвоить(н) {
                отвечаю н * 2;
            }
            йопта прибавитьОдин(н) {
                отвечаю н + 1;
            }
            гыы рез = 5 |> удвоить |> прибавитьОдин;
            "#,
        );
        assert_eq!(i.get("рез"), Some(Value::Number(11.0)));
    }

    #[test]
    fn void_operator() {
        let i = run_code(
            r#"
            гыы рез = куку 42;
            "#,
        );
        assert_eq!(i.get("рез"), Some(Value::Undefined));
    }

    #[test]
    fn string_key_in_object() {
        let i = run_code(
            r#"
            гыы о = {"моё имя": 42};
            гыы рез = о["моё имя"];
            "#,
        );
        assert_eq!(i.get("рез"), Some(Value::Number(42.0)));
    }

    #[test]
    fn for_of_with_break() {
        let i = run_code(
            r#"
            гыы рез = 0;
            го (э сашаГрей [1, 2, 3, 4, 5]) {
                вилкойвглаз (э === 4) {
                    харэ;
                }
                рез = рез + э;
            }
            "#,
        );
        assert_eq!(i.get("рез"), Some(Value::Number(6.0)));
    }

    #[test]
    fn for_of_with_continue() {
        let i = run_code(
            r#"
            гыы рез = 0;
            го (э сашаГрей [1, 2, 3, 4, 5]) {
                вилкойвглаз (э === 3) {
                    двигай;
                }
                рез = рез + э;
            }
            "#,
        );
        assert_eq!(i.get("рез"), Some(Value::Number(12.0)));
    }

    #[test]
    fn class_basic_constructor_and_fields() {
        let i = run_code(
            r#"
            клёво Чел {
                Чел(имя, возраст) {
                    тырыпыры.имя = имя;
                    тырыпыры.возраст = возраст;
                }
            }
            гыы п = захуярить Чел("Вася", 25);
            гыы имя = п.имя;
            гыы возраст = п.возраст;
            "#,
        );
        assert_eq!(i.get("имя"), Some(Value::String("Вася".to_string())));
        assert_eq!(i.get("возраст"), Some(Value::Number(25.0)));
    }

    #[test]
    fn class_method_call() {
        let i = run_code(
            r#"
            клёво Кот {
                Кот(имя) {
                    тырыпыры.имя = имя;
                }
                мяукнуть() {
                    отвечаю тырыпыры.имя;
                }
            }
            гыы к = захуярить Кот("Барсик");
            гыы рез = к.мяукнуть();
            "#,
        );
        assert_eq!(i.get("рез"), Some(Value::String("Барсик".to_string())));
    }

    #[test]
    fn class_inheritance() {
        let i = run_code(
            r#"
            клёво Животное {
                Животное(имя) {
                    тырыпыры.имя = имя;
                }
                представиться() {
                    отвечаю тырыпыры.имя;
                }
            }
            клёво Собака батя Животное {
                Собака(имя, порода) {
                    тырыпыры.имя = имя;
                    тырыпыры.вид = порода;
                }
                получитьВид() {
                    отвечаю тырыпыры.вид;
                }
            }
            гыы с = захуярить Собака("Шарик", "дворняга");
            гыы имя = с.представиться();
            гыы вид = с.получитьВид();
            "#,
        );
        assert_eq!(i.get("имя"), Some(Value::String("Шарик".to_string())));
        assert_eq!(i.get("вид"), Some(Value::String("дворняга".to_string())));
    }

    #[test]
    fn class_static_method() {
        let i = run_code(
            r#"
            клёво Матема {
                попонятия двойка() {
                    отвечаю 2;
                }
            }
            гыы рез = Матема.двойка();
            "#,
        );
        assert_eq!(i.get("рез"), Some(Value::Number(2.0)));
    }

    #[test]
    fn class_new_without_args() {
        let i = run_code(
            r#"
            клёво Пустой {
                Пустой() {
                    тырыпыры.х = 42;
                }
            }
            гыы о = захуярить Пустой();
            гыы рез = о.х;
            "#,
        );
        assert_eq!(i.get("рез"), Some(Value::Number(42.0)));
    }

    #[test]
    fn class_method_with_args() {
        let i = run_code(
            r#"
            клёво Калькулятор {
                Калькулятор() {}
                сложить(а, б) {
                    отвечаю а + б;
                }
            }
            гыы к = захуярить Калькулятор();
            гыы рез = к.сложить(3, 4);
            "#,
        );
        assert_eq!(i.get("рез"), Some(Value::Number(7.0)));
    }

    #[test]
    fn class_instanceof_check() {
        let i = run_code(
            r#"
            клёво Тест {
                Тест() {
                    тырыпыры.вал = 1;
                }
            }
            гыы т = захуярить Тест();
            гыы рез = чезажижан т;
            "#,
        );
        assert_eq!(i.get("рез"), Some(Value::String("объект".to_string())));
    }
}
