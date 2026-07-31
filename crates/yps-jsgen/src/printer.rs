use std::borrow::Cow;
use std::collections::BTreeSet;
use std::collections::HashSet;

use yps_lexer::Span;
use yps_parser::{
    ASSIGN_PRECEDENCE, CALL_PRECEDENCE, POSTFIX_PRECEDENCE, TERNARY_PRECEDENCE, UNARY_PRECEDENCE,
    binary_is_right_assoc, binary_precedence,
};
use yps_parser::{
    BinaryOp, Block, ClassMember, ExportKind, Expr, Identifier, ImportSpec, Literal, ObjectEntry, ObjectPatternProp,
    Param, Pattern, PostfixOp, Program, PropKey, Stmt, SwitchCase, TemplatePart, TemplateQuasi, UnaryOp,
};

use crate::TranspileError;
use crate::builtins::{Builtin, CONSOLE_MEMBERS, Helper, STDIN_SRC, is_unsupported_global, lookup};
use crate::scope::collect_declared;

const INDENT: &str = "  ";

pub fn print_program(program: &Program) -> Result<String, TranspileError> {
    let mut printer = Printer {
        out: String::new(),
        depth: 0,
        declared: collect_declared(program),
        helpers: BTreeSet::new(),
        error: None,
        switch_counter: 0,
    };
    printer.print_program(program);
    if let Some(error) = printer.error {
        return Err(error);
    }
    Ok(printer.finish())
}

struct Printer {
    out: String,
    depth: usize,
    declared: HashSet<String>,
    helpers: BTreeSet<Helper>,
    error: Option<TranspileError>,
    switch_counter: usize,
}

impl Printer {
    fn finish(self) -> String {
        if self.helpers.is_empty() {
            return self.out;
        }
        let mut prelude = String::new();
        if self.helpers.iter().any(|h| h.needs_stdin()) {
            prelude.push_str(STDIN_SRC);
            prelude.push_str("\n\n");
        }
        for helper in &self.helpers {
            prelude.push_str(helper.source());
            prelude.push_str("\n\n");
        }
        prelude.push_str(&self.out);
        prelude
    }

    fn fail(&mut self, message: impl Into<String>, span: Span) {
        if self.error.is_none() {
            self.error = Some(TranspileError { message: message.into(), span });
        }
    }

