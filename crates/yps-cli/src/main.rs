use yps_lexer::{Lexer, TokenKind};

fn main() {
    let source = "pachan x + 42";
    let mut lexer = Lexer::new(source);

    loop {
        let token = lexer.next_token();
        println!("{:?} @ {}..{}", token.kind, token.span.start, token.span.end);

        if matches!(token.kind, TokenKind::Eof) {
            break;
        }
    }

    for diagnostic in lexer.diagnostics() {
        eprintln!("{:?}: {}", diagnostic.severity, diagnostic.message)
    }
}
