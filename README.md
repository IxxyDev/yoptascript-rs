# yoptascript-rs

A Rust implementation of [YoptaScript](https://github.com/samgozman/YoptaScript) — a Russian joke programming language whose keywords are slang words instead of standard JS tokens (`if` → `вилкойвглаз`, `function` → `йопта`, `return` → `отвечаю`, etc.).

While the original [samgozman/YoptaScript](https://github.com/samgozman/YoptaScript) (2.2k★) is a JS-based transpiler, **yoptascript-rs** is a from-scratch reimplementation in Rust with its own lexer, parser, AST and tree-walking interpreter — built as a hands-on exercise in language implementation and Rust workspace design.

> ⚠️ The language uses Russian slang/profanity for keywords. This is an engineering exercise, not the language itself; semantics mirror JavaScript.

## Why this exists

I was contributing to [Biome](https://github.com/biomejs/biome) (a Rust-based linter/formatter for JS/TS) and wanted deeper hands-on experience with the full compiler frontend pipeline — lexer, parser, AST design, error recovery — without the complexity of full ECMAScript. YoptaScript turned out to be a perfect playground:

- Real, non-trivial grammar (control flow, classes, async, generators)
- Existing reference implementation to validate against
- Multi-character keyword tokens that force interesting lexer design decisions

## Architecture

A Cargo workspace with four crates:

```
crates/
├── yps-lexer        # Tokenizer: source → token stream
├── yps-parser       # Recursive descent parser: tokens → AST
├── yps-interpreter  # Tree-walking interpreter: evaluates AST
└── yps-cli          # Command-line entry point
```

Pipeline: `source code → lexer → tokens → parser → AST → interpreter → result`

Built on Rust 2024 edition with `resolver = "3"`. Tooling: clippy, rustfmt, cargo-deny, pre-commit hooks, GitHub Actions CI, Justfile for task automation.

## Example

```
йопта приветствие(имя) {
    отвечаю "Привет, " + имя;
}

участковый сообщение = приветствие("мир");
сказать(сообщение);
```

Equivalent JavaScript:

```js
function приветствие(имя) {
    return "Привет, " + имя;
}

const сообщение = приветствие("мир");
console.log(сообщение);
```

See [`DICTIONARY.md`](DICTIONARY.md) for the full keyword mapping and [`examples/`](examples/) for runnable programs.

## Quick start

Toolchain is pinned to stable Rust via [`rust-toolchain.toml`](rust-toolchain.toml).

```bash
# Build
cargo build --release

# Run a YoptaScript file
cargo run -p yps-cli -- path/to/program.yop

# Or use the Justfile shortcuts
just run path/to/program.yop
just test
just lint
```

## Status

- [x] Lexer: full keyword set from `DICTIONARY.md`, multi-token aliases
- [x] Parser: expressions, control flow, functions, blocks
- [x] Interpreter: tree-walking evaluator
- [x] Classes, inheritance, modifiers
- [x] Async / Promises (`СловоПацана`)
- [x] Module system (`спиздить` / `предъява`)
- [x] Standard library: `Матан`, `Помойка`, `Строка`, `Кент`, `Хуйня`, `Жсон`, `Карта`, `Набор`, `Симбол`, `Косяк`

This is an active learning project — see open issues for what's next.

## Project layout

```
.
├── crates/             # Workspace members (lexer, parser, interpreter, cli)
├── examples/           # Sample .yop programs
├── docs/               # Language documentation
├── DICTIONARY.md       # Keyword mapping (JS ↔ YoptaScript)
├── Justfile            # Task runner
├── rust-toolchain.toml # Pinned toolchain
├── clippy.toml
├── rustfmt.toml
└── deny.toml           # cargo-deny config
```

## Acknowledgments

Original language design and dictionary by [@samgozman](https://github.com/samgozman) — see the [original YoptaScript repo](https://github.com/samgozman/YoptaScript). All credit for the language concept and vocabulary belongs there; this repository is an independent Rust reimplementation of the runtime.

## License

Dual-licensed under MIT or Apache-2.0, at your option.