    fn helper(&mut self, helper: Helper) -> &'static str {
        self.helpers.insert(helper);
        helper.js_name()
    }

    fn write(&mut self, s: &str) {
        self.out.push_str(s);
    }

    fn indent(&mut self) {
        for _ in 0..self.depth {
            self.out.push_str(INDENT);
        }
    }

    fn newline(&mut self) {
        self.out.push('\n');
    }

    fn write_quoted(&mut self, value: &str) {
        self.out.push('"');
        for ch in value.chars() {
            match ch {
                '\n' => self.out.push_str("\\n"),
                '\t' => self.out.push_str("\\t"),
                '\r' => self.out.push_str("\\r"),
                '\0' => self.out.push_str("\\0"),
                '\\' => self.out.push_str("\\\\"),
                '"' => self.out.push_str("\\\""),
                other => self.out.push(other),
            }
        }
        self.out.push('"');
    }

    fn write_template_text(&mut self, value: &str) {
        for ch in value.chars() {
            match ch {
                '\\' => self.out.push_str("\\\\"),
                '`' => self.out.push_str("\\`"),
                '$' => self.out.push_str("\\$"),
                '\r' => self.out.push_str("\\r"),
                '\0' => self.out.push_str("\\0"),
                other => self.out.push(other),
            }
        }
    }

    fn print_program(&mut self, program: &Program) {
        for stmt in &program.items {
            self.print_stmt_line(stmt);
        }
    }

    fn print_stmt_line(&mut self, stmt: &Stmt) {
        self.indent();
        self.print_stmt(stmt);
        self.newline();
    }

    fn print_block(&mut self, block: &Block) {
        self.write("{");
        if block.stmts.is_empty() {
            self.write("}");
            return;
        }
        self.newline();
        self.depth += 1;
        for stmt in &block.stmts {
            self.print_stmt_line(stmt);
        }
        self.depth -= 1;
        self.indent();
        self.write("}");
    }

    fn print_branch(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Block(block) => self.print_block(block),
            _ => {
                self.write("{");
                self.newline();
                self.depth += 1;
                self.print_stmt_line(stmt);
                self.depth -= 1;
                self.indent();
                self.write("}");
            }
        }
    }

    fn print_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::VarDecl { pattern, init, is_const, .. } => {
                self.print_var_decl(pattern, init, *is_const);
            }
            Stmt::Using { name, init, is_await, .. } => {
                if *is_await {
                    self.write("await ");
                }
                self.write("using ");
                self.write(&name.name);
                self.write(" = ");
                self.print_expr(init, 0);
                self.write(";");
            }
            Stmt::Expr { expr, .. } => {
                self.wrapped(stmt_expr_needs_parens(expr), |p| p.print_expr(expr, 0));
                self.write(";");
            }
            Stmt::Block(block) => self.print_block(block),
            Stmt::Empty { .. } => self.write(";"),
            Stmt::If { condition, then_branch, else_branch, .. } => {
                self.write("if (");
                self.print_expr(condition, 0);
                self.write(") ");
                self.print_branch(then_branch);
                if let Some(else_branch) = else_branch {
                    self.write(" else ");
                    if matches!(else_branch.as_ref(), Stmt::If { .. }) {
                        self.print_stmt(else_branch);
                    } else {
                        self.print_branch(else_branch);
                    }
                }
            }
            Stmt::While { condition, body, .. } => {
                self.write("while (");
                self.print_expr(condition, 0);
                self.write(") ");
                self.print_branch(body);
            }
            Stmt::DoWhile { body, condition, .. } => {
                self.write("do ");
                self.print_branch(body);
                self.write(" while (");
                self.print_expr(condition, 0);
                self.write(");");
            }
            Stmt::For { init, condition, update, body, .. } => {
                self.write("for (");
                match init {
                    Some(init) => self.print_for_init(init),
                    None => self.write(";"),
                }
                if let Some(condition) = condition {
                    self.write(" ");
                    self.print_expr(condition, 0);
                }
                self.write(";");
                if let Some(update) = update {
                    self.write(" ");
                    self.print_expr(update, 0);
                }
                self.write(") ");
                self.print_branch(body);
            }
            Stmt::ForIn { variable, iterable, body, .. } => {
                self.print_for_each("for (let ", variable, " in ", iterable, body);
            }
            Stmt::ForOf { variable, iterable, body, .. } => {
                self.print_for_each("for (let ", variable, " of ", iterable, body);
            }
            Stmt::ForAwaitOf { variable, iterable, body, .. } => {
                self.print_for_each("for await (let ", variable, " of ", iterable, body);
            }
            Stmt::Break { label, .. } => {
                self.write("break");
                if let Some(label) = label {
                    self.write(" ");
                    self.write(&label.name);
                }
                self.write(";");
            }
            Stmt::Continue { label, .. } => {
                self.write("continue");
                if let Some(label) = label {
                    self.write(" ");
                    self.write(&label.name);
                }
                self.write(";");
            }
            Stmt::Labeled { label, body, .. } => {
                self.write(&label.name);
                self.write(": ");
                self.print_stmt(body);
            }
            Stmt::FunctionDecl { name, params, body, is_generator, is_async, .. } => {
                self.print_function_head(*is_async, *is_generator, Some(&name.name));
                self.print_params(params);
                self.write(" ");
                self.print_block(body);
            }
            Stmt::Return { value, .. } => {
                self.write("return");
                if let Some(value) = value {
                    self.write(" ");
                    self.print_expr(value, 0);
                }
                self.write(";");
            }
            Stmt::Throw { value, .. } => {
                self.write("throw ");
                self.print_expr(value, 0);
                self.write(";");
            }
            Stmt::TryCatch { try_block, catch_param, catch_block, finally_block, .. } => {
                self.write("try ");
                self.print_block(try_block);
                if let Some(catch_block) = catch_block {
                    self.write(" catch");
                    if let Some(param) = catch_param {
                        self.write(" (");
                        self.write(&param.name);
                        self.write(")");
                    }
                    self.write(" ");
                    self.print_block(catch_block);
                }
                if let Some(finally_block) = finally_block {
                    self.write(" finally ");
                    self.print_block(finally_block);
                }
            }
            Stmt::Switch { expr, cases, default, .. } => self.print_switch(expr, cases, default.as_ref()),
            Stmt::ClassDecl { name, super_class, members, decorators, .. } => {
                for decorator in decorators {
                    self.write("@");
                    self.print_expr(decorator, CALL_PRECEDENCE);
                    self.newline();
                    self.indent();
                }
                self.write("class ");
                self.write(&name.name);
                if let Some(super_class) = super_class {
                    self.write(" extends ");
                    self.print_expr(super_class, CALL_PRECEDENCE);
                }
                self.write(" {");
                if members.is_empty() {
                    self.write("}");
                    return;
                }
                self.newline();
                self.depth += 1;
                for member in members {
                    self.print_class_member(member);
                }
                self.depth -= 1;
                self.indent();
                self.write("}");
            }
            Stmt::Debugger { .. } => self.write("debugger;"),
            Stmt::Import { specifiers, source, attributes, .. } => {
                self.print_import(specifiers, source, attributes);
            }
            Stmt::Export { kind, .. } => {
                self.write("export ");
                match kind {
                    ExportKind::Named(names) => {
                        self.write("{ ");
                        for (i, name) in names.iter().enumerate() {
                            if i > 0 {
                                self.write(", ");
                            }
                            self.write(&name.name);
                        }
                        self.write(" };");
                    }
                    ExportKind::Declaration(inner) => self.print_stmt(inner),
                }
            }
        }
    }

    fn print_import(&mut self, specifiers: &[ImportSpec], source: &str, attributes: &[(String, String)]) {
        self.write("import ");
        let mut named = specifiers
            .iter()
            .filter_map(|spec| match spec {
                ImportSpec::Named { imported, local } => Some((imported, local)),
                _ => None,
            })
            .peekable();
        let mut first = true;
        for spec in specifiers {
            match spec {
                ImportSpec::Default { local } => {
                    if !first {
                        self.write(", ");
                    }
                    first = false;
                    self.write(&local.name);
                }
                ImportSpec::Namespace { local } => {
                    if !first {
                        self.write(", ");
                    }
                    first = false;
                    self.write("* as ");
                    self.write(&local.name);
                }
                ImportSpec::Named { .. } => {}
            }
        }
        if named.peek().is_some() {
            if !first {
                self.write(", ");
            }
            first = false;
            self.write("{ ");
            for (i, (imported, local)) in named.enumerate() {
                if i > 0 {
                    self.write(", ");
                }
                self.write(&imported.name);
                if imported.name != local.name {
                    self.write(" as ");
                    self.write(&local.name);
                }
            }
            self.write(" }");
        }
        if !first {
            self.write(" from ");
        }
        self.write_quoted(source);
        if !attributes.is_empty() {
            self.write(" with { ");
            for (i, (key, value)) in attributes.iter().enumerate() {
                if i > 0 {
                    self.write(", ");
                }
                self.write(key);
                self.write(": ");
                self.write_quoted(value);
            }
            self.write(" }");
        }
        self.write(";");
    }

    fn print_for_each(&mut self, head: &str, variable: &Pattern, keyword: &str, iterable: &Expr, body: &Stmt) {
        self.write(head);
        self.print_pattern(variable);
        self.write(keyword);
        self.print_expr(iterable, 0);
        self.write(") ");
        self.print_branch(body);
    }

    fn print_var_decl(&mut self, pattern: &Pattern, init: &Expr, is_const: bool) {
        self.write(if is_const { "const " } else { "let " });
        self.print_pattern(pattern);
        self.write(" = ");
        self.print_expr(init, 0);
        self.write(";");
    }

    fn print_for_init(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::VarDecl { pattern, init, is_const, .. } => self.print_var_decl(pattern, init, *is_const),
            Stmt::Expr { expr, .. } => {
                self.print_expr(expr, 0);
                self.write(";");
            }
            other => self.print_stmt(other),
        }
    }

    /// `базарпо` разворачивается в цепочку `if/else if`, а не в JS `switch`: тела `тема`
    /// не проваливаются, а `харэ` внутри тела адресует ОХВАТЫВАЮЩИЙ цикл, тогда как JS
    /// `break` всегда вышел бы из `switch`.
    fn print_switch(&mut self, expr: &Expr, cases: &[SwitchCase], default: Option<&Block>) {
        let tmp = format!("__ypsSwitch{}", self.switch_counter);
        self.switch_counter += 1;
        self.write("{");
        self.newline();
        self.depth += 1;
        self.indent();
        self.write("const ");
        self.write(&tmp);
        self.write(" = ");
        self.print_expr(expr, 0);
        self.write(";");
        self.newline();
        if !cases.is_empty() || default.is_some() {
            self.indent();
            for (i, case) in cases.iter().enumerate() {
                if i > 0 {
                    self.write(" else ");
                }
                self.write("if (");
                self.write(&tmp);
                self.write(" === ");
                self.print_expr(&case.value, binary_precedence(BinaryOp::StrictEquals) + 1);
                self.write(") ");
                self.print_block(&case.body);
            }
            if let Some(default) = default {
                if !cases.is_empty() {
                    self.write(" else ");
                }
                self.print_block(default);
            }
            self.newline();
        }
        self.depth -= 1;
        self.indent();
        self.write("}");
    }

    fn print_function_head(&mut self, is_async: bool, is_generator: bool, name: Option<&str>) {
        if is_async {
            self.write("async ");
        }
        self.write(if is_generator { "function*" } else { "function" });
        if let Some(name) = name {
            self.write(" ");
            self.write(name);
        }
    }

    fn print_class_member(&mut self, member: &ClassMember) {
        match member {
            ClassMember::Constructor { params, body, .. } => {
                self.indent();
                self.write("constructor");
                self.print_params(params);
                self.write(" ");
                self.print_block(body);
                self.newline();
            }
            ClassMember::Method { name, params, body, is_static, decorators, .. } => {
                self.print_member_decorators(decorators);
                self.indent();
                self.print_static_prefix(*is_static);
                self.write(&name.name);
                self.print_params(params);
                self.write(" ");
                self.print_block(body);
                self.newline();
            }
            ClassMember::Field { name, init, is_static, decorators, .. } => {
                self.print_member_decorators(decorators);
                self.indent();
                self.print_static_prefix(*is_static);
                self.write(&name.name);
                if let Some(init) = init {
                    self.write(" = ");
                    self.print_expr(init, 0);
                }
                self.write(";");
                self.newline();
            }
            ClassMember::Getter { name, body, is_static, decorators, .. } => {
                self.print_member_decorators(decorators);
                self.indent();
                self.print_static_prefix(*is_static);
                self.write("get ");
                self.write(&name.name);
                self.write("() ");
                self.print_block(body);
                self.newline();
            }
            ClassMember::Setter { name, param, body, is_static, decorators, .. } => {
                self.print_member_decorators(decorators);
                self.indent();
                self.print_static_prefix(*is_static);
                self.write("set ");
                self.write(&name.name);
                self.write("(");
                self.print_param(param);
                self.write(") ");
                self.print_block(body);
                self.newline();
            }
            ClassMember::StaticBlock { body, .. } => {
                self.indent();
                self.write("static ");
                self.print_block(body);
                self.newline();
            }
        }
    }

    fn print_static_prefix(&mut self, is_static: bool) {
        if is_static {
            self.write("static ");
        }
    }

    fn print_member_decorators(&mut self, decorators: &[Expr]) {
        for decorator in decorators {
            self.indent();
            self.write("@");
            self.print_expr(decorator, CALL_PRECEDENCE);
            self.newline();
        }
    }

    fn print_params(&mut self, params: &[Param]) {
        self.write("(");
        for (i, param) in params.iter().enumerate() {
            if i > 0 {
                self.write(", ");
            }
            self.print_param(param);
        }
        self.write(")");
    }

    fn print_param(&mut self, param: &Param) {
        if param.is_rest {
            self.write("...");
        }
        match &param.pattern {
            Some(pattern) => self.print_pattern(pattern),
            None => self.write(&param.name.name),
        }
        if let Some(default) = &param.default {
            self.write(" = ");
            self.print_expr(default, ASSIGN_PRECEDENCE + 1);
        }
    }

    fn print_pattern(&mut self, pattern: &Pattern) {
        match pattern {
            Pattern::Identifier(id) => self.write(&id.name),
            Pattern::Array { elements, rest, .. } => {
                self.write("[");
                let mut first = true;
                for element in elements {
                    if !first {
                        self.write(", ");
                    }
                    first = false;
                    if let Some(element) = element {
                        self.print_pattern(element);
                    }
                }
                if let Some(rest) = rest {
                    if !first {
                        self.write(", ");
                    }
                    self.write("...");
                    self.print_pattern(rest);
                }
                self.write("]");
            }
            Pattern::Object { properties, rest, .. } => {
                self.write("{ ");
                let mut first = true;
                for prop in properties {
                    if !first {
                        self.write(", ");
                    }
                    first = false;
                    self.print_object_pattern_prop(prop);
                }
                if let Some(rest) = rest {
                    if !first {
                        self.write(", ");
                    }
                    self.write("...");
                    self.print_pattern(rest);
                }
                self.write(" }");
            }
            Pattern::Default { pattern, default, .. } => {
                self.print_pattern(pattern);
                self.write(" = ");
                self.print_expr(default, ASSIGN_PRECEDENCE + 1);
            }
        }
    }

    fn print_object_pattern_prop(&mut self, prop: &ObjectPatternProp) {
        match &prop.value {
            None => self.write(&prop.key.name),
            Some(Pattern::Default { pattern, default, .. }) if is_ident_named(pattern, &prop.key.name) => {
                self.write(&prop.key.name);
                self.write(" = ");
                self.print_expr(default, ASSIGN_PRECEDENCE + 1);
            }
            Some(value) => {
                self.write(&prop.key.name);
                self.write(": ");
                self.print_pattern(value);
            }
        }
    }

    fn print_expr(&mut self, expr: &Expr, parent_prec: u8) {
        match expr {
            Expr::Grouping { expr, .. } => self.print_expr(expr, parent_prec),
            Expr::Identifier(id) => self.print_ident_ref(id),
            Expr::Literal(literal) => self.print_literal(literal),
            Expr::This { .. } => self.write("this"),
            Expr::Super { .. } => self.write("super"),
            Expr::Unary { op, expr: operand, .. } => {
                let wrap = UNARY_PRECEDENCE < parent_prec;
                self.wrapped(wrap, |p| {
                    p.write(unary_op_str(*op));
                    if unary_op_needs_space(*op) {
                        p.write(" ");
                    }
                    p.print_expr(operand, UNARY_PRECEDENCE);
                });
            }
            Expr::Postfix { op, expr: operand, .. } => {
                let wrap = POSTFIX_PRECEDENCE < parent_prec;
                self.wrapped(wrap, |p| {
                    p.print_expr(operand, POSTFIX_PRECEDENCE);
                    p.write(match op {
                        PostfixOp::Increment => "++",
                        PostfixOp::Decrement => "--",
                    });
                });
            }
            Expr::Binary { op, lhs, rhs, span } => self.print_binary(*op, lhs, rhs, *span, parent_prec),
            Expr::Assignment { target, value, .. } => {
                let wrap = ASSIGN_PRECEDENCE < parent_prec;
                self.wrapped(wrap, |p| {
                    p.print_ident_ref(target);
                    p.write(" = ");
                    p.print_expr(value, ASSIGN_PRECEDENCE);
                });
            }
            Expr::Conditional { condition, then_expr, else_expr, .. } => {
                let wrap = TERNARY_PRECEDENCE < parent_prec;
                self.wrapped(wrap, |p| {
                    p.print_expr(condition, TERNARY_PRECEDENCE + 1);
                    p.write(" ? ");
                    p.print_expr(then_expr, 0);
                    p.write(" : ");
                    p.print_expr(else_expr, TERNARY_PRECEDENCE);
                });
            }
            Expr::Call { callee, args, span } => self.print_call(callee, None, args, *span),
            Expr::OptionalCall { callee, args, .. } => {
                self.print_expr(callee, CALL_PRECEDENCE);
                self.write("?.");
                self.print_args(None, args);
            }
            Expr::New { callee, args, span } => {
                self.wrapped(CALL_PRECEDENCE < parent_prec, |p| p.print_new(callee, args, *span));
            }
            Expr::Index { object, index, .. } => {
                self.reject_date_namespace(object);
                self.print_expr(object, CALL_PRECEDENCE);
                self.write("[");
                self.print_expr(index, 0);
                self.write("]");
            }
            Expr::OptionalIndex { object, index, .. } => {
                self.reject_date_namespace(object);
                self.print_expr(object, CALL_PRECEDENCE);
                self.write("?.[");
                self.print_expr(index, 0);
                self.write("]");
            }
            Expr::Member { object, property, .. } => {
                if let Some(name) = self.console_member(object, &property.name) {
                    self.write(name);
                    return;
                }
                self.reject_date_namespace(object);
                self.print_expr(object, CALL_PRECEDENCE);
                self.write(".");
                self.write(&property.name);
            }
            Expr::OptionalMember { object, property, .. } => {
                self.reject_date_namespace(object);
                self.print_expr(object, CALL_PRECEDENCE);
                self.write("?.");
                self.write(&property.name);
            }
            Expr::ArrowFunction { params, body, is_async, .. } => {
                let wrap = parent_prec > ASSIGN_PRECEDENCE;
                self.wrapped(wrap, |p| {
                    if *is_async {
                        p.write("async ");
                    }
                    p.print_params(params);
                    p.write(" => ");
                    p.print_arrow_body(body);
                });
            }
            Expr::FunctionExpr { name, params, body, is_generator, is_async, .. } => {
                self.print_function_head(*is_async, *is_generator, name.as_ref().map(|n| n.name.as_str()));
                self.print_params(params);
                self.write(" ");
                self.print_block(body);
            }
            Expr::Spread { expr, .. } => {
                self.write("...");
                self.print_expr(expr, ASSIGN_PRECEDENCE + 1);
            }
            Expr::Yield { argument, delegate, .. } => {
                let wrap = ASSIGN_PRECEDENCE < parent_prec;
                self.wrapped(wrap, |p| {
                    p.write(if *delegate { "yield*" } else { "yield" });
                    if let Some(argument) = argument {
                        p.write(" ");
                        p.print_expr(argument, ASSIGN_PRECEDENCE);
                    }
                });
            }
            Expr::Await { argument, .. } => {
                let wrap = UNARY_PRECEDENCE < parent_prec;
                self.wrapped(wrap, |p| {
                    p.write("await ");
                    p.print_expr(argument, UNARY_PRECEDENCE);
                });
            }
            Expr::DynamicImport { source, .. } => {
                self.write("import(");
                self.print_expr(source, 0);
                self.write(")");
            }
            Expr::TemplateLiteral { parts, .. } => self.print_template_literal(parts),
            Expr::TaggedTemplate { tag, quasis, expressions, .. } => {
                self.print_expr(tag, CALL_PRECEDENCE);
                self.print_tagged_template(quasis, expressions);
            }
        }
    }

    fn wrapped<F: FnOnce(&mut Self)>(&mut self, wrap: bool, f: F) {
        if wrap {
            self.write("(");
        }
        f(self);
        if wrap {
            self.write(")");
        }
    }

    fn print_binary(&mut self, op: BinaryOp, lhs: &Expr, rhs: &Expr, span: Span, parent_prec: u8) {
        // `a |> f` не имеет аналога в JS: разворачиваем в прямой вызов `f(a)`.
        if op == BinaryOp::Pipeline {
            self.print_pipeline(lhs, rhs, span);
            return;
        }

        let prec = binary_precedence(op);
        let right_assoc = binary_is_right_assoc(op);
        let wrap = prec < parent_prec;
        self.wrapped(wrap, |p| {
            let left_prec = if right_assoc { prec + 1 } else { prec };
            let right_prec = if right_assoc { prec } else { prec + 1 };
            p.print_operand(lhs, left_prec, op, true);
            p.write(" ");
            p.write(binary_op_str(op));
            p.write(" ");
            p.print_operand(rhs, right_prec, op, false);
        });
    }

    fn print_operand(&mut self, operand: &Expr, prec: u8, parent_op: BinaryOp, is_left: bool) {
        if needs_js_parens(operand, parent_op, is_left) {
            self.write("(");
            self.print_expr(operand, 0);
            self.write(")");
            return;
        }
        self.print_expr(operand, prec);
    }

    fn print_pipeline(&mut self, lhs: &Expr, rhs: &Expr, span: Span) {
        match rhs {
            Expr::Call { callee, args, span: call_span } => self.print_call(callee, Some(lhs), args, *call_span),
            Expr::Identifier(_) | Expr::Member { .. } | Expr::Index { .. } | Expr::Grouping { .. } => {
                self.print_call(rhs, Some(lhs), &[], span);
            }
            other => self.print_plain_call(other, Some(lhs), &[]),
        }
    }

    fn print_plain_call(&mut self, callee: &Expr, piped: Option<&Expr>, args: &[Expr]) {
        self.print_expr(callee, CALL_PRECEDENCE);
        self.print_args(piped, args);
    }

    fn print_args(&mut self, piped: Option<&Expr>, args: &[Expr]) {
        self.write("(");
        for (i, arg) in piped.into_iter().chain(args).enumerate() {
            if i > 0 {
                self.write(", ");
            }
            self.print_expr(arg, ASSIGN_PRECEDENCE);
        }
        self.write(")");
    }

    fn print_arrow_body(&mut self, body: &Block) {
        if let [Stmt::Return { value: Some(value), .. }] = body.stmts.as_slice() {
            let needs_parens = matches!(strip_grouping(value), Expr::Literal(Literal::Object { .. }));
            self.wrapped(needs_parens, |p| p.print_expr(value, ASSIGN_PRECEDENCE));
        } else {
            self.print_block(body);
        }
    }

    fn builtin_key<'a>(&self, callee: &'a Expr) -> Option<Cow<'a, str>> {
        match strip_grouping(callee) {
            Expr::Identifier(id) if !self.declared.contains(&id.name) => Some(Cow::Borrowed(&id.name)),
            Expr::Member { object, property, .. } => self.console_key(object, &property.name).map(Cow::Owned),
            _ => None,
        }
    }

    fn console_key(&self, object: &Expr, property: &str) -> Option<String> {
        match strip_grouping(object) {
            Expr::Identifier(id)
                if id.name == "сказать" && !self.declared.contains(&id.name) && CONSOLE_MEMBERS.contains(&property) =>
            {
                Some(format!("сказать.{property}"))
            }
            _ => None,
        }
    }

    fn console_member(&self, object: &Expr, property: &str) -> Option<&'static str> {
        let key = self.console_key(object, property)?;
        match lookup(&key)? {
            Builtin::Plain(js) => Some(js),
            _ => None,
        }
    }

    fn print_call(&mut self, callee: &Expr, piped: Option<&Expr>, args: &[Expr], span: Span) {
        let Some(key) = self.builtin_key(callee) else {
            self.print_plain_call(callee, piped, args);
            return;
        };

        if is_unsupported_global(&key) {
            self.report_unsupported(&key, span);
            return;
        }

        match lookup(&key) {
            Some(Builtin::Plain(js)) => {
                self.write(js);
                self.print_args(piped, args);
            }
            Some(Builtin::Construct(js)) => {
                self.write("new ");
                self.write(js);
                self.print_args(piped, args);
            }
            Some(Builtin::Helper(helper)) => {
                let name = self.helper(helper);
                self.write(name);
                self.print_args(piped, args);
            }
            Some(Builtin::Length) => {
                let Some(arg) = only_arg(piped, args) else {
                    self.fail("'длина' при транспиляции в JS принимает ровно 1 аргумент", span);
                    return;
                };
                self.write("(");
                self.print_expr(arg, 0);
                self.write(").length");
            }
            Some(Builtin::IsError) => {
                let Some(arg) = only_arg(piped, args) else {
                    self.fail("'этоКосяк' при транспиляции в JS принимает ровно 1 аргумент", span);
                    return;
                };
                self.write("((");
                self.print_expr(arg, 0);
                self.write(") instanceof Error)");
            }
            None => self.print_plain_call(callee, piped, args),
        }
    }

    fn print_new(&mut self, callee: &Expr, args: &[Expr], span: Span) {
        if let Some(key) = self.builtin_key(callee) {
            if is_unsupported_global(&key) {
                self.report_unsupported(&key, span);
                return;
            }
            match lookup(&key) {
                Some(Builtin::Construct(js) | Builtin::Plain(js)) => {
                    self.write("new ");
                    self.write(js);
                    self.print_args(None, args);
                    return;
                }
                Some(_) => {
                    self.fail(format!("встроенную '{key}' нельзя использовать с 'захуярить' при транспиляции"), span);
                    return;
                }
                None => {}
            }
        }
        self.write("new ");
        self.print_plain_call(callee, None, args);
    }

    fn print_ident_ref(&mut self, id: &Identifier) {
        if self.declared.contains(&id.name) {
            self.write(&id.name);
            return;
        }
        if is_unsupported_global(&id.name) {
            self.report_unsupported(&id.name, id.span);
            return;
        }
        match lookup(&id.name) {
            Some(Builtin::Plain(js) | Builtin::Construct(js)) => self.write(js),
            Some(Builtin::Helper(helper)) => {
                let name = self.helper(helper);
                self.write(name);
            }
            Some(Builtin::Length) => {
                let name = self.helper(Helper::Len);
                self.write(name);
            }
            Some(Builtin::IsError) => {
                let name = self.helper(Helper::IsError);
                self.write(name);
            }
            None => self.write(&id.name),
        }
    }

    fn reject_date_namespace(&mut self, object: &Expr) {
        if let Expr::Identifier(id) = strip_grouping(object)
            && id.name == "Дата"
            && !self.declared.contains(&id.name)
        {
            self.fail(
                "'Дата' как пространство имён (например 'Дата.сейчас()') не поддерживается транспайлером в JS: \
                 транспилируется только вызов-конструктор 'захуярить Дата(...)'",
                id.span,
            );
        }
    }

    fn report_unsupported(&mut self, name: &str, span: Span) {
        self.fail(
            format!(
                "глобальный объект стандартной библиотеки '{name}' не поддерживается транспайлером в JS: \
                 транспилируются только синтаксис и свободные встроенные функции"
            ),
            span,
        );
    }

    fn print_literal(&mut self, literal: &Literal) {
        match literal {
            Literal::Number { raw, .. } => self.write(raw),
            Literal::BigInt { value, .. } => self.write(&format!("{value}n")),
            Literal::String { value, .. } => self.write_quoted(value),
            Literal::Boolean { value, .. } => self.write(if *value { "true" } else { "false" }),
            Literal::Null { .. } => self.write("null"),
            Literal::Undefined { .. } => self.write("undefined"),
            Literal::RegExp { pattern, flags, .. } => {
                self.write("/");
                self.write(pattern);
                self.write("/");
                self.write(flags);
            }
            Literal::Array { elements, .. } => {
                self.write("[");
                for (i, element) in elements.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.print_expr(element, ASSIGN_PRECEDENCE);
                }
                self.write("]");
            }
            Literal::Object { entries, .. } => {
                if entries.is_empty() {
                    self.write("{}");
                    return;
                }
                self.write("{ ");
                for (i, entry) in entries.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.print_object_entry(entry);
                }
                self.write(" }");
            }
        }
    }

    fn print_object_entry(&mut self, entry: &ObjectEntry) {
        match entry {
            ObjectEntry::Property { key, value } => {
                self.print_prop_key(key);
                self.write(": ");
                self.print_expr(value, ASSIGN_PRECEDENCE);
            }
            ObjectEntry::Spread(expr) => {
                self.write("...");
                self.print_expr(expr, ASSIGN_PRECEDENCE + 1);
            }
            ObjectEntry::Getter { key, body, .. } => {
                self.write("get ");
                self.print_prop_key(key);
                self.write("() ");
                self.print_block(body);
            }
            ObjectEntry::Setter { key, param, body, .. } => {
                self.write("set ");
                self.print_prop_key(key);
                self.write("(");
                self.print_param(param);
                self.write(") ");
                self.print_block(body);
            }
        }
    }

    fn print_prop_key(&mut self, key: &PropKey) {
        match key {
            PropKey::Identifier(id) => self.write(&id.name),
            PropKey::Computed(expr) => {
                self.write("[");
                self.print_expr(expr, 0);
                self.write("]");
            }
        }
    }

    fn print_template_literal(&mut self, parts: &[TemplatePart]) {
        self.write("`");
        for part in parts {
            match part {
                TemplatePart::Str(s) => self.write_template_text(s),
                TemplatePart::Expr(expr) => {
                    self.write("${");
                    self.print_expr(expr, 0);
                    self.write("}");
                }
            }
        }
        self.write("`");
    }

    fn print_tagged_template(&mut self, quasis: &[TemplateQuasi], expressions: &[Expr]) {
        self.write("`");
        let mut expr_iter = expressions.iter();
        let mut first = true;
        for quasi in quasis {
            if !first && let Some(expr) = expr_iter.next() {
                self.write("${");
                self.print_expr(expr, 0);
                self.write("}");
            }
            first = false;
            self.write(&quasi.raw);
        }
        self.write("`");
    }
}

