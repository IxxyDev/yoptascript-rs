import { test } from "node:test";
import assert from "node:assert/strict";
import { join, delimiter } from "node:path";
import {
  resolveServerPath,
  bundledBinaryFileName,
  bundledBinaryRelativePath
} from "../src/serverResolver.ts";

test("bundledBinaryFileName appends .exe only on win32", () => {
  assert.equal(bundledBinaryFileName("win32"), "yps-lsp.exe");
  assert.equal(bundledBinaryFileName("darwin"), "yps-lsp");
  assert.equal(bundledBinaryFileName("linux"), "yps-lsp");
});

test("bundledBinaryRelativePath maps platform-arch to bin/<platform>-<arch>/<binary>", () => {
  assert.equal(
    bundledBinaryRelativePath("darwin", "arm64"),
    join("bin", "darwin-arm64", "yps-lsp")
  );
  assert.equal(bundledBinaryRelativePath("win32", "x64"), "bin\\win32-x64\\yps-lsp.exe");
  assert.equal(
    bundledBinaryRelativePath("linux", "x64"),
    join("bin", "linux-x64", "yps-lsp")
  );
});

test("resolveServerPath prefers the configured path when set", () => {
  const result = resolveServerPath({
    extensionPath: "/ext",
    configuredPath: "  /custom/yps-lsp  ",
    platform: "darwin",
    arch: "arm64",
    pathEnv: "/usr/bin",
    fileExists: () => true
  });
  assert.deepEqual(result, { path: "/custom/yps-lsp", source: "config" });
});

test("resolveServerPath falls back to the bundled binary when no config is set", () => {
  const bundled = join("/ext", "bin", "darwin-arm64", "yps-lsp");
  const result = resolveServerPath({
    extensionPath: "/ext",
    configuredPath: undefined,
    platform: "darwin",
    arch: "arm64",
    pathEnv: "/usr/bin",
    fileExists: (p) => p === bundled
  });
  assert.deepEqual(result, { path: bundled, source: "bundled" });
});

test("resolveServerPath falls back to PATH when the config is blank and no bundled binary exists", () => {
  const pathDir1 = "/usr/local/bin";
  const pathDir2 = "/usr/bin";
  const expected = join(pathDir2, "yps-lsp");
  const result = resolveServerPath({
    extensionPath: "/ext",
    configuredPath: "   ",
    platform: "darwin",
    arch: "arm64",
    pathEnv: [pathDir1, pathDir2].join(delimiter),
    fileExists: (p) => p === expected
  });
  assert.deepEqual(result, { path: expected, source: "path" });
});

test("resolveServerPath looks for the .exe suffix on PATH when on win32", () => {
  const dir = "C:\\tools";
  const expected = `${dir}\\yps-lsp.exe`;
  const result = resolveServerPath({
    extensionPath: "C:\\ext",
    configuredPath: undefined,
    platform: "win32",
    arch: "x64",
    pathEnv: dir,
    fileExists: (p) => p === expected
  });
  assert.deepEqual(result, { path: expected, source: "path" });
});

test("resolveServerPath returns undefined when nothing is found", () => {
  const result = resolveServerPath({
    extensionPath: "/ext",
    configuredPath: undefined,
    platform: "linux",
    arch: "x64",
    pathEnv: "/usr/bin",
    fileExists: () => false
  });
  assert.equal(result, undefined);
});

test("resolveServerPath returns undefined when PATH is unset and nothing else matches", () => {
  const result = resolveServerPath({
    extensionPath: "/ext",
    configuredPath: undefined,
    platform: "linux",
    arch: "x64",
    pathEnv: undefined,
    fileExists: () => false
  });
  assert.equal(result, undefined);
});
