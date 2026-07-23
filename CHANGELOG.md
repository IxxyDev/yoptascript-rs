# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.8.0] - 2026-07-23

### Changed

- **Interpreter is measurably faster across the board** (criterion suite,
  cumulative): strings ~−16%, objects ~−8%, closures ~−8%, fib ~−6%,
  arrays ~−7%. Three changes: string values are interned `Rc<str>`
  (clones no longer allocate), a post-parse resolver gives unshadowed
  root-scope reads (builtins, top-level functions) a direct fast path,
  and `Value` shrank from 64 to 32 bytes by consolidating fat variants
  into shared payload structs.
- **VM property access is ~2× faster** (objects benchmark −46%):
  monomorphic inline caches validate via weak identity plus a structural
  generation counter, skipping map lookups and the per-read getter
  probe; getters, proxies and frozen checks always take the slow path.
- **VM compiler folds constant expressions** using exact runtime f64 and
  string semantics; BigInt and cross-type coercions stay at runtime.

### Added

- **Differential execution fuzzer** (`cargo +nightly fuzz run exec_diff`,
  weekly in CI): runs valid programs through both backends and fails on
  any output divergence. Its first session found five real parity gaps,
  now tracked in the roadmap backlog with skip-list markers.
- **ADR-0001**: the two backends keep separate `Value` representations
  by design; parity is enforced by the conformance suites.

## [1.7.0] - 2026-07-21

### Added

- **Async generators** (`ассо пиздюли`) on both backends: promise-wrapped
  `следующий`/`вернуть`/`кинуть`, `await` inside bodies, `yield*`
  delegation, and `for await` over native async generators, sync
  iterables and user `Симбол.асинхИтератор` objects.
- **Destructuring in `for-of`/`for-in` loop heads**
  (`го (ясенХуй [а, б] из пары)`) across parser, both backends,
  formatter and LSP, with per-iteration binding preserved.
- **Class static initialization blocks** (`попонятия { ... }`), run in
  declaration order interleaved with static fields, `this` bound to the
  class; static field initializers are now `this`-aware to match JS.
- **VM: native string and array instance methods** — the full
  interpreter surface (callbacks run VM closures, mutators share the
  receiver) instead of only `.втолкнуть`.
- **VM: `await using`** (`юзай сидетьНахуй`) with interpreter-matching
  disposal semantics.
- **VM: mark-sweep cycle collector** — closure, upvalue and object
  cycles no longer leak on the bytecode backend.
- **Full Proxy traps and Reflect methods**: ownKeys, prototype,
  descriptor and extensibility traps dispatch in enumeration, spread,
  `шкура` and the `Кент` APIs; `Отражение` gains the seven mirror
  methods.
- **`Дата` completed**: setters with rollover, UTC accessors, ISO-8601
  parsing with offsets and `Дата.разобрать`.
- **Map/Set `ключи`/`значения`/`записи` return real iterators**;
  `Кент.изЗаписей` accepts iterators.

## [1.6.1] - 2026-07-19

### Fixed

- **Nested delete works.** `ёбнуть массив[0][1]` and nested object paths
  were silently ignored by the tree-walking interpreter (only root-level
  deletes applied); both backends now mutate the addressed container,
  honoring sealed/frozen on the innermost object.
- **VM enforces `заморозить`/`запечатать`/`запретитьРасширение`.** The
  freeze-family statics used to set flags on a throwaway bridge copy, so
  VM-native writes mutated frozen objects; flags now live on the shared
  native object and every set/index-set/delete path honors them.
- **VM no longer leaks well-known-symbol keys** (`[встроенная Симбол.…]`)
  when printing objects, matching the interpreter's Display.

## [1.6.0] - 2026-07-19

### Added

- **CLI**: `--version`/`-V`, `--help`/`-h`, `-e`/`--eval "<код>"` for inline
  snippets and `yps -` for reading a program from stdin. Unknown flags now
  fail with an error instead of being silently ignored.
- **Criterion benchmarks** (`crates/yps-bench`, `just bench`): five workloads
  (fib, strings, objects, closures, arrays) run on both backends from the
  same parsed AST.
- **`Строка` namespace**: `raw`, `изСимволов`/`fromCharCode`,
  `изКодовТочек`/`fromCodePoint`; string instance methods
  `кодТочки`/`codePointAt` (surrogate-pair aware) and
  `нормализовать`/`normalize` (NFC/NFD/NFKC/NFKD via the new
  `unicode-normalization` dependency).
- **Array methods**: `заполнить`/`fill`, `копироватьВнутри`/`copyWithin`
  (Node clamp and overlap semantics) and iterator-returning
  `записи`/`entries`, `ключи`/`keys`, `значения`/`values`.
- **`Кент` statics**: `есть` (SameValue), `запечатать`, `запечатан`,
  `запретитьРасширение`, `расширяем`, `определитьСвойства`,
  `описатьСвойства`. `ObjectStore` tracks sealed/extensible alongside
  frozen, enforced on every write, delete and prototype-change path
  including the VM bridge; `заморозить` now implies sealed and
  non-extensible.
- **`Матан`**: 21 new functions (inverse trig, hyperbolic, `лог2`/`лог10`/
  `лог1п`, `эксп`/`экспМ1`, `кубическийКорень`, `гипотенуза`, `дробь32`,
  `нулиСлева32`, `умножить32`) and 6 constants (`ЛН2`, `ЛН10`, `ЛОГ2Е`,
  `ЛОГ10Е`, `КОРЕНЬ2`, `КОРЕНЬ0_5`), reachable on both backends.

### Fixed

- **GC runs in plain script execution and between event-loop ticks.**
  Pending micro- and macrotasks carry explicit GC roots, so the collector
  no longer bails out while queues are non-empty; long scripts and
  interval-driven programs finally reclaim cyclic garbage.
- **VM: string coercion honors user `вСтроку`/`Симбол.вПримитив`** in
  string concatenation, mirroring the interpreter's to-primitive protocol;
  template literals compile to a dedicated op preserving Display semantics.
- `tools/gen-golden.js` mangled `.yopta` case names and reported 100% SKIP;
  the oracle now actually verifies the conformance battery against Node.

### Changed

- **VM: class member lookup is hash-indexed** instead of a per-call linear
  scan, preserving insertion order and first-wins duplicate semantics.

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
