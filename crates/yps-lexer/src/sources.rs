use std::rc::Rc;

use crate::SourceFile;

#[derive(Debug, Default)]
pub struct Sources {
    files: Vec<Rc<SourceFile>>,
}

impl Sources {
    #[must_use]
    pub fn next_base(&self) -> usize {
        self.files.last().map_or(0, |file| file.base() + file.source.len() + 1)
    }

    pub fn add(&mut self, name: String, source: String) -> Rc<SourceFile> {
        let base = self.next_base();
        self.insert(SourceFile::with_base(name, source, base))
    }

    pub fn insert(&mut self, file: SourceFile) -> Rc<SourceFile> {
        assert!(file.base() >= self.next_base(), "исходник пересекается с уже зарегистрированным");
        let file = Rc::new(file);
        self.files.push(Rc::clone(&file));
        file
    }

    #[must_use]
    pub fn lookup(&self, offset: usize) -> Option<&Rc<SourceFile>> {
        let idx = self.files.partition_point(|file| file.base() <= offset);
        self.files[..idx].last().filter(|file| file.contains(offset))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Lexer, TokenKind};

    #[test]
    fn first_source_starts_at_zero() {
        let mut sources = Sources::default();

        let main = sources.add("main.yopta".into(), "abc".into());

        assert_eq!(main.base(), 0);
    }

    #[test]
    fn sources_never_overlap() {
        let mut sources = Sources::default();
        let main = sources.add("main.yopta".into(), "abc".into());

        let module = sources.add("mod.yopta".into(), "de".into());

        assert!(module.base() > main.base() + main.source.len());
        assert_eq!(sources.next_base(), module.base() + module.source.len() + 1);
    }

    #[test]
    fn lookup_finds_the_file_that_owns_an_offset() {
        let mut sources = Sources::default();
        let main = sources.add("main.yopta".into(), "abc".into());
        let module = sources.add("mod.yopta".into(), "de".into());

        assert_eq!(sources.lookup(main.base() + 3).map(|f| f.name.as_str()), Some("main.yopta"));
        assert_eq!(sources.lookup(module.base()).map(|f| f.name.as_str()), Some("mod.yopta"));
        assert_eq!(sources.lookup(module.base() + 2).map(|f| f.name.as_str()), Some("mod.yopta"));
        assert!(sources.lookup(module.base() + 3).is_none());
    }

    #[test]
    fn insert_registers_a_source_built_at_the_next_base() {
        let mut sources = Sources::default();
        sources.add("main.yopta".into(), "abc".into());
        let pending = SourceFile::with_base("<repl>".into(), "xy".into(), sources.next_base());
        let base = pending.base();

        let stored = sources.insert(pending);

        assert_eq!(stored.base(), base);
        assert_eq!(sources.lookup(base + 1).map(|f| f.name.as_str()), Some("<repl>"));
    }

    #[test]
    fn tokens_of_a_registered_source_resolve_back_to_it() {
        let mut sources = Sources::default();
        sources.add("main.yopta".into(), "гыы а = 1;".into());
        let module = sources.add("mod.yopta".into(), "\nгыы б = 2;".into());

        let (tokens, diags) = Lexer::new(&module).tokenize();

        assert!(diags.is_empty(), "{diags:?}");
        let ident = tokens.iter().find(|t| t.kind == TokenKind::Identifier).expect("identifier token");
        let owner = sources.lookup(ident.span.start).expect("owner");
        assert_eq!(owner.name, "mod.yopta");
        assert_eq!(owner.slice(ident.span), "б");
        assert_eq!(owner.position(ident.span.start), (2, 5));
    }
}
