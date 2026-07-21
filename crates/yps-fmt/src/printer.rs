use yps_parser::{
    ASSIGN_PRECEDENCE, CALL_PRECEDENCE, POSTFIX_PRECEDENCE, TERNARY_PRECEDENCE, UNARY_PRECEDENCE,
    binary_is_right_assoc, binary_precedence,
};
use yps_parser::{
    BinaryOp, Block, ClassMember, ExportKind, Expr, ImportSpec, Literal, ObjectEntry, ObjectPatternProp, Param,
    Pattern, PostfixOp, Program, PropKey, Stmt, SwitchCase, TemplatePart, TemplateQuasi, UnaryOp,
};

use crate::comments::CommentMap;
use crate::sourcemap::{SourceMap, SourceMapBuilder};

const INDENT: &str = "    ";
const MAX_WIDTH: usize = 100;

pub fn print_program(program: &Program) -> String {
    let mut p = Printer { out: String::new(), depth: 0, comments: None, sm: None, gen_line: 0 };
    p.print_program(program);
    p.out
}

pub fn print_program_with_comments(program: &Program, comments: &CommentMap) -> String {
    let mut p = Printer { out: String::new(), depth: 0, comments: Some(comments), sm: None, gen_line: 0 };
    p.print_program(program);
    p.out
}

pub fn print_program_with_map(program: &Program, comments: Option<&CommentMap>, source: &str) -> (String, SourceMap) {
    let mut p =
        Printer { out: String::new(), depth: 0, comments, sm: Some(SourceMapBuilder::new(source)), gen_line: 0 };
    p.print_program(program);
    let map = p.sm.unwrap().build("", "");
    (p.out, map)
}

struct Printer<'a> {
    out: String,
    depth: usize,
    comments: Option<&'a CommentMap>,
    sm: Option<SourceMapBuilder>,
    gen_line: u32,
}

