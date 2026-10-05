# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.17.4] - 2026-10-05

### Changed

- **Debug adapter follows the DAP specification more closely** — failed
  requests carry a short machine-readable `message` (`notStopped`,
  `cancelled`, `unsupported`, …) with the Russian text in `body.error`;
  `linesStartAt1` / `columnsStartAt1` are honoured and columns are counted
  in UTF-16 code units; `Variable.type` is sent only to clients that ask
  for it; `stopped` names the hit breakpoint and unverified breakpoints
  explain why; adapter-side messages use the `console` output category;
  `stackTrace` and `variables` page with `startFrame` / `levels` and
  `filter` / `start` / `count`; `step*` and `continue` sent while the
  program runs are rejected with `notStopped` instead of being queued.
- **Debug adapter `launch`** understands `noDebug`, `args` (they reach the
  script as `Процесс.аргументы`) and `cwd`, and `evaluate` resolves the
  name of an innermost-frame local.
- **`terminate` no longer shuts the adapter down** — it answers at once and
  sends `terminated`; only `disconnect` (or the client closing the stream)
  ends the adapter, and `exited` follows when the program really stops.

### Fixed

- **Debug adapter stepping cost** — every executed statement used to scan
  the source from the start to find its line, so a 100 KB program with a
  200 000-iteration loop took 18.7 s under the adapter; lines now come from
  a precomputed index and the same run takes 0.084 s, on par with plain
  `yps`.
- **Debug adapter no longer hangs on an interpreter panic** — the client
  always receives exactly one `exited`, with exit code 1 after a panic.
- **Debug adapter `pause`** — it is armed only while the program runs, so
  pausing at a breakpoint no longer produces a spurious stop after the next
  `continue`; `terminate` no longer emits a stray `stopped`.
- **Debug adapter launch errors** — only errors abort a launch (warnings
  are ignored), the message names `file:line:column`, an unreadable program
  fails the `launch` request, and a runtime error inside an imported module
  names the module's file and line. Breakpoints no longer fire on module
  code and module frames show the module's own file and line.
- **Debug adapter protocol robustness** — a malformed frame is reported
  instead of silently ending the session, header lines and bodies are
  capped (8 KiB / 64 MiB), a bad `frameId` no longer panics, requests
  queued while the program runs are answered with `cancelled` on
  `terminate`, `configurationDone` without `launch` gets a single reply,
  breakpoint ids are unique and messages that are not requests are ignored.

## [1.17.3] - 2026-10-04

### Changed

- **Script arguments** — everything after the program file (or after `-`)
  now goes to the program and shows up in `Процесс.аргументы`, with the
  file, `-` or `-e` as the first element; for `-e` the arguments follow
  `--`. `yps` flags must therefore precede the file: `yps file.yopta --vm`
  no longer selects the VM, it passes `--vm` to the script. Both backends
  now report the same list (the VM used to include `--vm` itself).
- **Exit codes** — bad command-line usage exits with `2` instead of `1`;
  `1` stays for program and file errors, lint findings and a failed
  `fmt --check`, `70` for internal errors. A piped (non-TTY) REPL session
  exits with `1` if any input failed, not only on an unterminated block.
- **One program source** — combining `-e` with a file or `-`, or repeating
  `-e`, is rejected instead of silently ignoring all but one of them.
- **`fmt --source-map` requires `--write`** and writes the map to
  `<file>.map`; it no longer prints the map after the code on stdout, and
  the map's `file` field names the formatted file rather than the map.
  `--check` cannot be combined with `--write` or `--source-map`.
- **Diagnostics label their severity in Russian** — `Ошибка:` /
  `Предупреждение` instead of `Error:` / `Warning`, in the CLI, `lint`
  and the playground.
- **Uncaught `кидай` reports the throw site** — in ordinary functions,
  methods, constructors, callbacks and at the top level the error position
  is the `кидай` statement itself, also after it passes through a
  `тюряжка` block (the call chain stays in the stack frames); the
  interpreter used to report the innermost call site and the VM `1:1`.
  Generators and `ассо` functions are unchanged: they still report the
  resuming call. Top-level `харэ` / `двигай` outside a loop also report
  their own line.

### Fixed

- **`fmt --write` no longer damages files** — it keeps the file's
  permissions, refuses a read-only file, formats the target of a symlink
  instead of replacing the link, never touches an unrelated
  `<file>.fmt_tmp`, leaves an already formatted file alone, and cleans up
  its temporary file when the write fails.
- **Closed stdout pipe** — `yps … | head` now ends quietly on Unix for
  every mode; the interpreter used to report an internal error with exit
  code 70, `ast` / `disasm` panicked, and the VM ignored the write errors
  and exited with 0.
