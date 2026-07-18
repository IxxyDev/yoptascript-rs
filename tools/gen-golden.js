#!/usr/bin/env node
'use strict';

// Офлайн-оракул для conformance-батареи YoptaScript-rs.
// НЕ запускается в CI. Разработчик запускает вручную:
//   node tools/gen-golden.js
//
// Для каждого mirror/*.js запускает Node, снимает stdout, нормализует CRLF→LF,
// сравнивает с golden/<name>.txt. Статусы:
//   OK         — совпадает
//   MISMATCH   — расходится (показывает diff)
//   DOCUMENTED — расходится, но задокументировано заголовком DIVERGENCE в .js
//   SKIP       — нет mirror-файла для cases/<name>.yopta
// Завершается с ненулевым кодом, если есть хотя бы один недокументированный MISMATCH.

const fs = require('fs');
const path = require('path');
const { execFileSync } = require('child_process');

const ROOT = path.resolve(__dirname, '..');
const CASES_DIR = path.join(ROOT, 'crates/yps-cli/tests/conformance/cases');
const GOLDEN_DIR = path.join(ROOT, 'crates/yps-cli/tests/conformance/golden');
const MIRROR_DIR = path.join(ROOT, 'crates/yps-cli/tests/conformance/mirror');
const NODE = process.execPath;

function readGolden(name) {
    const p = path.join(GOLDEN_DIR, `${name}.txt`);
    if (!fs.existsSync(p)) return null;
    return fs.readFileSync(p, 'utf8').replace(/\r\n/g, '\n');
}

function runMirror(mirrorPath) {
    const out = execFileSync(NODE, [mirrorPath], {
        cwd: MIRROR_DIR,
        encoding: 'utf8',
    });
    return out.replace(/\r\n/g, '\n');
}

function isDivergenceDeclared(mirrorPath) {
    const src = fs.readFileSync(mirrorPath, 'utf8');
    return /^\/\/ DIVERGENCE:/m.test(src);
}

function simpleDiff(expected, actual) {
    const eLines = expected.split('\n');
    const aLines = actual.split('\n');
    const maxLen = Math.max(eLines.length, aLines.length);
    const lines = [];
    for (let i = 0; i < maxLen; i++) {
        const e = eLines[i] ?? '(missing)';
        const a = aLines[i] ?? '(missing)';
        if (e !== a) {
            lines.push(`  line ${i + 1}:`);
            lines.push(`    expected (node): ${JSON.stringify(e)}`);
            lines.push(`    golden:           ${JSON.stringify(a)}`);
        }
    }
    return lines.join('\n');
}

// Collect all case names from cases/*.yopta (skip modules subdir entries)
const caseNames = fs.readdirSync(CASES_DIR)
    .filter(f => f.endsWith('.yopta'))
    .map(f => path.basename(f, '.yopta'))
    .sort();

let ok = 0, mismatch = 0, documented = 0, skip = 0;

for (const name of caseNames) {
    const mirrorPath = path.join(MIRROR_DIR, `${name}.js`);
    if (!fs.existsSync(mirrorPath)) {
        console.log(`SKIP       ${name}`);
        skip++;
        continue;
    }

    const golden = readGolden(name);
    let nodeOut;
    try {
        nodeOut = runMirror(mirrorPath);
    } catch (err) {
        console.log(`ERROR      ${name}: ${err.message}`);
        mismatch++;
        continue;
    }

    // Ensure both end with a single newline for fair comparison
    const normalizeTrailing = s => s.replace(/\n*$/, '\n');
    const goldenNorm = golden !== null ? normalizeTrailing(golden) : null;
    const nodeNorm = normalizeTrailing(nodeOut);

    if (goldenNorm === nodeNorm) {
        console.log(`OK         ${name}`);
        ok++;
    } else if (isDivergenceDeclared(mirrorPath)) {
        console.log(`DOCUMENTED ${name}`);
        documented++;
    } else {
        console.log(`MISMATCH   ${name}`);
        if (golden !== null) {
            console.log(simpleDiff(nodeNorm, goldenNorm));
        } else {
            console.log('  (no golden file)');
        }
        mismatch++;
    }
}

console.log('');
console.log(`Summary: ${ok} OK, ${mismatch} MISMATCH, ${documented} DOCUMENTED, ${skip} SKIP`);

if (mismatch > 0) process.exit(1);
