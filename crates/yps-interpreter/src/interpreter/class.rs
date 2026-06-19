use std::collections::HashMap;
use std::rc::Rc;

use indexmap::IndexMap;

use yps_lexer::Span;
use yps_parser::ast::{Block, ClassMember, Expr, Param};

use crate::environment::Environment;
use crate::error::RuntimeError;
use crate::symbols;
use crate::value::{ClassDef, Value};

use super::{ControlFlow, Interpreter};

impl Interpreter {
    pub(super) fn build_decorator_context(&self, kind: &str, name: &str, is_static: bool, is_private: bool) -> Value {
        let mut ctx = IndexMap::new();
        ctx.insert(symbols::DEC_KIND.to_string(), Value::String(kind.to_string()));
        ctx.insert(symbols::DEC_NAME.to_string(), Value::String(name.to_string()));
        ctx.insert(symbols::DEC_STATIC.to_string(), Value::Boolean(is_static));
        ctx.insert(symbols::DEC_PRIVATE.to_string(), Value::Boolean(is_private));
        ctx.insert(
            symbols::DEC_ADD_INITIALIZER.to_string(),
            Value::BuiltinFunction(symbols::ADD_INITIALIZER_BUILTIN.to_string()),
        );
        Value::object(ctx)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn apply_member_decorators(
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

    pub(super) fn exec_class_decl(
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
                    constructor = Some((params.clone(), body.clone(), self.env.snapshot()));
                }
                ClassMember::Method { name: m_name, params, body, is_static, is_private, .. } => {
                    let method_fn = Value::Function {
                        name: Rc::from(m_name.name.as_str()),
                        params: params.clone(),
                        body: body.clone(),
                        env: self.env.snapshot(),
                        is_generator: false,
                        is_async: false,
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
                        name: Rc::from(g_name.name.as_str()),
                        params: Rc::from([] as [Param; 0]),
                        body: body.clone(),
                        env: self.env.snapshot(),
                        is_generator: false,
                        is_async: false,
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
                        name: Rc::from(s_name.name.as_str()),
                        params: Rc::from([param.clone()]),
                        body: body.clone(),
                        env: self.env.snapshot(),
                        is_generator: false,
                        is_async: false,
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
            static_fields: std::cell::RefCell::new(static_fields),
            field_inits,
            getters,
            setters,
            static_getters,
            static_setters,
            parent,
            instance_initializers: instance_inits,
            prototype_cache: std::cell::OnceCell::new(),
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

    pub(crate) fn construct_instance(
        &mut self,
        class_val: Value,
        args: Vec<Value>,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if let Value::BuiltinFunction(_) = &class_val {
            return self.call_function(class_val, args, span);
        }
        if let Some((target, handler)) = class_val.proxy_parts() {
            return self.proxy_construct(&target, &handler, args, span);
        }
        let class_def = match &class_val {
            Value::Class(cls) => cls.clone(),
            _ => return Err(RuntimeError::new(format!("'{}' не является классом", class_val.type_name()), span)),
        };

        let mut seed = IndexMap::new();
        seed.insert(symbols::CLASS_TAG.to_string(), Value::String(class_def.name.clone()));
        seed.insert(symbols::PROTO.to_string(), Value::Class(Rc::clone(&class_def)));
        let mut instance_val = Value::object(seed);

        self.init_fields(&class_def, &mut instance_val, span)?;

        for init in &class_def.instance_initializers {
            let saved = self.env.clone();
            self.env.push_scope();
            self.env.define(symbols::THIS.to_string(), instance_val.clone(), false);
            self.call_function(init.clone(), vec![], span)?;
            instance_val = self.env.get(symbols::THIS).unwrap_or(instance_val);
            self.env = saved;
        }

        if let Some((ref params, ref body, ref env)) = class_def.constructor {
            let saved_env = self.env.clone();
            self.env = Environment::from_snapshot(Rc::clone(env), self.env.registry());
            self.env.push_scope();

            self.env.define(symbols::THIS.to_string(), instance_val.clone(), false);

            if let Some(parent) = &class_def.parent {
                self.env.define(symbols::SUPER.to_string(), Value::Class(Rc::clone(parent)), false);
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
                    self.env.define(param.name.name.clone(), Value::array(rest_values), false);
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

            self.push_frame(Rc::from(class_def.name.as_str()), span);
            let mut result = self.exec_block_stmts(&body.stmts);
            if let Err(e) = &mut result {
                e.attach_stack(self.snapshot_stack());
            }
            let frame_stack =
                if matches!(result, Ok(Some(ControlFlow::Throw(_)))) { self.snapshot_stack() } else { Vec::new() };
            let this_after = self.env.get(symbols::THIS).unwrap_or(instance_val);
            self.pop_frame();
            self.env = saved_env;

            match result? {
                Some(ControlFlow::Return(_)) | None => Ok(this_after),
                Some(ControlFlow::Throw(val)) => Err(RuntimeError::thrown_with_stack(val, span, frame_stack)),
                Some(ControlFlow::Break(label)) => Err(RuntimeError::new(
                    label.map_or_else(|| "'харэ' вне цикла".to_string(), |l| format!("Метка '{l}' не найдена")),
                    span,
                )),
                Some(ControlFlow::Continue(label)) => Err(RuntimeError::new(
                    label.map_or_else(|| "'двигай' вне цикла".to_string(), |l| format!("Метка '{l}' не найдена")),
                    span,
                )),
            }
        } else if let Some(ref parent) = class_def.parent {
            if let Some((ref params, ..)) = parent.constructor
                && !params.is_empty()
                && params.iter().filter(|p| p.default.is_none() && !p.is_rest).count() > 0
            {
                let parent_class_val = Value::Class(Rc::clone(parent));
                return self.construct_with_parent(parent_class_val, args, instance_val, span);
            }
            Ok(instance_val)
        } else {
            Ok(instance_val)
        }
    }

    pub(super) fn construct_with_parent(
        &mut self,
        parent_class_val: Value,
        args: Vec<Value>,
        child_instance: Value,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        let parent_def = match &parent_class_val {
            Value::Class(cls) => cls.clone(),
            _ => {
                return Err(RuntimeError::new(
                    format!("Родительский класс ожидался, получено '{}'", parent_class_val.type_name()),
                    span,
                ));
            }
        };

        let (params, body, env) = match &parent_def.constructor {
            Some(c) => (c.0.clone(), c.1.clone(), Rc::clone(&c.2)),
            None => {
                if let Some(grandparent) = &parent_def.parent {
                    let grandparent_val = Value::Class(Rc::clone(grandparent));
                    return self.construct_with_parent(grandparent_val, args, child_instance, span);
                }
                return Ok(child_instance);
            }
        };

        let saved_env = self.env.clone();
        self.env = Environment::from_snapshot(env, self.env.registry());
        self.env.push_scope();
        self.env.define(symbols::THIS.to_string(), child_instance.clone(), false);
        if let Some(grandparent) = &parent_def.parent {
            self.env.define(symbols::SUPER.to_string(), Value::Class(Rc::clone(grandparent)), false);
        }

        let required_count = params.iter().filter(|p| !p.is_rest && p.default.is_none()).count();
        if args.len() < required_count {
            self.env = saved_env;
            return Err(RuntimeError::new(
                format!(
                    "Конструктор '{}' ожидает минимум {} аргумент(ов), получено {}",
                    parent_def.name,
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
                self.env.define(param.name.name.clone(), Value::array(rest_values), false);
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
        let this_after = self.env.get(symbols::THIS).unwrap_or(child_instance);
        self.env = saved_env;

        match result? {
            Some(ControlFlow::Return(_)) | None => Ok(this_after),
            Some(ControlFlow::Throw(val)) => Err(RuntimeError::thrown(val, span)),
            Some(ControlFlow::Break(label)) => Err(RuntimeError::new(
                label.map_or_else(|| "'харэ' вне цикла".to_string(), |l| format!("Метка '{l}' не найдена")),
                span,
            )),
            Some(ControlFlow::Continue(label)) => Err(RuntimeError::new(
                label.map_or_else(|| "'двигай' вне цикла".to_string(), |l| format!("Метка '{l}' не найдена")),
                span,
            )),
        }
    }

    pub(super) fn init_fields(
        &mut self,
        class_def: &ClassDef,
        instance_val: &mut Value,
        span: Span,
    ) -> Result<(), RuntimeError> {
        if let Some(ref parent) = class_def.parent {
            self.init_fields(parent, instance_val, span)?;
        }
        let map = match instance_val {
            Value::Object(m) => Rc::clone(m),
            _ => return Ok(()),
        };
        for (name, init_body, transform) in &class_def.field_inits {
            let base_val = if let Some(body) = init_body {
                let saved_env = self.env.clone();
                self.env.push_scope();
                self.env.define(symbols::THIS.to_string(), instance_val.clone(), false);
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
            map.borrow_mut().insert(name.clone(), val);
        }
        Ok(())
    }

    pub(crate) fn instance_of_check(&self, value: &Value, target: &Rc<ClassDef>) -> bool {
        let mut current = value.clone();
        for _ in 0..256 {
            let Value::Object(map) = &current else {
                return false;
            };
            if let Some(cls) = Self::resolve_class_for_object(map, &self.env) {
                let mut walker: Option<&ClassDef> = Some(&cls);
                while let Some(c) = walker {
                    if Rc::ptr_eq(&cls, target) || c.name == target.name {
                        return true;
                    }
                    walker = c.parent.as_deref();
                }
            }
            let next = map.borrow().get(symbols::PROTO).cloned();
            match next {
                Some(proto @ Value::Object(_)) => current = proto,
                _ => return false,
            }
        }
        false
    }

    pub(super) fn find_method_in_class<'a>(
        class_def: &'a ClassDef,
        method_name: &str,
    ) -> Option<&'a crate::value::MethodDef> {
        if let Some(m) = class_def.methods.get(method_name) {
            return Some(m);
        }
        if let Some(ref parent) = class_def.parent {
            return Self::find_method_in_class(parent, method_name);
        }
        None
    }

    pub(super) fn has_dispose_method(value: &Value, env: &Environment) -> bool {
        if let Value::Object(map) = value {
            let dispose_sym = symbols::symbol_key(crate::stdlib::symbol::DISPOSE_ID);
            if let Some(Value::Function { .. }) = map.borrow().get(&dispose_sym) {
                return true;
            }
            if let Some(Value::Function { .. }) = map.borrow().get(symbols::DISPOSE_METHOD) {
                return true;
            }
            let class_name = match map.borrow().get(symbols::CLASS_TAG) {
                Some(Value::String(cn)) => Some(cn.clone()),
                _ => None,
            };
            if let Some(class_name) = class_name
                && let Some(Value::Class(cls)) = env.get(&class_name)
                && Self::find_method_in_class(&cls, symbols::DISPOSE_METHOD).is_some()
            {
                return true;
            }
        }
        false
    }

    pub(super) fn invoke_dispose(&mut self, resource: Value, span: Span) -> Result<(), RuntimeError> {
        if let Value::Object(map) = &resource {
            let dispose_sym = symbols::symbol_key(crate::stdlib::symbol::DISPOSE_ID);
            let dispose_fn = {
                let borrowed = map.borrow();
                borrowed.get(&dispose_sym).or_else(|| borrowed.get(symbols::DISPOSE_METHOD)).cloned()
            };
            if let Some(Value::Function { params, body, env, .. }) = dispose_fn {
                self.call_method_with_this(
                    Rc::from("<dispose>"),
                    &params,
                    &body,
                    &env,
                    vec![],
                    Some(resource.clone()),
                    span,
                )?;
                return Ok(());
            }
            let class_tag = map.borrow().get(symbols::CLASS_TAG).cloned();
            if let Some(Value::String(class_name)) = class_tag
                && let Some(Value::Class(cls)) = self.env.get(&class_name)
                && let Some(method) = Self::find_method_in_class(&cls, symbols::DISPOSE_METHOD)
            {
                let params = method.0.clone();
                let body = Rc::clone(&method.1);
                let env = Rc::clone(&method.2);
                self.call_method_with_this(Rc::from("<dispose>"), &params, &body, &env, vec![], Some(resource), span)?;
                return Ok(());
            }
        }
        Err(RuntimeError::new("Ресурс 'юзай' должен иметь метод 'расход'", span))
    }

    pub(crate) fn dispose_current_scope(&mut self, span: Span) -> Result<(), RuntimeError> {
        let disposables = self.env.take_disposables();
        let mut first_err: Option<RuntimeError> = None;
        for resource in disposables.into_iter().rev() {
            if matches!(resource, Value::Null | Value::Undefined) {
                continue;
            }
            if let Err(e) = self.invoke_dispose(resource, span)
                && first_err.is_none()
            {
                first_err = Some(e);
            }
        }
        if let Some(e) = first_err {
            return Err(e);
        }
        Ok(())
    }

    pub(super) fn find_method_owner_parent(class_def: &Rc<ClassDef>, method_name: &str) -> Option<Rc<ClassDef>> {
        if class_def.methods.contains_key(method_name) {
            return class_def.parent.clone();
        }
        if let Some(ref parent) = class_def.parent {
            return Self::find_method_owner_parent(parent, method_name);
        }
        None
    }

    pub(super) fn find_getter_owner_parent(class_def: &Rc<ClassDef>, name: &str) -> Option<Rc<ClassDef>> {
        if class_def.getters.contains_key(name) {
            return class_def.parent.clone();
        }
        if let Some(ref parent) = class_def.parent {
            return Self::find_getter_owner_parent(parent, name);
        }
        None
    }

    pub(super) fn find_static_method_in_class<'a>(
        class_def: &'a ClassDef,
        name: &str,
    ) -> Option<&'a crate::value::MethodDef> {
        if let Some(m) = class_def.static_methods.get(name) {
            return Some(m);
        }
        if let Some(ref parent) = class_def.parent {
            return Self::find_static_method_in_class(parent, name);
        }
        None
    }

    pub(super) fn find_static_method_owner_parent(class_def: &Rc<ClassDef>, name: &str) -> Option<Rc<ClassDef>> {
        if class_def.static_methods.contains_key(name) {
            return class_def.parent.clone();
        }
        if let Some(ref parent) = class_def.parent {
            return Self::find_static_method_owner_parent(parent, name);
        }
        None
    }

    pub(super) fn find_static_field_in_class(class_def: &ClassDef, name: &str) -> Option<Value> {
        if let Some(v) = class_def.static_fields.borrow().get(name) {
            return Some(v.clone());
        }
        if let Some(ref parent) = class_def.parent {
            return Self::find_static_field_in_class(parent, name);
        }
        None
    }

    pub(super) fn find_static_field_owner(class_def: &Rc<ClassDef>, name: &str) -> Option<Rc<ClassDef>> {
        if class_def.static_fields.borrow().contains_key(name) {
            return Some(Rc::clone(class_def));
        }
        if let Some(ref parent) = class_def.parent {
            return Self::find_static_field_owner(parent, name);
        }
        None
    }

    pub(super) fn find_static_getter_in_class<'a>(
        class_def: &'a ClassDef,
        name: &str,
    ) -> Option<&'a crate::value::MethodDef> {
        if let Some(g) = class_def.static_getters.get(name) {
            return Some(g);
        }
        if let Some(ref parent) = class_def.parent {
            return Self::find_static_getter_in_class(parent, name);
        }
        None
    }

    pub(super) fn find_static_setter_in_class<'a>(
        class_def: &'a ClassDef,
        name: &str,
    ) -> Option<&'a crate::value::MethodDef> {
        if let Some(s) = class_def.static_setters.get(name) {
            return Some(s);
        }
        if let Some(ref parent) = class_def.parent {
            return Self::find_static_setter_in_class(parent, name);
        }
        None
    }

    pub(super) fn find_getter_in_class<'a>(class_def: &'a ClassDef, name: &str) -> Option<&'a crate::value::MethodDef> {
        if let Some(g) = class_def.getters.get(name) {
            return Some(g);
        }
        if let Some(ref parent) = class_def.parent {
            return Self::find_getter_in_class(parent, name);
        }
        None
    }

    pub(super) fn find_setter_in_class<'a>(class_def: &'a ClassDef, name: &str) -> Option<&'a crate::value::MethodDef> {
        if let Some(s) = class_def.setters.get(name) {
            return Some(s);
        }
        if let Some(ref parent) = class_def.parent {
            return Self::find_setter_in_class(parent, name);
        }
        None
    }
}