- **Errors inside imported modules name the module** — a runtime error
  raised by module code is reported with the module's file, line and
  column on both backends instead of the main file's; a syntax error in a
  module is shown as a located diagnostic. In the REPL an error in code
  defined by an earlier input is reported as `<repl#N>:line:col`.
- **REPL** — template literals, block comments and strings may span
  lines; `:выход`, `:сброс` and `:история` work inside an unfinished block;
  inputs rejected by the lexer stay in `:история`; REPL commands are no
  longer saved to the persistent history; an interpreter panic resets the
  session instead of killing it; the history file is found on Windows.
- **Completion** — members of dotted builtins complete after the dot
  (`сказать.ош` → `сказать.ошибка`); destructured names, classes and
  imports are offered, declarations nested in function bodies are not.
- **Command line** — `fmt` accepts its file in any position; every
  subcommand answers `--help`; `repl` and `--version` reject stray
  arguments; a non-UTF-8 argument no longer panics; `transpile -o` refuses
  to overwrite its own input.

## [1.17.2] - 2026-10-02

### Changed

- **Class bodies are resolved to frame slots** — a class declaration no
  longer switches the whole program back to the name-map path. Methods,
  accessors, constructors, static blocks and field initializers now get
  slot coordinates like ordinary functions; constructors and static blocks
  also mark their lexical declarations as uninitialized (TDZ). Generators,
  async functions and imports still keep the previous path. A
  method-heavy loop over a small class runs about 23% faster.

### Fixed

- **Instance field initializers use the class definition scope** — the
  interpreter evaluated `поле = у` in the scope where the instance was
  created, so a class returned from a function read the caller's `у`
  instead of the one it closed over. The VM already behaved correctly.
- **Setter parameters are bound like any other call** — a destructuring
  setter parameter such as `set з({ х })` left `х` unbound in the
  interpreter, and the body read an outer variable instead, while the VM
  bound it correctly. Setter bodies also gained TDZ checks.

## [1.17.1] - 2026-09-24

### Changed

- **Invalid assignment and update targets are parse errors** — `1 = 2`,
  `!х += 1`, `о.а++` and `а?.б = 1` are reported by the parser, matching
  the early errors JavaScript raises for the same code, instead of failing
  at runtime in the interpreter and at compile time in the VM. Code such as
  `1 || 1++` that only one backend used to reach now fails identically on
  both. A plain assignment accepts identifiers, member and index
  expressions, parenthesized targets and destructuring patterns; compound
  and logical assignments accept identifiers, member and index
  expressions; `++`/`--` accept an identifier.

### Fixed

- **Object patterns read from non-objects with member semantics** —
  `гыы {длина} = "abc"` failed in the interpreter with "Невозможно
  деструктурировать строка как объект" while the VM produced 3. Strings,
  arrays and proxies now destructure the way a member access would in both
  backends; primitives without properties fail with the usual member-access
  error, and a rest element still requires a plain object.
- **VM accepts parenthesized targets in compound assignment** —
  `(х[0]) += 1` and `(о.а) *= 5` were rejected by the VM compiler.
- **`exec_diff` fuzz target masks nothing** — the two masks for VM-only
  assignment-target errors are gone.

## [1.17.0] - 2026-09-23

### Fixed

- **Formatter keeps the parentheses around an arrow body that starts
  with an object literal** — `() => ({ к: 1 })` and `() => ({ к: 1 }).к`
  were printed without them, so the body re-parsed as a block and the
  round-trip self-check refused the file. The check now looks through
  grouping and member chains to the leftmost expression.
- **RegExp values cross the VM stdlib bridge** — passing a regexp into
  `Жсон`, `Кент`, `Матан` or a `Посредник` trap under `--vm` failed with
  "значение типа 'регэксп' пока нельзя передать в stdlib интерпретатора".
  Both directions now convert the value and share its `lastIndex` state,
  and the `exec_diff` fuzz target no longer masks that message.
- **VM resolves `этоКосяк`, `прочестьСтроку` and `прочестьВсё`** — the
  interpreter exposed these three builtins as globals, but the VM reported
  "переменная не определена" for them. The weekly `exec_diff` fuzz run
  caught the divergence on `+этоКосяк;`. The VM now resolves all three
  through its builtin table, checks error objects natively and reads stdin
  through the interpreter's stdio helpers.

## [1.16.0] - 2026-09-17

### Changed

