# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Communication

Always respond in Russian. Never write comments in code.

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
cargo clippy --workspace --all-targets --all-features -D warnings  # Lint (matches CI)
cargo fmt --all --check  # Format check (matches CI)
```

Run a single test: `cargo test -p yps-interpreter test_name`

Run the CLI: `cargo run -p yps-cli -- examples/hello.yop`

Run the REPL: `cargo run -p yps-cli` or `cargo run -p yps-cli -- repl`

Format a `.yop` file: `cargo run -p yps-cli -- fmt examples/hello.yop [--write|-w] [--check]`

## Architecture

Cargo workspace with five crates:

- **yps-lexer** (`crates/yps-lexer/`) — Tokenizer. Handles UTF-8 Russian keywords, produces `Token` (with `TokenKind` + `Span`), and emits `Diagnostic` for errors. Entry point: `Lexer::new(&source).tokenize()` → `(Vec<Token>, Vec<Diagnostic>)`.

- **yps-parser** (`crates/yps-parser/`) — Recursive descent parser with Pratt parsing for expression precedence. Converts tokens into AST (`Program` → `Vec<Stmt>` → `Expr`). Entry point: `Parser::new(&tokens, &source).parse_program()` → `(Program, Vec<Diagnostic>)`.

- **yps-interpreter** (`crates/yps-interpreter/`) — Tree-walking interpreter. Evaluates AST with `Environment` (scope stack + const tracking). Entry point: `Interpreter::new().run(&program)` → `Result<(), RuntimeError>`. Has 6 builtins: `сказать` (print), `длина` (length), `тип` (typeof), `число` (to number), `строка` (to string), `втолкнуть` (array push).

- **yps-fmt** (`crates/yps-fmt/`) — AST-based source formatter (zero external deps). Entry point: `format_source(&source)` → `Result<FormatOutcome, FormatError>`. Pretty-prints the `Program` with canonical style, restores parentheses from the precedence table exported by `yps-parser` (`binary_precedence` / `UNARY_PRECEDENCE` / `TERNARY_PRECEDENCE` / `binary_is_right_assoc`), and guards correctness with a round-trip self-check (`parse(fmt(x)) ≡ parse(x)` via `normalize`). Comments are preserved via the lexer's additive `tokenize_with_trivia()` plus an attach pass (`comments.rs`); an unrecognized comment position (dangling) yields `FormatError::CommentRefused` rather than silent loss.

- **yps-cli** (`crates/yps-cli/`) — CLI that chains lex → parse → interpret on `.yop` files; also exposes the `fmt` subcommand (`yps fmt <file> [--write|-w] [--check]`) backed by `yps-fmt`.

## Language Keywords Mapping

| Keyword | Meaning |
|---------|---------|
| `гыы` / `ясенХуй` | variable declaration |
| `участковый` | constant declaration |
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

- **Dynamic typing** with 8 value variants: Number (f64), String, Boolean, Array, Object, Function, BuiltinFunction, Null.
- **Diagnostic messages are in Russian** to match the language theme.
- **Constant enforcement**: `Environment` tracks consts in a `HashSet<String>`, mutations are prevented at runtime.
- **Complex assignment paths**: interpreter handles nested structures like `arr[0].prop = x` via path collection.
- **Short-circuit evaluation** for `&&` and `||`.
- Tests are inline (`#[cfg(test)] mod tests`) within source files, not in separate test directories.

## CI

GitHub Actions (`.github/workflows/ci.yml`): fmt check → clippy → tests → cargo-deny audit. Coverage runs on PRs only.
