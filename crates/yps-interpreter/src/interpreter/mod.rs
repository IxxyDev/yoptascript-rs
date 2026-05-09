use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::rc::Rc;

use yps_lexer::Span;
use yps_parser::ast::{
    BinaryOp, Block, ExportKind, Expr, ImportSpec, Literal, ObjectEntry, Param, Pattern, Program, PropKey, Stmt,
    TemplatePart, UnaryOp,
};

use crate::builtins::{builtin_names, call_builtin};
use crate::environment::Environment;
use crate::error::RuntimeError;
use crate::symbols;
use crate::value::{ClassDef, Value};

pub(crate) type Microtask = Box<dyn FnOnce(&mut Interpreter, Span) -> Result<(), RuntimeError>>;

mod assign;
mod class;
mod delete;
mod member;
mod module_loader;
mod promise_rt;
mod types;

pub(super) use types::{AccessSegment, ControlFlow};

pub struct Interpreter {
    pub(super) env: Environment,
    pub(super) pending_initializers: Vec<Value>,
    pub(super) base_path: Option<PathBuf>,
    pub(super) module_cache: Rc<RefCell<HashMap<PathBuf, HashMap<String, Value>>>>,
    pub(super) current_exports: HashMap<String, Value>,
    pub(super) generator_buffer: Option<Vec<Value>>,
    pub(super) microtasks: VecDeque<Microtask>,
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
        for (name, value) in crate::stdlib::build_globals() {
            env.define(name, value, true);
        }
        env.define("нихуя".to_string(), Value::Number(f64::NAN), true);
        Self {
            env,
            pending_initializers: Vec::new(),
            base_path: None,
            module_cache: Rc::new(RefCell::new(HashMap::new())),
            current_exports: HashMap::new(),
            generator_buffer: None,
            microtasks: VecDeque::new(),
        }
    }

    pub fn set_base_path(&mut self, path: PathBuf) {
        self.base_path = Some(path);
    }

    pub fn get(&self, name: &str) -> Option<Value> {
        self.env.get(name)
    }

    pub fn run(&mut self, program: &Program) -> Result<(), RuntimeError> {
        for stmt in &program.items {
            let cf_opt = self.exec_stmt(stmt)?;
            self.drain_microtasks(Span { start: 0, end: 0 })?;
            if let Some(cf) = cf_opt {
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
        self.drain_microtasks(Span { start: 0, end: 0 })?;
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
            Stmt::FunctionDecl { name, params, body, is_generator, is_async, .. } => {
                let func = Value::Function {
                    name: Rc::from(name.name.as_str()),
                    params: params.clone(),
                    body: body.clone(),
                    env: self.env.snapshot(),
                    is_generator: *is_generator,
                    is_async: *is_async,
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
                if let Value::Iterator(rc) = val {
                    self.env.push_scope();
                    self.env.define(variable.name.clone(), Value::Undefined, false);
                    loop {
                        let next_val = {
                            let mut state = rc.borrow_mut();
                            crate::stdlib::iterator::next(self, &mut state, *span)?
                        };
                        let item = match next_val {
                            Some(v) => v,
                            None => break,
                        };
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
                    return Ok(None);
                }
                let items: Vec<Value> = match val {
                    Value::Array(elements) => elements,
                    Value::String(s) => s.chars().map(|c| Value::String(c.to_string())).collect(),
                    Value::Set(s) => s,
                    Value::Map(entries) => entries.into_iter().map(|(k, v)| Value::Array(vec![k, v])).collect(),
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
            Stmt::ClassDecl { name, super_class, members, decorators, span } => {
                self.exec_class_decl(name, super_class.as_ref(), members, decorators, *span)
            }
            Stmt::TryCatch { try_block, catch_param, catch_block, finally_block, .. } => {
                let try_result = self.exec_block(try_block);

                let result = match try_result {
                    Err(err) => {
                        if let Some(cb) = catch_block {
                            self.env.push_scope();
                            if let Some(param) = catch_param {
                                let mut map = HashMap::new();
                                map.insert(
                                    symbols::ERROR_NAME_FIELD.to_string(),
                                    Value::String(symbols::ERROR_NAME.to_string()),
                                );
                                map.insert(symbols::ERROR_MESSAGE_FIELD.to_string(), Value::String(err.message));
                                self.env.define(param.name.clone(), Value::Object(map), false);
                            }
                            let r = self.exec_block_stmts(&cb.stmts);
                            self.env.pop_scope();
                            r
                        } else {
                            Err(err)
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
                    match self.exec_block(fb) {
                        Err(finally_err) => {
                            return Err(match result {
                                Err(orig_err) => finally_err.with_cause(orig_err),
                                _ => finally_err,
                            });
                        }
                        Ok(Some(cf @ (ControlFlow::Return(_) | ControlFlow::Throw(_)))) => {
                            return Ok(Some(cf));
                        }
                        _ => {}
                    }
                }

                result
            }
            Stmt::Debugger { .. } => Ok(None),
            Stmt::Using { name, init, span } => {
                let value = self.eval_expr(init)?;
                if !matches!(value, Value::Null | Value::Undefined) {
                    if !Self::has_dispose_method(&value, &self.env) {
                        return Err(RuntimeError::new("Ресурс 'юзай' должен иметь метод 'расход'", *span));
                    }
                    self.env.add_disposable(value.clone());
                }
                self.env.define(name.name.clone(), value, true);
                Ok(None)
            }
            Stmt::Import { specifiers, source, span } => {
                let exports = self.load_module(source, *span)?;
                for spec in specifiers {
                    match spec {
                        ImportSpec::Default { local } => {
                            let val = exports.get("default").cloned().unwrap_or(Value::Undefined);
                            self.env.define(local.name.clone(), val, true);
                        }
                        ImportSpec::Named { imported, local } => {
                            let val = exports.get(&imported.name).cloned().ok_or_else(|| {
                                RuntimeError::new(
                                    format!("Модуль '{source}' не экспортирует '{}'", imported.name),
                                    *span,
                                )
                            })?;
                            self.env.define(local.name.clone(), val, true);
                        }
                        ImportSpec::Namespace { local } => {
                            let mut map = HashMap::new();
                            for (k, v) in exports.iter() {
                                map.insert(k.clone(), v.clone());
                            }
                            self.env.define(local.name.clone(), Value::Object(map), true);
                        }
                    }
                }
                Ok(None)
            }
            Stmt::Export { kind, span } => match kind {
                ExportKind::Declaration(decl) => {
                    let names = collect_decl_names(decl);
                    let result = self.exec_stmt(decl)?;
                    for name in names {
                        if let Some(val) = self.env.get(&name) {
                            self.current_exports.insert(name, val);
                        }
                    }
                    Ok(result)
                }
                ExportKind::Named(idents) => {
                    for ident in idents {
                        let val = self.env.get(&ident.name).ok_or_else(|| {
                            RuntimeError::new(format!("Нельзя экспортировать неопределённое '{}'", ident.name), *span)
                        })?;
                        self.current_exports.insert(ident.name.clone(), val);
                    }
                    Ok(None)
                }
            },
        }
    }

    fn exec_block(&mut self, block: &Block) -> Result<Option<ControlFlow>, RuntimeError> {
        self.env.push_scope();
        let result = self.exec_block_stmts(&block.stmts);
        let dispose_result = self.dispose_current_scope(block.span);
        self.env.pop_scope();
        match (result, dispose_result) {
            (Err(e), _) => Err(e),
            (Ok(_), Err(e)) => Err(e),
            (Ok(cf), Ok(())) => Ok(cf),
        }
    }

    pub(super) fn exec_block_stmts(&mut self, stmts: &[Stmt]) -> Result<Option<ControlFlow>, RuntimeError> {
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

    pub(super) fn eval_expr(&mut self, expr: &Expr) -> Result<Value, RuntimeError> {
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
                    if matches!(
                        obj,
                        Value::Array(_)
                            | Value::String(_)
                            | Value::Number(_)
                            | Value::Map(_)
                            | Value::Set(_)
                            | Value::Symbol { .. }
                            | Value::Promise { .. }
                            | Value::Iterator(_)
                    ) {
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
                    if matches!(obj, Value::Object(_))
                        && let Value::Function { params, body, env, .. } = &func
                    {
                        let result = self.call_method_returning_this(params, body, env, arg_values, obj, *span)?;
                        self.write_back_object(object, result.1, *span)?;
                        return Ok(result.0);
                    }
                    self.call_function(func, arg_values, *span)
                } else if let Expr::Super { span: super_span } = callee.as_ref() {
                    let super_val = self.env.get(symbols::SUPER).ok_or_else(|| {
                        RuntimeError::new("'яга' (super) используется вне класса-наследника", *super_span)
                    })?;
                    if let Value::Class(cls) = &super_val
                        && let Some((ref params, ref body, ref env)) = cls.constructor
                    {
                        let arg_values = self.eval_args(args)?;
                        let this_val = self.env.get(symbols::THIS);
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
                Ok(Value::String(result))
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
            Expr::Yield { argument, delegate, span } => self.eval_yield(argument.as_deref(), *delegate, *span),
        }
    }

    fn eval_yield(&mut self, argument: Option<&Expr>, delegate: bool, span: Span) -> Result<Value, RuntimeError> {
        if self.generator_buffer.is_none() {
            return Err(RuntimeError::new("'поебалу' можно использовать только внутри 'пиздюли'", span));
        }
        if delegate {
            let arg = argument.ok_or_else(|| RuntimeError::new("'поебалуна' требует аргумент", span))?;
            let val = self.eval_expr(arg)?;
            let items: Vec<Value> = match val {
                Value::Array(elements) => elements,
                Value::String(s) => s.chars().map(|c| Value::String(c.to_string())).collect(),
                Value::Set(s) => s,
                Value::Map(entries) => entries.into_iter().map(|(k, v)| Value::Array(vec![k, v])).collect(),
                other => {
                    return Err(RuntimeError::new(
                        format!("Нельзя итерировать по типу '{}' в 'поебалуна'", other.type_name()),
                        span,
                    ));
                }
            };
            if let Some(buf) = self.generator_buffer.as_mut() {
                buf.extend(items);
            }
        } else {
            let val = match argument {
                Some(arg) => self.eval_expr(arg)?,
                None => Value::Undefined,
            };
            if let Some(buf) = self.generator_buffer.as_mut() {
                buf.push(val);
            }
        }
        Ok(Value::Undefined)
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
                            Value::Set(s) => values.extend(s),
                            Value::Map(entries) => {
                                values.extend(entries.into_iter().map(|(k, v)| Value::Array(vec![k, v])));
                            }
                            Value::String(s) => values.extend(s.chars().map(|c| Value::String(c.to_string()))),
                            Value::Iterator(rc) => {
                                values.extend(crate::stdlib::iterator::drain(self, &rc, *span)?);
                            }
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
                        ObjectEntry::Getter { key, body, .. } => {
                            let key_str = match key {
                                PropKey::Identifier(ident) => ident.name.clone(),
                                PropKey::Computed(expr) => {
                                    let k = self.eval_expr(expr)?;
                                    k.to_string()
                                }
                            };
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
                            let key_str = match key {
                                PropKey::Identifier(ident) => ident.name.clone(),
                                PropKey::Computed(expr) => {
                                    let k = self.eval_expr(expr)?;
                                    k.to_string()
                                }
                            };
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

    pub(super) fn eval_binary(
        &self,
        op: BinaryOp,
        left: Value,
        right: Value,
        span: Span,
    ) -> Result<Value, RuntimeError> {
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
                let left_class_name = match &left {
                    Value::Object(map) => match map.get(symbols::CLASS_TAG) {
                        Some(Value::String(name)) => name.clone(),
                        _ => return Ok(Value::Boolean(false)),
                    },
                    _ => return Ok(Value::Boolean(false)),
                };
                let left_cls = match self.env.get(&left_class_name) {
                    Some(Value::Class(c)) => c,
                    _ => return Ok(Value::Boolean(false)),
                };
                let mut current: Option<&ClassDef> = Some(&left_cls);
                while let Some(c) = current {
                    if c.name == right_class.name {
                        return Ok(Value::Boolean(true));
                    }
                    current = c.parent.as_deref();
                }
                Ok(Value::Boolean(false))
            }
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

    fn call_method_returning_this(
        &mut self,
        params: &[yps_parser::ast::Param],
        body: &Rc<Block>,
        env: &Rc<RefCell<crate::environment::EnvFrame>>,
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

        let result = self.exec_block_stmts(&body.stmts);
        let updated_this = self.env.get(symbols::THIS).unwrap_or(this_val);

        self.env = saved_env;

        match result? {
            Some(ControlFlow::Return(val)) => Ok((val, updated_this)),
            Some(ControlFlow::Break) => Err(RuntimeError::new("'харэ' вне цикла", span)),
            Some(ControlFlow::Continue) => Err(RuntimeError::new("'двигай' вне цикла", span)),
            Some(ControlFlow::Throw(val)) => Err(RuntimeError::new(format!("Необработанное исключение: {val}"), span)),
            None => Ok((Value::Undefined, updated_this)),
        }
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
                    let saved_buffer = self.generator_buffer.take();
                    self.generator_buffer = Some(Vec::new());
                    let result = self.exec_block_stmts(&body.stmts);
                    let collected = self.generator_buffer.take().unwrap_or_default();
                    self.generator_buffer = saved_buffer;
                    self.env = saved_env;
                    match result? {
                        Some(ControlFlow::Return(_)) | None => Ok(Value::Array(collected)),
                        Some(ControlFlow::Break) => Err(RuntimeError::new("'харэ' вне цикла", span)),
                        Some(ControlFlow::Continue) => Err(RuntimeError::new("'двигай' вне цикла", span)),
                        Some(ControlFlow::Throw(val)) => {
                            Err(RuntimeError::new(format!("Необработанное исключение: {val}"), span))
                        }
                    }
                } else if is_async {
                    let result = self.exec_block_stmts(&body.stmts);
                    self.env = saved_env;
                    let promise = match result {
                        Ok(Some(ControlFlow::Return(val))) => Self::make_fulfilled_promise(val),
                        Ok(None) => Self::make_fulfilled_promise(Value::Undefined),
                        Ok(Some(ControlFlow::Throw(val))) => Self::make_rejected_promise(val),
                        Ok(Some(ControlFlow::Break)) => return Err(RuntimeError::new("'харэ' вне цикла", span)),
                        Ok(Some(ControlFlow::Continue)) => return Err(RuntimeError::new("'двигай' вне цикла", span)),
                        Err(e) => return Err(e),
                    };
                    Ok(promise)
                } else {
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
            _ => Err(RuntimeError::new(format!("'{}' не является функцией", func.type_name()), span)),
        }
    }

    pub(super) fn call_method_with_this(
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

fn collect_decl_names(stmt: &Stmt) -> Vec<String> {
    match stmt {
        Stmt::VarDecl { pattern, .. } => {
            let mut names = Vec::new();
            collect_pattern_names(pattern, &mut names);
            names
        }
        Stmt::FunctionDecl { name, .. } | Stmt::ClassDecl { name, .. } => vec![name.name.clone()],
        _ => Vec::new(),
    }
}

fn collect_pattern_names(pattern: &Pattern, out: &mut Vec<String>) {
    match pattern {
        Pattern::Identifier(ident) => out.push(ident.name.clone()),
        Pattern::Array { elements, rest, .. } => {
            for el in elements.iter().flatten() {
                collect_pattern_names(el, out);
            }
            if let Some(r) = rest {
                collect_pattern_names(r, out);
            }
        }
        Pattern::Object { properties, rest, .. } => {
            for prop in properties {
                if let Some(value) = &prop.value {
                    collect_pattern_names(value, out);
                } else {
                    out.push(prop.key.name.clone());
                }
            }
            if let Some(r) = rest {
                collect_pattern_names(r, out);
            }
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
                результат = е.message;
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
    fn finally_runtime_error_preserves_original_via_cause() {
        let err = run_code_err(
            r#"
            хапнуть {
                гыы а = неизвестнаяОригинальная;
            } тюряжка {
                гыы б = неизвестнаяВФинале;
            }
            "#,
        );
        assert!(
            err.message.contains("неизвестнаяВФинале"),
            "ожидается сообщение от финального исключения, получено: {}",
            err.message
        );
        let cause = err.cause.as_deref().expect("ожидается cause с оригинальной ошибкой");
        assert!(
            cause.message.contains("неизвестнаяОригинальная"),
            "cause должен содержать оригинал, получено: {}",
            cause.message
        );
    }

    #[test]
    fn finally_runtime_error_alone_has_no_cause() {
        let err = run_code_err(
            r#"
            хапнуть {
                гыы а = 1;
            } тюряжка {
                гыы б = неизвестная;
            }
            "#,
        );
        assert!(err.message.contains("неизвестная"));
        assert!(err.cause.is_none(), "при отсутствии оригинальной ошибки cause должен быть None");
    }

    #[test]
    fn finally_error_display_shows_cause_chain() {
        let err = run_code_err(
            r#"
            хапнуть {
                гыы а = первая;
            } тюряжка {
                гыы б = вторая;
            }
            "#,
        );
        let s = format!("{err}");
        assert!(s.contains("вторая"), "отображение должно включать финальную ошибку: {s}");
        assert!(s.contains("первая"), "отображение должно включать оригинал: {s}");
        assert!(s.contains("причина"), "отображение должно явно метить причину: {s}");
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
    fn extra_args_ignored_like_js() {
        let interp = run_code(
            r#"
            йопта фн(а) {
                отвечаю а;
            }
            гыы р = фн(1, 2, 3);
            "#,
        );
        assert_eq!(interp.get("р"), Some(Value::Number(1.0)));
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
    fn delete_array_index_creates_undefined_hole() {
        let i = run_code(
            r#"
            гыы а = [10, 20, 30];
            ёбнуть а[1];
            гыы н = а[1];
            гыы д = длина(а);
            "#,
        );
        assert_eq!(i.get("н"), Some(Value::Undefined));
        assert_eq!(i.get("д"), Some(Value::Number(3.0)));
    }

    #[test]
    fn delete_array_preserves_other_elements() {
        let i = run_code(
            r#"
            гыы а = [10, 20, 30];
            ёбнуть а[1];
            гыы х = а[0];
            гыы з = а[2];
            "#,
        );
        assert_eq!(i.get("х"), Some(Value::Number(10.0)));
        assert_eq!(i.get("з"), Some(Value::Number(30.0)));
    }

    #[test]
    fn delete_array_out_of_bounds_is_noop() {
        let i = run_code(
            r#"
            гыы а = [10, 20];
            ёбнуть а[10];
            гыы д = длина(а);
            "#,
        );
        assert_eq!(i.get("д"), Some(Value::Number(2.0)));
    }

    #[test]
    fn delete_string_index_is_runtime_error() {
        let err = run_code_err(
            r#"
            гыы с = "абв";
            ёбнуть с[0];
            "#,
        );
        assert!(
            err.message.to_lowercase().contains("стро"),
            "ошибка должна упоминать строки, получено: {}",
            err.message
        );
    }

    #[test]
    fn break_outside_loop_errors() {
        let err = run_code_err("харэ;");
        assert!(err.message.contains("'харэ'"), "got: {}", err.message);
        assert!(err.message.contains("вне цикла"), "got: {}", err.message);
    }

    #[test]
    fn continue_outside_loop_errors() {
        let err = run_code_err("двигай;");
        assert!(err.message.contains("'двигай'"), "got: {}", err.message);
        assert!(err.message.contains("вне цикла"), "got: {}", err.message);
    }

    #[test]
    fn division_by_zero_errors() {
        let err = run_code_err("гыы х = 5 / 0;");
        assert!(err.message.contains("Деление на ноль"), "got: {}", err.message);
    }

    #[test]
    fn array_index_must_be_number() {
        let err = run_code_err(
            r#"
            гыы а = [1, 2, 3];
            а["ключ"] = 9;
            "#,
        );
        assert!(
            err.message.contains("индекс") || err.message.contains("Индекс") || err.message.contains("индексировать"),
            "got: {}",
            err.message
        );
    }

    #[test]
    fn assignment_lhs_must_be_assignable() {
        let err = run_code_err("42 = 7;");
        assert!(err.message.contains("Левая сторона"), "got: {}", err.message);
    }

    #[test]
    fn increment_on_non_variable_errors() {
        let err = run_code_err("42++;");
        assert!(err.message.contains("'++'") || err.message.contains("переменной"), "got: {}", err.message);
    }

    #[test]
    fn this_outside_method_errors() {
        let err = run_code_err("гыы х = тырыпыры;");
        assert!(err.message.contains("тырыпыры") || err.message.contains("this"), "got: {}", err.message);
        assert!(err.message.contains("вне"), "got: {}", err.message);
    }

    #[test]
    fn super_outside_subclass_errors() {
        let err = run_code_err(
            r#"
            клёво А {
                метод() { отвечаю яга.чтото(); }
            }
            гыы а = захуярить А();
            а.метод();
            "#,
        );
        assert!(err.message.contains("яга") || err.message.contains("super"), "got: {}", err.message);
    }

    #[test]
    fn calling_non_function_errors() {
        let err = run_code_err(
            r#"
            гыы х = 5;
            гыы у = х();
            "#,
        );
        assert!(err.message.contains("не является функцией") || err.message.contains("функц"), "got: {}", err.message);
    }

    #[test]
    fn unary_minus_on_string_errors() {
        let err = run_code_err(r#"гыы х = -"абв";"#);
        assert!(err.message.contains("'-'") || err.message.contains("тип"), "got: {}", err.message);
    }

    #[test]
    fn increment_on_string_errors() {
        let err = run_code_err(
            r#"
            гыы х = "стр";
            х++;
            "#,
        );
        assert!(err.message.contains("число") || err.message.contains("'++'"), "got: {}", err.message);
    }

    #[test]
    fn set_property_on_number_errors() {
        let err = run_code_err(
            r#"
            гыы х = 5;
            х.поле = 1;
            "#,
        );
        assert!(err.message.contains("свойство") || err.message.contains("Нельзя"), "got: {}", err.message);
    }

    #[test]
    fn instanceof_operator_requires_class_on_right() {
        let err = run_code_err(
            r#"
            гыы рез = 42 шкура 10;
            "#,
        );
        assert!(err.message.contains("шкура"));
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
    fn class_implicit_constructor_forwards_to_parent() {
        let i = run_code(
            r#"
            клёво Машина {
                Машина(модель) {
                    тырыпыры.модель = модель;
                }
            }
            клёво Грузовик батя Машина {
            }
            гыы г = захуярить Грузовик("Камаз");
            гыы рез = г.модель;
            "#,
        );
        assert_eq!(i.get("рез"), Some(Value::String("Камаз".to_string())));
    }

    #[test]
    fn class_implicit_constructor_preserves_class_tag() {
        let i = run_code(
            r#"
            клёво Базовый {
                Базовый(значение) {
                    тырыпыры.значение = значение;
                }
            }
            клёво Производный батя Базовый {
            }
            гыы э = захуярить Производный(42);
            гыы знач = э.значение;
            гыы класс = э.__class__;
            "#,
        );
        assert_eq!(i.get("знач"), Some(Value::Number(42.0)));
        assert_eq!(i.get("класс"), Some(Value::String("Производный".to_string())));
    }

    #[test]
    fn catch_receives_runtime_error_as_object() {
        let i = run_code(
            r#"
            гыы имя = "";
            гыы текст = "";
            хапнуть {
                гыы х = неопределённая_переменная;
            } гоп(е) {
                имя = е.name;
                текст = е.message;
            }
            "#,
        );
        assert_eq!(i.get("имя"), Some(Value::String("Косяк".to_string())));
        match i.get("текст") {
            Some(Value::String(s)) => assert!(s.contains("неопределённая_переменная")),
            other => panic!("ожидалась строка с сообщением, получено {other:?}"),
        }
    }

    #[test]
    fn catch_thrown_kosyak_object_preserves_fields() {
        let i = run_code(
            r#"
            гыы имя = "";
            гыы текст = "";
            хапнуть {
                кидай захуярить Косяк("плохо");
            } гоп(е) {
                имя = е.name;
                текст = е.message;
            }
            "#,
        );
        assert_eq!(i.get("имя"), Some(Value::String("Косяк".to_string())));
        assert_eq!(i.get("текст"), Some(Value::String("плохо".to_string())));
    }

    #[test]
    fn catch_thrown_string_passes_through() {
        let i = run_code(
            r#"
            гыы рез = "";
            хапнуть {
                кидай "плоская строка";
            } гоп(е) {
                рез = е;
            }
            "#,
        );
        assert_eq!(i.get("рез"), Some(Value::String("плоская строка".to_string())));
    }

    #[test]
    fn instanceof_distinguishes_unrelated_classes() {
        let i = run_code(
            r#"
            клёво А { А() {} }
            клёво Б { Б() {} }
            гыы а = захуярить А();
            гыы тот = а шкура А;
            гыы нетот = а шкура Б;
            "#,
        );
        assert_eq!(i.get("тот"), Some(Value::Boolean(true)));
        assert_eq!(i.get("нетот"), Some(Value::Boolean(false)));
    }

    #[test]
    fn instanceof_walks_parent_chain() {
        let i = run_code(
            r#"
            клёво Животное { Животное() {} }
            клёво Собака батя Животное { Собака() {} }
            клёво Овчарка батя Собака { Овчарка() {} }
            гыы о = захуярить Овчарка();
            гыы есть_овчарка = о шкура Овчарка;
            гыы есть_собака = о шкура Собака;
            гыы есть_животное = о шкура Животное;
            "#,
        );
        assert_eq!(i.get("есть_овчарка"), Some(Value::Boolean(true)));
        assert_eq!(i.get("есть_собака"), Some(Value::Boolean(true)));
        assert_eq!(i.get("есть_животное"), Some(Value::Boolean(true)));
    }

    #[test]
    fn instanceof_false_for_non_instance() {
        let i = run_code(
            r#"
            клёво К { К() {} }
            гыы х = 42;
            гыы строка = "abc";
            гыы массив = [1, 2];
            гыы а = х шкура К;
            гыы б = строка шкура К;
            гыы в = массив шкура К;
            "#,
        );
        assert_eq!(i.get("а"), Some(Value::Boolean(false)));
        assert_eq!(i.get("б"), Some(Value::Boolean(false)));
        assert_eq!(i.get("в"), Some(Value::Boolean(false)));
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

    #[test]
    fn private_field_access_inside_class() {
        let i = run_code(
            r#"
            клёво Счёт {
                Счёт(нач) {
                    тырыпыры.#баланс = нач;
                }
                получить() {
                    отвечаю тырыпыры.#баланс;
                }
                добавить(с) {
                    тырыпыры.#баланс = тырыпыры.#баланс + с;
                }
            }
            гыы с = захуярить Счёт(100);
            с.добавить(50);
            гыы рез = с.получить();
            "#,
        );
        assert_eq!(i.get("рез"), Some(Value::Number(150.0)));
    }

    #[test]
    fn private_field_access_outside_class_fails() {
        let err = run_code_err(
            r#"
            клёво Кошелёк {
                Кошелёк() {
                    тырыпыры.#бабки = 500;
                }
            }
            гыы к = захуярить Кошелёк();
            гыы х = к.#бабки;
            "#,
        );
        assert!(err.message.contains("приватному полю"));
    }

    #[test]
    fn private_field_declaration() {
        let i = run_code(
            r#"
            клёво Бокс {
                #значение = 42;
                получить() {
                    отвечаю тырыпыры.#значение;
                }
            }
            гыы б = захуярить Бокс();
            гыы рез = б.получить();
            "#,
        );
        assert_eq!(i.get("рез"), Some(Value::Number(42.0)));
    }

    #[test]
    fn class_getter() {
        let i = run_code(
            r#"
            клёво Круг {
                Круг(р) {
                    тырыпыры.радиус = р;
                }
                get площадь() {
                    отвечаю 3 * тырыпыры.радиус * тырыпыры.радиус;
                }
            }
            гыы к = захуярить Круг(10);
            гыы рез = к.площадь;
            "#,
        );
        assert_eq!(i.get("рез"), Some(Value::Number(300.0)));
    }

    #[test]
    fn class_setter() {
        let i = run_code(
            r#"
            клёво Ящик {
                Ящик() {
                    тырыпыры.ширина = 0;
                    тырыпыры.высота = 0;
                }
                get площадь() {
                    отвечаю тырыпыры.ширина * тырыпыры.высота;
                }
                set размер(с) {
                    тырыпыры.ширина = с;
                    тырыпыры.высота = с;
                }
            }
            гыы я = захуярить Ящик();
            я.размер = 5;
            гыы рез = я.площадь;
            "#,
        );
        assert_eq!(i.get("рез"), Some(Value::Number(25.0)));
    }

    #[test]
    fn object_getter_setter() {
        let i = run_code(
            r#"
            гыы об = {
                _имя: "мир",
                get имя() {
                    отвечаю тырыпыры._имя;
                },
                set имя(н) {
                    тырыпыры._имя = н;
                }
            };
            гыы до = об.имя;
            об.имя = "всем";
            гыы после = об.имя;
            "#,
        );
        assert_eq!(i.get("до"), Some(Value::String("мир".to_string())));
        assert_eq!(i.get("после"), Some(Value::String("всем".to_string())));
    }

    #[test]
    fn static_getter() {
        let i = run_code(
            r#"
            клёво Конфиг {
                попонятия #версия = 1;
                попонятия get версия() {
                    отвечаю 42;
                }
            }
            гыы рез = Конфиг.версия;
            "#,
        );
        assert_eq!(i.get("рез"), Some(Value::Number(42.0)));
    }

    #[test]
    fn test_method_decorator() {
        let interp = run_code(
            r#"
            йопта обёртка(метод, контекст) {
                отвечаю (...аргс) => {
                    отвечаю метод(аргс[0], аргс[1]) * 2;
                };
            }

            клёво К {
                @обёртка
                сложить(а, б) {
                    отвечаю а + б;
                }
            }

            гыы к = захуярить К();
            гыы рез = к.сложить(3, 4);
            "#,
        );
        assert_eq!(interp.get("рез"), Some(Value::Number(14.0)));
    }

    #[test]
    fn test_field_decorator() {
        let interp = run_code(
            r#"
            йопта удвоить(_, контекст) {
                отвечаю (начальное) => {
                    отвечаю начальное * 2;
                };
            }

            клёво К {
                @удвоить
                значение = 21;
            }

            гыы к = захуярить К();
            гыы рез = к.значение;
            "#,
        );
        assert_eq!(interp.get("рез"), Some(Value::Number(42.0)));
    }

    #[test]
    fn test_class_decorator() {
        let interp = run_code(
            r#"
            гыы сохр = ноль;
            йопта запомнить(класс, контекст) {
                сохр = контекст;
                отвечаю класс;
            }

            @запомнить
            клёво МойКласс { }
            "#,
        );
        let ctx = interp.get("сохр").unwrap();
        match ctx {
            Value::Object(map) => {
                assert_eq!(map.get("вид"), Some(&Value::String("класс".to_string())));
                assert_eq!(map.get("имя"), Some(&Value::String("МойКласс".to_string())));
                assert_eq!(map.get("статичное"), Some(&Value::Boolean(false)));
                assert_eq!(map.get("приватное"), Some(&Value::Boolean(false)));
            }
            _ => panic!("Expected Object context"),
        }
    }

    #[test]
    fn test_class_decorator_passthrough() {
        let interp = run_code(
            r#"
            йопта нуп(класс, контекст) {
                отвечаю класс;
            }

            @нуп
            клёво К {
                метод() { отвечаю 42; }
            }

            гыы к = захуярить К();
            гыы рез = к.метод();
            "#,
        );
        assert_eq!(interp.get("рез"), Some(Value::Number(42.0)));
    }

    #[test]
    fn test_add_initializer_instance() {
        let interp = run_code(
            r#"
            гыы счётчик = 0;
            йопта отслеживание(метод, контекст) {
                контекст.добавитьИнициализатор(() => {
                    счётчик += 1;
                });
                отвечаю метод;
            }

            клёво К {
                @отслеживание
                метод() { }
            }

            гыы к1 = захуярить К();
            гыы к2 = захуярить К();
            гыы рез = счётчик;
            "#,
        );
        assert_eq!(interp.get("рез"), Some(Value::Number(2.0)));
    }

    #[test]
    fn test_add_initializer_static() {
        let interp = run_code(
            r#"
            гыы инициализирован = лож;
            йопта регистрация(_, контекст) {
                контекст.добавитьИнициализатор(() => {
                    инициализирован = правда;
                });
            }

            клёво К {
                @регистрация
                попонятия х = 1;
            }

            гыы рез = инициализирован;
            "#,
        );
        assert_eq!(interp.get("рез"), Some(Value::Boolean(true)));
    }

    #[test]
    fn test_decorator_execution_order() {
        let interp = run_code(
            r#"
            гыы журнал = [];
            йопта д(тег) {
                журнал = втолкнуть(журнал, "выч:" + тег);
                отвечаю (значение, контекст) => {
                    журнал = втолкнуть(журнал, "прим:" + тег + ">" + контекст.вид);
                    отвечаю значение;
                };
            }

            @д("класс")
            клёво К {
                @д("метод")
                м() { }

                @д("поле")
                х = 1;
            }

            гыы рез = журнал;
            "#,
        );
        let log = interp.get("рез").unwrap();
        match log {
            Value::Array(items) => {
                let strs: Vec<String> = items.iter().map(|v| v.to_string()).collect();
                assert_eq!(
                    strs,
                    vec![
                        "выч:класс",
                        "выч:метод",
                        "выч:поле",
                        "прим:метод>метод",
                        "прим:поле>поле",
                        "прим:класс>класс",
                    ]
                );
            }
            _ => panic!("Expected Array"),
        }
    }

    #[test]
    fn test_multiple_decorators_order() {
        let interp = run_code(
            r#"
            гыы журнал = [];
            йопта первый(м, к) { журнал = втолкнуть(журнал, "первый"); отвечаю м; }
            йопта второй(м, к) { журнал = втолкнуть(журнал, "второй"); отвечаю м; }

            клёво К {
                @первый
                @второй
                метод() { }
            }

            гыы рез = журнал;
            "#,
        );
        let log = interp.get("рез").unwrap();
        match log {
            Value::Array(items) => {
                let strs: Vec<String> = items.iter().map(|v| v.to_string()).collect();
                assert_eq!(strs, vec!["второй", "первый"]);
            }
            _ => panic!("Expected Array"),
        }
    }

    #[test]
    fn test_stdlib_math_basic() {
        let interp = run_code(
            r#"
            гыы а = Матан.пол(3.7);
            гыы б = Матан.потолок(3.2);
            гыы в = Матан.округлить(3.5);
            гыы г = Матан.модуль(-5);
            гыы д = Матан.мин(1, 2, 3);
            гыы е = Матан.макс(1, 2, 3);
            гыы ё = Матан.степень(2, 10);
            гыы ж = Матан.корень(16);
            "#,
        );
        assert_eq!(interp.get("а"), Some(Value::Number(3.0)));
        assert_eq!(interp.get("б"), Some(Value::Number(4.0)));
        assert_eq!(interp.get("в"), Some(Value::Number(4.0)));
        assert_eq!(interp.get("г"), Some(Value::Number(5.0)));
        assert_eq!(interp.get("д"), Some(Value::Number(1.0)));
        assert_eq!(interp.get("е"), Some(Value::Number(3.0)));
        assert_eq!(interp.get("ё"), Some(Value::Number(1024.0)));
        assert_eq!(interp.get("ж"), Some(Value::Number(4.0)));
    }

    #[test]
    fn test_stdlib_math_constants() {
        let interp = run_code(
            r#"
            гыы пи = Матан.ПИ;
            гыы е = Матан.Е;
            "#,
        );
        assert_eq!(interp.get("пи"), Some(Value::Number(std::f64::consts::PI)));
        assert_eq!(interp.get("е"), Some(Value::Number(std::f64::consts::E)));
    }

    #[test]
    fn test_stdlib_array_push_pop() {
        let interp = run_code(
            r#"
            гыы а = [1, 2];
            а.push(3);
            а.push(4);
            гыы последний = а.pop();
            "#,
        );
        assert_eq!(
            interp.get("а"),
            Some(Value::Array(vec![Value::Number(1.0), Value::Number(2.0), Value::Number(3.0)]))
        );
        assert_eq!(interp.get("последний"), Some(Value::Number(4.0)));
    }

    #[test]
    fn test_stdlib_array_length_property() {
        let interp = run_code(
            r#"
            гыы а = [1, 2, 3, 4, 5];
            гыы д = а.length;
            гыы д2 = а.длина;
            "#,
        );
        assert_eq!(interp.get("д"), Some(Value::Number(5.0)));
        assert_eq!(interp.get("д2"), Some(Value::Number(5.0)));
    }

    #[test]
    fn test_stdlib_array_map_filter_reduce() {
        let interp = run_code(
            r#"
            гыы а = [1, 2, 3, 4, 5];
            гыы у = а.map((x) => x * 2);
            гыы ф = а.filter((x) => x > 2);
            гыы с = а.reduce((а, б) => а + б, 0);
            "#,
        );
        assert_eq!(
            interp.get("у"),
            Some(Value::Array(vec![
                Value::Number(2.0),
                Value::Number(4.0),
                Value::Number(6.0),
                Value::Number(8.0),
                Value::Number(10.0),
            ]))
        );
        assert_eq!(
            interp.get("ф"),
            Some(Value::Array(vec![Value::Number(3.0), Value::Number(4.0), Value::Number(5.0)]))
        );
        assert_eq!(interp.get("с"), Some(Value::Number(15.0)));
    }

    #[test]
    fn test_stdlib_array_find_includes_indexof() {
        let interp = run_code(
            r#"
            гыы а = [10, 20, 30, 40];
            гыы н = а.find((x) => x > 15);
            гыы и = а.includes(30);
            гыы ин = а.indexOf(40);
            гыы ин2 = а.indexOf(99);
            "#,
        );
        assert_eq!(interp.get("н"), Some(Value::Number(20.0)));
        assert_eq!(interp.get("и"), Some(Value::Boolean(true)));
        assert_eq!(interp.get("ин"), Some(Value::Number(3.0)));
        assert_eq!(interp.get("ин2"), Some(Value::Number(-1.0)));
    }

    #[test]
    fn test_stdlib_array_join_slice_reverse() {
        let interp = run_code(
            r#"
            гыы а = [1, 2, 3];
            гыы дж = а.join("-");
            гыы ср = а.slice(1, 3);
            гыы пр = а.toReversed();
            "#,
        );
        assert_eq!(interp.get("дж"), Some(Value::String("1-2-3".to_string())));
        assert_eq!(interp.get("ср"), Some(Value::Array(vec![Value::Number(2.0), Value::Number(3.0)])));
        assert_eq!(
            interp.get("пр"),
            Some(Value::Array(vec![Value::Number(3.0), Value::Number(2.0), Value::Number(1.0)]))
        );
    }

    #[test]
    fn test_stdlib_array_at() {
        let interp = run_code(
            r#"
            гыы а = [10, 20, 30];
            гыы п = а.at(0);
            гыы пос = а.at(-1);
            гыы внеДиапазона = а.at(99);
            "#,
        );
        assert_eq!(interp.get("п"), Some(Value::Number(10.0)));
        assert_eq!(interp.get("пос"), Some(Value::Number(30.0)));
        assert_eq!(interp.get("внеДиапазона"), Some(Value::Undefined));
    }

    #[test]
    fn test_stdlib_array_flat() {
        let interp = run_code(
            r#"
            гыы а = [1, [2, [3, [4]]]];
            гыы пл1 = а.flat();
            гыы пл2 = а.flat(2);
            "#,
        );
        assert_eq!(
            interp.get("пл1"),
            Some(Value::Array(vec![
                Value::Number(1.0),
                Value::Number(2.0),
                Value::Array(vec![Value::Number(3.0), Value::Array(vec![Value::Number(4.0)])]),
            ]))
        );
        assert_eq!(
            interp.get("пл2"),
            Some(Value::Array(vec![
                Value::Number(1.0),
                Value::Number(2.0),
                Value::Number(3.0),
                Value::Array(vec![Value::Number(4.0)]),
            ]))
        );
    }

    #[test]
    fn test_stdlib_string_basic() {
        let interp = run_code(
            r#"
            гыы с = "Привет, Мир";
            гыы в = с.toUpperCase();
            гыы н = с.toLowerCase();
            гыы и = с.indexOf("Мир");
            гыы вкл = с.includes("Привет");
            "#,
        );
        assert_eq!(interp.get("в"), Some(Value::String("ПРИВЕТ, МИР".to_string())));
        assert_eq!(interp.get("н"), Some(Value::String("привет, мир".to_string())));
        assert_eq!(interp.get("и"), Some(Value::Number(8.0)));
        assert_eq!(interp.get("вкл"), Some(Value::Boolean(true)));
    }

    #[test]
    fn test_stdlib_string_slice_trim_split() {
        let interp = run_code(
            r#"
            гыы с = "  привет  ";
            гыы об = с.trim();
            гыы сл = "a,b,c".split(",");
            гыы отр = "hello".slice(1, 4);
            "#,
        );
        assert_eq!(interp.get("об"), Some(Value::String("привет".to_string())));
        assert_eq!(
            interp.get("сл"),
            Some(Value::Array(vec![
                Value::String("a".to_string()),
                Value::String("b".to_string()),
                Value::String("c".to_string()),
            ]))
        );
        assert_eq!(interp.get("отр"), Some(Value::String("ell".to_string())));
    }

    #[test]
    fn test_stdlib_string_length() {
        let interp = run_code(
            r#"
            гыы с = "Привет";
            гыы д = с.length;
            гыы д2 = с.длина;
            "#,
        );
        assert_eq!(interp.get("д"), Some(Value::Number(6.0)));
        assert_eq!(interp.get("д2"), Some(Value::Number(6.0)));
    }

    #[test]
    fn test_stdlib_string_repeat_pad() {
        let interp = run_code(
            r#"
            гыы а = "abc".repeat(3);
            гыы б = "5".padStart(3, "0");
            гыы в = "5".padEnd(4, "-");
            "#,
        );
        assert_eq!(interp.get("а"), Some(Value::String("abcabcabc".to_string())));
        assert_eq!(interp.get("б"), Some(Value::String("005".to_string())));
        assert_eq!(interp.get("в"), Some(Value::String("5---".to_string())));
    }

    #[test]
    fn test_stdlib_object_keys_values_entries() {
        let interp = run_code(
            r#"
            гыы о = { а: 1, б: 2 };
            гыы к = Кент.ключи(о);
            гыы з = Кент.значения(о);
            "#,
        );
        if let Some(Value::Array(mut keys)) = interp.get("к") {
            keys.sort_by_key(|v| v.to_string());
            assert_eq!(keys, vec![Value::String("а".to_string()), Value::String("б".to_string())]);
        } else {
            panic!("Expected Array");
        }
        if let Some(Value::Array(mut values)) = interp.get("з") {
            values.sort_by_key(|v| v.to_string());
            assert_eq!(values, vec![Value::Number(1.0), Value::Number(2.0)]);
        } else {
            panic!("Expected Array");
        }
    }

    #[test]
    fn test_stdlib_json_stringify_parse_roundtrip() {
        let interp = run_code(
            r#"
            гыы о = { имя: "Саня", возраст: 25, активен: правда };
            гыы с = Жсон.вСтроку(о);
            гыы об = Жсон.разобрать(с);
            гыы имя = об.имя;
            гыы возраст = об.возраст;
            гыы активен = об.активен;
            "#,
        );
        assert_eq!(interp.get("имя"), Some(Value::String("Саня".to_string())));
        assert_eq!(interp.get("возраст"), Some(Value::Number(25.0)));
        assert_eq!(interp.get("активен"), Some(Value::Boolean(true)));
    }

    #[test]
    fn test_stdlib_json_parse_array() {
        let interp = run_code(
            r#"
            гыы а = Жсон.разобрать("[1, 2, 3]");
            "#,
        );
        assert_eq!(
            interp.get("а"),
            Some(Value::Array(vec![Value::Number(1.0), Value::Number(2.0), Value::Number(3.0)]))
        );
    }

    #[test]
    fn json_parse_rejects_trailing_chars() {
        let err = run_code_err(r#"гыы а = Жсон.разобрать("{} мусор");"#);
        assert!(err.message.contains("Лишние символы"), "got: {}", err.message);
    }

    #[test]
    fn json_parse_object_missing_colon() {
        let err = run_code_err(r#"гыы а = Жсон.разобрать("{\"к\" 1}");"#);
        assert!(err.message.contains("':'") || err.message.contains("JSON"), "got: {}", err.message);
    }

    #[test]
    fn json_parse_array_missing_comma() {
        let err = run_code_err(r#"гыы а = Жсон.разобрать("[1 2]");"#);
        assert!(err.message.contains("','") || err.message.contains("']'"), "got: {}", err.message);
    }

    #[test]
    fn json_parse_unexpected_token() {
        let err = run_code_err(r#"гыы а = Жсон.разобрать("чушь");"#);
        assert!(err.message.contains("JSON"), "got: {}", err.message);
    }

    #[test]
    fn json_parse_incomplete_unicode_escape() {
        let err = run_code_err(r#"гыы а = Жсон.разобрать("\"\\u00\"");"#);
        assert!(err.message.contains("\\u") || err.message.contains("escape"), "got: {}", err.message);
    }

    #[test]
    fn json_stringify_rejects_function() {
        let err = run_code_err(
            r#"
            гыы ф = () => 1;
            гыы с = Жсон.вСтроку(ф);
            "#,
        );
        assert!(err.message.contains("Функции") || err.message.contains("JSON"), "got: {}", err.message);
    }

    #[test]
    fn json_stringify_rejects_symbol() {
        let err = run_code_err(
            r#"
            гыы с = Симбол("х");
            гыы стр = Жсон.вСтроку(с);
            "#,
        );
        assert!(err.message.contains("Символ") || err.message.contains("JSON"), "got: {}", err.message);
    }

    #[test]
    fn object_keys_rejects_non_object() {
        let err = run_code_err(r#"гыы к = Кент.ключи(42);"#);
        assert!(err.message.contains("Кент.ключи"), "got: {}", err.message);
    }

    #[test]
    fn promise_constructor_requires_function() {
        let err = run_code_err(r#"гыы p = захуярить СловоПацана(5);"#);
        assert!(err.message.contains("исполнитель") || err.message.contains("СловоПацана"), "got: {}", err.message);
    }

    #[test]
    fn promise_race_rejects_empty_array() {
        let err = run_code_err(r#"гыы p = СловоПацана.гонка([]);"#);
        assert!(err.message.contains("гонка") || err.message.contains("пуст"), "got: {}", err.message);
    }

    #[test]
    fn array_reduce_empty_without_initial_errors() {
        let err = run_code_err(
            r#"
            гыы а = [];
            гыы р = а.свернуть((а, в) => а + в);
            "#,
        );
        assert!(err.message.contains("reduce") || err.message.contains("пуст"), "got: {}", err.message);
    }

    #[test]
    fn iterator_reduce_empty_without_initial_errors() {
        let err = run_code_err(
            r#"
            гыы и = Итератор.от([]);
            гыы р = и.свернуть((а, в) => а + в);
            "#,
        );
        assert!(err.message.contains("reduce") || err.message.contains("пуст"), "got: {}", err.message);
    }

    #[test]
    fn string_repeat_negative_count_errors() {
        let err = run_code_err(
            r#"
            гыы с = "а";
            гыы р = с.повторить(-1);
            "#,
        );
        assert!(err.message.contains("повторений") || err.message.contains("Некорректное"), "got: {}", err.message);
    }

    #[test]
    fn test_stdlib_number_checks() {
        let interp = run_code(
            r#"
            гыы кон = Хуйня.конечна(5);
            гыы кон2 = Хуйня.конечна(5.5);
            гыы цел = Хуйня.целая(5);
            гыы цел2 = Хуйня.целая(5.5);
            "#,
        );
        assert_eq!(interp.get("кон"), Some(Value::Boolean(true)));
        assert_eq!(interp.get("кон2"), Some(Value::Boolean(true)));
        assert_eq!(interp.get("цел"), Some(Value::Boolean(true)));
        assert_eq!(interp.get("цел2"), Some(Value::Boolean(false)));
    }

    #[test]
    fn test_stdlib_array_is_array() {
        let interp = run_code(
            r#"
            гыы а = Помойка.являетсяПомойкой([1, 2]);
            гыы б = Помойка.являетсяПомойкой("строка");
            "#,
        );
        assert_eq!(interp.get("а"), Some(Value::Boolean(true)));
        assert_eq!(interp.get("б"), Some(Value::Boolean(false)));
    }

    #[test]
    fn test_stdlib_array_sort() {
        let interp = run_code(
            r#"
            гыы а = [3, 1, 4, 1, 5, 9, 2, 6];
            а.sort((л, п) => л - п);
            "#,
        );
        assert_eq!(
            interp.get("а"),
            Some(Value::Array(vec![
                Value::Number(1.0),
                Value::Number(1.0),
                Value::Number(2.0),
                Value::Number(3.0),
                Value::Number(4.0),
                Value::Number(5.0),
                Value::Number(6.0),
                Value::Number(9.0),
            ]))
        );
    }

    #[test]
    fn test_stdlib_array_splice() {
        let interp = run_code(
            r#"
            гыы а = [1, 2, 3, 4, 5];
            гыы удалённые = а.splice(1, 2, 9, 9);
            "#,
        );
        assert_eq!(
            interp.get("а"),
            Some(Value::Array(vec![
                Value::Number(1.0),
                Value::Number(9.0),
                Value::Number(9.0),
                Value::Number(4.0),
                Value::Number(5.0),
            ]))
        );
        assert_eq!(interp.get("удалённые"), Some(Value::Array(vec![Value::Number(2.0), Value::Number(3.0)])));
    }

    #[test]
    fn test_stdlib_array_to_spliced() {
        let interp = run_code(
            r#"
            гыы а = [1, 2, 3, 4];
            гыы б = а.toSpliced(1, 1, 8, 9);
            "#,
        );
        assert_eq!(
            interp.get("а"),
            Some(Value::Array(vec![Value::Number(1.0), Value::Number(2.0), Value::Number(3.0), Value::Number(4.0),]))
        );
        assert_eq!(
            interp.get("б"),
            Some(Value::Array(vec![
                Value::Number(1.0),
                Value::Number(8.0),
                Value::Number(9.0),
                Value::Number(3.0),
                Value::Number(4.0),
            ]))
        );
    }

    #[test]
    fn test_karta_get_or_insert() {
        let interp = run_code(
            r#"
            гыы м = захуярить Карта();
            м.set("а", 1);
            гыы существ = м.getOrInsert("а", 99);
            гыы новое = м.getOrInsert("б", 7);
            гыы итог = м.get("б");
            "#,
        );
        assert_eq!(interp.get("существ"), Some(Value::Number(1.0)));
        assert_eq!(interp.get("новое"), Some(Value::Number(7.0)));
        assert_eq!(interp.get("итог"), Some(Value::Number(7.0)));
    }

    #[test]
    fn test_karta_get_or_insert_computed() {
        let interp = run_code(
            r#"
            гыы м = захуярить Карта();
            гыы вызовов = 0;
            гыы вычислить = (к) => {
                вызовов += 1;
                отвечаю к + "!";
            };
            гыы а = м.getOrInsertComputed("привет", вычислить);
            гыы б = м.getOrInsertComputed("привет", вычислить);
            "#,
        );
        assert_eq!(interp.get("а"), Some(Value::String("привет!".to_string())));
        assert_eq!(interp.get("б"), Some(Value::String("привет!".to_string())));
        assert_eq!(interp.get("вызовов"), Some(Value::Number(1.0)));
    }

    #[test]
    fn test_kosyak_construct() {
        let interp = run_code(
            r#"
            гыы е = захуярить Косяк("плохо");
            гыы имя = е.name;
            гыы сообщ = е.message;
            "#,
        );
        assert_eq!(interp.get("имя"), Some(Value::String("Косяк".to_string())));
        assert_eq!(interp.get("сообщ"), Some(Value::String("плохо".to_string())));
    }

    #[test]
    fn test_kosyak_without_new() {
        let interp = run_code(
            r#"
            гыы е = Косяк("сообщение");
            гыы имя = е.name;
            "#,
        );
        assert_eq!(interp.get("имя"), Some(Value::String("Косяк".to_string())));
    }

    #[test]
    fn test_kosyak_throw_catch() {
        let interp = run_code(
            r#"
            гыы пойман = ноль;
            хапнуть {
                кидай захуярить Косяк("упало");
            } гоп (е) {
                пойман = е.message;
            }
            "#,
        );
        assert_eq!(interp.get("пойман"), Some(Value::String("упало".to_string())));
    }

    #[test]
    fn test_kosyak_with_cause() {
        let interp = run_code(
            r#"
            гыы первый = захуярить Косяк("первая ошибка");
            гыы второй = захуярить Косяк("обёртка", { cause: первый });
            гыы причина = второй.cause;
            гыы сообщ = причина.message;
            "#,
        );
        assert_eq!(interp.get("сообщ"), Some(Value::String("первая ошибка".to_string())));
    }

    #[test]
    fn test_kent_group_by() {
        let interp = run_code(
            r#"
            гыы числа = [1, 2, 3, 4, 5, 6, 7];
            гыы по_чётности = Кент.группировать(числа, (n) => n % 2 === 0 ? "чётные" : "нечётные");
            гыы чётные = по_чётности["чётные"];
            гыы нечётные = по_чётности["нечётные"];
            "#,
        );
        assert_eq!(
            interp.get("чётные"),
            Some(Value::Array(vec![Value::Number(2.0), Value::Number(4.0), Value::Number(6.0)]))
        );
        assert_eq!(
            interp.get("нечётные"),
            Some(Value::Array(vec![Value::Number(1.0), Value::Number(3.0), Value::Number(5.0), Value::Number(7.0),]))
        );
    }

    #[test]
    fn test_huynya_parse_int() {
        let interp = run_code(
            r#"
            гыы а = Хуйня.разобратьЦелое("42");
            гыы б = Хуйня.разобратьЦелое("  -17  ");
            гыы в = Хуйня.разобратьЦелое("1010", 2);
            гыы г = Хуйня.разобратьЦелое("ff", 16);
            гыы д = Хуйня.разобратьЦелое("123abc");
            гыы е = Хуйня.разобратьЦелое("xyz");
            "#,
        );
        assert_eq!(interp.get("а"), Some(Value::Number(42.0)));
        assert_eq!(interp.get("б"), Some(Value::Number(-17.0)));
        assert_eq!(interp.get("в"), Some(Value::Number(10.0)));
        assert_eq!(interp.get("г"), Some(Value::Number(255.0)));
        assert_eq!(interp.get("д"), Some(Value::Number(123.0)));
        if let Some(Value::Number(n)) = interp.get("е") {
            assert!(n.is_nan(), "ожидалось NaN, получено {n}");
        } else {
            panic!("ожидалось Number(NaN)");
        }
    }

    #[test]
    fn test_huynya_parse_float() {
        let interp = run_code(
            r#"
            гыы а = Хуйня.разобратьЧисло("2.5");
            гыы б = Хуйня.разобратьЧисло("  -2.5e2  ");
            гыы в = Хуйня.разобратьЧисло("123abc");
            гыы г = Хуйня.разобратьЧисло("");
            "#,
        );
        assert_eq!(interp.get("а"), Some(Value::Number(2.5)));
        assert_eq!(interp.get("б"), Some(Value::Number(-250.0)));
        if let Some(Value::Number(n)) = interp.get("г") {
            assert!(n.is_nan());
        } else {
            panic!("ожидалось NaN");
        }
        let _ = interp.get("в");
    }

    #[test]
    fn test_spread_set_into_array() {
        let interp = run_code(
            r#"
            гыы н = захуярить Набор([1, 2, 3]);
            гыы а = [0, ...н, 4];
            "#,
        );
        assert_eq!(
            interp.get("а"),
            Some(Value::Array(vec![
                Value::Number(0.0),
                Value::Number(1.0),
                Value::Number(2.0),
                Value::Number(3.0),
                Value::Number(4.0),
            ]))
        );
    }

    #[test]
    fn test_spread_map_into_array() {
        let interp = run_code(
            r#"
            гыы к = захуярить Карта([["а", 1], ["б", 2]]);
            гыы а = [...к];
            "#,
        );
        assert_eq!(
            interp.get("а"),
            Some(Value::Array(vec![
                Value::Array(vec![Value::String("а".to_string()), Value::Number(1.0)]),
                Value::Array(vec![Value::String("б".to_string()), Value::Number(2.0)]),
            ]))
        );
    }

    #[test]
    fn test_spread_string_into_array() {
        let interp = run_code(
            r#"
            гыы а = [..."абв"];
            "#,
        );
        assert_eq!(
            interp.get("а"),
            Some(Value::Array(vec![
                Value::String("а".to_string()),
                Value::String("б".to_string()),
                Value::String("в".to_string()),
            ]))
        );
    }

    #[test]
    fn test_spread_set_into_args() {
        let interp = run_code(
            r#"
            гыы н = захуярить Набор([1, 5, 3, 9, 2]);
            гыы макс = Матан.макс(...н);
            "#,
        );
        assert_eq!(interp.get("макс"), Some(Value::Number(9.0)));
    }

    #[test]
    fn test_for_of_set() {
        let interp = run_code(
            r#"
            гыы н = захуярить Набор([10, 20, 30]);
            гыы сумма = 0;
            го (х сашаГрей н) {
                сумма += х;
            }
            "#,
        );
        assert_eq!(interp.get("сумма"), Some(Value::Number(60.0)));
    }

    #[test]
    fn test_for_of_map_yields_pairs() {
        let interp = run_code(
            r#"
            гыы к = захуярить Карта([["а", 1], ["б", 2], ["в", 3]]);
            гыы ключи = [];
            гыы суммаЗнч = 0;
            го (пара сашаГрей к) {
                ключи.push(пара[0]);
                суммаЗнч += пара[1];
            }
            "#,
        );
        assert_eq!(
            interp.get("ключи"),
            Some(Value::Array(vec![
                Value::String("а".to_string()),
                Value::String("б".to_string()),
                Value::String("в".to_string()),
            ]))
        );
        assert_eq!(interp.get("суммаЗнч"), Some(Value::Number(6.0)));
    }

    #[test]
    fn test_nabor_basic_operations() {
        let interp = run_code(
            r#"
            гыы н = захуярить Набор();
            н.add(1);
            н.add(2);
            н.add(2);
            гыы есть = н.has(1);
            гыы размер = н.size;
            "#,
        );
        assert_eq!(interp.get("есть"), Some(Value::Boolean(true)));
        assert_eq!(interp.get("размер"), Some(Value::Number(2.0)));
    }

    #[test]
    fn test_nabor_construct_from_array() {
        let interp = run_code(
            r#"
            гыы н = захуярить Набор([1, 2, 2, 3, 3, 3]);
            гыы размер = н.size;
            "#,
        );
        assert_eq!(interp.get("размер"), Some(Value::Number(3.0)));
    }

    #[test]
    fn test_nabor_delete_clear() {
        let interp = run_code(
            r#"
            гыы н = захуярить Набор([1, 2, 3]);
            гыы убрал = н.delete(2);
            гыы естьЛи = н.has(2);
            н.clear();
            гыы пустой = н.size;
            "#,
        );
        assert_eq!(interp.get("убрал"), Some(Value::Boolean(true)));
        assert_eq!(interp.get("естьЛи"), Some(Value::Boolean(false)));
        assert_eq!(interp.get("пустой"), Some(Value::Number(0.0)));
    }

    #[test]
    fn test_nabor_values() {
        let interp = run_code(
            r#"
            гыы н = захуярить Набор([3, 1, 2]);
            гыы зн = н.values();
            "#,
        );
        assert_eq!(
            interp.get("зн"),
            Some(Value::Array(vec![Value::Number(3.0), Value::Number(1.0), Value::Number(2.0)]))
        );
    }

    #[test]
    fn test_nabor_for_each() {
        let interp = run_code(
            r#"
            гыы н = захуярить Набор([10, 20, 30]);
            гыы сумма = 0;
            н.forEach((x) => { сумма += x; });
            "#,
        );
        assert_eq!(interp.get("сумма"), Some(Value::Number(60.0)));
    }

    #[test]
    fn test_nabor_set_operations() {
        let interp = run_code(
            r#"
            гыы а = захуярить Набор([1, 2, 3]);
            гыы б = захуярить Набор([3, 4, 5]);
            гыы пересечение = а.intersection(б).size;
            гыы объединение = а.union(б).size;
            гыы разница = а.difference(б).size;
            гыы симРазн = а.symmetricDifference(б).size;
            "#,
        );
        assert_eq!(interp.get("пересечение"), Some(Value::Number(1.0)));
        assert_eq!(interp.get("объединение"), Some(Value::Number(5.0)));
        assert_eq!(interp.get("разница"), Some(Value::Number(2.0)));
        assert_eq!(interp.get("симРазн"), Some(Value::Number(4.0)));
    }

    #[test]
    fn test_nabor_subset_superset_disjoint() {
        let interp = run_code(
            r#"
            гыы малый = захуярить Набор([1, 2]);
            гыы большой = захуярить Набор([1, 2, 3, 4]);
            гыы отдельный = захуярить Набор([99]);
            гыы под = малый.isSubsetOf(большой);
            гыы над = большой.isSupersetOf(малый);
            гыы непер = малый.isDisjointFrom(отдельный);
            "#,
        );
        assert_eq!(interp.get("под"), Some(Value::Boolean(true)));
        assert_eq!(interp.get("над"), Some(Value::Boolean(true)));
        assert_eq!(interp.get("непер"), Some(Value::Boolean(true)));
    }

    #[test]
    fn test_karta_basic_operations() {
        let interp = run_code(
            r#"
            гыы к = захуярить Карта();
            к.set("а", 1);
            к.set("б", 2);
            гыы есть = к.has("а");
            гыы значение = к.get("б");
            гыы размер = к.size;
            "#,
        );
        assert_eq!(interp.get("есть"), Some(Value::Boolean(true)));
        assert_eq!(interp.get("значение"), Some(Value::Number(2.0)));
        assert_eq!(interp.get("размер"), Some(Value::Number(2.0)));
    }

    #[test]
    fn test_karta_construct_from_pairs() {
        let interp = run_code(
            r#"
            гыы к = захуярить Карта([["а", 1], ["б", 2]]);
            гыы а = к.get("а");
            гыы б = к.get("б");
            "#,
        );
        assert_eq!(interp.get("а"), Some(Value::Number(1.0)));
        assert_eq!(interp.get("б"), Some(Value::Number(2.0)));
    }

    #[test]
    fn test_karta_delete_clear() {
        let interp = run_code(
            r#"
            гыы к = захуярить Карта([["а", 1], ["б", 2], ["в", 3]]);
            к.delete("б");
            гыы есть = к.has("б");
            гыы рдо = к.size;
            к.clear();
            гыы рпосле = к.size;
            "#,
        );
        assert_eq!(interp.get("есть"), Some(Value::Boolean(false)));
        assert_eq!(interp.get("рдо"), Some(Value::Number(2.0)));
        assert_eq!(interp.get("рпосле"), Some(Value::Number(0.0)));
    }

    #[test]
    fn test_karta_keys_values_entries_preserve_insertion_order() {
        let interp = run_code(
            r#"
            гыы к = захуярить Карта();
            к.set("первый", 1);
            к.set("второй", 2);
            к.set("третий", 3);
            гыы клч = к.keys();
            гыы знч = к.values();
            "#,
        );
        assert_eq!(
            interp.get("клч"),
            Some(Value::Array(vec![
                Value::String("первый".to_string()),
                Value::String("второй".to_string()),
                Value::String("третий".to_string()),
            ]))
        );
        assert_eq!(
            interp.get("знч"),
            Some(Value::Array(vec![Value::Number(1.0), Value::Number(2.0), Value::Number(3.0)]))
        );
    }

    #[test]
    fn test_karta_overwrite_keeps_position() {
        let interp = run_code(
            r#"
            гыы к = захуярить Карта();
            к.set("а", 1);
            к.set("б", 2);
            к.set("а", 99);
            гыы знч = к.values();
            "#,
        );
        assert_eq!(interp.get("знч"), Some(Value::Array(vec![Value::Number(99.0), Value::Number(2.0)])));
    }

    #[test]
    fn test_karta_static_ot_par() {
        let interp = run_code(
            r#"
            гыы к = Карта.отПар([["x", 10], ["y", 20]]);
            гыы x = к.get("x");
            "#,
        );
        assert_eq!(interp.get("x"), Some(Value::Number(10.0)));
    }

    #[test]
    fn test_karta_for_each() {
        let interp = run_code(
            r#"
            гыы к = захуярить Карта([["а", 1], ["б", 2]]);
            гыы сумма = 0;
            к.forEach((значение, ключ) => {
                сумма += значение;
            });
            "#,
        );
        assert_eq!(interp.get("сумма"), Some(Value::Number(3.0)));
    }

    #[test]
    fn test_karta_keys_supports_non_string() {
        let interp = run_code(
            r#"
            гыы к = захуярить Карта();
            к.set(1, "один");
            к.set(2, "два");
            гыы а = к.get(1);
            гыы б = к.get(2);
            "#,
        );
        assert_eq!(interp.get("а"), Some(Value::String("один".to_string())));
        assert_eq!(interp.get("б"), Some(Value::String("два".to_string())));
    }

    #[test]
    fn test_eto_kosyak() {
        let interp = run_code(
            r#"
            гыы а = этоКосяк(захуярить Косяк("ой"));
            гыы б = этоКосяк("просто строка");
            гыы в = этоКосяк({ name: "Другое" });
            "#,
        );
        assert_eq!(interp.get("а"), Some(Value::Boolean(true)));
        assert_eq!(interp.get("б"), Some(Value::Boolean(false)));
        assert_eq!(interp.get("в"), Some(Value::Boolean(false)));
    }

    #[test]
    fn using_disposes_on_scope_exit() {
        let interp = run_code(
            r#"
            гыы счёт = 0;
            {
                юзай р = { расход: () => { счёт = счёт + 1; } };
            }
            "#,
        );
        assert_eq!(interp.get("счёт"), Some(Value::Number(1.0)));
    }

    #[test]
    fn using_disposes_in_lifo_order() {
        let interp = run_code(
            r#"
            гыы лог = [];
            {
                юзай а = { расход: () => { лог.push("а"); } };
                юзай б = { расход: () => { лог.push("б"); } };
                юзай в = { расход: () => { лог.push("в"); } };
            }
            "#,
        );
        let log = interp.get("лог").unwrap();
        let Value::Array(items) = log else { panic!("expected array") };
        assert_eq!(items.len(), 3);
        assert_eq!(items[0], Value::String("в".to_string()));
        assert_eq!(items[1], Value::String("б".to_string()));
        assert_eq!(items[2], Value::String("а".to_string()));
    }

    #[test]
    fn using_skips_null_resource() {
        let interp = run_code(
            r#"
            гыы счёт = 0;
            {
                юзай р = ноль;
            }
            "#,
        );
        assert_eq!(interp.get("счёт"), Some(Value::Number(0.0)));
    }

    #[test]
    fn using_requires_dispose_method() {
        let err = run_code_err(
            r#"
            {
                юзай р = { данные: 42 };
            }
            "#,
        );
        assert!(err.message.contains("расход"));
    }

    #[test]
    fn using_with_class_instance() {
        let interp = run_code(
            r#"
            гыы счёт = 0;
            клёво Файл {
                расход() {
                    счёт = счёт + 10;
                }
            }
            {
                юзай ф = захуярить Файл();
            }
            "#,
        );
        assert_eq!(interp.get("счёт"), Some(Value::Number(10.0)));
    }

    #[test]
    fn symbol_create_and_typeof() {
        let interp = run_code(
            r#"
            гыы с = Симбол("привет");
            гыы т = чезажижан с;
            "#,
        );
        assert_eq!(interp.get("т"), Some(Value::String("символ".to_string())));
    }

    #[test]
    fn symbol_unique_identity() {
        let interp = run_code(
            r#"
            гыы а = Симбол("ключ");
            гыы б = Симбол("ключ");
            гыы равны = а === б;
            гыы самСебя = а === а;
            "#,
        );
        assert_eq!(interp.get("равны"), Some(Value::Boolean(false)));
        assert_eq!(interp.get("самСебя"), Some(Value::Boolean(true)));
    }

    #[test]
    fn symbol_for_returns_shared() {
        let interp = run_code(
            r#"
            гыы а = Симбол.для("общий");
            гыы б = Симбол.для("общий");
            гыы в = Симбол.для("другой");
            гыы равны1 = а === б;
            гыы равны2 = а === в;
            "#,
        );
        assert_eq!(interp.get("равны1"), Some(Value::Boolean(true)));
        assert_eq!(interp.get("равны2"), Some(Value::Boolean(false)));
    }

    #[test]
    fn symbol_description_property() {
        let interp = run_code(
            r#"
            гыы с = Симбол("моёОписание");
            гыы оп = с.описание;
            "#,
        );
        assert_eq!(interp.get("оп"), Some(Value::String("моёОписание".to_string())));
    }

    #[test]
    fn symbol_well_known_iterator_dispose() {
        let interp = run_code(
            r#"
            гыы и1 = Симбол.итератор;
            гыы и2 = Симбол.итератор;
            гыы р1 = Симбол.расход;
            гыы итерРасх = и1 === р1;
            гыы итерИтер = и1 === и2;
            "#,
        );
        assert_eq!(interp.get("итерРасх"), Some(Value::Boolean(false)));
        assert_eq!(interp.get("итерИтер"), Some(Value::Boolean(true)));
    }

    #[test]
    fn symbol_to_string_method() {
        let interp = run_code(
            r#"
            гыы с = Симбол("м");
            гыы стр = с.вСтроку();
            "#,
        );
        assert_eq!(interp.get("стр"), Some(Value::String("Симбол(м)".to_string())));
    }

    #[test]
    fn dict_alias_chounastoot_for_in() {
        let interp = run_code(
            r#"
            гыы об = { а: 1, б: 2 };
            гыы ключи = [];
            го (гыы к чоунастут об) {
                ключи.push(к);
            }
            гыы длина = ключи.length;
            "#,
        );
        assert_eq!(interp.get("длина"), Some(Value::Number(2.0)));
    }

    #[test]
    fn dict_alias_nan_global() {
        let interp = run_code(
            r#"
            гыы н = нихуя;
            гыы это_нан = Хуйня.нихуя(н);
            "#,
        );
        assert_eq!(interp.get("это_нан"), Some(Value::Boolean(true)));
    }

    #[test]
    fn dict_alias_nan_is_const() {
        let err = run_code_err(
            r#"
            нихуя = 1;
            "#,
        );
        assert!(err.message.contains("константу") || err.message.contains("const"));
    }

    #[test]
    fn dict_modifier_private_field_blocks_outside_access() {
        let err = run_code_err(
            r#"
            клёво К {
                Кошелёк() {}
                мой бабки = 500;
            }
            гыы к = захуярить К();
            гыы х = к.#бабки;
            "#,
        );
        assert!(err.message.contains("приватному полю"));
    }

    #[test]
    fn dict_modifier_private_field_accessible_inside_method() {
        let i = run_code(
            r#"
            клёво К {
                мой значение = 42;
                читать() { отвечаю тырыпыры.#значение; }
            }
            гыы о = захуярить К();
            гыы р = о.читать();
            "#,
        );
        assert_eq!(i.get("р"), Some(Value::Number(42.0)));
    }

    #[test]
    fn dict_modifier_public_protected_parse_as_public() {
        let i = run_code(
            r#"
            клёво К {
                ебанное публ = 1;
                подкрыша прот = 2;
                ебанное взять() { отвечаю тырыпыры.публ + тырыпыры.прот; }
            }
            гыы о = захуярить К();
            гыы а = о.публ;
            гыы б = о.прот;
            гыы в = о.взять();
            "#,
        );
        assert_eq!(i.get("а"), Some(Value::Number(1.0)));
        assert_eq!(i.get("б"), Some(Value::Number(2.0)));
        assert_eq!(i.get("в"), Some(Value::Number(3.0)));
    }

    #[test]
    fn generator_collects_yielded_values() {
        let i = run_code(
            r#"
            пиздюли диапазон(н) {
                го (гыы и = 0; и < н; и += 1) {
                    поебалу и;
                }
            }
            гыы рез = диапазон(3);
            "#,
        );
        assert_eq!(i.get("рез"), Some(Value::Array(vec![Value::Number(0.0), Value::Number(1.0), Value::Number(2.0)])));
    }

    #[test]
    fn generator_yield_without_argument() {
        let i = run_code(
            r#"
            пиздюли пусто() {
                поебалу;
                поебалу;
            }
            гыы рез = пусто();
            "#,
        );
        assert_eq!(i.get("рез"), Some(Value::Array(vec![Value::Undefined, Value::Undefined])));
    }

    #[test]
    fn generator_yield_delegate_flattens_iterable() {
        let i = run_code(
            r#"
            пиздюли вн() {
                поебалу 10;
                поебалу 20;
            }
            пиздюли внеш() {
                поебалу 1;
                поебалуна вн();
                поебалу 2;
            }
            гыы рез = внеш();
            "#,
        );
        assert_eq!(
            i.get("рез"),
            Some(Value::Array(vec![Value::Number(1.0), Value::Number(10.0), Value::Number(20.0), Value::Number(2.0),]))
        );
    }

    #[test]
    fn generator_iterable_in_for_of() {
        let i = run_code(
            r#"
            пиздюли тройка() {
                поебалу 1;
                поебалу 2;
                поебалу 3;
            }
            гыы сумма = 0;
            го (гыы х из тройка()) {
                сумма += х;
            }
            "#,
        );
        assert_eq!(i.get("сумма"), Some(Value::Number(6.0)));
    }

    #[test]
    fn generator_early_return_stops_collection() {
        let i = run_code(
            r#"
            пиздюли стоп() {
                поебалу 1;
                отвечаю;
                поебалу 2;
            }
            гыы рез = стоп();
            "#,
        );
        assert_eq!(i.get("рез"), Some(Value::Array(vec![Value::Number(1.0)])));
    }

    #[test]
    fn yield_outside_generator_errors() {
        let err = run_code_err(
            r#"
            йопта обыч() { поебалу 1; }
            обыч();
            "#,
        );
        assert!(err.message.contains("пиздюли"));
    }

    #[test]
    fn dict_debugger_is_noop() {
        let interp = run_code(
            r#"
            гыы а = 1;
            логопед;
            гыы б = 2;
            "#,
        );
        assert_eq!(interp.get("а"), Some(Value::Number(1.0)));
        assert_eq!(interp.get("б"), Some(Value::Number(2.0)));
    }

    #[test]
    fn test_stdlib_chained_methods() {
        let interp = run_code(
            r#"
            гыы рез = [1, 2, 3, 4, 5]
                .filter((x) => x > 1)
                .map((x) => x * x)
                .reduce((а, б) => а + б, 0);
            "#,
        );
        assert_eq!(interp.get("рез"), Some(Value::Number(54.0)));
    }

    #[test]
    fn test_async_function_then_runs_without_explicit_await() {
        let interp = run_code(
            r#"
            ассо йопта f() { отвечаю 42; }
            гыы итог = 0;
            f().потом((v) => { итог = v; });
            "#,
        );
        assert_eq!(interp.get("итог"), Some(Value::Number(42.0)));
    }

    #[test]
    fn test_async_function_returns_promise() {
        let interp = run_code(
            r#"
            ассо йопта f() { отвечаю 42; }
            гыы p = f();
            гыы т = тип(p);
            "#,
        );
        assert_eq!(interp.get("т"), Some(Value::String("обещание".to_string())));
        match interp.get("p") {
            Some(Value::Promise { .. }) => {}
            other => panic!("Ожидался Promise, получено {other:?}"),
        }
    }

    #[test]
    fn test_async_await_chain_then() {
        let interp = run_code(
            r#"
            ассо йопта f() { отвечаю 1; }
            ассо йопта g() {
                гыы x = сидетьНахуй f();
                отвечаю x + 1;
            }
            гыы итог = 0;
            g().потом((v) => { итог = v; });
            "#,
        );
        assert_eq!(interp.get("итог"), Some(Value::Number(2.0)));
    }

    #[test]
    fn test_promise_resolve_then() {
        let interp = run_code(
            r#"
            гыы итог = 0;
            СловоПацана.решить(5).потом((v) => { итог = v; });
            "#,
        );
        assert_eq!(interp.get("итог"), Some(Value::Number(5.0)));
    }

    #[test]
    fn test_promise_all_resolved() {
        let interp = run_code(
            r#"
            гыы итог = ноль;
            СловоПацана.всех([СловоПацана.решить(1), СловоПацана.решить(2)]).потом((v) => { итог = v; });
            "#,
        );
        assert_eq!(interp.get("итог"), Some(Value::Array(vec![Value::Number(1.0), Value::Number(2.0)])));
    }

    #[test]
    fn test_await_rejected_throws_catchable() {
        let interp = run_code(
            r#"
            ассо йопта плохо() {
                кидай "беда";
            }
            ассо йопта тест() {
                хапнуть {
                    сидетьНахуй плохо();
                    отвечаю "ок";
                } гоп (e) {
                    отвечаю "поймал";
                }
            }
            гыы итог = "пусто";
            тест().потом((v) => { итог = v; });
            "#,
        );
        assert_eq!(interp.get("итог"), Some(Value::String("поймал".to_string())));
    }

    #[test]
    fn test_promise_then_on_resolved_executes_once() {
        let interp = run_code(
            r#"
            гыы счёт = 0;
            гыы p = СловоПацана.решить(1);
            p.потом((v) => { счёт = счёт + v; });
            "#,
        );
        assert_eq!(interp.get("счёт"), Some(Value::Number(1.0)));
    }

    #[test]
    fn test_promise_then_stored_then_chained() {
        let interp = run_code(
            r#"
            гыы итог = 0;
            гыы p = СловоПацана.решить(10);
            p.потом((v) => { итог = v; });
            "#,
        );
        assert_eq!(interp.get("итог"), Some(Value::Number(10.0)));
    }

    #[test]
    fn test_promise_catch_on_rejected() {
        let interp = run_code(
            r#"
            гыы итог = "нет";
            СловоПацана.отвергнуть("ошибка").ловить((e) => { итог = e; });
            "#,
        );
        assert_eq!(interp.get("итог"), Some(Value::String("ошибка".to_string())));
    }

    #[test]
    fn test_promise_finally_on_fulfilled() {
        let interp = run_code(
            r#"
            гыы итог = 0;
            СловоПацана.решить(7).наконец(() => { итог = 1; });
            "#,
        );
        assert_eq!(interp.get("итог"), Some(Value::Number(1.0)));
    }

    #[test]
    fn test_promise_then_chained_twice() {
        let interp = run_code(
            r#"
            гыы итог = 0;
            СловоПацана.решить(3)
                .потом((v) => { отвечаю v + 1; })
                .потом((v) => { итог = v; });
            "#,
        );
        assert_eq!(interp.get("итог"), Some(Value::Number(4.0)));
    }

    #[test]
    fn test_iterator_from_array_to_array() {
        let interp = run_code(
            r#"
            гыы и = Итератор.от([1, 2, 3]);
            гыы рез = и.вМассив();
            "#,
        );
        assert_eq!(
            interp.get("рез"),
            Some(Value::Array(vec![Value::Number(1.0), Value::Number(2.0), Value::Number(3.0)]))
        );
    }

    #[test]
    fn test_iterator_map_lazy() {
        let interp = run_code(
            r#"
            гыы рез = Итератор.от([1, 2, 3]).преобразовать((х) => х * 10).вМассив();
            "#,
        );
        assert_eq!(
            interp.get("рез"),
            Some(Value::Array(vec![Value::Number(10.0), Value::Number(20.0), Value::Number(30.0)]))
        );
    }

    #[test]
    fn test_iterator_filter_take_drop_chain() {
        let interp = run_code(
            r#"
            гыы рез = Итератор.от([1, 2, 3, 4, 5, 6, 7, 8])
                .отфильтровать((х) => х % 2 == 0)
                .пропустить(1)
                .взять(2)
                .вМассив();
            "#,
        );
        assert_eq!(interp.get("рез"), Some(Value::Array(vec![Value::Number(4.0), Value::Number(6.0)])));
    }

    #[test]
    fn test_iterator_reduce() {
        let interp = run_code(
            r#"
            гыы сумма = Итератор.от([1, 2, 3, 4]).свернуть((а, б) => а + б, 0);
            "#,
        );
        assert_eq!(interp.get("сумма"), Some(Value::Number(10.0)));
    }

    #[test]
    fn test_iterator_for_of_drains() {
        let interp = run_code(
            r#"
            гыы итог = 0;
            го (х сашаГрей Итератор.от([10, 20, 30])) {
                итог = итог + х;
            }
            "#,
        );
        assert_eq!(interp.get("итог"), Some(Value::Number(60.0)));
    }

    #[test]
    fn test_iterator_concat() {
        let interp = run_code(
            r#"
            гыы рез = Итератор.склеить([1, 2], [3, 4], [5]).вМассив();
            "#,
        );
        assert_eq!(
            interp.get("рез"),
            Some(Value::Array(vec![
                Value::Number(1.0),
                Value::Number(2.0),
                Value::Number(3.0),
                Value::Number(4.0),
                Value::Number(5.0)
            ]))
        );
    }

    #[test]
    fn test_iterator_some_every_find() {
        let interp = run_code(
            r#"
            гыы есть = Итератор.от([1, 2, 3]).некоторые((х) => х > 2);
            гыы все = Итератор.от([2, 4, 6]).все((х) => х % 2 == 0);
            гыы первое = Итератор.от([1, 2, 3, 4]).найти((х) => х > 2);
            "#,
        );
        assert_eq!(interp.get("есть"), Some(Value::Boolean(true)));
        assert_eq!(interp.get("все"), Some(Value::Boolean(true)));
        assert_eq!(interp.get("первое"), Some(Value::Number(3.0)));
    }

    #[test]
    fn test_iterator_next_protocol() {
        let interp = run_code(
            r#"
            гыы и = Итератор.от([7, 8]);
            гыы а = и.следующий();
            гыы б = и.следующий();
            гыы в = и.следующий();
            "#,
        );
        let a = interp.get("а").unwrap();
        let b = interp.get("б").unwrap();
        let c = interp.get("в").unwrap();
        if let Value::Object(m) = a {
            assert_eq!(m.get("значение"), Some(&Value::Number(7.0)));
            assert_eq!(m.get("готово"), Some(&Value::Boolean(false)));
        } else {
            panic!("expected Object");
        }
        if let Value::Object(m) = b {
            assert_eq!(m.get("значение"), Some(&Value::Number(8.0)));
            assert_eq!(m.get("готово"), Some(&Value::Boolean(false)));
        } else {
            panic!("expected Object");
        }
        if let Value::Object(m) = c {
            assert_eq!(m.get("значение"), Some(&Value::Undefined));
            assert_eq!(m.get("готово"), Some(&Value::Boolean(true)));
        } else {
            panic!("expected Object");
        }
    }

    #[test]
    fn test_iterator_from_string() {
        let interp = run_code(
            r#"
            гыы рез = Итератор.от("abc").вМассив();
            "#,
        );
        assert_eq!(
            interp.get("рез"),
            Some(Value::Array(vec![
                Value::String("a".to_string()),
                Value::String("b".to_string()),
                Value::String("c".to_string())
            ]))
        );
    }

    #[test]
    fn test_iterator_for_of_break_stops_lazy_chain() {
        let interp = run_code(
            r#"
            гыы счёт = 0;
            го (х сашаГрей Итератор.от([1, 2, 3, 4, 5]).преобразовать((в) => { счёт = счёт + 1; отвечаю в; })) {
                вилкойвглаз (х == 3) { харэ; }
            }
            "#,
        );
        assert_eq!(interp.get("счёт"), Some(Value::Number(3.0)));
    }

    #[test]
    fn test_iterator_for_of_yields_in_order_without_materializing() {
        let interp = run_code(
            r#"
            гыы итог = "";
            го (х сашаГрей Итератор.от([10, 20, 30]).преобразовать((в) => в + 1)) {
                итог = итог + строка(х) + ",";
            }
            "#,
        );
        assert_eq!(interp.get("итог"), Some(Value::String("11,21,31,".to_string())));
    }

    #[test]
    fn test_iterator_spread_into_array() {
        let interp = run_code(
            r#"
            гыы рез = [0, ...Итератор.от([1, 2, 3]).преобразовать((х) => х + 1), 99];
            "#,
        );
        assert_eq!(
            interp.get("рез"),
            Some(Value::Array(vec![
                Value::Number(0.0),
                Value::Number(2.0),
                Value::Number(3.0),
                Value::Number(4.0),
                Value::Number(99.0)
            ]))
        );
    }
}