- **Interpreter locals are resolved to frame slots** — a post-parse
  resolver assigns `(hops, slot)` coordinates to every identifier whose
  binding is statically known, and `EnvFrame` stores those bindings in a
  slot vector with bitmask-tracked TDZ/const state instead of hashing the
  name up the scope chain on every read and write. Scopes with more than
  64 bindings, classes, generators, async functions and imports keep the
  previous name-map path. Criterion (interpreter variants): closures
  −44%, arrays −19%, fib −17%, strings −16%, objects −15%.
- **VM method calls are inline-cached** — `Op::Invoke` on a class instance
  caches the resolved method per call site, keyed by class identity and
  revalidated on every hit (own properties still shadow, getters, proxies,
  host receivers, static and super calls never enter the cache). The GC
  now marks cache tables through function prototypes. New `methods`
  benchmark: VM −13%.
- **VM compiler peephole pass** — emit-time rewrites with a fold barrier
  at every captured jump target: `!cond` folds into an inverted jump
  (`JumpIfTrue`), literal `вилкойвглаз`/`потрещим` conditions drop the
  test or the dead block branch, and side-effect-free push/pop pairs are
  not emitted. New `loops` benchmark: VM −13%.

### Fixed

- **Parenthesized arrow function spans** — `(а, б) => …` recorded a token
  index instead of a byte offset as its span start, so diagnostics,
  editor ranges and any span-keyed pass pointed at the wrong place.
- **Scope frames leaked on error** — an exception thrown from a loop head
  (`го` init/condition/update, `for-in`/`for-of` body or iterator) or a
  failed parameter bind on a call, method, constructor or `super` call
  left the callee/loop frame installed in the caller's environment.
  Previously masked by name lookup walking past the extra frame.

### Added

- **`Value` size guards** — tests pin `size_of::<Value>()` to 32 bytes
  for the interpreter and 64 for the VM so layout regressions fail CI.

## [1.15.1] - 2026-09-14

### Fixed

- **`нихуя` (NaN) global on the bytecode VM** — the interpreter defines
  the global alias `нихуя` as `NaN`, but the VM did not know it and
  failed with "variable not defined". Found by the weekly `exec_diff`
  fuzz run; the VM now resolves it like the other interpreter globals.

## [1.15.0] - 2026-08-30

### Added

- **`yps-lsp` hover for user-declared symbols** — hovering over a
  user-declared function, variable, constant, parameter, catch
  parameter, class, or import now shows its kind (and, for functions,
  its parameter list), instead of only resolving keywords, builtins,
  and stdlib types/members.
