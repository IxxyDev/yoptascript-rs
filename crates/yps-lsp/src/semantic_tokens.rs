use std::collections::HashMap;

use tower_lsp::lsp_types::{SemanticToken, SemanticTokenModifier, SemanticTokenType, SemanticTokensLegend};
use yps_lexer::{Lexer, PunctuationKind, SourceFile, Span, Token, TokenKind};
use yps_parser::Parser;
use yps_parser::ast::{ClassMember, Expr, Literal, ObjectEntry, Param, Pattern, Program, PropKey, Stmt, TemplatePart};

use crate::position::byte_to_pos;

const FUNCTION: u32 = 0;
const VARIABLE: u32 = 1;
const PARAMETER: u32 = 2;
const PROPERTY: u32 = 3;
const STRING: u32 = 4;
const NUMBER: u32 = 5;
const KEYWORD: u32 = 6;
const COMMENT: u32 = 7;
const OPERATOR: u32 = 8;

const DECLARATION: u32 = 1 << 0;
const READONLY: u32 = 1 << 1;

#[must_use]
pub fn legend() -> SemanticTokensLegend {
    SemanticTokensLegend {
        token_types: vec![
            SemanticTokenType::FUNCTION,
            SemanticTokenType::VARIABLE,
            SemanticTokenType::PARAMETER,
            SemanticTokenType::PROPERTY,
            SemanticTokenType::STRING,
            SemanticTokenType::NUMBER,
            SemanticTokenType::KEYWORD,
            SemanticTokenType::COMMENT,
            SemanticTokenType::OPERATOR,
        ],
        token_modifiers: vec![SemanticTokenModifier::DECLARATION, SemanticTokenModifier::READONLY],
    }
}

type ClassifiedMap = HashMap<(usize, usize), (u32, u32)>;

fn insert(map: &mut ClassifiedMap, span: Span, ty: u32, mods: u32) {
    map.insert((span.start, span.end), (ty, mods));
}

fn walk_pattern(pattern: &Pattern, map: &mut ClassifiedMap, ty: u32, mods: u32) {
    match pattern {
        Pattern::Identifier(ident) => insert(map, ident.span, ty, mods),
        Pattern::Array { elements, rest, .. } => {
            for el in elements.iter().flatten() {
                walk_pattern(el, map, ty, mods);
            }
            if let Some(rest) = rest {
                walk_pattern(rest, map, ty, mods);
            }
        }
        Pattern::Object { properties, rest, .. } => {
            for prop in properties {
                match &prop.value {
                    Some(value) => walk_pattern(value, map, ty, mods),
                    None => insert(map, prop.key.span, ty, mods),
                }
            }
            if let Some(rest) = rest {
                walk_pattern(rest, map, ty, mods);
            }
        }
        Pattern::Default { pattern, default, .. } => {
            walk_pattern(pattern, map, ty, mods);
            walk_expr(default, map);
        }
    }
}

fn walk_params(params: &[Param], map: &mut ClassifiedMap) {
    for param in params {
        match &param.pattern {
            Some(pattern) => walk_pattern(pattern, map, PARAMETER, DECLARATION),
            None => insert(map, param.name.span, PARAMETER, DECLARATION),
        }
        if let Some(default) = &param.default {
            walk_expr(default, map);
        }
    }
}

fn walk_prop_key(key: &PropKey, map: &mut ClassifiedMap) {
    match key {
        PropKey::Identifier(ident) => insert(map, ident.span, PROPERTY, 0),
        PropKey::Computed(expr) => walk_expr(expr, map),
    }
}

fn walk_literal(lit: &Literal, map: &mut ClassifiedMap) {
    match lit {
        Literal::Array { elements, .. } => {
            for el in elements {
                walk_expr(el, map);
            }
        }
        Literal::Object { entries, .. } => {
            for entry in entries {
                match entry {
                    ObjectEntry::Property { key, value } => {
                        walk_prop_key(key, map);
                        walk_expr(value, map);
                    }
                    ObjectEntry::Spread(expr) => walk_expr(expr, map),
                    ObjectEntry::Getter { key, body, .. } => {
                        walk_prop_key(key, map);
                        walk_stmts(&body.stmts, map);
                    }
                    ObjectEntry::Setter { key, param, body, .. } => {
                        walk_prop_key(key, map);
                        walk_params(std::slice::from_ref(param), map);
                        walk_stmts(&body.stmts, map);
                    }
                }
            }
        }
        Literal::Number { .. }
        | Literal::BigInt { .. }
        | Literal::String { .. }
        | Literal::Boolean { .. }
        | Literal::Null { .. }
        | Literal::Undefined { .. }
        | Literal::RegExp { .. } => {}
    }
}

