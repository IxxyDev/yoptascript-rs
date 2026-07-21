use std::rc::Rc;

use indexmap::IndexMap;

use yps_lexer::Span;
use yps_parser::ast::{Block, ExportKind, Expr, Identifier, ImportSpec, Pattern, Stmt};

use crate::error::RuntimeError;
use crate::symbols;
use crate::value::Value;

use super::{ControlFlow, Interpreter, LoopOp};

impl Interpreter {
    pub(super) fn exec_stmt(&mut self, stmt: &Stmt) -> Result<Option<ControlFlow>, RuntimeError> {
        stacker::maybe_grow(super::STACK_RED_ZONE, super::STACK_GROW_SIZE, || self.exec_stmt_inner(stmt))
    }

    fn exec_stmt_inner(&mut self, stmt: &Stmt) -> Result<Option<ControlFlow>, RuntimeError> {
        let incoming_label = self.pending_label.take();
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
                let label = incoming_label;
                loop {
                    let cond = self.eval_expr(condition)?;
                    if !cond.is_truthy() {
                        break;
                    }
                    if let Some(cf) = self.exec_stmt(body)? {
                        match cf.for_loop(label.as_deref()) {
                            LoopOp::Break => break,
                            LoopOp::Continue => continue,
                            LoopOp::Exit(cf) => return Ok(Some(cf)),
                        }
                    }
                }
                Ok(None)
            }
            Stmt::For { init, condition, update, body, .. } => {
                let label = incoming_label;
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
                        match cf.for_loop(label.as_deref()) {
                            LoopOp::Break => break,
                            LoopOp::Continue => {}
                            LoopOp::Exit(cf) => {
                                self.env.pop_scope();
                                return Ok(Some(cf));
                            }
                        }
                    }
                    self.env.fork_current();
                    if let Some(upd) = update {
                        self.eval_expr(upd)?;
                    }
                }
                self.env.pop_scope();
                Ok(None)
            }
            Stmt::Break { label, .. } => Ok(Some(ControlFlow::Break(label.as_ref().map(|l| l.name.clone())))),
            Stmt::Continue { label, .. } => Ok(Some(ControlFlow::Continue(label.as_ref().map(|l| l.name.clone())))),
            Stmt::Labeled { label, body, .. } => {
                self.pending_label = Some(label.name.clone());
                let result = self.exec_stmt(body);
                self.pending_label = None;
                match result? {
                    Some(ControlFlow::Break(Some(l))) if l == label.name => Ok(None),
                    Some(ControlFlow::Continue(Some(l))) if l == label.name => Ok(None),
                    other => Ok(other),
                }
            }
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
                    Value::Array(elements) => elements.borrow().0.clone(),
                    Value::TypedArray { buffer, offset, length, kind } => {
                        crate::stdlib::typed_array::ta_elements(&buffer, offset, length, kind)
                    }
                    Value::Proxy { target, handler } => self.proxy_own_keys(&target, &handler, *span)?,
                    Value::Object(map) => map.borrow().keys().map(|k| Value::String(k.clone())).collect(),
                    other => {
                        return Err(RuntimeError::new(
                            format!("Нельзя итерировать по типу '{}'", other.type_name()),
                            *span,
                        ));
                    }
                };
                let label = incoming_label;
                self.env.push_scope();
                self.env.define(variable.name.clone(), Value::Undefined, false);
                for item in items {
                    self.env.fork_current();
                    self.env.set(&variable.name, item);
                    if let Some(cf) = self.exec_stmt(body)? {
                        match cf.for_loop(label.as_deref()) {
                            LoopOp::Break => break,
                            LoopOp::Continue => continue,
                            LoopOp::Exit(cf) => {
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
                let label = incoming_label;
                loop {
                    if let Some(cf) = self.exec_stmt(body)? {
                        match cf.for_loop(label.as_deref()) {
                            LoopOp::Break => break,
                            LoopOp::Continue => {}
                            LoopOp::Exit(cf) => return Ok(Some(cf)),
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
                self.exec_for_of_loop(variable, iterable, body, *span, false, incoming_label)
            }
            Stmt::ForAwaitOf { variable, iterable, body, span, .. } => {
                self.exec_for_of_loop(variable, iterable, body, *span, true, incoming_label)
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
                let stack_depth = self.call_stack.len();
                let try_result = self.exec_block(try_block);

                let result = match try_result {
                    Err(err) => {
                        debug_assert_eq!(self.call_stack.len(), stack_depth, "стек вызовов разбалансирован после try");
                        match catch_block {
                            Some(cb) => {
                                let thrown = err.thrown.map(|t| *t).unwrap_or_else(|| {
                                    let mut map = IndexMap::new();
                                    map.insert(
                                        symbols::ERROR_NAME_FIELD.to_string(),
                                        Value::String(symbols::ERROR_NAME.to_string()),
                                    );
                                    map.insert(symbols::ERROR_MESSAGE_FIELD.to_string(), Value::String(err.message));
                                    Value::object(map)
                                });
                                self.run_catch(cb, catch_param.as_ref(), thrown)
                            }
                            None => Err(err),
                        }
                    }
                    Ok(Some(ControlFlow::Throw(val))) => {
                        debug_assert_eq!(self.call_stack.len(), stack_depth, "стек вызовов разбалансирован после try");
                        match catch_block {
                            Some(cb) => self.run_catch(cb, catch_param.as_ref(), val),
                            None => Ok(Some(ControlFlow::Throw(val))),
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
            Stmt::Using { name, init, is_await, span } => {
                let value = self.eval_expr(init)?;
                if !matches!(value, Value::Null | Value::Undefined) {
                    if *is_await {
                        if !Self::has_async_dispose_method(&value, &self.env)
                            && !Self::has_dispose_method(&value, &self.env)
                        {
                            return Err(RuntimeError::new(
                                "Ресурс 'юзай сидетьНахуй' должен иметь метод 'асинхРасход' или 'расход'",
                                *span,
                            ));
                        }
                    } else if !Self::has_dispose_method(&value, &self.env) {
                        return Err(RuntimeError::new("Ресурс 'юзай' должен иметь метод 'расход'", *span));
                    }
                    self.env.add_disposable(value.clone(), *is_await);
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
                let pending_module =
                    if import_type == Some("json") { None } else { self.loading_module_path(source, *span) };
                for spec in specifiers {
                    match spec {
                        ImportSpec::Default { local } => {
                            let val = exports.get("default").cloned().unwrap_or(Value::Undefined);
                            self.env.define(local.name.clone(), val, true);
                            if let Some(path) = &pending_module {
                                self.register_module_link(path.clone(), &local.name, "default");
                            }
                        }
                        ImportSpec::Named { imported, local } => {
                            let val = match exports.get(&imported.name) {
                                Some(v) => v.clone(),
                                None if pending_module.is_some() => Value::Undefined,
                                None => {
                                    return Err(RuntimeError::new(
                                        format!("Модуль '{source}' не экспортирует '{}'", imported.name),
                                        *span,
                                    ));
                                }
                            };
                            self.env.define(local.name.clone(), val, true);
                            if let Some(path) = &pending_module {
                                self.register_module_link(path.clone(), &local.name, &imported.name);
                            }
                        }
                        ImportSpec::Namespace { local } => {
                            let mut map = IndexMap::new();
                            for (k, v) in exports.iter() {
                                map.insert(k.clone(), v.clone());
                            }
                            self.env.define(local.name.clone(), Value::object(map), true);
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
                            self.record_export(name, val);
                        }
                    }
                    Ok(result)
                }
                ExportKind::Named(idents) => {
                    for ident in idents {
                        let val = self.env.get(&ident.name).ok_or_else(|| {
                            RuntimeError::new(format!("Нельзя экспортировать неопределённое '{}'", ident.name), *span)
                        })?;
                        self.record_export(ident.name.clone(), val);
                    }
                    Ok(None)
                }
            },
        }
    }

    fn run_catch(
        &mut self,
        catch_block: &Block,
        catch_param: Option<&Identifier>,
        thrown: Value,
    ) -> Result<Option<ControlFlow>, RuntimeError> {
        self.env.push_scope();
        if let Some(param) = catch_param {
            self.env.define(param.name.clone(), thrown, false);
        }
        let r = self.exec_block_stmts(&catch_block.stmts);
        self.env.pop_scope();
        r
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
        self.hoist_functions(stmts);
        for stmt in stmts {
            if let Some(cf) = self.exec_stmt(stmt)? {
                return Ok(Some(cf));
            }
        }
        Ok(None)
    }

    pub(super) fn hoist_functions(&mut self, stmts: &[Stmt]) {
        for stmt in stmts {
            if let Stmt::FunctionDecl { name, params, body, is_generator, is_async, .. } = stmt {
                let func = Value::Function {
                    name: Rc::from(name.name.as_str()),
                    params: params.clone(),
                    body: body.clone(),
                    env: self.env.snapshot(),
                    is_generator: *is_generator,
                    is_async: *is_async,
                };
                self.env.define(name.name.clone(), func, false);
            }
        }
    }

    pub(super) fn destructure_pattern(
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
            Pattern::Default { pattern: inner, default, .. } => {
                let value = if matches!(value, Value::Undefined) { self.eval_expr(default)? } else { value };
                self.destructure_pattern(inner, value, is_const, span)
            }
            Pattern::Array { elements, rest, .. } => {
                let items: Vec<Value> = match &value {
                    Value::Array(arr) => arr.borrow().0.clone(),
                    other => {
                        let iterator_obj = self.get_user_iterator(other, span)?;
                        match iterator_obj {
                            Some(iterator_obj) => self.collect_user_iterable(iterator_obj, span)?,
                            None => {
                                return Err(RuntimeError::new(
                                    format!("Невозможно деструктурировать {} как массив", value.type_name()),
                                    span,
                                ));
                            }
                        }
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
                    self.destructure_pattern(rest_pat, Value::array(rest_items), is_const, span)?;
                }

                Ok(())
            }
            Pattern::Object { properties, rest, .. } => {
                let map: IndexMap<String, Value> = match value {
                    Value::Object(map) => map.borrow().map.clone(),
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
                    let mut rest_map = map;
                    for key in &used_keys {
                        rest_map.shift_remove(key);
                    }
                    self.destructure_pattern(rest_pat, Value::object(rest_map), is_const, span)?;
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
        label: Option<String>,
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
                self.env.fork_current();
                self.env.set(&variable.name, item);
                let body_result = self.exec_stmt(body);
                let cf = match body_result {
                    Ok(cf) => cf,
                    Err(e) => {
                        let mut state = rc.borrow_mut();
                        let _ = crate::stdlib::iterator::close(self, &mut state, span);
                        self.env.pop_scope();
                        return Err(e);
                    }
                };
                if let Some(cf) = cf {
                    match cf.for_loop(label.as_deref()) {
                        LoopOp::Break => {
                            let mut state = rc.borrow_mut();
                            crate::stdlib::iterator::close(self, &mut state, span)?;
                            break;
                        }
                        LoopOp::Continue => continue,
                        LoopOp::Exit(cf) => {
                            {
                                let mut state = rc.borrow_mut();
                                crate::stdlib::iterator::close(self, &mut state, span)?;
                            }
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
            Value::Array(elements) => elements.borrow().0.clone(),
            Value::String(s) => s.chars().map(|c| Value::String(c.to_string())).collect(),
            Value::Set(s) => s.borrow().iter().map(|k| k.as_value().clone()).collect(),
            Value::Map(entries) => {
                entries.borrow().iter().map(|(k, v)| Value::array(vec![k.as_value().clone(), v.clone()])).collect()
            }
            Value::TypedArray { buffer, offset, length, kind } => {
                crate::stdlib::typed_array::ta_elements(&buffer, offset, length, kind)
            }
            Value::Object(_) => {
                if let Some(iterator_obj) = self.get_user_iterator(&val, span)? {
                    let next_method_name = "следующий";
                    self.env.push_scope();
                    self.env.define(variable.name.clone(), Value::Undefined, false);
                    loop {
                        let next_fn = self.eval_member(iterator_obj.clone(), next_method_name, span)?;
                        let result = self.call_value_with_this(next_fn, Some(iterator_obj.clone()), span)?;
                        let done = match &result {
                            Value::Object(r) => r.borrow().get(crate::symbols::ITER_DONE).cloned(),
                            _ => None,
                        };
                        if matches!(done, Some(Value::Boolean(true))) {
                            break;
                        }
                        let item = match &result {
                            Value::Object(r) => {
                                r.borrow().get(crate::symbols::ITER_VALUE).cloned().unwrap_or(Value::Undefined)
                            }
                            _ => Value::Undefined,
                        };
                        let item = if is_await { self.do_await(item, span)? } else { item };
                        self.env.set(&variable.name, item);
                        let body_result = self.exec_stmt(body);
                        match body_result {
                            Ok(Some(cf)) => match cf.for_loop(label.as_deref()) {
                                LoopOp::Break => break,
                                LoopOp::Continue => continue,
                                LoopOp::Exit(cf) => {
                                    self.env.pop_scope();
                                    return Ok(Some(cf));
                                }
                            },
                            Ok(None) => {}
                            Err(e) => {
                                self.env.pop_scope();
                                return Err(e);
                            }
                        }
                    }
                    self.env.pop_scope();
                    return Ok(None);
                }
                return Err(RuntimeError::new(
                    "Нельзя итерировать по типу 'объект' (нет Symbol.iterator)".to_string(),
                    span,
                ));
            }
            other => {
                return Err(RuntimeError::new(format!("Нельзя итерировать по типу '{}'", other.type_name()), span));
            }
        };
        self.env.push_scope();
        self.env.define(variable.name.clone(), Value::Undefined, false);
        for item in items {
            let item = if is_await { self.do_await(item, span)? } else { item };
            self.env.fork_current();
            self.env.set(&variable.name, item);
            if let Some(cf) = self.exec_stmt(body)? {
                match cf.for_loop(label.as_deref()) {
                    LoopOp::Break => break,
                    LoopOp::Continue => continue,
                    LoopOp::Exit(cf) => {
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
        Pattern::Default { pattern, .. } => collect_pattern_names(pattern, out),
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
    fn eval(src: &str) -> crate::value::Value {
        let source = yps_lexer::SourceFile::new("test".to_string(), src.to_string());
        let (tokens, _) = yps_lexer::Lexer::new(&source).tokenize();
        let (program, _) = yps_parser::Parser::new(&tokens, &source).parse_program();
        crate::interpreter::Interpreter::new().run_repl(&program).unwrap().unwrap()
    }

    #[test]
    fn switch_nan_no_match() {
        let src = r#"
            гыы r = "нет";
            базарпо (нихуя) {
                тема (нихуя): r = "есть";
            }
            r;
        "#;
        assert_eq!(eval(src), crate::value::Value::String("нет".to_string()));
    }
}
