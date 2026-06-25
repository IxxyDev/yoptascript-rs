# yoptascript-rs

A Rust implementation of [YoptaScript](https://github.com/samgozman/YoptaScript) — a Russian joke programming language whose keywords are slang words instead of standard JS tokens (`if` → `вилкойвглаз`, `function` → `йопта`, `return` → `отвечаю`, etc.).

While the original [samgozman/YoptaScript](https://github.com/samgozman/YoptaScript) (2.2k★) is a JS-based transpiler, **yoptascript-rs** is a from-scratch reimplementation in Rust with its own lexer, parser, AST, tree-walking interpreter and bytecode VM — built as a hands-on exercise in language implementation and Rust workspace design.

The language surface tracks ES6–ES2026: closures, classes, generators, async/await, modules, destructuring, BigInt, RegExp, typed arrays, Map/Set/WeakMap, Proxy/Reflect, decorators and a русско-названная standard library (`Матан`, `Кент`, `Жсон`, …). Two backends run the same AST — a tree-walking interpreter and a stack bytecode VM (`yps --vm`) — and a conformance suite asserts they produce byte-for-byte identical output.

> ⚠️ The language uses Russian slang/profanity for keywords. This is an engineering exercise, not the language itself; semantics mirror JavaScript.

## Why this exists

I was contributing to [Biome](https://github.com/biomejs/biome) (a Rust-based linter/formatter for JS/TS) and wanted deeper hands-on experience with the full compiler frontend pipeline — lexer, parser, AST design, error recovery — without the complexity of full ECMAScript. YoptaScript turned out to be a perfect playground:

- Real, non-trivial grammar (control flow, classes, async, generators)
- Existing reference implementation to validate against
- Multi-character keyword tokens that force interesting lexer design decisions

## Architecture

A Cargo workspace with seven crates:

```
crates/
├── yps-lexer        # Tokenizer: source → token stream
├── yps-parser       # Recursive descent parser: tokens → AST
├── yps-interpreter  # Tree-walking interpreter: evaluates AST
├── yps-vm           # Bytecode compiler + stack VM (parity backend)
├── yps-fmt          # AST-based formatter with round-trip self-check
├── yps-lsp          # Language server (diagnostics, hover, completion, symbols, formatting, go-to-definition)
└── yps-cli          # Command-line entry point (run a file, --vm, repl, fmt)
```

Pipeline: `source code → lexer → tokens → parser → AST → interpreter` (or `→ bytecode → VM`) `→ result`

The formatter (`yps fmt`) pretty-prints a `.yop` file to canonical style. It restores parentheses from the same precedence table the parser uses and refuses to emit output unless `parse(fmt(x)) ≡ parse(x)` holds, so it can never silently change semantics or lose comments.

The language server (`yps-lsp`) speaks LSP over stdio and is ready to back an editor extension. It provides live diagnostics, hover docs for keywords, completion (keywords, builtins and declarations from the current file), a document outline (`textDocument/documentSymbol`), whole-document formatting via `yps-fmt` (`textDocument/formatting`) and go-to-definition for functions, classes, variables and parameters (`textDocument/definition`). All UTF-8 ↔ UTF-16 position mapping accounts for Cyrillic identifiers.

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

# Run it on the bytecode VM backend instead of the tree-walker
cargo run -p yps-cli -- --vm path/to/program.yop

# Start the REPL (line editing and up/down history via rustyline;
# the runtime's other deliberate dependencies are the regex engines —
# regex for plain patterns, fancy-regex for lookaround and backreferences)
cargo run -p yps-cli

# Format a .yop file (--write to apply, --check for CI)
cargo run -p yps-cli -- fmt path/to/program.yop

# Or use the Justfile shortcuts
just run path/to/program.yop
just test
just lint

# Fuzz the lexer/parser/formatter (requires nightly + cargo-fuzz)
just fuzz lexer
```

## Status

- [x] Lexer: full keyword set from `DICTIONARY.md`, multi-token aliases
- [x] Parser: expressions, control flow, functions, blocks
- [x] Interpreter: tree-walking evaluator
- [x] Bytecode VM: stack backend at full parity with the interpreter (`--vm`)
- [x] Classes, inheritance, modifiers
- [x] Async / Promises (`СловоПацана`)
- [x] Module system (`спиздить` / `предъява`)
- [x] Standard library: `Матан`, `Помойка`, `Строка`, `Кент`, `Хуйня`, `Жсон`, `Карта`, `Набор`, `Симбол`, `Косяк`
- [x] Weak collections: `СлабаяКарта`, `СлабыйНабор`, `СлабаяСсылка`, `РеестрФинализации`
- [x] Formatter (`yps fmt`) with round-trip self-check and comment preservation
- [x] Fuzzing: libFuzzer targets for lexer, parser and formatter round-trip (`fuzz/`, weekly CI job)
- [x] Conformance suite: golden cases checked against Node.js semantics, plus a VM/interpreter parity suite (`crates/yps-cli/tests/`)

This is an active learning project — see open issues for what's next.

## Conformance suite

A Test262-inspired golden battery lives in `crates/yps-cli/tests/conformance/` and runs as part of `cargo test -p yps-cli`. Every top-level `cases/*.yop` file is discovered automatically and its CLI output is compared against `golden/<name>.txt`.

```bash
# Run the battery
cargo test -p yps-cli --test conformance

# Run a subset (comma-separated case-name prefixes)
YPS_CONFORMANCE_FILTER=gen_,async_ cargo test -p yps-cli --test conformance

# Regenerate golden files after an intentional behavior change
YPS_CONFORMANCE_BLESS=1 cargo test -p yps-cli --test conformance

# Check golden files against live Node.js (developer-only oracle, not in CI)
node tools/gen-golden.js
```

Most cases have a hand-written Node.js mirror in `mirror/<name>.js`; `tools/gen-golden.js` runs each mirror and diffs its output against the golden file, so the suite tracks real ECMAScript semantics rather than freezing the interpreter's own behavior. Intentional differences are flagged with a `// DIVERGENCE:` header in the relevant mirror file.

## Project layout

```
.
├── crates/             # Workspace members (lexer, parser, interpreter, vm, fmt, lsp, cli)
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