fn strip_grouping(expr: &Expr) -> &Expr {
    match expr {
        Expr::Grouping { expr, .. } => strip_grouping(expr),
        other => other,
    }
}

fn only_arg<'a>(piped: Option<&'a Expr>, args: &'a [Expr]) -> Option<&'a Expr> {
    match (piped, args) {
        (Some(arg), []) | (None, [arg]) => Some(arg),
        _ => None,
    }
}

fn is_ident_named(pattern: &Pattern, name: &str) -> bool {
    matches!(pattern, Pattern::Identifier(id) if id.name == name)
}

/// JS запрещает часть комбинаций, которые таблица приоритетов YoptaScript допускает без скобок:
/// смешение `??` с `&&`/`||` и унарный операнд слева от `**`.
fn needs_js_parens(operand: &Expr, parent_op: BinaryOp, is_left: bool) -> bool {
    let operand = strip_grouping(operand);
    if parent_op == BinaryOp::Exp && is_left && matches!(operand, Expr::Unary { .. } | Expr::Await { .. }) {
        return true;
    }
    let nullish_parent = parent_op == BinaryOp::NullishCoalescing;
    let logical_parent = matches!(parent_op, BinaryOp::And | BinaryOp::Or);
    match operand {
        Expr::Binary { op, .. } => {
            let nullish_child = *op == BinaryOp::NullishCoalescing;
            let logical_child = matches!(op, BinaryOp::And | BinaryOp::Or);
            (nullish_parent && logical_child) || (logical_parent && nullish_child)
        }
        _ => false,
    }
}

