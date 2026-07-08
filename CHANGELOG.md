# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.5.0] - 2026-07-09

### Added

- **Static namespace imports**: `спиздить * как ns из "модуль";` binds a
  namespace object with all of the module's exports (the dynamic-import
  namespace object already existed; the static form is now parseable too).
- **User iterables everywhere.** Objects implementing `Symbol.iterator`
  (`Симбол.итератор`) now work in array-literal spread, call-argument spread
  and array destructuring (including rest) — in both the tree-walking
  interpreter and the bytecode VM. The VM gained a `NormalizeIterable` op and
  routes spread, `for…of` and `yield*` through a single shared iterator pump;
  an interpreter-vs-VM conformance case pins identical behavior.
- **`await using`** (`юзай сидетьНахуй`) with the new well-known symbol
  `Симбол.асинхРасход` (`Symbol.asyncDispose`): async disposal is awaited on
  scope exit, falls back to the sync `расход` method, and preserves LIFO
  order and first-error-wins semantics with mixed sync/async resources.
- **Property descriptor API on `Кент`**: `определитьСвойство`
  (defineProperty, data and accessor forms) and `описатьСвойство`
  (getOwnPropertyDescriptor).
- **Bound method extraction**: builtin array and string methods can be
  extracted as values and called later (`гыы м = массив.map; м(ф)`), with
  the receiver kept alive and shared (`гыы п = массив.втолкнуть; п(4)`
  mutates the original array).
- **`Set.keys` / `Set.entries`** (`ключи` / `записи`), matching JS semantics
  (`keys` aliases `values`, `entries` yields `[value, value]` pairs).
- **LSP: scope-aware rename** with `prepareRename` support — an AST-driven
  binding resolver renames declarations and uses per lexical scope, leaves
  shadowed same-named variables and member/object-literal property names
  untouched, and conservatively refuses on builtins and keywords.

### Fixed

- `Кент.имеетСвоё` now reports accessor-defined (getter/setter) properties
  instead of only plain data properties.
- The formatter printed the English `* as` instead of `* как` for namespace
  import specifiers.
- The module loader unit tests used non-existent keywords
  (`импортировать`/`экспортировать`) and tautological assertions, so they
  never validated anything; they now use the real syntax and assert concrete
  exported values.

## [1.4.1] - 2026-06-29

### Fixed

- **Parser no longer panics on malformed string, template or import/export
  string tokens.** A bare quote or backtick (which the lexer emits with a
  diagnostic) produced a length-1 token that the parser byte-sliced as
  `raw[1..len-1]`, panicking on the empty range or on a non-char-boundary with
  Cyrillic content. All such slices now go through a bounds- and
  boundary-safe helper.
- **Parser no longer blows up exponentially on deeply nested parenthesised
  input** such as `(п=(п=(п=…`. A `(` speculatively parsed arrow-function
  parameters and, on failure, re-parsed the same group as a grouping, giving
  O(2ⁿ) time. A cheap token lookahead now attempts the arrow parse only when
  the matching `)` is followed by `=>`.

## [1.4.0] - 2026-06-28

### Fixed

- **VM: per-iteration loop-variable binding.** Closures created inside `for`,
  `for…of` and `for…in` loops on the bytecode VM now capture a fresh binding per
  iteration (matching `let` semantics and the tree-walking interpreter) instead of
  all sharing the final value. A new non-popping `CloseUpvalueTo` opcode is emitted
  at each loop's continue point when the loop variable is captured.
- **VM: array method-call syntax.** `массив.втолкнуть(...)` (aliases `push` /
  `добавить`) now works as a method call with the same semantics as the interpreter
  — variadic push returning the new length — not only as the free function
  `втолкнуть(массив, значение)`.
- **Interpreter: per-iteration binding inside generators.** Closures over a loop
  variable inside a generator body now capture per-iteration values, including when
  the loop body is a nested block or a `хапнуть`/try statement. Fixes an underlying
  scope leak where the generator state machine pushed block and try scopes without
  popping them on normal completion.

## [1.3.2] - 2026-06-28

### Added

- **LSP — built-in class/type hints.** The language server now knows the built-in
  classes, constructors and namespaces (`Матан`/Math, `Жсон`/JSON, `Кент`/Object,
  `Карта`/Map, `Набор`/Set, `Симбол`/Symbol, `Дата`/Date, `СловоПацана`/Promise,
  typed arrays, and more):
  - they are offered in global completion (with their JS equivalents) and resolve
    on hover;
  - typing `получатель.` triggers member completion — namespaced builtins
    (`сказать.ошибка`), the static/instance members of a recognized built-in type,
    or a best-effort union of all known members for an unknown receiver;
  - the server advertises `.` as a completion trigger character.

## [1.3.1] - 2026-06-26

Maintenance release: refresh the toolchain and dependencies.

### Changed

- **Minimum supported Rust version** raised to `1.96` (was `1.88`); the CI MSRV
  job now checks against `1.96`.
- **Dependencies** updated to their latest releases: `rustyline` `18.0.1` and
  refreshed `Cargo.lock` (`regex` `1.12.4`, and other compatible patches).

## [1.3.0] - 2026-06-26

Interop release: align the language surface with the original
[samgozman/YoptaScript](https://github.com/samgozman/YoptaScript).

### Changed

- **BREAKING — `const`/`let` keywords** now follow upstream YoptaScript:
  `ясенХуй`/`ЯсенХуй` declare a constant and `участковый` declares a mutable
  binding (previously the two were inverted). Sources relying on the old mapping
  must swap these keywords.
- **BREAKING — file extension** is now `.yopta` (was `.yop`), matching upstream.
  The CLI, the module resolver (interpreter and VM), the conformance harnesses and
  the VS Code extension all use `.yopta`; `.yop` is no longer recognized.
- **`DICTIONARY.md`** rewritten to match the upstream dictionary, with a section
  documenting intentionally unsupported entries (browser DOM methods, Java-only
  keywords).

### Added

- **Operator word aliases** from the upstream dictionary: `чобля` (`!`),
  `плюсуюНа` (`++`), `слилсяНа` (`--`).
- **VS Code extension**: highlights the new operator aliases and `нихуя` (`NaN`),
  and associates the `.yopta` file extension.
- **`examples/interop.yopta`**: a program in upstream style, covered by the
  interpreter/VM parity suite.

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
