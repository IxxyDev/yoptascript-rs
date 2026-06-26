# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.2.0] - 2026-06-26

### Added

- **VS Code extension** (`editors/vscode`): TextMate syntax highlighting for `.yop`
  files, a `vscode-languageclient` that launches `yps-lsp`, function and method call
  highlighting, an extension icon and a file icon, an F5 debug launch config, and a
  CI job that builds, type-checks and tests it.
- **yps-lsp**: JavaScript-equivalent documentation for builtin functions (the console
  family, type coercions, timers, stdio, etc.), shown on hover and attached to
  completion items.

### Fixed

- **VS Code**: disable ambiguous-character (Unicode) highlighting for the yoptascript
  language so Cyrillic identifiers that resemble Latin letters are not flagged.

## [1.1.0] - 2026-06-25

### Added

- **yps-lsp**: document outline via `textDocument/documentSymbol` for functions,
  classes (with their members) and top-level variable declarations.
- **yps-lsp**: whole-document formatting via `textDocument/formatting`, backed by
  `yps-fmt` (no edits when the source is already canonical or fails to parse).
- **yps-lsp**: go-to-definition via `textDocument/definition`, resolving functions,
  classes, variables and parameters through a full recursive walk of the AST.
- **yps-lsp**: completion now also suggests functions, classes and variables
  declared in the current file, alongside keywords and builtins.

### Changed

- **yps-lsp**: the server binary was split into a testable library (`lib.rs` plus
  per-feature modules), with `main.rs` reduced to a thin tower-lsp wrapper.

### Removed

- Dropped the stale `KNOWN_DIVERGENCES.md` catalogue (conformance divergences now
  live inline as `// DIVERGENCE:` headers in the mirror files) and an obsolete
  decorators planning note.

## [1.0.0] - 2026

### Added

- Initial release: lexer, recursive-descent parser, tree-walking interpreter and a
  bytecode VM backend with byte-for-byte parity, an AST-based formatter (`yps fmt`)
  with a round-trip self-check, a baseline language server (diagnostics, hover,
  keyword completion) and the `yps` CLI.

[1.2.0]: https://github.com/IxxyDev/yoptascript-rs/compare/v1.1.0...v1.2.0
[1.1.0]: https://github.com/IxxyDev/yoptascript-rs/compare/v1.0.0...v1.1.0
[1.0.0]: https://github.com/IxxyDev/yoptascript-rs/releases/tag/v1.0.0