fn walk_expr(expr: &Expr, map: &mut ClassifiedMap) {
    match expr {
        Expr::Identifier(ident) => {
            map.entry((ident.span.start, ident.span.end)).or_insert((VARIABLE, 0));
        }
        Expr::This { .. } | Expr::Super { .. } => {}
        Expr::Literal(lit) => walk_literal(lit, map),
        Expr::Unary { expr, .. }
        | Expr::Postfix { expr, .. }
        | Expr::Grouping { expr, .. }
        | Expr::Spread { expr, .. }
        | Expr::Await { argument: expr, .. } => walk_expr(expr, map),
        Expr::Binary { lhs, rhs, .. } => {
            walk_expr(lhs, map);
            walk_expr(rhs, map);
        }
        Expr::Assignment { target, value, .. } => {
            insert(map, target.span, VARIABLE, 0);
            walk_expr(value, map);
        }
        Expr::Call { callee, args, .. } | Expr::OptionalCall { callee, args, .. } | Expr::New { callee, args, .. } => {
            if let Expr::Identifier(ident) = callee.as_ref() {
                insert(map, ident.span, FUNCTION, 0);
            } else {
                walk_expr(callee, map);
            }
            for arg in args {
                walk_expr(arg, map);
            }
        }
        Expr::Index { object, index, .. } | Expr::OptionalIndex { object, index, .. } => {
            walk_expr(object, map);
            walk_expr(index, map);
        }
        Expr::Member { object, property, .. } | Expr::OptionalMember { object, property, .. } => {
            walk_expr(object, map);
            insert(map, property.span, PROPERTY, 0);
        }
        Expr::Conditional { condition, then_expr, else_expr, .. } => {
            walk_expr(condition, map);
            walk_expr(then_expr, map);
            walk_expr(else_expr, map);
        }
        Expr::ArrowFunction { params, body, .. } => {
            walk_params(params, map);
            walk_stmts(&body.stmts, map);
        }
        Expr::FunctionExpr { name, params, body, .. } => {
            if let Some(name) = name {
                insert(map, name.span, FUNCTION, DECLARATION);
            }
            walk_params(params, map);
            walk_stmts(&body.stmts, map);
        }
        Expr::TemplateLiteral { parts, .. } => {
            for part in parts {
                if let TemplatePart::Expr(e) = part {
                    walk_expr(e, map);
                }
            }
        }
        Expr::TaggedTemplate { tag, expressions, .. } => {
            walk_expr(tag, map);
            for e in expressions {
                walk_expr(e, map);
            }
        }
        Expr::Yield { argument, .. } => {
            if let Some(argument) = argument {
                walk_expr(argument, map);
            }
        }
        Expr::DynamicImport { source, .. } => walk_expr(source, map),
    }
}

fn walk_members(members: &[ClassMember], map: &mut ClassifiedMap) {
    for member in members {
        match member {
            ClassMember::Constructor { params, body, .. } => {
                walk_params(params, map);
                walk_stmts(&body.stmts, map);
            }
            ClassMember::Method { name, params, body, decorators, .. } => {
                insert(map, name.span, FUNCTION, DECLARATION);
                for d in decorators {
                    walk_expr(d, map);
                }
                walk_params(params, map);
                walk_stmts(&body.stmts, map);
            }
            ClassMember::Field { name, init, decorators, .. } => {
                insert(map, name.span, PROPERTY, DECLARATION);
                for d in decorators {
                    walk_expr(d, map);
                }
                if let Some(init) = init {
                    walk_expr(init, map);
                }
            }
            ClassMember::Getter { name, body, decorators, .. } => {
                insert(map, name.span, PROPERTY, DECLARATION);
                for d in decorators {
                    walk_expr(d, map);
                }
                walk_stmts(&body.stmts, map);
            }
            ClassMember::Setter { name, param, body, decorators, .. } => {
                insert(map, name.span, PROPERTY, DECLARATION);
                for d in decorators {
                    walk_expr(d, map);
                }
                walk_params(std::slice::from_ref(param), map);
                walk_stmts(&body.stmts, map);
            }
            ClassMember::StaticBlock { body, .. } => walk_stmts(&body.stmts, map),
        }
    }
}

fn walk_stmts(stmts: &[Stmt], map: &mut ClassifiedMap) {
    for stmt in stmts {
        walk_stmt(stmt, map);
    }
}

