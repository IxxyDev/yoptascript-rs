use tower_lsp::lsp_types::{
    Documentation, MarkupContent, MarkupKind, ParameterInformation, ParameterLabel, SignatureHelp, SignatureInformation,
};
use yps_lexer::{Lexer, PunctuationKind, SourceFile, Token, TokenKind};
use yps_parser::Parser;
use yps_parser::ast::{ClassMember, ExportKind, Expr, Param, Pattern, Stmt};

use crate::builtins::builtin_doc;

struct BuiltinSig {
    name: &'static str,
    params: &'static [&'static str],
}

const BUILTIN_SIGNATURES: &[BuiltinSig] = &[
    BuiltinSig { name: "сказать", params: &["...аргументы"] },
    BuiltinSig { name: "длина", params: &["значение"] },
    BuiltinSig { name: "тип", params: &["значение"] },
    BuiltinSig { name: "число", params: &["значение"] },
    BuiltinSig { name: "строка", params: &["значение"] },
    BuiltinSig { name: "втолкнуть", params: &["массив", "значение"] },
    BuiltinSig { name: "БигЦелое", params: &["значение"] },
    BuiltinSig { name: "Косяк", params: &["сообщение"] },
    BuiltinSig { name: "чутка", params: &["колбэк", "мс"] },
    BuiltinSig { name: "интервал", params: &["колбэк", "мс"] },
    BuiltinSig { name: "подождать", params: &["мс"] },
];

enum Frame {
    Paren { callee: Option<String>, commas: u32 },
    Bracket,
    Brace,
}

fn active_call(tokens: &[Token], text: &str, byte_pos: usize) -> Option<(u32, String)> {
    let mut stack: Vec<Frame> = Vec::new();
    let mut prev_ident: Option<&str> = None;

    for tok in tokens {
        if tok.span.start >= byte_pos {
            break;
        }
        match &tok.kind {
            TokenKind::Punctuation(PunctuationKind::LParen) => {
                let callee = prev_ident.map(str::to_string);
                stack.push(Frame::Paren { callee, commas: 0 });
            }
            TokenKind::Punctuation(PunctuationKind::RParen) => {
                stack.pop();
            }
            TokenKind::Punctuation(PunctuationKind::LBracket) => stack.push(Frame::Bracket),
            TokenKind::Punctuation(PunctuationKind::RBracket) => {
                stack.pop();
            }
            TokenKind::Punctuation(PunctuationKind::LBrace) => stack.push(Frame::Brace),
            TokenKind::Punctuation(PunctuationKind::RBrace) => {
                stack.pop();
            }
            TokenKind::Punctuation(PunctuationKind::Comma) => {
                if let Some(Frame::Paren { commas, .. }) = stack.last_mut() {
                    *commas += 1;
                }
            }
            _ => {}
        }
        prev_ident = (tok.kind == TokenKind::Identifier).then(|| &text[tok.span.start..tok.span.end]);
    }

    stack.iter().rev().find_map(|frame| match frame {
        Frame::Paren { callee: Some(name), commas } => Some((*commas, name.clone())),
        _ => None,
    })
}

fn pattern_label(pattern: &Pattern) -> String {
    match pattern {
        Pattern::Identifier(ident) => ident.name.clone(),
        Pattern::Array { .. } => "[...]".to_string(),
        Pattern::Object { .. } => "{...}".to_string(),
        Pattern::Default { pattern, .. } => pattern_label(pattern),
    }
}

fn param_label(param: &Param) -> String {
    let base = match &param.pattern {
        Some(pattern) => pattern_label(pattern),
        None => param.name.name.clone(),
    };
    let base = if param.is_rest { format!("...{base}") } else { base };
    if param.default.is_some() { format!("{base}?") } else { base }
}

fn register_expr_fn<'a>(name: &'a str, expr: &'a Expr, out: &mut Vec<(&'a str, &'a [Param])>) {
    match expr {
        Expr::FunctionExpr { params, .. } | Expr::ArrowFunction { params, .. } => out.push((name, params)),
        _ => {}
    }
}

fn collect_functions_expr<'a>(expr: &'a Expr, out: &mut Vec<(&'a str, &'a [Param])>) {
    match expr {
        Expr::FunctionExpr { body, .. } | Expr::ArrowFunction { body, .. } => collect_functions(&body.stmts, out),
        _ => {}
    }
}

fn collect_functions<'a>(stmts: &'a [Stmt], out: &mut Vec<(&'a str, &'a [Param])>) {
    for stmt in stmts {
        collect_functions_stmt(stmt, out);
    }
}