- **`yps-lsp` `workspace/symbol`** — search for a symbol by name
  across all open files (e.g. VS Code's Cmd+T/Ctrl+T "Go to Symbol in
  Workspace"), including nested class members with their containing
  class as `container_name`.
- **`yps-lsp` `textDocument/documentHighlight`** — highlights every
  occurrence of the symbol under the cursor within the current file.

## [1.14.1] - 2026-08-21

### Fixed

- **Large hex/octal/binary literals** — `0xeeeeeeeeee001000` and similar
  values above `i64::MAX` overflowed to `NaN` and were then rejected as
  an "invalid number" runtime error, instead of coercing like real JS
  (and like `--vm` already did).
- **Calls with fewer arguments than parameters** — the interpreter threw
  when a function or constructor call passed fewer arguments than its
  non-default parameters, instead of binding the missing ones to
  `undefined` like real JS (and like `--vm` already did).

## [1.14.0] - 2026-08-21

### Added

- **`yps-lint` rule set** — four new lint rules, in addition to the
  existing unused-variable, unreachable-code and shadowed-declaration
  checks: `unused-import` (an imported binding that's never read),
  `duplicate-object-key` (the same identifier key repeated in an
  object literal), `self-assignment` (`x = x`), and `duplicate-param`
  (the same simple parameter name declared twice in one function).
  All four flow through to `yps lint` and the language server the
  same way the existing rules do.

## [1.13.1] - 2026-08-15

### Fixed

- **`--vm` regex iteration** — the bytecode VM couldn't iterate a
  `найтиВсе`/matchAll result via `for-of` or spread (`Нельзя
  итерировать по типу 'итератор'`); it now drains the iterator like
  the tree-walking interpreter does. Fixing this also surfaced that
  `RegExp(...)` had no native VM constructor and that
  `последнийИндекс`/`lastIndex` assignment on a `RegExp` wasn't
  handled — both added.
- **`--vm` destructuring-assignment expressions** — `[a, b] = pair;`
  and `{x, y} = point;` used as a standalone assignment (not a
  `гыы`/`ясенХуй` declaration) previously failed to compile under
  `--vm` while working under the interpreter; the VM compiler now
  supports rest elements, default values and object rest for this
  form.
- **Mixed-type arithmetic** — `"5" - 2`, `true * 3` and similar raised
  a runtime error in the interpreter instead of coercing operands to
  numbers like `+`, the VM, and JavaScript already do; `-`, `*`, `/`,
  `%` and `**` now perform the same coercion.

## [1.13.0] - 2026-08-15

### Added

- **`yps transpile` namespace support** — `Матан`, `Жсон` and `Отражение`
  now transpile to native `Math`, `JSON` and `Reflect` instead of always
  raising `TranspileError`, covering all Math constants/methods, both
  JSON methods and all thirteen Reflect methods. `округлить` and
  `гипотенуза` route through small JS shims (`__ypsRound`/`__ypsHypot`)
  so half-value rounding and large-magnitude sums match the interpreter's
  Rust semantics exactly instead of silently diverging from bare
  `Math.round`/`Math.hypot`. Referencing an unknown member on one of
  these namespaces now reports that the member is unsupported, distinct
  from the "namespace not supported" diagnostic used for `Карта` and
  friends.

## [1.12.0] - 2026-08-11

### Added

- **VS Code debugging** — the extension now contributes a `yoptascript`
  debugger backed by the `yps-dap` adapter: press F5 on a `.yopta` file
  (no `launch.json` needed) for breakpoints, step over/in/out, pause,
  call stack, locals and `сказать` output in the Debug Console. The
  adapter binary is resolved via the new `yoptascript.dap.path` setting,
  then the vsix-bundled `bin/<platform>-<arch>/yps-dap` (packaged by
  `npm run package:local` alongside `yps-lsp`), then `PATH`.

### Fixed

- **DAP protocol stream safety** — debuggee `сказать`/`сказать.*` output
  is captured through the interpreter's `OutputSink` and forwarded as DAP
  `output` events instead of interleaving with the Content-Length-framed
  protocol on the adapter's stdout, and `прочестьСтроку`/`прочестьВсё`
  now raise a catchable runtime error under the debugger (new additive
  `Interpreter::block_stdin`) instead of stealing protocol bytes from
  stdin. Both guarantees extend to imported modules: the sub-interpreter
  spawned by `спиздить` now inherits the output sink and the stdin block
  (this also lets the WASM playground capture module output).
- **Per-file breakpoint honesty in `yps-dap`** — `setBreakpoints` for a
  file other than the launched program now answers with unverified
  breakpoints instead of silently re-resolving the lines against the
  wrong source and clobbering the real breakpoint set.

## [1.11.1] - 2026-08-11

### Added

- **Browser playground on GitHub Pages** — the WASM playground is now
  deployed to <https://ixxydev.github.io/yoptascript-rs/> by a new
  `pages.yml` workflow on every `master` push touching `crates/**`.
  The UI got a RU/EN language toggle: chrome strings and example
  titles are bilingual, the choice persists in `localStorage` and
  defaults to the browser language.
- **Execution step budget** — `Interpreter::set_step_limit` (counts
  statements) and `Vm::set_step_limit` / `run_to_string_with_limit`
  (counts instructions) let embedders bound runaway programs. Used by
  the differential fuzzer so infinite loops no longer kill the weekly
  job via libFuzzer timeouts.

### Fixed

- **Unary `~` follows JS coercion** — `~значение` now applies ToNumber
  to non-numbers in the tree-walking interpreter (`~";"` is `-1`)
  instead of raising, and both backends return a BigInt for `~бигцелое`
  (`~5n` is `-6n`; the VM previously produced the number `-1`).
- **Labels are validated at parse time** — `харэ`/`двигай` with an
  undefined label, `двигай` targeting a non-loop label, and bare
  `харэ`/`двигай` outside any loop are now parser diagnostics matching
  the JS early SyntaxError, instead of diverging between backends when
  the statement sat in dead code. Labels are scoped per function and
  do not leak into nested functions.
- **`ясенХуй` follows JS `const` semantics** — const-ness is resolved
  in the scope where the binding lives, so a `гыы` binding shadowing a
  const (including builtins like `строка`) is assignable again, and
  redeclaring a name in the same scope clears the stale const flag.
  Property and index writes through a const root
  (`ясенХуй о = {}; о.х = 1;`) and mutating builtin methods on a const
  receiver are allowed: const forbids rebinding, not mutation.

## [1.11.0] - 2026-08-03

### Fixed

- **Real `await` suspension** — async functions now run synchronously up
  to (and suspend at) each `await`, resuming later via the microtask
  queue, instead of blocking the whole call by eagerly draining the
  task queue. Both the tree-walking interpreter and the bytecode VM
  implement this consistently, including inside `for`/`do-while`
  conditions and updates, `switch` case bodies, and `for-await-of`
  loop bodies.
- **Temporal dead zone (TDZ)** — reading a block-scoped `гыы`/`ясенХуй`
  binding before its declaration now throws a catchable
  `ReferenceError`, matching `let`, instead of silently reading an
  outer-scope value of the same name. Enforced consistently in plain
  functions, arrow functions, async functions, and after an `await`
  suspension point.
- `Хуйня.разобратьЦелое(s, 0)` (`parseInt` with radix `0`) now treats
  the radix as unspecified (defaulting to base 10, or base 16 for a
  `0x`/`0X` prefix) instead of always returning `NaN`.
- `Дата.разобрать(...)` (`Date.parse`) on a date-time string with no
  `Z`/offset now parses in the machine's real local time zone;
  date-only strings continue to default to UTC.
- `TypedArray` equality (`==`) is now reference identity, matching
  every other reference-like value in the language, instead of
  comparing buffer/offset/length/kind structurally.
- `yps-lint`'s unused-variable rule no longer treats a discarded
  compound assignment (`х += 2;`) or postfix `х++;`/`х--;` as a "use"
  of the variable when the result is never read.

### Internal

- Workspace-wide test-suite audit: removed or strengthened dozens of
  tests across every crate that were tautological, duplicated a
  stronger sibling, or didn't actually exercise what their name
  claimed.

## [1.10.0] - 2026-07-31

### Added

- **JS transpiler** — new `yps-jsgen` crate and `yps transpile <file>
  [-o out.js]` subcommand. Full statement/expression syntax transpiles
  1:1 to modern JS; the free builtins (`сказать`, `длина`, `число`,
  timers, ...) map to native JS or a small `__yps*` prelude shim
  emitted only when used. Stdlib namespace globals (`Матан`,
  `Помойка`, `Карта`, typed arrays, ...) and Russian instance-method
  names are deliberately out of scope — referencing one is a
  compile-time diagnostic with a span, never silently-wrong JS.
- **Debug Adapter Protocol server** — new `yps-dap` crate/binary
  speaking DAP over stdio: breakpoints, step over/in/out, call-stack
  and variable inspection, driven through an additive debug hook on
  the interpreter (zero behavior change when unused). Statement-level
  stepping; VS Code editor wiring is a separate, deferred step.
- **WASM playground** — new `yps-wasm` crate (`run_yopta`) plus a
  static browser page (`crates/yps-wasm/www/`, plain ES modules) to
  try the language without installing the Rust toolchain. Output
  capture uses an additive output-sink hook on the interpreter, same
  zero-cost-when-unused design as the debug hook. Live hosting is a
  separate, deferred decision.
- **Binary releases** — tag push (`v*.*.*`) now builds `yps`/`yps-lsp`
  for linux/macos/windows via `.github/workflows/release.yml` and
  publishes them to a GitHub Release with checksums.

### Changed

- `yps-lexer` and `yps-parser` are now publishable to crates.io
  (`publish = true` + metadata); every other crate stays unpublished.

## [1.9.0] - 2026-07-28

### Added

- **Linter** — new `yps-lint` crate and `yps lint <file>` subcommand
  (exit code 1 on findings). Three rules: unused variables and
  parameters (ESLint-style after-used semantics; closures, template
  interpolation, destructuring and `_`-prefixed names are respected),
  unreachable code after `отвечаю`/`кидай`/`харэ`/`двигай` (function
  declarations are hoisted and stay reachable), and declarations that
  shadow an outer binding (builtins are not treated as outer scope).
- **Language server: navigation and highlighting** — find references
  (built on the rename resolver), semantic tokens with UTF-16-correct
  delta encoding for Cyrillic identifiers, and signature help for both
  user-defined functions and builtins (active parameter tracks nested
  calls).
- **Language server: lint integration** — lint findings are published as
  diagnostics (source `yps-lint`) with quick fixes: rename an unused
  binding to its `_`-prefixed form across all occurrences, or delete an
  unreachable statement.
- **CLI introspection** — `yps ast <file>` dumps the parse tree and
  `yps disasm <file>` prints the disassembled VM bytecode, including
  nested function prototypes.
- **REPL quality of life** — history persists across sessions
  (`$YPS_HISTORY_FILE` or `~/.yps_history`) and Tab completes keywords,
  builtins and identifiers declared in the current session.
- **VS Code extension 1.9.0** — the LSP binary is resolved from the
  `yoptascript.server.path` setting, then from a binary bundled inside
  the extension, then from `PATH`; `npm run package:local` builds
  `yps-lsp` and packages a platform vsix with the binary included.

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
