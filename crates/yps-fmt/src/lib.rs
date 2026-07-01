pub mod comments;
pub mod normalize;
pub mod printer;
pub mod sourcemap;
mod tests;

pub use sourcemap::SourceMap;

use yps_lexer::{Lexer, SourceFile, Trivia};
use yps_parser::Parser;

#[derive(Debug)]
pub struct FormatOutcome {
    pub text: String,
    pub already_formatted: bool,
}

#[derive(Debug)]
pub enum FormatError {
    ParseError(Vec<yps_lexer::Diagnostic>),
    RoundTripFailed(String),
    CommentRefused(String),
}

impl std::fmt::Display for FormatError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FormatError::ParseError(_) => write!(f, "ошибка синтаксического анализа"),
            FormatError::RoundTripFailed(msg) => write!(f, "самопроверка не прошла: {msg}"),
            FormatError::CommentRefused(msg) => write!(f, "форматирование отклонено из-за комментария: {msg}"),
        }
    }
}

fn lex_and_parse(source: &str) -> Result<(yps_parser::Program, Vec<Trivia>), FormatError> {
    let sf = SourceFile::new("<fmt>".to_string(), source.to_string());
    let (tokens, trivia, lex_diags) = Lexer::new(&sf).tokenize_with_trivia();
    if !lex_diags.is_empty() {
        return Err(FormatError::ParseError(lex_diags));
    }
    let (program, parse_diags) = Parser::new(&tokens, &sf).parse_program();
    if !parse_diags.is_empty() {
        return Err(FormatError::ParseError(parse_diags));
    }
    Ok((program, trivia))
}

fn verify_round_trip(
    original: &yps_parser::Program,
    original_trivia: &[Trivia],
    formatted: &str,
) -> Result<(), FormatError> {
    let sf = SourceFile::new("<fmt-check>".to_string(), formatted.to_string());
    let (tokens, trivia, lex_diags) = Lexer::new(&sf).tokenize_with_trivia();
    if !lex_diags.is_empty() {
        return Err(FormatError::RoundTripFailed("вывод форматтера не лексируется".to_string()));
    }
    let (program, parse_diags) = Parser::new(&tokens, &sf).parse_program();
    if !parse_diags.is_empty() {
        return Err(FormatError::RoundTripFailed("вывод форматтера не парсируется".to_string()));
    }
    if !normalize::programs_equivalent(original, &program) {
        return Err(FormatError::RoundTripFailed("вывод форматтера структурно не эквивалентен исходнику".to_string()));
    }
    if !comment_texts_equal(original_trivia, &trivia) {
        return Err(FormatError::RoundTripFailed("множество комментариев изменилось при форматировании".to_string()));
    }
    Ok(())
}

pub fn format_source(source: &str) -> Result<FormatOutcome, FormatError> {
    let (program, trivia) = lex_and_parse(source)?;

    let formatted = if trivia.is_empty() {
        printer::print_program(&program)
    } else {
        let comment_map = comments::attach_comments(&program, &trivia, source).map_err(FormatError::CommentRefused)?;
        printer::print_program_with_comments(&program, &comment_map)
    };

    verify_round_trip(&program, &trivia, &formatted)?;

    let already_formatted = formatted == source;
    Ok(FormatOutcome { text: formatted, already_formatted })
}

pub fn format_source_with_map(source: &str) -> Result<(FormatOutcome, SourceMap), FormatError> {
    let (program, trivia) = lex_and_parse(source)?;

    let (formatted, map) = if trivia.is_empty() {
        printer::print_program_with_map(&program, None, source)
    } else {
        let comment_map = comments::attach_comments(&program, &trivia, source).map_err(FormatError::CommentRefused)?;
        printer::print_program_with_map(&program, Some(&comment_map), source)
    };

    verify_round_trip(&program, &trivia, &formatted)?;

    let already_formatted = formatted == source;
    Ok((FormatOutcome { text: formatted, already_formatted }, map))
}

fn comment_texts_equal(a: &[Trivia], b: &[Trivia]) -> bool {
    let mut left: Vec<&str> = a.iter().map(|t| t.text.as_str()).collect();
    let mut right: Vec<&str> = b.iter().map(|t| t.text.as_str()).collect();
    left.sort_unstable();
    right.sort_unstable();
    left == right
}

#[cfg(test)]
mod trivia_tests {
    use super::{FormatError, format_source, format_source_with_map};

    #[test]
    fn preserves_leading_and_trailing_comments() {
        let src = "// шапка\nгыы х = 1; // хвост\n";
        let out = format_source(src).unwrap();
        assert!(out.text.contains("// шапка"));
        assert!(out.text.contains("// хвост"));
    }

    #[test]
    fn formatting_with_comments_is_idempotent() {
        let src = "// шапка\nгыы х = 1; // хвост\n// конец\n";
        let first = format_source(src).unwrap().text;
        let second = format_source(&first).unwrap().text;
        assert_eq!(first, second);
    }

    #[test]
    fn dangling_comment_refused_without_data_loss() {
        let src = "йопта ф() {\n    // пусто\n}\n";
        let err = format_source(src).unwrap_err();
        assert!(matches!(err, FormatError::CommentRefused(_)));
    }

    #[test]
    fn slash_inside_string_is_not_a_comment() {
        let src = "гыы у = \"http://пример\";\n";
        let out = format_source(src).unwrap();
        assert!(out.text.contains("\"http://пример\""));
    }

    #[test]
    fn with_map_preserves_comments() {
        let src = "// шапка\nгыы х = 1; // хвост\n";
        let (out, _map) = format_source_with_map(src).unwrap();
        assert!(out.text.contains("// шапка"));
        assert!(out.text.contains("// хвост"));
    }

    #[test]
    fn with_map_refuses_dangling_comment() {
        let src = "йопта ф() {\n    // пусто\n}\n";
        let err = format_source_with_map(src).unwrap_err();
        assert!(matches!(err, FormatError::CommentRefused(_)));
    }

    #[test]
    fn with_map_produces_at_least_one_mapping() {
        let src = "гыы х = 1;\nгыы у = 2;\n";
        let (_out, map) = format_source_with_map(src).unwrap();
        assert!(map.mappings.len() >= 2);
    }
}
