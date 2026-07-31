# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Communication

Always respond in Russian. Never write comments in code.

Git commit messages and pull request titles/descriptions MUST be in English. Russian-keyword language identifiers (e.g. `йопта`, `Косяк`, `хапнуть`) may appear inline when they name code, but the prose around them is English. Phase numbers (e.g. "Phase 15", "15B") are internal planning labels — never put them in commit messages or PR titles.

## Project Overview

YoptaScript-rs is a Rust implementation of YoptaScript — an esoteric programming language with Russian slang keywords. The project implements a full pipeline: lexer → parser → tree-walking interpreter.

## Build & Test Commands

```bash
cargo build              # Build all crates
cargo test               # Run all tests
cargo test -p yps-lexer  # Run lexer tests only
cargo test -p yps-parser # Run parser tests only
cargo test -p yps-interpreter # Run interpreter tests only
cargo test -p yps-fmt  # Run formatter tests only
cargo test -p yps-jsgen  # Run JS transpiler tests only
cargo test -p yps-dap  # Run DAP debug-adapter tests only
cargo test -p yps-wasm  # Run WASM playground bindings tests (native target)
cargo clippy --workspace --all-targets --all-features -- -D warnings  # Lint (matches CI)
cargo fmt --all --check  # Format check (matches CI)
```

Run a single test: `cargo test -p yps-interpreter test_name`

Run the CLI: `cargo run -p yps-cli -- examples/hello.yopta`

Run the REPL: `cargo run -p yps-cli` or `cargo run -p yps-cli -- repl`

Format a `.yopta` file: `cargo run -p yps-cli -- fmt examples/hello.yopta [--write|-w] [--check]`

Transpile a `.yopta` file to JS: `cargo run -p yps-cli -- transpile examples/hello.yopta [-o out.js]`

Run the debug adapter (speaks DAP over stdin/stdout, driven by a DAP client, not by hand): `cargo run -p yps-dap`

## Architecture

Cargo workspace with eight crates:

- **yps-lexer** (`crates/yps-lexer/`) — Tokenizer. Handles UTF-8 Russian keywords, produces `Token` (with `TokenKind` + `Span`), and emits `Diagnostic` for errors. Entry point: `Lexer::new(&source).tokenize()` → `(Vec<Token>, Vec<Diagnostic>)`.

- **yps-parser** (`crates/yps-parser/`) — Recursive descent parser with Pratt parsing for expression precedence. Converts tokens into AST (`Program` → `Vec<Stmt>` → `Expr`). Entry point: `Parser::new(&tokens, &source).parse_program()` → `(Program, Vec<Diagnostic>)`.

- **yps-interpreter** (`crates/yps-interpreter/`) — Tree-walking interpreter. Evaluates AST with `Environment` (scope stack + const tracking). Entry point: `Interpreter::new().run(&program)` → `Result<(), RuntimeError>`. Builtins are listed in `builtin_names()` in `builtins.rs` (28 names as of now: `сказать` and its `сказать.*` console family, `длина`, `тип`, `число`, `строка`, `втолкнуть`, `БигЦелое`, `Косяк`, `RegExp`, `Дата`, timers `чутка`/`интервал`/`сразу`, `подождать`, `прочестьСтроку` etc.) — treat that function as the source of truth, not this file.

- **yps-fmt** (`crates/yps-fmt/`) — AST-based source formatter (no direct external deps; transitively pulls `stacker` via `yps-parser`). Entry point: `format_source(&source)` → `Result<FormatOutcome, FormatError>`. Pretty-prints the `Program` with canonical style, restores parentheses from the precedence table exported by `yps-parser` (`binary_precedence` / `UNARY_PRECEDENCE` / `TERNARY_PRECEDENCE` / `binary_is_right_assoc`), and guards correctness with a round-trip self-check (`parse(fmt(x)) ≡ parse(x)` via `normalize`). Comments are preserved via the lexer's additive `tokenize_with_trivia()` plus an attach pass (`comments.rs`); an unrecognized comment position (dangling) yields `FormatError::CommentRefused` rather than silent loss.

- **yps-jsgen** (`crates/yps-jsgen/`) — AST → JavaScript transpiler. Entry point: `transpile(&program)` → `Result<String, TranspileError>`. Prints every `Stmt`/`Expr` as the equivalent modern-JS construct (`|>` is desugared to a direct call; `базарпо`/switch is lowered to an if/else-if chain over a hidden temp, not a JS `switch`, so `харэ` inside a case still breaks the enclosing loop instead of the switch), rewrites the free builtins (`сказать` → `console.log`, `число` → `Number`, timers → `setTimeout`/…) and emits `__yps*` prelude shims only for the helpers actually used. Deliberately NOT full stdlib parity: referencing a stdlib namespace global (`Матан`, `Помойка`, `Карта`, typed arrays, …) is a `TranspileError` with a span rather than silently broken JS; Russian instance-method names are left untouched. Shadowing is a best-effort whole-program name collection (`scope.rs`), not real lexical resolution.