fn stmt_expr_needs_parens(expr: &Expr) -> bool {
    matches!(
        starting_expr(expr),
        Expr::Literal(Literal::Object { .. }) | Expr::ArrowFunction { .. } | Expr::FunctionExpr { .. }
    )
}

fn starting_expr(expr: &Expr) -> &Expr {
    match expr {
        Expr::Binary { lhs, .. } => starting_expr(lhs),
        Expr::Assignment { .. } => expr,
        Expr::Conditional { condition, .. } => starting_expr(condition),
        Expr::Postfix { expr, .. } => starting_expr(expr),
        Expr::Member { object, .. }
        | Expr::OptionalMember { object, .. }
        | Expr::Index { object, .. }
        | Expr::OptionalIndex { object, .. } => starting_expr(object),
        Expr::Call { callee, .. } | Expr::OptionalCall { callee, .. } => starting_expr(callee),
        Expr::TaggedTemplate { tag, .. } => starting_expr(tag),
        Expr::Grouping { expr, .. } => starting_expr(expr),
        other => other,
    }
}

const fn unary_op_str(op: UnaryOp) -> &'static str {
    match op {
        UnaryOp::Plus => "+",
        UnaryOp::Minus => "-",
        UnaryOp::Not => "!",
        UnaryOp::BitwiseNot => "~",
        UnaryOp::Typeof => "typeof",
        UnaryOp::Delete => "delete",
        UnaryOp::Void => "void",
    }
}