fn walk_stmt(stmt: &Stmt, map: &mut ClassifiedMap) {
    match stmt {
        Stmt::VarDecl { pattern, init, is_const, .. } => {
            let mods = if *is_const { DECLARATION | READONLY } else { DECLARATION };
            walk_pattern(pattern, map, VARIABLE, mods);
            walk_expr(init, map);
        }
        Stmt::Using { name, init, .. } => {
            insert(map, name.span, VARIABLE, DECLARATION);
            walk_expr(init, map);
        }
        Stmt::Expr { expr, .. } | Stmt::Throw { value: expr, .. } => walk_expr(expr, map),
        Stmt::Block(block) => walk_stmts(&block.stmts, map),
        Stmt::Empty { .. } | Stmt::Break { .. } | Stmt::Continue { .. } | Stmt::Debugger { .. } => {}
        Stmt::If { condition, then_branch, else_branch, .. } => {
            walk_expr(condition, map);
            walk_stmt(then_branch, map);
            if let Some(else_branch) = else_branch {
                walk_stmt(else_branch, map);
            }
        }
        Stmt::While { condition, body, .. } | Stmt::DoWhile { condition, body, .. } => {
            walk_expr(condition, map);
            walk_stmt(body, map);
        }
        Stmt::For { init, condition, update, body, .. } => {
            if let Some(init) = init {
                walk_stmt(init, map);
            }
            if let Some(condition) = condition {
                walk_expr(condition, map);
            }
            if let Some(update) = update {
                walk_expr(update, map);
            }
            walk_stmt(body, map);
        }
        Stmt::ForIn { variable, iterable, body, .. }
        | Stmt::ForOf { variable, iterable, body, .. }
        | Stmt::ForAwaitOf { variable, iterable, body, .. } => {
            walk_expr(iterable, map);
            walk_pattern(variable, map, VARIABLE, DECLARATION);
            walk_stmt(body, map);
        }
        Stmt::Labeled { body, .. } => walk_stmt(body, map),
        Stmt::FunctionDecl { name, params, body, .. } => {
            insert(map, name.span, FUNCTION, DECLARATION);
            walk_params(params, map);
            walk_stmts(&body.stmts, map);
        }
        Stmt::Return { value, .. } => {
            if let Some(value) = value {
                walk_expr(value, map);
            }
        }
        Stmt::TryCatch { try_block, catch_param, catch_block, finally_block, .. } => {
            walk_stmts(&try_block.stmts, map);
            if let Some(catch_param) = catch_param {
                insert(map, catch_param.span, VARIABLE, DECLARATION);
            }
            if let Some(catch_block) = catch_block {
                walk_stmts(&catch_block.stmts, map);
            }
            if let Some(finally_block) = finally_block {
                walk_stmts(&finally_block.stmts, map);
            }
        }
        Stmt::Switch { expr, cases, default, .. } => {
            walk_expr(expr, map);
            for case in cases {
                walk_expr(&case.value, map);
                walk_stmts(&case.body.stmts, map);
            }
            if let Some(default) = default {
                walk_stmts(&default.stmts, map);
            }
        }
        Stmt::ClassDecl { name, super_class, members, decorators, .. } => {
            insert(map, name.span, VARIABLE, DECLARATION);
            if let Some(super_class) = super_class {
                walk_expr(super_class, map);
            }
            for d in decorators {
                walk_expr(d, map);
            }
            walk_members(members, map);
        }
        Stmt::Import { specifiers, .. } => {
            for spec in specifiers {
                let local = match spec {
                    yps_parser::ast::ImportSpec::Default { local }
                    | yps_parser::ast::ImportSpec::Named { local, .. }
                    | yps_parser::ast::ImportSpec::Namespace { local } => local,
                };
                insert(map, local.span, VARIABLE, DECLARATION);
            }
        }
        Stmt::Export { kind, .. } => match kind {
            yps_parser::ast::ExportKind::Declaration(inner) => walk_stmt(inner, map),
            yps_parser::ast::ExportKind::Named(idents) => {
                for ident in idents {
                    map.entry((ident.span.start, ident.span.end)).or_insert((VARIABLE, 0));
                }
            }
        },
    }
}

fn classify_program(program: &Program) -> ClassifiedMap {
    let mut map = HashMap::new();
    walk_stmts(&program.items, &mut map);
    map
}

fn utf16_len(s: &str) -> u32 {
    u32::try_from(s.chars().map(char::len_utf16).sum::<usize>()).unwrap_or(u32::MAX)
}

fn lexical_token_type(tok: &Token) -> Option<u32> {
    match &tok.kind {
        TokenKind::Keyword(_) => Some(KEYWORD),
        TokenKind::Number => Some(NUMBER),
        TokenKind::StringLiteral
        | TokenKind::TemplateNoSub
        | TokenKind::TemplateHead
        | TokenKind::TemplateMiddle
        | TokenKind::TemplateTail
        | TokenKind::RegexLiteral => Some(STRING),
        TokenKind::Operator(_) => Some(OPERATOR),
        TokenKind::Punctuation(PunctuationKind::Arrow) => Some(OPERATOR),
        _ => None,
    }
}

