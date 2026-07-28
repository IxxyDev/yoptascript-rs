import { execFileSync } from "node:child_process";
import { copyFileSync, chmodSync, mkdirSync, existsSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import {
  bundledBinaryFileName,
  bundledBinaryRelativePath
} from "../src/serverResolver.ts";

const here = dirname(fileURLToPath(import.meta.url));
const extensionRoot = join(here, "..");
const repoRoot = join(extensionRoot, "..", "..");

console.log("[package:local] building yps-lsp (cargo build --release -p yps-lsp)...");
execFileSync("cargo", ["build", "--release", "-p", "yps-lsp"], {
  cwd: repoRoot,
  stdio: "inherit"
});

const builtBinary = join(
  repoRoot,
  "target",
  "release",
  process.platform === "win32" ? "yps-lsp.exe" : "yps-lsp"
);
if (!existsSync(builtBinary)) {
  console.error(`[package:local] expected built binary at ${builtBinary}, but it is missing`);
  process.exit(1);
}

const destRelative = bundledBinaryRelativePath(process.platform, process.arch);
const destPath = join(extensionRoot, destRelative);
mkdirSync(dirname(destPath), { recursive: true });
copyFileSync(builtBinary, destPath);
if (process.platform !== "win32") {
  chmodSync(destPath, 0o755);
}
console.log(`[package:local] copied ${builtBinary} -> ${destPath}`);
console.log(`[package:local] bundled binary name: ${bundledBinaryFileName(process.platform)}`);

console.log("[package:local] running vsce package...");
execFileSync("npx", ["vsce", "package", "--no-dependencies"], {
  cwd: extensionRoot,
  stdio: "inherit"
});

console.log("[package:local] done");