const fn unary_op_needs_space(op: UnaryOp) -> bool {
    matches!(op, UnaryOp::Typeof | UnaryOp::Delete | UnaryOp::Void)
}

const fn binary_op_str(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::Add => "+",
        BinaryOp::Sub => "-",
        BinaryOp::Mul => "*",
        BinaryOp::Div => "/",
        BinaryOp::Mod => "%",
        BinaryOp::Exp => "**",
        BinaryOp::Assign => "=",
        BinaryOp::PlusAssign => "+=",
        BinaryOp::MinusAssign => "-=",
        BinaryOp::MulAssign => "*=",
        BinaryOp::DivAssign => "/=",
        BinaryOp::ExpAssign => "**=",
        BinaryOp::Equals => "==",
        BinaryOp::StrictEquals => "===",
        BinaryOp::NotEquals => "!=",
        BinaryOp::StrictNotEquals => "!==",
        BinaryOp::Less => "<",
        BinaryOp::Greater => ">",
        BinaryOp::LessOrEqual => "<=",
        BinaryOp::GreaterOrEqual => ">=",
        BinaryOp::And => "&&",
        BinaryOp::Or => "||",
        BinaryOp::NullishCoalescing => "??",
        BinaryOp::NullishAssign => "??=",
        BinaryOp::AndAssign => "&&=",
        BinaryOp::OrAssign => "||=",
        BinaryOp::Pipeline => "|>",
        BinaryOp::Instanceof => "instanceof",
        BinaryOp::In => "in",
        BinaryOp::BitAnd => "&",
        BinaryOp::BitOr => "|",
        BinaryOp::BitXor => "^",
        BinaryOp::LeftShift => "<<",
        BinaryOp::RightShift => ">>",
        BinaryOp::UnsignedRightShift => ">>>",
        BinaryOp::ModAssign => "%=",
        BinaryOp::BitAndAssign => "&=",
        BinaryOp::BitOrAssign => "|=",
        BinaryOp::BitXorAssign => "^=",
        BinaryOp::ShlAssign => "<<=",
        BinaryOp::ShrAssign => ">>=",
        BinaryOp::UshrAssign => ">>>=",
    }
}