impl Printer<'_> {
    fn write(&mut self, s: &str) {
        self.gen_line += s.bytes().filter(|&b| b == b'\n').count() as u32;
        self.out.push_str(s);
    }

    fn indent(&mut self) {
        for _ in 0..self.depth {
            self.out.push_str(INDENT);
        }
    }

    fn newline(&mut self) {
        self.gen_line += 1;
        self.out.push('\n');
    }

    fn current_col(&self) -> usize {
        match self.out.rfind('\n') {
            Some(idx) => self.out[idx + 1..].chars().count(),
            None => self.out.chars().count(),
        }
    }

    fn capture<F: FnOnce(&mut Self)>(&mut self, f: F) -> String {
        let saved = std::mem::take(&mut self.out);
        let saved_line = self.gen_line;
        f(self);
        self.gen_line = saved_line;
        std::mem::replace(&mut self.out, saved)
    }

    fn print_delimited<F>(&mut self, open: &str, close: &str, pad: bool, count: usize, mut print_item: F)
    where
        F: FnMut(&mut Self, usize),
    {
        if count == 0 {
            self.write(open);
            self.write(close);
            return;
        }

        let start_col = self.current_col() + open.chars().count();
        let inline = self.capture(|p| {
            for i in 0..count {
                if i > 0 {
                    p.write(", ");
                }
                print_item(p, i);
            }
        });

        let pad_str = if pad { " " } else { "" };
        let inline_width = start_col + pad_str.len() * 2 + inline.chars().count() + close.chars().count();
        if !inline.contains('\n') && inline_width <= MAX_WIDTH {
            self.write(open);
            self.write(pad_str);
            self.write(&inline);
            self.write(pad_str);
            self.write(close);
            return;
        }

        self.write(open);
        self.newline();
        self.depth += 1;
        for i in 0..count {
            self.indent();
            print_item(self, i);
            if i + 1 < count {
                self.write(",");
            }
            self.newline();
        }
        self.depth -= 1;
        self.indent();
        self.write(close);
    }

    fn record_mapping(&mut self, span_start: usize) {
        if let Some(sm) = &mut self.sm {
            let gen_col = match self.out.rfind('\n') {
                Some(idx) => self.out[idx + 1..].encode_utf16().count() as u32,
                None => self.out.encode_utf16().count() as u32,
            };
            sm.add_mapping(self.gen_line, gen_col, span_start);
        }
    }

    fn emit_leading(&mut self, stmt: &Stmt) {
        if let Some(comments) = self.comments
            && let Some(leading) = comments.leading(stmt.span().start)
        {
            let texts: Vec<String> = leading.to_vec();
            for text in texts {
                self.indent();
                self.write(&text);
                self.newline();
            }
        }
    }

    fn emit_trailing(&mut self, stmt: &Stmt) {
        if let Some(comments) = self.comments
            && let Some(trailing) = comments.trailing(stmt.span().start)
        {
            let text = trailing.to_string();
            self.write(" ");
            self.write(&text);
        }
    }

    fn print_stmt_line(&mut self, stmt: &Stmt) {
        self.emit_leading(stmt);
        self.indent();
        self.print_stmt(stmt);
        self.emit_trailing(stmt);
        self.newline();
    }

    fn print_program(&mut self, program: &Program) {
        let mut prev_top: Option<bool> = None;
        for stmt in &program.items {
            if matches!(stmt, Stmt::Empty { .. }) {
                continue;
            }
            let is_decl = is_top_level_decl(stmt);
            if let Some(prev_decl) = prev_top
                && (prev_decl || is_decl)
            {
                self.newline();
            }
            self.print_stmt_line(stmt);
            prev_top = Some(is_decl);
        }
        if let Some(comments) = self.comments {
            let eof: Vec<String> = comments.eof_trailing().to_vec();
            for text in eof {
                self.write(&text);
                self.newline();
            }
        }
        if self.out.is_empty() {
            self.out.push('\n');
        }
    }

    fn print_block(&mut self, block: &Block) {
        self.write("{");
        let stmts: Vec<_> = block.stmts.iter().filter(|s| !matches!(s, Stmt::Empty { .. })).collect();
        if stmts.is_empty() {
            self.write("}");
            return;
        }
        self.newline();
        self.depth += 1;
        for stmt in stmts {
            self.print_stmt_line(stmt);
        }
        self.depth -= 1;
        self.indent();
        self.write("}");
    }

    fn print_var_decl(&mut self, pattern: &Pattern, init: &Expr, is_const: bool) {
        self.write(if is_const { "ясенХуй" } else { "гыы" });
        self.write(" ");
        self.print_pattern(pattern);
        self.write(" = ");
        self.print_expr(init, 0);
        self.write(";");
    }

    fn print_static_prefix(&mut self, is_static: bool) {
        if is_static {
            self.write("попонятия ");
        }
    }

    fn print_stmt(&mut self, stmt: &Stmt) {
        self.record_mapping(stmt.span().start);
        match stmt {
            Stmt::VarDecl { pattern, init, is_const, .. } => {
                self.print_var_decl(pattern, init, *is_const);
            }
            Stmt::Using { name, init, is_await, .. } => {
                self.write("юзай ");
                if *is_await {
                    self.write("сидетьНахуй ");
                }
                self.write(&name.name);
                self.write(" = ");
                self.print_expr(init, 0);
                self.write(";");
            }
            Stmt::Expr { expr, .. } => {
                let needs_wrap = stmt_expr_needs_parens(expr);
                if needs_wrap {
                    self.write("(");
                }
                self.print_expr(expr, 0);
                if needs_wrap {
                    self.write(")");
                }
                self.write(";");
            }
            Stmt::Block(block) => {
                self.print_block(block);
            }
            Stmt::Empty { .. } => {
                self.write(";");
            }
            Stmt::If { condition, then_branch, else_branch, .. } => {
                self.write("вилкойвглаз (");
                self.print_expr(condition, 0);
                self.write(") ");
                self.print_branch(then_branch);
                if let Some(else_branch) = else_branch {
                    self.write(" иливжопураз ");
                    if matches!(else_branch.as_ref(), Stmt::If { .. }) {
                        self.print_stmt(else_branch);
                    } else {
                        self.print_branch(else_branch);
                    }
                }
            }
            Stmt::While { condition, body, .. } => {
                self.write("потрещим (");
                self.print_expr(condition, 0);
                self.write(") ");
                self.print_branch(body);
            }
            Stmt::DoWhile { body, condition, .. } => {
                self.write("крутани ");
                self.print_branch(body);
                self.write(" потрещим (");
                self.print_expr(condition, 0);
                self.write(");");
            }
            Stmt::For { init, condition, update, body, .. } => {
                self.write("го (");
                if let Some(init) = init {
                    self.print_for_init(init);
                } else {
                    self.write(";");
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
                self.write("го (гыы ");
                self.print_pattern(variable);
                self.write(" из ");
                self.print_expr(iterable, 0);
                self.write(") ");
                self.print_branch(body);
            }
            Stmt::ForOf { variable, iterable, body, .. } => {
                self.write("го (гыы ");
                self.print_pattern(variable);
                self.write(" сашаГрей ");
                self.print_expr(iterable, 0);
                self.write(") ");
                self.print_branch(body);
            }
            Stmt::ForAwaitOf { variable, iterable, body, .. } => {
                self.write("го сидетьНахуй (гыы ");
                self.print_pattern(variable);
                self.write(" сашаГрей ");
                self.print_expr(iterable, 0);
                self.write(") ");
                self.print_branch(body);
            }
            Stmt::Break { label, .. } => {
                self.write("харэ");
                if let Some(label) = label {
                    self.write(" ");
                    self.write(&label.name);
                }
                self.write(";");
            }
            Stmt::Continue { label, .. } => {
                self.write("двигай");
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
                if *is_async {
                    self.write("ассо ");
                }
                self.write(if *is_generator { "пиздюли" } else { "йопта" });
                self.write(" ");
                self.write(&name.name);
                self.print_params(params);
                self.write(" ");
                self.print_block(body);
            }
            Stmt::Return { value, .. } => {
                self.write("отвечаю");
                if let Some(value) = value {
                    self.write(" ");
                    self.print_expr(value, 0);
                }
                self.write(";");
            }
            Stmt::Throw { value, .. } => {
                self.write("кидай ");
                self.print_expr(value, 0);
                self.write(";");
            }
            Stmt::TryCatch { try_block, catch_param, catch_block, finally_block, .. } => {
                self.write("хапнуть ");
                self.print_block(try_block);
                if let Some(catch_block) = catch_block {
                    self.write(" гоп");
                    if let Some(param) = catch_param {
                        self.write(" (");
                        self.write(&param.name);
                        self.write(")");
                    }
                    self.write(" ");
                    self.print_block(catch_block);
                }
                if let Some(finally_block) = finally_block {
                    self.write(" тюряжка ");
                    self.print_block(finally_block);
                }
            }
            Stmt::Switch { expr, cases, default, .. } => {
                self.write("базарпо (");
                self.print_expr(expr, 0);
                self.write(") {");
                self.newline();
                self.depth += 1;
                for case in cases {
                    self.print_switch_case(case);
                }
                if let Some(default) = default {
                    self.indent();
                    self.write("нуичо ");
                    self.print_block(default);
                    self.newline();
                }
                self.depth -= 1;
                self.indent();
                self.write("}");
            }
            Stmt::ClassDecl { name, super_class, members, decorators, .. } => {
                for decorator in decorators {
                    self.write("@");
                    self.print_expr(decorator, UNARY_PRECEDENCE);
                    self.newline();
                    self.indent();
                }
                self.write("клёво ");
                self.write(&name.name);
                if let Some(super_class) = super_class {
                    self.write(" батя ");
                    self.print_expr(super_class, 0);
                }
                self.write(" {");
                if members.is_empty() {
                    self.write("}");
                    return;
                }
                self.newline();
                self.depth += 1;
                let mut first = true;
                for member in members {
                    if !first {
                        self.newline();
                    }
                    first = false;
                    self.print_class_member(member, &name.name);
                }
                self.depth -= 1;
                self.indent();
                self.write("}");
            }
            Stmt::Debugger { .. } => {
                self.write("логопед;");
            }
            Stmt::Import { specifiers, source, attributes, .. } => {
                self.write("спиздить ");
                self.print_import_specifiers(specifiers);
                self.write(" из ");
                self.write(&quote_string(source));
                if !attributes.is_empty() {
                    self.write(" with { ");
                    let mut first = true;
                    for (key, value) in attributes {
                        if !first {
                            self.write(", ");
                        }
                        first = false;
                        self.write(key);
                        self.write(": ");
                        self.write(&quote_string(value));
                    }
                    self.write(" }");
                }
                self.write(";");
            }
            Stmt::Export { kind, .. } => {
                self.write("предъява");
                match kind {
                    ExportKind::Named(names) => {
                        self.write(" { ");
                        let mut first = true;
                        for name in names {
                            if !first {
                                self.write(", ");
                            }
                            first = false;
                            self.write(&name.name);
                        }
                        self.write(" };");
                    }
                    ExportKind::Declaration(inner) => {
                        self.write(" ");
                        self.print_stmt(inner);
                    }
                }
            }
        }
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

    fn print_for_init(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::VarDecl { pattern, init, is_const, .. } => {
                self.print_var_decl(pattern, init, *is_const);
            }
            Stmt::Expr { expr, .. } => {
                self.print_expr(expr, 0);
                self.write(";");
            }
            _ => {
                self.print_stmt(stmt);
            }
        }
    }

    fn print_switch_case(&mut self, case: &SwitchCase) {
        self.indent();
        self.write("тема ");
        self.print_expr(&case.value, 0);
        self.write(": ");
        self.print_block(&case.body);
        self.newline();
    }

    fn print_class_member(&mut self, member: &ClassMember, class_name: &str) {
        match member {
            ClassMember::Constructor { params, body, .. } => {
                self.indent();
                self.write(class_name);
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
                self.write("попонятия ");
                self.print_block(body);
                self.newline();
            }
        }
    }

    fn print_member_decorators(&mut self, decorators: &[Expr]) {
        for decorator in decorators {
            self.indent();
            self.write("@");
            self.print_expr(decorator, UNARY_PRECEDENCE);
            self.newline();
        }
    }

    fn print_import_specifiers(&mut self, specifiers: &[ImportSpec]) {
        let all_named = !specifiers.is_empty() && specifiers.iter().all(|s| matches!(s, ImportSpec::Named { .. }));
        if all_named {
            self.write("{ ");
            let mut first = true;
            for spec in specifiers {
                if let ImportSpec::Named { imported, .. } = spec {
                    if !first {
                        self.write(", ");
                    }
                    first = false;
                    self.write(&imported.name);
                }
            }
            self.write(" }");
            return;
        }
        for spec in specifiers {
            match spec {
                ImportSpec::Default { local } => self.write(&local.name),
                ImportSpec::Namespace { local } => {
                    self.write("* как ");
                    self.write(&local.name);
                }
                ImportSpec::Named { imported, .. } => self.write(&imported.name),
            }
        }
    }

    fn print_params(&mut self, params: &[Param]) {
        self.print_delimited("(", ")", false, params.len(), |p, i| {
            p.print_param(&params[i]);
        });
    }

    fn print_param(&mut self, param: &Param) {
        if param.is_rest {
            self.write("...");
        }
        self.write(&param.name.name);
        if let Some(default) = &param.default {
            self.write(" = ");
            self.print_expr(default, 0);
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
                self.print_expr(default, 0);
            }
        }
    }

    fn print_object_pattern_prop(&mut self, prop: &ObjectPatternProp) {
        match &prop.value {
            None => self.write(&prop.key.name),
            Some(Pattern::Default { pattern, default, .. }) if is_ident_named(pattern, &prop.key.name) => {
                self.write(&prop.key.name);
                self.write(" = ");
                self.print_expr(default, 0);
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
            Expr::Identifier(id) => self.write(&id.name),
            Expr::Literal(literal) => self.print_literal(literal),
            Expr::This { .. } => self.write("тырыпыры"),
            Expr::Super { .. } => self.write("яга"),
            Expr::Unary { op, expr: operand, .. } => {
                let wrap = UNARY_PRECEDENCE < parent_prec;
                if wrap {
                    self.write("(");
                }
                self.write(unary_op_str(*op));
                if unary_op_needs_space(*op) {
                    self.write(" ");
                }
                self.print_expr(operand, UNARY_PRECEDENCE);
                if wrap {
                    self.write(")");
                }
            }
            Expr::Postfix { op, expr: operand, .. } => {
                let wrap = POSTFIX_PRECEDENCE < parent_prec;
                if wrap {
                    self.write("(");
                }
                self.print_expr(operand, POSTFIX_PRECEDENCE);
                self.write(postfix_op_str(*op));
                if wrap {
                    self.write(")");
                }
            }
            Expr::Binary { op, lhs, rhs, .. } => {
                let prec = binary_precedence(*op);
                let right_assoc = binary_is_right_assoc(*op);
                let wrap = prec < parent_prec;
                if wrap {
                    self.write("(");
                }
                let left_prec = if right_assoc { prec + 1 } else { prec };
                let right_prec = if right_assoc { prec } else { prec + 1 };
                self.print_expr(lhs, left_prec);
                self.write(" ");
                self.write(binary_op_str(*op));
                self.write(" ");
                self.print_expr(rhs, right_prec);
                if wrap {
                    self.write(")");
                }
            }
            Expr::Assignment { target, value, .. } => {
                let wrap = ASSIGN_PRECEDENCE < parent_prec;
                if wrap {
                    self.write("(");
                }
                self.write(&target.name);
                self.write(" = ");
                self.print_expr(value, 0);
                if wrap {
                    self.write(")");
                }
            }
            Expr::Conditional { condition, then_expr, else_expr, .. } => {
                let wrap = TERNARY_PRECEDENCE < parent_prec;
                if wrap {
                    self.write("(");
                }
                self.print_expr(condition, TERNARY_PRECEDENCE + 1);
                self.write(" ? ");
                self.print_expr(then_expr, 0);
                self.write(" : ");
                self.print_expr(else_expr, TERNARY_PRECEDENCE);
                if wrap {
                    self.write(")");
                }
            }
            Expr::Call { callee, args, .. } => {
                self.print_expr(callee, CALL_PRECEDENCE);
                self.print_args(args);
            }
            Expr::OptionalCall { callee, args, .. } => {
                self.print_expr(callee, CALL_PRECEDENCE);
                self.write("?.");
                self.print_args(args);
            }
            Expr::New { callee, args, .. } => {
                let wrap = CALL_PRECEDENCE < parent_prec;
                if wrap {
                    self.write("(");
                }
                self.write("захуярить ");
                self.print_expr(callee, CALL_PRECEDENCE);
                self.print_args(args);
                if wrap {
                    self.write(")");
                }
            }
            Expr::Index { object, index, .. } => {
                self.print_expr(object, CALL_PRECEDENCE);
                self.write("[");
                self.print_expr(index, 0);
                self.write("]");
            }
            Expr::OptionalIndex { object, index, .. } => {
                self.print_expr(object, CALL_PRECEDENCE);
                self.write("?.[");
                self.print_expr(index, 0);
                self.write("]");
            }
            Expr::Member { object, property, .. } => {
                self.print_expr(object, CALL_PRECEDENCE);
                self.write(".");
                self.write(&property.name);
            }
            Expr::OptionalMember { object, property, .. } => {
                self.print_expr(object, CALL_PRECEDENCE);
                self.write("?.");
                self.write(&property.name);
            }
            Expr::ArrowFunction { params, body, is_async, .. } => {
                let wrap = parent_prec > ASSIGN_PRECEDENCE;
                if wrap {
                    self.write("(");
                }
                if *is_async {
                    self.write("ассо ");
                }
                self.print_params(params);
                self.write(" => ");
                self.print_arrow_body(body);
                if wrap {
                    self.write(")");
                }
            }
            Expr::FunctionExpr { name, params, body, is_generator, is_async, .. } => {
                if *is_async {
                    self.write("ассо ");
                }
                self.write(if *is_generator { "пиздюли" } else { "йопта" });
                if let Some(name) = name {
                    self.write(" ");
                    self.write(&name.name);
                }
                self.print_params(params);
                self.write(" ");
                self.print_block(body);
            }
            Expr::Spread { expr, .. } => {
                self.write("...");
                self.print_expr(expr, 0);
            }
            Expr::Yield { argument, delegate, .. } => {
                let wrap = ASSIGN_PRECEDENCE < parent_prec;
                if wrap {
                    self.write("(");
                }
                self.write(if *delegate { "поебалуна" } else { "поебалу" });
                if let Some(argument) = argument {
                    self.write(" ");
                    self.print_expr(argument, 0);
                }
                if wrap {
                    self.write(")");
                }
            }
            Expr::Await { argument, .. } => {
                let wrap = UNARY_PRECEDENCE < parent_prec;
                if wrap {
                    self.write("(");
                }
                self.write("сидетьНахуй ");
                self.print_expr(argument, UNARY_PRECEDENCE);
                if wrap {
                    self.write(")");
                }
            }
            Expr::DynamicImport { source, .. } => {
                self.write("спиздить(");
                self.print_expr(source, 0);
                self.write(")");
            }
            Expr::TemplateLiteral { parts, .. } => {
                self.print_template_literal(parts);
            }
            Expr::TaggedTemplate { tag, quasis, expressions, .. } => {
                self.print_expr(tag, CALL_PRECEDENCE);
                self.print_tagged_template(quasis, expressions);
            }
        }
    }

    fn print_args(&mut self, args: &[Expr]) {
        self.print_delimited("(", ")", false, args.len(), |p, i| {
            p.print_expr(&args[i], 0);
        });
    }

    fn print_arrow_body(&mut self, body: &Block) {
        if let [Stmt::Return { value: Some(value), .. }] = body.stmts.as_slice() {
            let needs_parens = matches!(value, Expr::Literal(Literal::Object { .. }));
            if needs_parens {
                self.write("(");
            }
            self.print_expr(value, 0);
            if needs_parens {
                self.write(")");
            }
        } else {
            self.print_block(body);
        }
    }

    fn print_literal(&mut self, literal: &Literal) {
        match literal {
            Literal::Number { raw, .. } => self.write(raw),
            Literal::BigInt { value, .. } => self.write(&format!("{value}n")),
            Literal::String { value, .. } => self.write(&quote_string(value)),
            Literal::Boolean { value, .. } => self.write(if *value { "правда" } else { "лож" }),
            Literal::Null { .. } => self.write("ноль"),
            Literal::Undefined { .. } => self.write("неибу"),
            Literal::RegExp { pattern, flags, .. } => {
                self.write("/");
                self.write(pattern);
                self.write("/");
                self.write(flags);
            }
            Literal::Array { elements, .. } => {
                self.print_delimited("[", "]", false, elements.len(), |p, i| {
                    p.print_expr(&elements[i], 0);
                });
            }
            Literal::Object { entries, .. } => {
                self.print_delimited("{", "}", true, entries.len(), |p, i| {
                    p.print_object_entry(&entries[i]);
                });
            }
        }
    }

    fn print_object_entry(&mut self, entry: &ObjectEntry) {
        match entry {
            ObjectEntry::Property { key, value } => {
                if let (PropKey::Identifier(id), Expr::Identifier(v)) = (key, value)
                    && id.name == v.name
                {
                    self.write(&id.name);
                    return;
                }
                self.print_prop_key(key);
                self.write(": ");
                self.print_expr(value, 0);
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
                TemplatePart::Str(s) => self.write(&escape_template(s)),
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

fn is_top_level_decl(stmt: &Stmt) -> bool {
    matches!(stmt, Stmt::FunctionDecl { .. } | Stmt::ClassDecl { .. })
}

fn is_ident_named(pattern: &Pattern, name: &str) -> bool {
    matches!(pattern, Pattern::Identifier(id) if id.name == name)
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

fn unary_op_str(op: UnaryOp) -> &'static str {
    match op {
        UnaryOp::Plus => "+",
        UnaryOp::Minus => "-",
        UnaryOp::Not => "!",
        UnaryOp::BitwiseNot => "~",
        UnaryOp::Typeof => "чезажижан",
        UnaryOp::Delete => "ёбнуть",
        UnaryOp::Void => "куку",
    }
}

fn unary_op_needs_space(op: UnaryOp) -> bool {
    matches!(op, UnaryOp::Typeof | UnaryOp::Delete | UnaryOp::Void)
}

fn postfix_op_str(op: PostfixOp) -> &'static str {
    match op {
        PostfixOp::Increment => "++",
        PostfixOp::Decrement => "--",
    }
}

fn binary_op_str(op: BinaryOp) -> &'static str {
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
        BinaryOp::Instanceof => "шкура",
        BinaryOp::In => "из",
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

fn quote_string(value: &str) -> String {
    let has_double = value.contains('"');
    let has_single = value.contains('\'');
    let use_single = has_double && !has_single;
    let quote = if use_single { '\'' } else { '"' };
    let mut out = String::with_capacity(value.len() + 2);
    out.push(quote);
    for ch in value.chars() {
        match ch {
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            '\0' => out.push_str("\\0"),
            '\\' => out.push_str("\\\\"),
            '\'' if quote == '\'' => out.push_str("\\'"),
            '"' if quote == '"' => out.push_str("\\\""),
            other => out.push(other),
        }
    }
    out.push(quote);
    out
}

fn escape_template(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '`' => out.push_str("\\`"),
            '$' => out.push_str("\\$"),
            '\r' => out.push_str("\\r"),
            '\0' => out.push_str("\\0"),
            other => out.push(other),
        }
    }
    out
}

#[cfg(test)]
mod style_tests {
    use yps_lexer::{Lexer, SourceFile};
    use yps_parser::Parser;

    use super::print_program;

    fn fmt(source: &str) -> String {
        let sf = SourceFile::new("<t>".to_string(), source.to_string());
        let (tokens, diags) = Lexer::new(&sf).tokenize();
        assert!(diags.is_empty(), "лексер: {diags:?}");
        let (program, pdiags) = Parser::new(&tokens, &sf).parse_program();
        assert!(pdiags.is_empty(), "парсер: {pdiags:?}");
        print_program(&program)
    }

    #[test]
    fn short_array_stays_inline() {
        assert_eq!(fmt("гыы а = [1, 2, 3];\n"), "гыы а = [1, 2, 3];\n");
    }

    #[test]
    fn long_array_breaks_multiline_without_trailing_comma() {
        let src = "гыы а = [элементПервый, элементВторой, элементТретий, элементЧетвёртый, элементПятый, элементШестой, элементСедьмой, элементВосьмой];\n";
        let out = fmt(src);
        assert!(out.contains("[\n    элементПервый,\n"), "вывод:\n{out}");
        assert!(out.contains("элементВосьмой\n];"), "вывод:\n{out}");
        assert!(!out.contains(",\n]"), "не должно быть trailing comma:\n{out}");
    }

    #[test]
    fn short_object_stays_inline() {
        assert_eq!(fmt("гыы о = { а: 1, б: 2 };\n"), "гыы о = { а: 1, б: 2 };\n");
    }

    #[test]
    fn switch_uses_block_bodies_and_default_without_colon() {
        let src = "базарпо (х) { тема 1: { сказать(1); } нуичо { сказать(2); } }\n";
        let out = fmt(src);
        assert!(out.contains("тема 1: {"));
        assert!(out.contains("нуичо {"));
        assert!(!out.contains("нуичо:"));
    }

    #[test]
    fn do_while_roundtrips() {
        let src = "крутани { и++; } потрещим (и < 3);\n";
        let out = fmt(src);
        assert!(out.starts_with("крутани {"));
        assert!(out.contains("} потрещим (и < 3);"));
    }

    #[test]
    fn arrow_argument_has_no_redundant_parens() {
        let out = fmt("ф(() => 1);\n");
        assert_eq!(out, "ф(() => 1);\n");
    }
}