- **yps-dap** (`crates/yps-dap/`) — Debug Adapter Protocol server over stdio (`yps-dap` binary; entry point `serve(input, &mut output)`). Drives the tree-walking interpreter through the additive `DebugHook` exported by `yps-interpreter` (`set_debug_hook` / `set_debug_resume` / `debug_call_stack` / `debug_visible_locals`): the interpreter runs on its own thread, the protocol loop on the main one, connected by two `mpsc` channels. Supports initialize/launch/setBreakpoints/configurationDone/threads/stackTrace/scopes/variables/continue/next/stepIn/stepOut/pause/disconnect/terminate. Breakpoint lines resolve to the first statement line at or after the request (`breakpoints::resolve_line`); step granularity is per-statement, driven by `call_stack` depth comparisons. Scope boundary and known limitations (stdout passthrough, only the innermost frame has locals, flat variable rendering, VS Code wiring deferred) are in the crate doc comment.

- **yps-wasm** (`crates/yps-wasm/`) — cdylib bindings for the browser playground. Exports one wasm-bindgen function `run_yopta(code) -> String` that lexes → parses → interprets and returns everything `сказать` printed, or throws a yps-cli-style Russian diagnostic (`песочница:строка:колонка: …`). Output capture uses the additive `OutputSink` trait exported by `yps-interpreter` (`set_output_sink`; `None` by default, so every other caller keeps writing to real stdout/stderr — only `сказать`/`сказать.*` are routed). Static page in `crates/yps-wasm/www/` (plain ES modules, no bundler): build with `cd crates/yps-wasm && npx --yes wasm-pack build --target web --out-dir www/pkg`, serve with `python3 -m http.server 8000 --directory crates/yps-wasm/www`. `www/pkg/` is a build artifact and is gitignored. Browser limits (ФС/Сеть surface as catchable runtime errors; system clock, timers and `сказать.время` are unsupported and abort the call) are in the crate README and doc comment. Live deployment is a deliberately deferred, separate decision.

- **yps-cli** (`crates/yps-cli/`) — CLI that chains lex → parse → interpret on `.yopta` files; also exposes the `fmt` subcommand (`yps fmt <file> [--write|-w] [--check]`) backed by `yps-fmt` and the `transpile` subcommand (`yps transpile <file> [-o out.js]`) backed by `yps-jsgen`.

## Language Keywords Mapping

| Keyword | Meaning |
|---------|---------|
| `гыы` / `участковый` | variable declaration |
| `ясенХуй` | constant declaration |
| `вилкойвглаз` / `иливжопураз` | if / else |
| `потрещим` | while |
| `го` | for |
| `харэ` / `двигай` | break / continue |
| `йопта` | function declaration |
| `отвечаю` | return |
| `правда` / `лож` / `ноль` | true / false / null |
| `хапнуть` / `побратски` | try |
| `гоп` / `аченетак` | catch |
| `тюряжка` | finally |
| `кидай` | throw |

## Key Design Decisions

- **Dynamic typing**: `enum Value` in `value.rs` is the source of truth (32 variants as of now). User-facing values — Number (f64), BigInt, String, Boolean, Array, Object, Map, Set, Function, BuiltinFunction, Class, Symbol, Promise, Iterator, RegExp, Date, ArrayBuffer, TypedArray, DataView, Proxy, Undefined, Null — plus internal runtime continuations (the `Promise*`/`Abort*` handler variants).
- **Diagnostic messages are in Russian** to match the language theme.
- **Runtime limits** keep hostile `.yopta` input from crashing the process; exceeding them raises a catchable runtime error or parser diagnostic: `MAX_CALL_DEPTH=1000` (recursion), `MAX_PARSE_DEPTH=200` and `MAX_CHAIN_LEN=10000` (parser nesting/chains), `MAX_JSON_DEPTH=128`, `MAX_ITERATOR_DEPTH=200` (adapter chains), `MAX_STRING_LEN=50MB` (`repeat`/`pad*`). Deep recursion in the parser and interpreter grows the native stack via `stacker::maybe_grow`.
- **Constant enforcement**: `Environment` tracks consts in a `HashSet<String>`, mutations are prevented at runtime.
- **Complex assignment paths**: interpreter handles nested structures like `arr[0].prop = x` via path collection.
- **Short-circuit evaluation** for `&&` and `||`.
- Tests are inline (`#[cfg(test)] mod tests`) within source files, not in separate test directories.

## CI

GitHub Actions (`.github/workflows/ci.yml`): fmt check → clippy → tests → cargo-deny audit. Coverage runs on PRs only.
