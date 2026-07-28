# YoptaScript for VS Code

[![VS Code Marketplace](https://img.shields.io/visual-studio-marketplace/v/ixxydev.yoptascript?label=VS%20Code%20Marketplace&color=007ACC)](https://marketplace.visualstudio.com/items?itemName=IxxyDev.yoptascript)
[![Installs](https://img.shields.io/visual-studio-marketplace/i/ixxydev.yoptascript)](https://marketplace.visualstudio.com/items?itemName=IxxyDev.yoptascript)

Language support for [YoptaScript](https://github.com/IxxyDev/yoptascript-rs) — an
esoteric language with Russian slang keywords, implemented in Rust.

## Installation

Install from the [VS Code Marketplace](https://marketplace.visualstudio.com/items?itemName=IxxyDev.yoptascript):
search for "YoptaScript" in the Extensions view, or run

```bash
code --install-extension ixxydev.yoptascript
```

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

The extension is a thin client for the `yps-lsp` language server. On activation it looks for
a server binary in this order:

1. **`yoptascript.server.path` setting**, if set — an explicit path you give it.
2. **Bundled binary** — a `yps-lsp` executable shipped inside the extension itself, at
   `bin/<platform>-<arch>/yps-lsp` (e.g. `bin/darwin-arm64/yps-lsp`, `bin/win32-x64/yps-lsp.exe`).
   Marketplace builds packaged with `npm run package:local` include this; a bare `npm run package`
   does not.
3. **`PATH`** — a `yps-lsp` (or `yps-lsp.exe` on Windows) executable found on your system `PATH`.

If none of these resolve to a real file, the extension shows an error message instead of
starting (no crash, no restart loop) and tells you how to fix it:

```bash
cargo build --release -p yps-lsp
```

Then either put the binary on `PATH`, or point the extension at it directly:

```jsonc
{
  "yoptascript.server.path": "/path/to/yoptascript-rs/target/release/yps-lsp"
}
```

## Settings

| Setting | Default | Description |
| --- | --- | --- |
| `yoptascript.server.path` | `` (empty) | Explicit path to the `yps-lsp` executable. Leave empty to use the bundled binary (if present) or search `PATH`. |
| `yoptascript.trace.server` | `off` | Trace LSP traffic (`off` / `messages` / `verbose`). |

## Building the extension

```bash
cd editors/vscode
npm ci
npm run compile      # bundle src/extension.ts -> dist/extension.js with esbuild
npm test             # tokenization + server-resolution tests
npm run package      # produce a .vsix without a bundled server (requires @vscode/vsce)
npm run package:local # cargo build the yps-lsp release binary, bundle it under bin/, then package a .vsix
```

## License

MIT OR Apache-2.0
