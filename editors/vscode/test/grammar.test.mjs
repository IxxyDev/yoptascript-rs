import { test } from "node:test";
import assert from "node:assert/strict";
import { createRequire } from "node:module";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const require = createRequire(import.meta.url);
const vsctm = require("vscode-textmate");
const oniguruma = require("vscode-oniguruma");

const here = dirname(fileURLToPath(import.meta.url));
const grammarPath = join(here, "..", "syntaxes", "yoptascript.tmLanguage.json");

async function loadGrammar() {
  const wasmPath = require.resolve("vscode-oniguruma/release/onig.wasm");
  const wasmBin = readFileSync(wasmPath).buffer;
  await oniguruma.loadWASM(wasmBin);
  const onigLib = Promise.resolve({
    createOnigScanner: (sources) => new oniguruma.OnigScanner(sources),
    createOnigString: (s) => new oniguruma.OnigString(s)
  });
  const registry = new vsctm.Registry({
    onigLib,
    loadGrammar: async (scopeName) => {
      if (scopeName === "source.yoptascript") {
        const content = readFileSync(grammarPath, "utf8");
        return vsctm.parseRawGrammar(content, grammarPath);
      }
      return null;
    }
  });
  const grammar = await registry.loadGrammar("source.yoptascript");
  assert.ok(grammar, "grammar should load");
  return grammar;
}

function scopesAt(tokens, index) {
  const tok = tokens.find((t) => t.startIndex <= index && index < t.endIndex);
  assert.ok(tok, `expected a token at index ${index}`);
  return tok.scopes;
}

test("grammar metadata", () => {
  const raw = JSON.parse(readFileSync(grammarPath, "utf8"));
  assert.equal(raw.scopeName, "source.yoptascript");
});

test("tokenizes keywords, control flow and strings", async () => {
  const grammar = await loadGrammar();
  const line = 'йопта фу() { вилкойвглаз (x) { отвечаю "привет"; } }';
  const { tokens } = grammar.tokenizeLine(line, vsctm.INITIAL);

  const fn = scopesAt(tokens, line.indexOf("йопта"));
  assert.ok(
    fn.some((s) => s.startsWith("storage.")),
    `expected storage scope for 'йопта', got ${fn.join(",")}`
  );

  const ctrl = scopesAt(tokens, line.indexOf("вилкойвглаз"));
  assert.ok(
    ctrl.includes("keyword.control.yoptascript"),
    `expected keyword.control for 'вилкойвглаз', got ${ctrl.join(",")}`
  );

  const ret = scopesAt(tokens, line.indexOf("отвечаю"));
  assert.ok(
    ret.includes("keyword.control.yoptascript"),
    `expected keyword.control for 'отвечаю', got ${ret.join(",")}`
  );

  const str = scopesAt(tokens, line.indexOf('"привет"') + 1);
  assert.ok(
    str.some((s) => s.startsWith("string.")),
    `expected string scope for the literal, got ${str.join(",")}`
  );
});

test("tokenizes constants and numbers", async () => {
  const grammar = await loadGrammar();
  const line = "участковый x = правда; гыы y = 0xFF; гыы z = 3.14;";
  const { tokens } = grammar.tokenizeLine(line, vsctm.INITIAL);

  const decl = scopesAt(tokens, line.indexOf("участковый"));
  assert.ok(decl.some((s) => s.startsWith("storage.")), `decl scopes: ${decl.join(",")}`);

  const bool = scopesAt(tokens, line.indexOf("правда"));
  assert.ok(
    bool.includes("constant.language.yoptascript"),
    `expected constant.language for 'правда', got ${bool.join(",")}`
  );

  const hex = scopesAt(tokens, line.indexOf("0xFF"));
  assert.ok(hex.some((s) => s.startsWith("constant.numeric")), `hex scopes: ${hex.join(",")}`);

  const dec = scopesAt(tokens, line.indexOf("3.14"));
  assert.ok(dec.some((s) => s.startsWith("constant.numeric")), `dec scopes: ${dec.join(",")}`);
});
