use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use yps_lexer::Span;
use yps_parser::ast::{
    BinaryOp, Block, ClassMember, ExportKind, Expr, ImportSpec, Literal, ObjectEntry, Pattern, PostfixOp, Program,
    PropKey, Stmt, TemplatePart, UnaryOp,
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
    pending_initializers: Vec<Value>,
    base_path: Option<PathBuf>,
    module_cache: Rc<RefCell<HashMap<PathBuf, HashMap<String, Value>>>>,
    current_exports: HashMap<String, Value>,
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
        Self {
            env,
            pending_initializers: Vec::new(),
            base_path: None,
            module_cache: Rc::new(RefCell::new(HashMap::new())),
            current_exports: HashMap::new(),
        }
    }

    pub fn set_base_path(&mut self, path: PathBuf) {
        self.base_path = Some(path);
    }

    fn resolve_module_path(&self, source: &str, span: Span) -> Result<PathBuf, RuntimeError> {
        let base = self.base_path.clone().unwrap_or_else(|| PathBuf::from("."));
        let mut candidate = base.join(source);
        if candidate.extension().is_none() {
            candidate.set_extension("yop");
        }
        candidate
            .canonicalize()
            .map_err(|e| RuntimeError::new(format!("Не удалось разрешить путь модуля '{source}': {e}"), span))
    }

    fn load_module(&mut self, source: &str, span: Span) -> Result<HashMap<String, Value>, RuntimeError> {
        let resolved = self.resolve_module_path(source, span)?;

        if let Some(cached) = self.module_cache.borrow().get(&resolved) {
            return Ok(cached.clone());
        }

        let code = std::fs::read_to_string(&resolved).map_err(|e| {
            RuntimeError::new(format!("Не удалось прочитать модуль '{}': {e}", resolved.display()), span)
        })?;
        let source_file = yps_lexer::SourceFile::new(resolved.display().to_string(), code);
        let lexer = yps_lexer::Lexer::new(&source_file);
        let (tokens, lex_diags) = lexer.tokenize();
        if !lex_diags.is_empty() {
            return Err(RuntimeError::new(
                format!("Ошибки лексера в модуле '{}': {:?}", resolved.display(), lex_diags),
                span,
            ));
        }
        let parser = yps_parser::Parser::new(&tokens, &source_file);
        let (program, parse_diags) = parser.parse_program();
        if !parse_diags.is_empty() {
            return Err(RuntimeError::new(
                format!("Ошибки парсера в модуле '{}': {:?}", resolved.display(), parse_diags),
                span,
            ));
        }

        let mut sub = Interpreter::new();
        sub.module_cache = Rc::clone(&self.module_cache);
        sub.base_path = resolved.parent().map(Path::to_path_buf);

        let exports = sub.run_module(&program, &resolved)?;
        Ok(exports)
    }

    pub fn run_module(&mut self, program: &Program, path: &Path) -> Result<HashMap<String, Value>, RuntimeError> {
        self.run(program)?;
        let exports = std::mem::take(&mut self.current_exports);
        self.module_cache.borrow_mut().insert(path.to_path_buf(), exports.clone());
        Ok(exports)
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
                    if matches!(
                        obj,
                        Value::Array(_) | Value::String(_) | Value::Number(_) | Value::Map(_) | Value::Set(_)
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
                            Value::Set(s) => values.extend(s),
                            Value::Map(entries) => {
                                values.extend(entries.into_iter().map(|(k, v)| Value::Array(vec![k, v])));
                            }
                            Value::String(s) => values.extend(s.chars().map(|c| Value::String(c.to_string()))),
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
                                name: format!("get {key_str}"),
                                params: vec![],
                                body: Rc::new(body.clone()),
                                env: self.env.snapshot(),
                            };
                            map.insert(format!("__get_{key_str}__"), getter_fn);
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
                                name: format!("set {key_str}"),
                                params: vec![param.clone()],
                                body: Rc::new(body.clone()),
                                env: self.env.snapshot(),
                            };
                            map.insert(format!("__set_{key_str}__"), setter_fn);
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
            Expr::Member { object, property, .. } => {
                if let Some(result) = self.try_call_setter(object, &property.name, value.clone(), span)? {
                    return Ok(result);
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
                let setter_key = format!("__set_{property}__");
                if let Some(Value::Function { params, body, env, .. }) = map.get(&setter_key) {
                    let params = params.clone();
                    let body = Rc::clone(body);
                    let env = Rc::clone(env);
                    let updated = self.call_setter_returning_this(&params, &body, &env, value.clone(), obj, span)?;
                    self.write_back_object(object_expr, updated, span)?;
                    return Ok(Some(value));
                }
                if let Some(Value::String(class_name)) = map.get("__class__")
                    && let Some(Value::Class(cls)) = self.env.get(class_name).as_ref()
                    && let Some((params, body, env)) = Self::find_setter_in_class(cls, property)
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
        env: &Rc<RefCell<crate::environment::EnvFrame>>,
        value: Value,
        this_val: Value,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        let saved_env = self.env.clone();
        self.env = Environment::from_snapshot(Rc::clone(env));
        self.env.push_scope();
        self.env.define("тырыпыры".to_string(), this_val.clone(), false);
        if let Some(param) = params.first() {
            self.env.define(param.name.name.clone(), value, false);
        }
        let result = self.exec_block_stmts(&body.stmts);
        let updated_this = self.env.get("тырыпыры").unwrap_or(this_val);
        self.env = saved_env;
        match result? {
            Some(ControlFlow::Throw(val)) => Err(RuntimeError::new(format!("Необработанное исключение: {val}"), span)),
            _ => Ok(updated_this),
        }
    }

    fn write_back_object(&mut self, object_expr: &Expr, updated: Value, span: Span) -> Result<(), RuntimeError> {
        match object_expr {
            Expr::Identifier(ident) => {
                if self.env.is_const(&ident.name) {
                    return Err(RuntimeError::new(format!("Нельзя изменить константу '{}'", ident.name), span));
                }
                self.env.set(&ident.name, updated);
                Ok(())
            }
            Expr::This { .. } => {
                self.env.set("тырыпыры", updated);
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
        let in_method = self.env.get("тырыпыры").is_some();
        if !in_method {
            return Err(RuntimeError::new(
                format!("Нельзя обращаться к приватному полю '{property}' за пределами класса"),
                span,
            ));
        }
        Ok(())
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

        self.env.define("тырыпыры".to_string(), this_val.clone(), false);

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
        let updated_this = self.env.get("тырыпыры").unwrap_or(this_val);

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
            && bname == "__добавитьИнициализатор__"
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
            Value::Function { name, params, body, env } => {
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
                    Value::Set(s) => values.extend(s),
                    Value::String(s) => values.extend(s.chars().map(|c| Value::String(c.to_string()))),
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

    fn build_decorator_context(&self, kind: &str, name: &str, is_static: bool, is_private: bool) -> Value {
        let mut ctx = HashMap::new();
        ctx.insert("вид".to_string(), Value::String(kind.to_string()));
        ctx.insert("имя".to_string(), Value::String(name.to_string()));
        ctx.insert("статичное".to_string(), Value::Boolean(is_static));
        ctx.insert("приватное".to_string(), Value::Boolean(is_private));
        ctx.insert(
            "добавитьИнициализатор".to_string(),
            Value::BuiltinFunction("__добавитьИнициализатор__".to_string()),
        );
        Value::Object(ctx)
    }

    #[allow(clippy::too_many_arguments)]
    fn apply_member_decorators(
        &mut self,
        value: Value,
        decorator_fns: &[Value],
        kind: &str,
        name: &str,
        is_static: bool,
        is_private: bool,
        span: Span,
    ) -> Result<(Value, Vec<Value>), RuntimeError> {
        if decorator_fns.is_empty() {
            return Ok((value, vec![]));
        }

        let mut current = value;
        let mut collected_initializers = Vec::new();

        for decorator_fn in decorator_fns.iter().rev() {
            self.pending_initializers.clear();
            let context = self.build_decorator_context(kind, name, is_static, is_private);
            let result = self.call_function(decorator_fn.clone(), vec![current.clone(), context], span)?;
            collected_initializers.append(&mut self.pending_initializers);
            if !matches!(result, Value::Undefined) {
                current = result;
            }
        }

        Ok((current, collected_initializers))
    }

    fn exec_class_decl(
        &mut self,
        name: &yps_parser::ast::Identifier,
        super_class: Option<&Expr>,
        members: &[ClassMember],
        decorators: &[Expr],
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

        // --- PASS 1: Evaluate ALL decorator expressions top-to-bottom ---
        let mut class_dec_fns = Vec::new();
        for dec_expr in decorators {
            class_dec_fns.push(self.eval_expr(dec_expr)?);
        }

        struct MemberDecFns {
            decorator_fns: Vec<Value>,
        }
        let mut member_dec_fns: Vec<Option<MemberDecFns>> = Vec::new();
        for member in members {
            let dec_exprs = match member {
                ClassMember::Method { decorators, .. }
                | ClassMember::Field { decorators, .. }
                | ClassMember::Getter { decorators, .. }
                | ClassMember::Setter { decorators, .. } => decorators,
                ClassMember::Constructor { .. } => {
                    member_dec_fns.push(None);
                    continue;
                }
            };
            let mut fns = Vec::new();
            for dec_expr in dec_exprs {
                fns.push(self.eval_expr(dec_expr)?);
            }
            member_dec_fns.push(Some(MemberDecFns { decorator_fns: fns }));
        }

        // --- PASS 2: Process members, apply decorators by category ---
        let mut constructor = None;
        let mut methods = HashMap::new();
        let mut static_methods = HashMap::new();
        let mut static_fields = HashMap::new();
        let mut field_inits = Vec::new();
        let mut getters = HashMap::new();
        let mut setters = HashMap::new();
        let mut static_getters = HashMap::new();
        let mut static_setters = HashMap::new();
        let mut static_inits = Vec::new();
        let mut instance_inits = Vec::new();

        // Pass 2a: methods, getters, setters (applied first per TC39)
        for (i, member) in members.iter().enumerate() {
            let dec_fns = member_dec_fns[i].as_ref().map_or(&[] as &[Value], |d| &d.decorator_fns);
            match member {
                ClassMember::Constructor { params, body, .. } => {
                    constructor = Some((params.clone(), Rc::new(body.clone()), self.env.snapshot()));
                }
                ClassMember::Method { name: m_name, params, body, is_static, is_private, .. } => {
                    let method_fn = Value::Function {
                        name: m_name.name.clone(),
                        params: params.clone(),
                        body: Rc::new(body.clone()),
                        env: self.env.snapshot(),
                    };
                    let (decorated, inits) = self.apply_member_decorators(
                        method_fn,
                        dec_fns,
                        "метод",
                        &m_name.name,
                        *is_static,
                        *is_private,
                        span,
                    )?;
                    let entry = match decorated {
                        Value::Function { params, body, env, .. } => (params, body, env),
                        _ => return Err(RuntimeError::new("Декоратор метода должен вернуть функцию", span)),
                    };
                    if *is_static {
                        static_methods.insert(m_name.name.clone(), entry);
                        static_inits.extend(inits);
                    } else {
                        methods.insert(m_name.name.clone(), entry);
                        instance_inits.extend(inits);
                    }
                }
                ClassMember::Getter { name: g_name, body, is_static, is_private, .. } => {
                    let getter_fn = Value::Function {
                        name: g_name.name.clone(),
                        params: vec![],
                        body: Rc::new(body.clone()),
                        env: self.env.snapshot(),
                    };
                    let (decorated, inits) = self.apply_member_decorators(
                        getter_fn,
                        dec_fns,
                        "геттер",
                        &g_name.name,
                        *is_static,
                        *is_private,
                        span,
                    )?;
                    let entry = match decorated {
                        Value::Function { params, body, env, .. } => (params, body, env),
                        _ => return Err(RuntimeError::new("Декоратор геттера должен вернуть функцию", span)),
                    };
                    if *is_static {
                        static_getters.insert(g_name.name.clone(), entry);
                        static_inits.extend(inits);
                    } else {
                        getters.insert(g_name.name.clone(), entry);
                        instance_inits.extend(inits);
                    }
                }
                ClassMember::Setter { name: s_name, param, body, is_static, is_private, .. } => {
                    let setter_fn = Value::Function {
                        name: s_name.name.clone(),
                        params: vec![param.clone()],
                        body: Rc::new(body.clone()),
                        env: self.env.snapshot(),
                    };
                    let (decorated, inits) = self.apply_member_decorators(
                        setter_fn,
                        dec_fns,
                        "сеттер",
                        &s_name.name,
                        *is_static,
                        *is_private,
                        span,
                    )?;
                    let entry = match decorated {
                        Value::Function { params, body, env, .. } => (params, body, env),
                        _ => return Err(RuntimeError::new("Декоратор сеттера должен вернуть функцию", span)),
                    };
                    if *is_static {
                        static_setters.insert(s_name.name.clone(), entry);
                        static_inits.extend(inits);
                    } else {
                        setters.insert(s_name.name.clone(), entry);
                        instance_inits.extend(inits);
                    }
                }
                ClassMember::Field { .. } => {}
            }
        }

        // Pass 2b: fields (applied after methods/getters/setters per TC39)
        for (i, member) in members.iter().enumerate() {
            if let ClassMember::Field { name: f_name, init, is_static, is_private, .. } = member {
                let dec_fns = member_dec_fns[i].as_ref().map_or(&[] as &[Value], |d| &d.decorator_fns);
                let (init_transform, inits) = self.apply_member_decorators(
                    Value::Undefined,
                    dec_fns,
                    "поле",
                    &f_name.name,
                    *is_static,
                    *is_private,
                    span,
                )?;
                let transform = if matches!(init_transform, Value::Undefined) { None } else { Some(init_transform) };

                if *is_static {
                    let base_val =
                        if let Some(init_expr) = init { self.eval_expr(init_expr)? } else { Value::Undefined };
                    let val = if let Some(ref tf) = transform {
                        self.call_function(tf.clone(), vec![base_val], span)?
                    } else {
                        base_val
                    };
                    static_fields.insert(f_name.name.clone(), val);
                    static_inits.extend(inits);
                } else {
                    let body = init.as_ref().map(|expr| {
                        Rc::new(Block {
                            stmts: vec![yps_parser::ast::Stmt::Return { value: Some(expr.clone()), span }],
                            span,
                        })
                    });
                    field_inits.push((f_name.name.clone(), body, transform));
                    instance_inits.extend(inits);
                }
            }
        }

        // --- PASS 3: Build ClassDef, apply class decorators ---
        let class_def = ClassDef {
            name: name.name.clone(),
            constructor,
            methods,
            static_methods,
            static_fields,
            field_inits,
            getters,
            setters,
            static_getters,
            static_setters,
            parent: parent.map(|p| Box::new((*p).clone())),
            instance_initializers: instance_inits,
        };

        let mut class_val = Value::Class(Rc::new(class_def));

        for decorator_fn in class_dec_fns.iter().rev() {
            self.pending_initializers.clear();
            let context = self.build_decorator_context("класс", &name.name, false, false);
            let result = self.call_function(decorator_fn.clone(), vec![class_val.clone(), context], span)?;
            static_inits.append(&mut self.pending_initializers);
            if !matches!(result, Value::Undefined) {
                class_val = result;
            }
        }

        for init in &static_inits {
            self.call_function(init.clone(), vec![], span)?;
        }

        self.env.define(name.name.clone(), class_val, false);
        Ok(None)
    }

    fn construct_instance(&mut self, class_val: Value, args: Vec<Value>, span: Span) -> Result<Value, RuntimeError> {
        if let Value::BuiltinFunction(_) = &class_val {
            return self.call_function(class_val, args, span);
        }
        let class_def = match &class_val {
            Value::Class(cls) => cls.clone(),
            _ => return Err(RuntimeError::new(format!("'{}' не является классом", class_val.type_name()), span)),
        };

        let mut instance = HashMap::new();
        instance.insert("__class__".to_string(), Value::String(class_def.name.clone()));

        self.init_fields(&class_def, &mut instance, span)?;

        let mut instance_val = Value::Object(instance);

        for init in &class_def.instance_initializers {
            let saved = self.env.clone();
            self.env.push_scope();
            self.env.define("тырыпыры".to_string(), instance_val.clone(), false);
            self.call_function(init.clone(), vec![], span)?;
            instance_val = self.env.get("тырыпыры").unwrap_or(instance_val);
            self.env = saved;
        }

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
        span: Span,
    ) -> Result<(), RuntimeError> {
        if let Some(ref parent) = class_def.parent {
            self.init_fields(parent, instance, span)?;
        }
        for (name, init_body, transform) in &class_def.field_inits {
            let base_val = if let Some(body) = init_body {
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
            let val =
                if let Some(tf) = transform { self.call_function(tf.clone(), vec![base_val], span)? } else { base_val };
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

    fn find_getter_in_class<'a>(class_def: &'a ClassDef, name: &str) -> Option<&'a crate::value::MethodDef> {
        if let Some(g) = class_def.getters.get(name) {
            return Some(g);
        }
        if let Some(ref parent) = class_def.parent {
            return Self::find_getter_in_class(parent, name);
        }
        None
    }

    fn find_setter_in_class<'a>(class_def: &'a ClassDef, name: &str) -> Option<&'a crate::value::MethodDef> {
        if let Some(s) = class_def.setters.get(name) {
            return Some(s);
        }
        if let Some(ref parent) = class_def.parent {
            return Self::find_setter_in_class(parent, name);
        }
        None
    }

    fn eval_member(&mut self, obj: Value, property: &str, span: Span) -> Result<Value, RuntimeError> {
        match &obj {
            Value::Array(arr) => {
                if property == "length" || property == "длина" {
                    return Ok(Value::Number(arr.len() as f64));
                }
                Ok(Value::Undefined)
            }
            Value::Map(entries) => {
                if property == "size" || property == "размер" {
                    return Ok(Value::Number(entries.len() as f64));
                }
                Ok(Value::Undefined)
            }
            Value::Set(items) => {
                if property == "size" || property == "размер" {
                    return Ok(Value::Number(items.len() as f64));
                }
                Ok(Value::Undefined)
            }
            Value::String(s) => {
                if property == "length" || property == "длина" {
                    return Ok(Value::Number(s.chars().count() as f64));
                }
                Ok(Value::Undefined)
            }
            Value::BuiltinFunction(name) => Ok(Value::BuiltinFunction(format!("{name}.{property}"))),
            Value::Object(map) => {
                if property.starts_with('#') {
                    let in_class =
                        if let Some(Value::String(class_name)) = map.get("__class__") {
                            self.env.get("тырыпыры").is_some()
                                && self
                                    .env
                                    .get("тырыпыры")
                                    .and_then(|this| {
                                        if let Value::Object(m) = &this { m.get("__class__").cloned() } else { None }
                                    })
                                    .is_some_and(|c| if let Value::String(cn) = c { cn == *class_name } else { false })
                        } else {
                            false
                        };
                    if !in_class {
                        return Err(RuntimeError::new(
                            format!("Нельзя обращаться к приватному полю '{property}' за пределами класса"),
                            span,
                        ));
                    }
                }

                let getter_key = format!("__get_{property}__");
                if let Some(Value::Function { params, body, env, .. }) = map.get(&getter_key) {
                    let params = params.clone();
                    let body = Rc::clone(body);
                    let env = Rc::clone(env);
                    return self.call_method_with_this(&params, &body, &env, vec![], Some(obj), span);
                }
                if let Some(val) = map.get(property) {
                    return Ok(val.clone());
                }
                if let Some(Value::String(class_name)) = map.get("__class__")
                    && let Some(Value::Class(cls)) = self.env.get(class_name).as_ref()
                {
                    if let Some((params, body, env)) = Self::find_getter_in_class(cls, property) {
                        let params = params.clone();
                        let body = Rc::clone(body);
                        let env = Rc::clone(env);
                        return self.call_method_with_this(&params, &body, &env, vec![], Some(obj), span);
                    }
                    if let Some((params, body, env)) = Self::find_method_in_class(cls, property) {
                        return Ok(Value::Function {
                            name: property.to_string(),
                            params: params.clone(),
                            body: Rc::clone(body),
                            env: Rc::clone(env),
                        });
                    }
                }
                Ok(Value::Undefined)
            }
            Value::Class(cls) => {
                if let Some((params, body, env)) = cls.static_getters.get(property) {
                    return self.call_method_with_this(params, body, env, vec![], None, span);
                }
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
}