fn collect_functions_stmt<'a>(stmt: &'a Stmt, out: &mut Vec<(&'a str, &'a [Param])>) {
    match stmt {
        Stmt::FunctionDecl { name, params, body, .. } => {
            out.push((&name.name, params));
            collect_functions(&body.stmts, out);
        }
        Stmt::VarDecl { pattern, init, .. } => {
            if let Pattern::Identifier(ident) = pattern {
                register_expr_fn(&ident.name, init, out);
            }
            collect_functions_expr(init, out);
        }
        Stmt::Block(block) => collect_functions(&block.stmts, out),
        Stmt::If { then_branch, else_branch, .. } => {
            collect_functions_stmt(then_branch, out);
            if let Some(e) = else_branch {
                collect_functions_stmt(e, out);
            }
        }
        Stmt::While { body, .. } | Stmt::DoWhile { body, .. } | Stmt::Labeled { body, .. } => {
            collect_functions_stmt(body, out);
        }
        Stmt::For { init, body, .. } => {
            if let Some(init) = init {
                collect_functions_stmt(init, out);
            }
            collect_functions_stmt(body, out);
        }
        Stmt::ForIn { body, .. } | Stmt::ForOf { body, .. } | Stmt::ForAwaitOf { body, .. } => {
            collect_functions_stmt(body, out);
        }
        Stmt::TryCatch { try_block, catch_block, finally_block, .. } => {
            collect_functions(&try_block.stmts, out);
            if let Some(b) = catch_block {
                collect_functions(&b.stmts, out);
            }
            if let Some(b) = finally_block {
                collect_functions(&b.stmts, out);
            }
        }
        Stmt::Switch { cases, default, .. } => {
            for case in cases {
                collect_functions(&case.body.stmts, out);
            }
            if let Some(d) = default {
                collect_functions(&d.stmts, out);
            }
        }
        Stmt::ClassDecl { members, .. } => {
            for m in members {
                match m {
                    ClassMember::Method { name, params, body, .. } => {
                        out.push((&name.name, params));
                        collect_functions(&body.stmts, out);
                    }
                    ClassMember::Constructor { body, .. } => collect_functions(&body.stmts, out),
                    _ => {}
                }
            }
        }
        Stmt::Export { kind: ExportKind::Declaration(inner), .. } => collect_functions_stmt(inner, out),
        _ => {}
    }
}

fn build_signature(label: String, param_labels: Vec<String>, doc: Option<String>, active_param: u32) -> SignatureHelp {
    let has_params = !param_labels.is_empty();
    let parameters: Vec<ParameterInformation> = param_labels
        .into_iter()
        .map(|p| ParameterInformation { label: ParameterLabel::Simple(p), documentation: None })
        .collect();
    let active = if has_params { active_param.min(parameters.len() as u32 - 1) } else { 0 };

    SignatureHelp {
        signatures: vec![SignatureInformation {
            label,
            documentation: doc
                .map(|value| Documentation::MarkupContent(MarkupContent { kind: MarkupKind::Markdown, value })),
            parameters: Some(parameters),
            active_parameter: None,
        }],
        active_signature: Some(0),
        active_parameter: Some(active),
    }
}

#[must_use]
pub fn signature_help(text: &str, byte_pos: usize) -> Option<SignatureHelp> {
    let sf = SourceFile::new("inline".to_string(), text.to_string());
    let (tokens, _) = Lexer::new(&sf).tokenize();
    let (active_param, callee_name) = active_call(&tokens, text, byte_pos)?;

    let (program, _) = Parser::new(&tokens, &sf).parse_program();
    let mut functions = Vec::new();
    collect_functions(&program.items, &mut functions);

    if let Some((_, params)) = functions.iter().find(|(name, _)| *name == callee_name) {
        let param_labels: Vec<String> = params.iter().map(param_label).collect();
        let label = format!("{callee_name}({})", param_labels.join(", "));
        return Some(build_signature(label, param_labels, None, active_param));
    }

    if let Some(sig) = BUILTIN_SIGNATURES.iter().find(|s| s.name == callee_name) {
        let param_labels: Vec<String> = sig.params.iter().map(|s| (*s).to_string()).collect();
        let label = format!("{}({})", sig.name, param_labels.join(", "));
        let doc = builtin_doc(sig.name).map(str::to_string);
        return Some(build_signature(label, param_labels, doc, active_param));
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn help_at(src: &str, needle: &str) -> Option<SignatureHelp> {
        let byte = src.find(needle).unwrap() + needle.len();
        signature_help(src, byte)
    }

    #[test]
    fn active_parameter_zero_for_first_argument() {
        let src = "йопта фу(a, b) { отвечаю a; }\nфу(1";
        let help = help_at(src, "фу(1").expect("should resolve signature");
        assert_eq!(help.active_parameter, Some(0));
        assert_eq!(help.signatures[0].label, "фу(a, b)");
    }

    #[test]
    fn active_parameter_one_after_comma() {
        let src = "йопта фу(a, b) { отвечаю a; }\nфу(1, ";
        let help = help_at(src, "фу(1, ").expect("should resolve signature");
        assert_eq!(help.active_parameter, Some(1));
    }

    #[test]
    fn nested_call_uses_inner_signature() {
        let src = "йопта фу(a) { отвечаю a; }\nйопта бар(x, y) { отвечаю x; }\nфу(бар(1, ";
        let help = help_at(src, "бар(1, ").expect("should resolve inner signature");
        assert_eq!(help.signatures[0].label, "бар(x, y)");
        assert_eq!(help.active_parameter, Some(1));
    }

    #[test]
    fn builtin_signature_is_resolved() {
        let src = "чутка(колбэк, ";
        let help = help_at(src, "чутка(колбэк, ").expect("should resolve builtin signature");
        assert_eq!(help.signatures[0].label, "чутка(колбэк, мс)");
        assert_eq!(help.active_parameter, Some(1));
        assert!(help.signatures[0].documentation.is_some());
    }

    #[test]
    fn default_and_rest_params_are_marked() {
        let src = "йопта фу(a, b = 1, ...rest) { отвечаю a; }\nфу(1, ";
        let help = help_at(src, "фу(1, ").expect("should resolve signature");
        assert_eq!(help.signatures[0].label, "фу(a, b?, ...rest)");
    }

    #[test]
    fn no_enclosing_call_returns_none() {
        let src = "гыы x = 1;";
        assert!(signature_help(src, src.len()).is_none());
    }

    #[test]
    fn unknown_callee_returns_none() {
        let src = "неизвестная(1, ";
        assert!(signature_help(src, src.len()).is_none());
    }
}
