#![no_main]

use libfuzzer_sys::fuzz_target;
use yps_lexer::{Lexer, SourceFile};
use yps_parser::Parser;

fuzz_target!(|data: &str| {
    let source = SourceFile::new("fuzz".to_string(), data.to_string());
    let (tokens, _) = Lexer::new(&source).tokenize();
    let _ = Parser::new(&tokens, &source).parse_program();
});
