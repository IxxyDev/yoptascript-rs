use std::collections::HashMap;
use std::rc::Rc;

use yps_lexer::Span;
use yps_parser::ast::{BinaryOp, Expr, Literal, ObjectEntry, Param, PropKey, TemplatePart, UnaryOp};

use crate::error::RuntimeError;
use crate::symbols;
use crate::value::{ClassDef, Value};

use super::Interpreter;

impl Interpreter {
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
                            | Value::RegExp { .. }
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
            Expr::Yield { span, .. } => Err(RuntimeError::new(
                "'поебалу' разрешён только как самостоятельный оператор или правая часть присваивания/декларации внутри 'пиздюли'",
                *span,
            )),
            Expr::DynamicImport { source, span } => {
                let source_val = self.eval_expr(source)?;
                let path = match source_val {
                    Value::String(s) => s,
                    other => {
                        return Ok(Self::make_rejected_promise(Value::String(format!(
                            "Аргумент динамического импорта должен быть строкой, получено '{}'",
                            other.type_name()
                        ))));
                    }
                };
                match self.load_module(&path, *span) {
                    Ok(exports) => {
                        let mut map = HashMap::new();
                        for (k, v) in exports {
                            map.insert(k, v);
                        }
                        Ok(Self::make_fulfilled_promise(Value::Object(map)))
                    }
                    Err(err) => Ok(Self::make_rejected_promise(Value::String(err.message))),
                }
            }
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
            Literal::RegExp { pattern, flags, span } => {
                let compiled = crate::stdlib::regexp::compile(pattern, flags, *span)?;
                Ok(Value::RegExp { pattern: pattern.clone(), flags: flags.clone(), compiled })
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
}