#[cfg(test)]
mod tests {
    use yps_lexer::{Lexer, SourceFile};
    use yps_parser::Parser;

    use super::print_program;
    use crate::TranspileError;

    fn parse(source: &str) -> yps_parser::Program {
        let sf = SourceFile::new("<t>".to_string(), source.to_string());
        let (tokens, diags) = Lexer::new(&sf).tokenize();
        assert!(diags.is_empty(), "лексер: {diags:?}");
        let (program, pdiags) = Parser::new(&tokens, &sf).parse_program();
        assert!(pdiags.is_empty(), "парсер: {pdiags:?}");
        program
    }

    fn js(source: &str) -> String {
        print_program(&parse(source)).expect("ожидалась успешная транспиляция")
    }

    fn js_err(source: &str) -> TranspileError {
        print_program(&parse(source)).expect_err("ожидалась ошибка транспиляции")
    }

    fn assert_contains(haystack: &str, needle: &str) {
        assert!(haystack.contains(needle), "не найдено {needle:?} в:\n{haystack}");
    }

    #[test]
    fn var_decl_uses_let_and_const() {
        assert_eq!(js("гыы а = 1;\nясенХуй б = 2;\n"), "let а = 1;\nconst б = 2;\n");
    }

    #[test]
    fn if_else_chain() {
        let out = js("вилкойвглаз (а) { б(); } иливжопураз вилкойвглаз (в) { г(); } иливжопураз { д(); }\n");
        assert_contains(&out, "if (а) {");
        assert_contains(&out, "} else if (в) {");
        assert_contains(&out, "} else {");
    }

    #[test]
    fn loops_and_jumps() {
        let out = js("метка: потрещим (правда) { харэ метка; двигай; }\n");
        assert_contains(&out, "метка: while (true) {");
        assert_contains(&out, "break метка;");
        assert_contains(&out, "continue;");
    }

    #[test]
    fn do_while_and_for() {
        assert_contains(&js("крутани { и++; } потрещим (и < 3);\n"), "do {");
        assert_contains(&js("крутани { и++; } потрещим (и < 3);\n"), "} while (и < 3);");
        assert_contains(&js("го (гыы и = 0; и < 3; и++) { ф(и); }\n"), "for (let и = 0; и < 3; и++) {");
    }

    #[test]
    fn for_in_of_and_await_of() {
        assert_contains(&js("го (гыы к из о) { ф(к); }\n"), "for (let к in о) {");
        assert_contains(&js("го (гыы к сашаГрей о) { ф(к); }\n"), "for (let к of о) {");
        let out = js("ассо йопта ф() { го сидетьНахуй (гыы к сашаГрей о) { г(к); } }\n");
        assert_contains(&out, "for await (let к of о) {");
        assert_contains(&out, "async function ф() {");
    }

    #[test]
    fn switch_lowers_to_if_else_chain() {
        let out = js("базарпо (х) { тема 1: { ф(1); } тема 2: { ф(2); } нуичо { ф(3); } }\n");
        assert!(!out.contains("switch ("), "JS switch перехватил бы 'харэ':\n{out}");
        assert_contains(&out, "const __ypsSwitch0 = х;");
        assert_contains(&out, "if (__ypsSwitch0 === 1) {");
        assert_contains(&out, "} else if (__ypsSwitch0 === 2) {");
        assert_contains(&out, "} else {");
    }

    #[test]
    fn switch_case_value_keeps_low_precedence_parens() {
        let out = js("базарпо (х) { тема а || б: { ф(1); } тема в ? 1 : 2: { ф(2); } }\n");
        assert_contains(&out, "if (__ypsSwitch0 === (а || б)) {");
        assert_contains(&out, "if (__ypsSwitch0 === (в ? 1 : 2)) {");
    }

