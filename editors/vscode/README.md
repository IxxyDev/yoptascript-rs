# YoptaScript for VS Code

Language support for [YoptaScript](https://github.com/IxxyDev/yoptascript-rs) — an
esoteric language with Russian slang keywords, implemented in Rust.

## Features

- **Syntax highlighting** for `.yopta` files via a TextMate grammar (keywords, strings,
  template literals, numbers, comments, constants and operators).
- **Language server features** backed by `yps-lsp`:
  - live diagnostics (lexer + parser errors)
  - hover docs for keywords
  - completion (keywords, builtins and declarations from the current file)
  - document outline / breadcrumbs (`textDocument/documentSymbol`)
  - formatting (`textDocument/formatting`, powered by `yps-fmt`)
  - go-to-definition (`textDocument/definition`)

## Requirements

The extension is a thin client; it needs the `yps-lsp` language server on your machine.

Build it from the YoptaScript repository:

```bash
cargo build --release -p yps-lsp
```

Then point the extension at the binary (or put it on your `PATH`):

```jsonc
{
  // absolute path to target/release/yps-lsp, or just "yps-lsp" if it is on PATH
  "yoptascript.server.path": "/path/to/yoptascript-rs/target/release/yps-lsp"
}
```

## Settings

| Setting | Default | Description |
| --- | --- | --- |
| `yoptascript.server.path` | `yps-lsp` | Path to the `yps-lsp` executable. |
| `yoptascript.trace.server` | `off` | Trace LSP traffic (`off` / `messages` / `verbose`). |

## Building the extension

```bash
cd editors/vscode
npm ci
npm run compile      # bundle src/extension.ts -> dist/extension.js with esbuild
npm test             # tokenization tests for the TextMate grammar
npm run package      # produce a .vsix (requires @vscode/vsce)
```

## License

MIT OR Apache-2.0
