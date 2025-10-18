use yps_lexer::{Lexer, SourceFile};

fn main() {
    let source = SourceFile::new("test.yop".into(), "pachan x + 42".into());
    let lexer = Lexer::new(&source);

    let (tokens, diagnostics) = lexer.tokenize();

    for token in tokens {
        println!("{:?} @ {}..{}", token.kind, token.span.start, token.span.end);
    }

    for diagnostic in diagnostics {
        eprintln!("{:?}: {}", diagnostic.severity, diagnostic.message);
    }
}
