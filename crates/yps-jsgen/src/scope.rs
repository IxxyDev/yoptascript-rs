use std::collections::HashSet;

use yps_parser::{
    Block, ClassMember, ExportKind, Expr, ImportSpec, Literal, ObjectEntry, Param, Pattern, Program, PropKey, Stmt,
    TemplatePart,
};

/// Best-effort whole-program collection of every name bound anywhere in the file.
/// Not a real lexical-scope resolver: a name shadowed in one function is treated as
/// shadowed everywhere, so the unsupported-global check errs towards silence.
pub(crate) fn collect_declared(program: &Program) -> HashSet<String> {
    let mut names = HashSet::new();
    for stmt in &program.items {
        collect_stmt(stmt, &mut names);
    }
    names
}

fn collect_pattern(pattern: &Pattern, names: &mut HashSet<String>) {
    match pattern {
        Pattern::Identifier(id) => {
            names.insert(id.name.clone());
        }
        Pattern::Array { elements, rest, .. } => {
            for element in elements.iter().flatten() {
                collect_pattern(element, names);
            }
            if let Some(rest) = rest {
                collect_pattern(rest, names);
            }
        }
        Pattern::Object { properties, rest, .. } => {
            for prop in properties {
                match &prop.value {
                    Some(value) => collect_pattern(value, names),
                    None => {
                        names.insert(prop.key.name.clone());
                    }
                }
            }
            if let Some(rest) = rest {
                collect_pattern(rest, names);
            }
        }
        Pattern::Default { pattern, default, .. } => {
            collect_pattern(pattern, names);
            collect_expr(default, names);
        }
    }
}

fn collect_params(params: &[Param], names: &mut HashSet<String>) {
    for param in params {
        match &param.pattern {
            Some(pattern) => collect_pattern(pattern, names),
            None => {
                names.insert(param.name.name.clone());
            }
        }
        if let Some(default) = &param.default {
            collect_expr(default, names);
        }
    }
}

fn collect_block(block: &Block, names: &mut HashSet<String>) {
    for stmt in &block.stmts {
        collect_stmt(stmt, names);
    }
}

fn collect_stmt(stmt: &Stmt, names: &mut HashSet<String>) {
    match stmt {
        Stmt::VarDecl { pattern, init, .. } => {
            collect_pattern(pattern, names);
            collect_expr(init, names);
        }
        Stmt::Using { name, init, .. } => {
            names.insert(name.name.clone());
            collect_expr(init, names);
        }
        Stmt::Expr { expr, .. } | Stmt::Throw { value: expr, .. } => collect_expr(expr, names),
        Stmt::Block(block) => collect_block(block, names),
        Stmt::Empty { .. } | Stmt::Break { .. } | Stmt::Continue { .. } | Stmt::Debugger { .. } => {}
        Stmt::If { condition, then_branch, else_branch, .. } => {
            collect_expr(condition, names);
            collect_stmt(then_branch, names);
            if let Some(else_branch) = else_branch {
                collect_stmt(else_branch, names);
            }
        }
        Stmt::While { condition, body, .. } | Stmt::DoWhile { condition, body, .. } => {
            collect_expr(condition, names);
            collect_stmt(body, names);
        }
        Stmt::For { init, condition, update, body, .. } => {
            if let Some(init) = init {
                collect_stmt(init, names);
            }
            if let Some(condition) = condition {
                collect_expr(condition, names);
            }
            if let Some(update) = update {
                collect_expr(update, names);
            }
            collect_stmt(body, names);
        }
        Stmt::ForIn { variable, iterable, body, .. }
        | Stmt::ForOf { variable, iterable, body, .. }
        | Stmt::ForAwaitOf { variable, iterable, body, .. } => {
            collect_pattern(variable, names);
            collect_expr(iterable, names);
            collect_stmt(body, names);
        }
        Stmt::Labeled { body, .. } => collect_stmt(body, names),
        Stmt::FunctionDecl { name, params, body, .. } => {
            names.insert(name.name.clone());
            collect_params(params, names);
            collect_block(body, names);
        }
        Stmt::Return { value, .. } => {
            if let Some(value) = value {
                collect_expr(value, names);
            }
        }
        Stmt::TryCatch { try_block, catch_param, catch_block, finally_block, .. } => {
            collect_block(try_block, names);
            if let Some(param) = catch_param {
                names.insert(param.name.clone());
            }
            if let Some(block) = catch_block {
                collect_block(block, names);
            }
            if let Some(block) = finally_block {
                collect_block(block, names);
            }
        }
        Stmt::Switch { expr, cases, default, .. } => {
            collect_expr(expr, names);
            for case in cases {
                collect_expr(&case.value, names);
                collect_block(&case.body, names);
            }
            if let Some(default) = default {
                collect_block(default, names);
            }
        }
        Stmt::ClassDecl { name, super_class, members, decorators, .. } => {
            names.insert(name.name.clone());
            if let Some(super_class) = super_class {
                collect_expr(super_class, names);
            }
            collect_decorators(decorators, names);
            for member in members {
                collect_member(member, names);
            }
        }
        Stmt::Import { specifiers, .. } => {
            for spec in specifiers {
                let local = match spec {
                    ImportSpec::Default { local } | ImportSpec::Namespace { local } => local,
                    ImportSpec::Named { local, .. } => local,
                };
                names.insert(local.name.clone());
            }
        }
        Stmt::Export { kind, .. } => match kind {
            ExportKind::Declaration(inner) => collect_stmt(inner, names),
            ExportKind::Named(_) => {}
        },
    }
}

