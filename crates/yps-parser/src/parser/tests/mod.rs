use super::*;

fn parse_expr_from_source(src: &str) -> Result<Expr, Vec<Diagnostic>> {
    let source = SourceFile::new("test.yop".to_string(), src.to_string());
    let lexer = yps_lexer::Lexer::new(&source);
    let (tokens, lex_diags) = lexer.tokenize();

    if !lex_diags.is_empty() {
        return Err(lex_diags);
    }

    let mut parser = Parser::new(&tokens, &source);
    match parser.parse_expr() {
        Ok(expr) => Ok(expr),
        Err(()) => Err(parser.diagnostics),
    }
}

fn parse_program_from_source(src: &str) -> (Program, Vec<Diagnostic>) {
    let source = SourceFile::new("test.yop".to_string(), src.to_string());
    let (tokens, _) = yps_lexer::Lexer::new(&source).tokenize();
    Parser::new(&tokens, &source).parse_program()
}

fn diag_messages(diags: &[Diagnostic]) -> Vec<&str> {
    diags.iter().map(|d| d.message.as_str()).collect()
}

mod diagnostics;
mod expressions;
mod functions;
mod literals;
mod modules;
mod statements;