    #[test]
    fn switch_break_inside_loop_targets_the_loop() {
        let out = js("го (гыы и = 0; и < 4; и++) { базарпо (и) { тема 2: { харэ; } нуичо { ф(и); } } }\n");
        assert!(!out.contains("switch ("), "нужен if/else, иначе break выйдет из switch:\n{out}");
        assert_contains(&out, "break;");
    }

    #[test]
    fn nested_switches_use_distinct_temporaries() {
        let out = js("базарпо (а) { тема 1: { базарпо (б) { тема 2: { ф(); } } } }\n");
        assert_contains(&out, "const __ypsSwitch0 = а;");
        assert_contains(&out, "const __ypsSwitch1 = б;");
    }

    #[test]
    fn typeof_helper_reports_classes_as_class() {
        let out = js("клёво К {}\nсказать(тип(К));\n");
        assert_contains(&out, r#"? "класс" : "функция""#);
    }

    #[test]
    fn date_used_as_a_namespace_is_a_diagnostic() {
        let err = js_err("сказать(Дата.сейчас());\n");
        assert_contains(&err.message, "'Дата'");
        assert_contains(&js_err("сказать(Дата[\"сейчас\"]());\n").message, "'Дата'");
        assert_contains(&js("гыы д = захуярить Дата();\nсказать(д.часы());\n"), "д.часы()");
        assert_contains(&js("гыы Дата = 5;\nсказать(Дата.х);\n"), "Дата.х");
    }

    #[test]
    fn try_catch_finally_and_throw() {
        let out = js("хапнуть { ф(); } гоп (е) { кидай е; } тюряжка { г(); }\n");
        assert_contains(&out, "try {");
        assert_contains(&out, "} catch (е) {");
        assert_contains(&out, "throw е;");
        assert_contains(&out, "} finally {");
    }

    #[test]
    fn generator_and_yield() {
        let out = js("пиздюли ф() { поебалу 1; поебалуна г(); }\n");
        assert_contains(&out, "function* ф() {");
        assert_contains(&out, "yield 1;");
        assert_contains(&out, "yield* г();");
    }

    #[test]
    fn async_await_and_arrow() {
        let out = js("ясенХуй ф = ассо () => { сидетьНахуй г(); };\n");
        assert_contains(&out, "const ф = async () => {");
        assert_contains(&out, "await г();");
    }

    #[test]
    fn class_with_all_member_kinds() {
        let src = "клёво Тачка батя Транспорт {\n  попонятия счёт = 0;\n  поле = 1;\n  #секрет = 2;\n  Тачка(а) { яга(); тырыпыры.а = а; }\n  метод() { отвечаю тырыпыры.поле; }\n  попонятия статМетод() { отвечаю 1; }\n  get размер() { отвечаю 1; }\n  set размер(з) { тырыпыры.поле = з; }\n  попонятия { Тачка.счёт = 5; }\n}\n";
        let out = js(src);
        assert_contains(&out, "class Тачка extends Транспорт {");
        assert_contains(&out, "static счёт = 0;");
        assert_contains(&out, "поле = 1;");
        assert_contains(&out, "#секрет = 2;");
        assert_contains(&out, "constructor(а) {");
        assert_contains(&out, "super();");
        assert_contains(&out, "this.а = а;");
        assert_contains(&out, "static статМетод() {");
        assert_contains(&out, "get размер() {");
        assert_contains(&out, "set размер(з) {");
        assert_contains(&out, "static {");
    }

    #[test]
    fn class_and_method_decorators() {
        let out = js("клёво К {\n  @лог\n  м() { отвечаю 1; }\n}\n");
        assert_contains(&out, "@лог");
        assert_contains(&out, "class К {");
    }

    #[test]
    fn destructuring_patterns() {
        let out = js("гыы [а, , б = 2, ...ост] = сп;\nгыы { ш = 1, ц: о = \"к\", ...прочее } = об;\n");
        assert_contains(&out, "let [а, , б = 2, ...ост] = сп;");
        assert_contains(&out, "let { ш = 1, ц: о = \"к\", ...прочее } = об;");
    }

    #[test]
    fn params_with_defaults_and_rest() {
        assert_contains(&js("йопта ф(а, б = 2, ...ост) { отвечаю а; }\n"), "function ф(а, б = 2, ...ост) {");
    }

    #[test]
    fn template_literal_and_tagged_template() {
        assert_contains(&js("гыы с = `а${х}б`;\n"), "let с = `а${х}б`;");
        assert_contains(&js("гыы с = тег`а${х}б`;\n"), "let с = тег`а${х}б`;");
    }

    #[test]
    fn optional_chaining_and_spread_and_new() {
        assert_contains(&js("гыы а = о?.п;\n"), "let а = о?.п;");
        assert_contains(&js("гыы а = о?.[к];\n"), "let а = о?.[к];");
        assert_contains(&js("гыы а = ф?.(1);\n"), "let а = ф?.(1);");
        assert_contains(&js("гыы а = [...б, 1];\n"), "let а = [...б, 1];");
        assert_contains(&js("гыы а = захуярить К(1);\n"), "let а = new К(1);");
    }

    #[test]
    fn literals_operators_and_conditional() {
        let out = js(
            "гыы а = правда;\nгыы б = лож;\nгыы в = ноль;\nгыы г = неибу;\nгыы д = 10n;\nгыы е = /аб/gi;\nгыы ж = { к: 1 };\nгыы з = чезажижан а;\nгыы и = а шкура К;\nгыы к = \"к\" из ж;\nгыы л = а ? 1 : 2;\nгыы м = а ?? 2;\nгыы н = 2 ** 3;\nгыы о = ёбнуть ж.к;\nгыы п = куку 0;\n",
        );
        assert_contains(&out, "let а = true;");
        assert_contains(&out, "let б = false;");
        assert_contains(&out, "let в = null;");
        assert_contains(&out, "let г = undefined;");
        assert_contains(&out, "let д = 10n;");
        assert_contains(&out, "let е = /аб/gi;");
        assert_contains(&out, "let ж = { к: 1 };");
        assert_contains(&out, "let з = typeof а;");
        assert_contains(&out, "let и = а instanceof К;");
        assert_contains(&out, "let к = \"к\" in ж;");
        assert_contains(&out, "let л = а ? 1 : 2;");
        assert_contains(&out, "let м = а ?? 2;");
        assert_contains(&out, "let н = 2 ** 3;");
        assert_contains(&out, "let о = delete ж.к;");
        assert_contains(&out, "let п = void 0;");
    }

    #[test]
    fn number_literal_raw_text_is_reused() {
        let out = js("гыы а = 0xFF;\nгыы б = 1e3;\nгыы в = 0b1010;\nгыы г = 0o17;\n");
        assert_contains(&out, "let а = 0xFF;");
        assert_contains(&out, "let б = 1e3;");
        assert_contains(&out, "let в = 0b1010;");
        assert_contains(&out, "let г = 0o17;");
    }

    #[test]
    fn using_declarations_and_debugger_and_empty() {
        assert_contains(&js("юзай р = ф();\n"), "using р = ф();");
        assert_contains(&js("ассо йопта ф() { юзай сидетьНахуй р = г(); }\n"), "await using р = г();");
        assert_contains(&js("логопед;\n"), "debugger;");
        assert_contains(&js("{ ф(); }\n"), "{\n  ф();\n}");
    }

    #[test]
    fn modules_import_export_and_dynamic_import() {
        assert_contains(&js("спиздить { а } из \"м\";\n"), "import { а } from \"м\";");
        assert_contains(&js("спиздить * как м из \"м\";\n"), "import * as м from \"м\";");
        assert_contains(&js("спиздить д из \"м\";\n"), "import д from \"м\";");
        assert_contains(&js("предъява йопта ф() { отвечаю 1; }\n"), "export function ф() {");
        assert_contains(&js("гыы а = 1;\nпредъява { а };\n"), "export { а };");
        assert_contains(&js("ассо йопта ф() { отвечаю сидетьНахуй спиздить(\"м\"); }\n"), "await import(\"м\")");
    }

    #[test]
    fn pipeline_is_desugared_to_a_call() {
        assert_contains(&js("гыы а = 5 |> ф;\n"), "let а = ф(5);");
        assert_contains(&js("гыы а = 5 |> ф(2);\n"), "let а = ф(5, 2);");
    }

    #[test]
    fn compound_assignment_operators() {
        let out = js(
            "х += 1;\nх -= 1;\nх *= 1;\nх /= 1;\nх %= 1;\nх **= 1;\nх &&= 1;\nх ||= 1;\nх ??= 1;\nх &= 1;\nх |= 1;\nх ^= 1;\nх <<= 1;\nх >>= 1;\nх >>>= 1;\n",
        );
        for op in ["+=", "-=", "*=", "/=", "%=", "**=", "&&=", "||=", "??=", "&=", "|=", "^=", "<<=", ">>=", ">>>="] {
            assert_contains(&out, &format!("х {op} 1"));
        }
    }

    #[test]
    fn nullish_mixed_with_logical_gets_parens() {
        assert_contains(&js("гыы а = б || в ?? г;\n"), "let а = б || (в ?? г);");
    }

    #[test]
    fn console_family_is_renamed() {
        let out = js(
            "сказать(1);\nсказать.ошибка(2);\nсказать.предупреждение(3);\nсказать.инфо(4);\nсказать.отладка(5);\nсказать.таблица(6);\nсказать.время(\"т\");\nсказать.времяСтоп(\"т\");\n",
        );
        assert_contains(&out, "console.log(1);");
        assert_contains(&out, "console.error(2);");
        assert_contains(&out, "console.warn(3);");
        assert_contains(&out, "console.info(4);");
        assert_contains(&out, "console.debug(5);");
        assert_contains(&out, "console.table(6);");
        assert_contains(&out, "console.time(\"т\");");
        assert_contains(&out, "console.timeEnd(\"т\");");
    }

    #[test]
    fn simple_builtin_renames() {
        let out = js(
            "гыы а = число(\"1\");\nгыы б = строка(1);\nгыы в = БигЦелое(1);\nгыы г = RegExp(\"а\");\nгыы д = Дата();\nгыы е = захуярить Дата();\nкидай захуярить Косяк(\"бэ\");\n",
        );
        assert_contains(&out, "let а = Number(\"1\");");
        assert_contains(&out, "let б = String(1);");
        assert_contains(&out, "let в = BigInt(1);");
        assert_contains(&out, "let г = RegExp(\"а\");");
        assert_contains(&out, "let д = new Date();");
        assert_contains(&out, "let е = new Date();");
        assert_contains(&out, "throw new Error(\"бэ\");");
    }

    #[test]
    fn timer_builtins_are_renamed() {
        let out = js(
            "гыы т = чутка(ф, 1);\nотменаЧутки(т);\nгыы и = интервал(ф, 1);\nотменаИнтервала(и);\nсразу(ф);\nнаСледующемТике(ф);\nсОчередить(ф);\n",
        );
        assert_contains(&out, "let т = setTimeout(ф, 1);");
        assert_contains(&out, "clearTimeout(т);");
        assert_contains(&out, "let и = setInterval(ф, 1);");
        assert_contains(&out, "clearInterval(и);");
        assert_contains(&out, "setImmediate(ф);");
        assert_contains(&out, "process.nextTick(ф);");
        assert_contains(&out, "queueMicrotask(ф);");
    }

    #[test]
    fn dlina_becomes_length_and_is_error_becomes_instanceof() {
        assert_contains(&js("гыы а = длина(с);\n"), "let а = (с).length;");
        assert_contains(&js("гыы а = этоКосяк(е);\n"), "let а = ((е) instanceof Error);");
    }

    #[test]
    fn prelude_helpers_are_emitted_only_when_used() {
        let plain = js("сказать(1);\n");
        assert!(!plain.contains("__yps"), "префикс не должен появляться:\n{plain}");

        let typed = js("сказать(тип(1));\n");
        assert!(typed.starts_with("function __ypsTypeof(v) {"), "пролог отсутствует:\n{typed}");
        assert_contains(&typed, "console.log(__ypsTypeof(1));");
        assert!(!typed.contains("__ypsPush"), "лишний хелпер:\n{typed}");

        let pushed = js("втолкнуть(а, 1);\n");
        assert_contains(&pushed, "function __ypsPush(arr, val) {");
        assert_contains(&pushed, "__ypsPush(а, 1);");

        let slept = js("ассо йопта ф() { сидетьНахуй подождать(10); }\n");
        assert_contains(&slept, "function __ypsSleep(ms) {");
        assert_contains(&slept, "await __ypsSleep(10);");

        let stdin = js("гыы с = прочестьСтроку();\nгыы в = прочестьВсё();\n");
        assert_contains(&stdin, "function __ypsStdin() {");
        assert_contains(&stdin, "let с = __ypsReadLine();");
        assert_contains(&stdin, "let в = __ypsReadAll();");
    }

    #[test]
    fn builtin_in_value_position_uses_helper() {
        let out = js("гыы ф = длина;\nгыы г = тип;\n");
        assert_contains(&out, "let ф = __ypsLen;");
        assert_contains(&out, "let г = __ypsTypeof;");
    }

    #[test]
    fn unsupported_stdlib_global_is_a_diagnostic() {
        let err = js_err("сказать(Матан.пи);\n");
        assert_contains(&err.message, "Матан");
        assert!(err.span.start > 0);

        let err = js_err("гыы м = захуярить Карта();\n");
        assert_contains(&err.message, "Карта");

        let err = js_err("гыы б = захуярить Ц8Массив(4);\n");
        assert_contains(&err.message, "Ц8Массив");
    }

    #[test]
    fn shadowed_reserved_name_does_not_trigger_diagnostic() {
        let out = js("гыы Помойка = 5;\nсказать(Помойка);\n");
        assert_contains(&out, "let Помойка = 5;");
        assert_contains(&out, "console.log(Помойка);");
    }

    #[test]
    fn shadowed_builtin_is_not_renamed() {
        let out = js("йопта длина(а) { отвечаю 1; }\nсказать(длина(2));\n");
        assert_contains(&out, "function длина(а) {");
        assert_contains(&out, "console.log(длина(2));");
    }
}