#[must_use]
pub fn semantic_tokens_full(text: &str) -> Vec<SemanticToken> {
    let sf = SourceFile::new("inline".to_string(), text.to_string());
    let (tokens, trivia, _) = Lexer::new(&sf).tokenize_with_trivia();
    let (program, _) = Parser::new(&tokens, &sf).parse_program();
    let classified = classify_program(&program);

    let mut raw: Vec<(Span, u32, u32)> = Vec::new();

    for tok in &tokens {
        if tok.kind == TokenKind::Identifier {
            let (ty, mods) = classified.get(&(tok.span.start, tok.span.end)).copied().unwrap_or((VARIABLE, 0));
            raw.push((tok.span, ty, mods));
        } else if let Some(ty) = lexical_token_type(tok) {
            raw.push((tok.span, ty, 0));
        }
    }

    for t in &trivia {
        raw.push((t.span, COMMENT, 0));
    }

    raw.sort_by_key(|(span, ..)| span.start);

    let mut result = Vec::with_capacity(raw.len());
    let mut prev_line = 0u32;
    let mut prev_char = 0u32;
    for (span, ty, mods) in raw {
        let pos = byte_to_pos(text, span.start);
        let length = utf16_len(&text[span.start..span.end]);
        let delta_line = pos.line - prev_line;
        let delta_start = if delta_line == 0 { pos.character.saturating_sub(prev_char) } else { pos.character };
        result.push(SemanticToken { delta_line, delta_start, length, token_type: ty, token_modifiers_bitset: mods });
        prev_line = pos.line;
        prev_char = pos.character;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode(tokens: &[SemanticToken]) -> Vec<(u32, u32, u32, u32, u32)> {
        tokens.iter().map(|t| (t.delta_line, t.delta_start, t.length, t.token_type, t.token_modifiers_bitset)).collect()
    }

    #[test]
    fn legend_matches_minimum_required_types() {
        let l = legend();
        assert_eq!(l.token_types.len(), 9);
        assert!(l.token_modifiers.contains(&SemanticTokenModifier::DECLARATION));
        assert!(l.token_modifiers.contains(&SemanticTokenModifier::READONLY));
    }

    #[test]
    fn classifies_variable_declaration() {
        let src = "гыы x = 1;";
        let tokens = semantic_tokens_full(src);
        assert!(tokens.iter().any(|t| t.token_type == KEYWORD));
        assert!(tokens.iter().any(|t| t.token_type == VARIABLE && t.token_modifiers_bitset & DECLARATION != 0));
        assert!(tokens.iter().any(|t| t.token_type == NUMBER));
    }

    #[test]
    fn const_declaration_is_readonly() {
        let src = "ясенХуй x = 1;";
        let tokens = semantic_tokens_full(src);
        let decl = tokens.iter().find(|t| t.token_type == VARIABLE).expect("variable token");
        assert_eq!(decl.token_modifiers_bitset, DECLARATION | READONLY);
    }

    #[test]
    fn classifies_function_declaration_and_string() {
        let src = "йопта фу() { отвечаю \"привет\"; }";
        let tokens = semantic_tokens_full(src);
        assert!(tokens.iter().any(|t| t.token_type == FUNCTION && t.token_modifiers_bitset & DECLARATION != 0));
        assert!(tokens.iter().any(|t| t.token_type == STRING));
    }

    #[test]
    fn classifies_comment() {
        let src = "// коммент\nгыы x = 1;";
        let tokens = semantic_tokens_full(src);
        assert!(tokens.iter().any(|t| t.token_type == COMMENT));
    }

    #[test]
    fn full_snippet_delta_encoding_first_and_subsequent_lines() {
        let src = "гыы x = 1;\nясенХуй y = 2;\nйопта фу() {}\n// коммент\nсказать(\"текст\");";
        let tokens = semantic_tokens_full(src);
        assert!(!tokens.is_empty());
        let first = tokens[0];
        assert_eq!(first.delta_line, 0);
        let total_lines: u32 = tokens.iter().map(|t| t.delta_line).sum();
        assert_eq!(total_lines, 4);
        let decoded = decode(&tokens);
        assert!(decoded.iter().any(|(_, _, _, ty, _)| *ty == COMMENT));
    }

    #[test]
    fn multibyte_identifiers_use_utf16_length() {
        let src = "гыы привет = 1;";
        let tokens = semantic_tokens_full(src);
        let ident = tokens.iter().find(|t| t.token_type == VARIABLE).expect("variable token");
        assert_eq!(ident.length, "привет".encode_utf16().count() as u32);
    }
}
