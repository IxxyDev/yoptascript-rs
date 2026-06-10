#![no_main]

use libfuzzer_sys::fuzz_target;
use yps_lexer::{Lexer, SourceFile};

fuzz_target!(|data: &str| {
    let source = SourceFile::new("fuzz".to_string(), data.to_string());
    let _ = Lexer::new(&source).tokenize();
});
