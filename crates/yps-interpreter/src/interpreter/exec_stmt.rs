use std::collections::HashMap;
use std::rc::Rc;

use yps_lexer::Span;
use yps_parser::ast::{Block, ExportKind, Expr, Identifier, ImportSpec, Pattern, Stmt};

use crate::error::RuntimeError;
use crate::symbols;
use crate::value::Value;

use super::{ControlFlow, Interpreter};

impl Interpreter {
    pub(super) fn exec_stmt(&mut self, stmt: &Stmt) -> Result<Option<ControlFlow>, RuntimeError> {
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
                self.exec_for_of_loop(variable, iterable, body, *span, false)
            }
            Stmt::ForAwaitOf { variable, iterable, body, span, .. } => {
                self.exec_for_of_loop(variable, iterable, body, *span, true)
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
                                let bound = if let Some(thrown) = err.thrown {
                                    thrown
                                } else {
                                    let mut map = HashMap::new();
                                    map.insert(
                                        symbols::ERROR_NAME_FIELD.to_string(),
                                        Value::String(symbols::ERROR_NAME.to_string()),
                                    );
                                    map.insert(symbols::ERROR_MESSAGE_FIELD.to_string(), Value::String(err.message));
                                    Value::Object(map)
                                };
                                self.env.define(param.name.clone(), bound, false);
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
            Stmt::Import { specifiers, source, attributes, span } => {
                let import_type = attributes.iter().find(|(k, _)| k == "type").map(|(_, v)| v.as_str());
                let exports = if import_type == Some("json") {
                    self.load_json_module(source, *span)?
                } else {
                    self.load_module(source, *span)?
                };
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

    fn exec_for_of_loop(
        &mut self,
        variable: &Identifier,
        iterable: &Expr,
        body: &Stmt,
        span: Span,
        is_await: bool,
    ) -> Result<Option<ControlFlow>, RuntimeError> {
        let val = self.eval_expr(iterable)?;
        let val = if is_await { self.do_await(val, span)? } else { val };
        if let Value::Iterator(rc) = val {
            self.env.push_scope();
            self.env.define(variable.name.clone(), Value::Undefined, false);
            loop {
                let next_val = {
                    let mut state = rc.borrow_mut();
                    crate::stdlib::iterator::next(self, &mut state, span)?
                };
                let item = match next_val {
                    Some(v) => v,
                    None => break,
                };
                let item = if is_await { self.do_await(item, span)? } else { item };
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
                return Err(RuntimeError::new(format!("Нельзя итерировать по типу '{}'", other.type_name()), span));
            }
        };
        self.env.push_scope();
        self.env.define(variable.name.clone(), Value::Undefined, false);
        for item in items {
            let item = if is_await { self.do_await(item, span)? } else { item };
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
