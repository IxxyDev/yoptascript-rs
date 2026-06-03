use std::collections::HashMap;

use yps_lexer::{Span, Trivia};
use yps_parser::{Block, ClassMember, ExportKind, Program, Stmt, SwitchCase};

pub struct CommentMap {
    leading: HashMap<usize, Vec<String>>,
    trailing: HashMap<usize, String>,
    eof_trailing: Vec<String>,
}

impl CommentMap {
    pub fn leading(&self, start: usize) -> Option<&[String]> {
        self.leading.get(&start).map(|v| v.as_slice())
    }

    pub fn trailing(&self, start: usize) -> Option<&str> {
        self.trailing.get(&start).map(|s| s.as_str())
    }

    pub fn eof_trailing(&self) -> &[String] {
        &self.eof_trailing
    }

    pub fn is_empty(&self) -> bool {
        self.leading.is_empty() && self.trailing.is_empty() && self.eof_trailing.is_empty()
    }
}

pub fn attach_comments(program: &Program, trivia: &[Trivia], source: &str) -> Result<CommentMap, String> {
    let mut spans = Vec::new();
    collect_stmt_spans(&program.items, &mut spans);
    spans.sort_by_key(|s| s.start);

    let mut leading: HashMap<usize, Vec<String>> = HashMap::new();
    let mut trailing: HashMap<usize, String> = HashMap::new();
    let mut eof_trailing: Vec<String> = Vec::new();

    for tr in trivia {
        let comment_start = tr.span.start;
        let comment_end = tr.span.end;

        let preceding = spans.iter().filter(|s| s.end <= comment_start).max_by_key(|s| s.end).copied();

        let is_trailing_inline = match preceding {
            Some(s) => !source.get(s.end..comment_start).is_none_or(|gap| gap.contains('\n')),
            None => false,
        };

        if is_trailing_inline {
            let s = preceding.unwrap();
            if trailing.contains_key(&s.start) {
                return Err("несколько trailing-комментариев на одной строке не поддерживаются".to_string());
            }
            trailing.insert(s.start, tr.text.clone());
            continue;
        }

        let next = spans.iter().filter(|s| s.start >= comment_end).min_by_key(|s| s.start).copied();

        match next {
            Some(s) => leading.entry(s.start).or_default().push(tr.text.clone()),
            None => {
                let contained = spans.iter().any(|s| comment_start >= s.start && comment_end <= s.end);
                if contained {
                    return Err("комментарий в нераспознанной позиции (dangling в блоке)".to_string());
                }
                eof_trailing.push(tr.text.clone());
            }
        }
    }

    Ok(CommentMap { leading, trailing, eof_trailing })
}

fn collect_stmt_spans(stmts: &[Stmt], out: &mut Vec<Span>) {
    for stmt in stmts {
        if matches!(stmt, Stmt::Empty { .. }) {
            continue;
        }
        out.push(stmt.span());
        collect_children(stmt, out);
    }
}

fn collect_children(stmt: &Stmt, out: &mut Vec<Span>) {
    match stmt {
        Stmt::Block(block) => collect_block(block, out),
        Stmt::If { then_branch, else_branch, .. } => {
            collect_stmt_spans(std::slice::from_ref(then_branch), out);
            if let Some(else_branch) = else_branch {
                collect_stmt_spans(std::slice::from_ref(else_branch), out);
            }
        }
        Stmt::While { body, .. }
        | Stmt::DoWhile { body, .. }
        | Stmt::For { body, .. }
        | Stmt::ForIn { body, .. }
        | Stmt::ForOf { body, .. }
        | Stmt::ForAwaitOf { body, .. }
        | Stmt::Labeled { body, .. } => {
            collect_stmt_spans(std::slice::from_ref(body), out);
        }
        Stmt::FunctionDecl { body, .. } => collect_block(body, out),
        Stmt::TryCatch { try_block, catch_block, finally_block, .. } => {
            collect_block(try_block, out);
            if let Some(catch_block) = catch_block {
                collect_block(catch_block, out);
            }
            if let Some(finally_block) = finally_block {
                collect_block(finally_block, out);
            }
        }
        Stmt::Switch { cases, default, .. } => {
            for SwitchCase { body, .. } in cases {
                collect_block(body, out);
            }
            if let Some(default) = default {
                collect_block(default, out);
            }
        }
        Stmt::ClassDecl { members, .. } => {
            for member in members {
                collect_member(member, out);
            }
        }
        Stmt::Export { kind: ExportKind::Declaration(inner), .. } => {
            collect_stmt_spans(std::slice::from_ref(inner), out);
        }
        _ => {}
    }
}

fn collect_block(block: &Block, out: &mut Vec<Span>) {
    collect_stmt_spans(&block.stmts, out);
}

fn collect_member(member: &ClassMember, out: &mut Vec<Span>) {
    match member {
        ClassMember::Constructor { body, .. }
        | ClassMember::Method { body, .. }
        | ClassMember::Getter { body, .. }
        | ClassMember::Setter { body, .. } => collect_block(body, out),
        ClassMember::Field { .. } => {}
    }
}

#[cfg(test)]
mod tests {
    use yps_lexer::{Lexer, SourceFile};
    use yps_parser::Parser;

    use super::attach_comments;

    fn build(source: &str) -> (yps_parser::Program, Vec<yps_lexer::Trivia>) {
        let sf = SourceFile::new("<t>".to_string(), source.to_string());
        let (tokens, trivia, diags) = Lexer::new(&sf).tokenize_with_trivia();
        assert!(diags.is_empty(), "лексер выдал диагностику: {diags:?}");
        let (program, pdiags) = Parser::new(&tokens, &sf).parse_program();
        assert!(pdiags.is_empty(), "парсер выдал диагностику: {pdiags:?}");
        (program, trivia)
    }

    #[test]
    fn leading_comment_attaches_to_next_stmt() {
        let src = "// шапка\nгыы х = 1;\n";
        let (program, trivia) = build(src);
        let map = attach_comments(&program, &trivia, src).unwrap();
        let target = program.items[0].span().start;
        assert_eq!(map.leading(target).unwrap(), ["// шапка".to_string()]);
        assert!(map.trailing(target).is_none());
    }

    #[test]
    fn trailing_inline_attaches_to_preceding_stmt() {
        let src = "гыы х = 1; // хвост\n";
        let (program, trivia) = build(src);
        let map = attach_comments(&program, &trivia, src).unwrap();
        let target = program.items[0].span().start;
        assert_eq!(map.trailing(target).unwrap(), "// хвост");
        assert!(map.leading(target).is_none());
    }

    #[test]
    fn eof_comment_becomes_program_trailing() {
        let src = "гыы х = 1;\n// конец\n";
        let (program, trivia) = build(src);
        let map = attach_comments(&program, &trivia, src).unwrap();
        assert_eq!(map.eof_trailing(), ["// конец".to_string()]);
    }

    #[test]
    fn dangling_comment_in_empty_block_is_refused() {
        let src = "йопта ф() {\n    // пусто\n}\n";
        let (program, trivia) = build(src);
        assert!(attach_comments(&program, &trivia, src).is_err());
    }
}