fn collect_member(member: &ClassMember, names: &mut HashSet<String>) {
    match member {
        ClassMember::Constructor { params, body, .. } => {
            collect_params(params, names);
            collect_block(body, names);
        }
        ClassMember::Method { params, body, decorators, .. } => {
            collect_decorators(decorators, names);
            collect_params(params, names);
            collect_block(body, names);
        }
        ClassMember::Field { init, decorators, .. } => {
            collect_decorators(decorators, names);
            if let Some(init) = init {
                collect_expr(init, names);
            }
        }
        ClassMember::Getter { body, decorators, .. } => {
            collect_decorators(decorators, names);
            collect_block(body, names);
        }
        ClassMember::Setter { param, body, decorators, .. } => {
            collect_decorators(decorators, names);
            collect_params(std::slice::from_ref(param), names);
            collect_block(body, names);
        }
        ClassMember::StaticBlock { body, .. } => collect_block(body, names),
    }
}

fn collect_decorators(decorators: &[Expr], names: &mut HashSet<String>) {
    for decorator in decorators {
        collect_expr(decorator, names);
    }
}

fn collect_expr(expr: &Expr, names: &mut HashSet<String>) {
    match expr {
        Expr::Identifier(_) | Expr::This { .. } | Expr::Super { .. } => {}
        Expr::Literal(literal) => collect_literal(literal, names),
        Expr::Unary { expr, .. }
        | Expr::Postfix { expr, .. }
        | Expr::Grouping { expr, .. }
        | Expr::Spread { expr, .. } => collect_expr(expr, names),
        Expr::Binary { lhs, rhs, .. } => {
            collect_expr(lhs, names);
            collect_expr(rhs, names);
        }
        Expr::Assignment { value, .. } => collect_expr(value, names),
        Expr::Call { callee, args, .. } | Expr::OptionalCall { callee, args, .. } | Expr::New { callee, args, .. } => {
            collect_expr(callee, names);
            for arg in args {
                collect_expr(arg, names);
            }
        }
        Expr::Index { object, index, .. } | Expr::OptionalIndex { object, index, .. } => {
            collect_expr(object, names);
            collect_expr(index, names);
        }
        Expr::Member { object, .. } | Expr::OptionalMember { object, .. } => collect_expr(object, names),
        Expr::Conditional { condition, then_expr, else_expr, .. } => {
            collect_expr(condition, names);
            collect_expr(then_expr, names);
            collect_expr(else_expr, names);
        }
        Expr::ArrowFunction { params, body, .. } => {
            collect_params(params, names);
            collect_block(body, names);
        }
        Expr::FunctionExpr { name, params, body, .. } => {
            if let Some(name) = name {
                names.insert(name.name.clone());
            }
            collect_params(params, names);
            collect_block(body, names);
        }
        Expr::TemplateLiteral { parts, .. } => {
            for part in parts {
                if let TemplatePart::Expr(expr) = part {
                    collect_expr(expr, names);
                }
            }
        }
        Expr::TaggedTemplate { tag, expressions, .. } => {
            collect_expr(tag, names);
            for expr in expressions {
                collect_expr(expr, names);
            }
        }
        Expr::Yield { argument, .. } => {
            if let Some(argument) = argument {
                collect_expr(argument, names);
            }
        }
        Expr::Await { argument: inner, .. } | Expr::DynamicImport { source: inner, .. } => collect_expr(inner, names),
    }
}

fn collect_literal(literal: &Literal, names: &mut HashSet<String>) {
    match literal {
        Literal::Array { elements, .. } => {
            for element in elements {
                collect_expr(element, names);
            }
        }
        Literal::Object { entries, .. } => {
            for entry in entries {
                match entry {
                    ObjectEntry::Property { key, value } => {
                        collect_prop_key(key, names);
                        collect_expr(value, names);
                    }
                    ObjectEntry::Spread(expr) => collect_expr(expr, names),
                    ObjectEntry::Getter { key, body, .. } => {
                        collect_prop_key(key, names);
                        collect_block(body, names);
                    }
                    ObjectEntry::Setter { key, param, body, .. } => {
                        collect_prop_key(key, names);
                        collect_params(std::slice::from_ref(param), names);
                        collect_block(body, names);
                    }
                }
            }
        }
        _ => {}
    }
}

fn collect_prop_key(key: &PropKey, names: &mut HashSet<String>) {
    if let PropKey::Computed(expr) = key {
        collect_expr(expr, names);
    }
}
