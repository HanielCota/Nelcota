#!/usr/bin/env node
// Size budget per entry point: the gzip size of every module an import pulls
// in (its static import closure), compiled without comments and before any
// minifier, which shrinks it further. The budget also keeps the dependency
// graph honest, e.g. that importing the REST client never drags auth along.

import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { gzipSync } from 'node:zlib';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const dist = mkdtempSync(join(tmpdir(), 'nelcota-sdk-size-'));
const tsc = spawnSync(
  process.execPath,
  [join(root, 'node_modules/typescript/bin/tsc'), '-p', join(root, 'tsconfig.build.json'), '--outDir', dist, '--removeComments', '--declaration', 'false', '--isolatedDeclarations', 'false'],
  { stdio: 'inherit' },
);
if (tsc.status !== 0) process.exit(tsc.status ?? 1);
process.on('exit', () => rmSync(dist, { recursive: true, force: true }));

const BUDGETS = {
  // Includes shared cancellation and explicit row/bucket contract validation.
  'index.js': 13_000,
  // Standalone factories include HTTP and keep the other feature modules out.
  'rest/index.js': 8_000,
  'auth/index.js': 7_500,
  'storage/index.js': 6_000,
};

function closure(entry, seen = new Set()) {
  if (seen.has(entry)) return seen;
  seen.add(entry);
  const code = readFileSync(entry, 'utf8');
  // Runtime imports only: `import type` is erased by tsc.
  for (const match of code.matchAll(/(?:^|\n)\s*(?:import|export)\s[^'"]*?from\s+['"](\.[^'"]+)['"]/g)) {
    closure(resolve(dirname(entry), match[1]), seen);
  }
  return seen;
}

let failed = false;
for (const [entry, budget] of Object.entries(BUDGETS)) {
  const files = [...closure(resolve(dist, entry))];
  const source = files.map((f) => readFileSync(f, 'utf8')).join('\n');
  const size = gzipSync(source, { level: 9 }).length;
  const ok = size <= budget;
  failed ||= !ok;
  const modules = files.map((f) => f.slice(dist.length + 1).replace(/\\/g, '/')).sort();
  console.log(`${ok ? 'ok  ' : 'OVER'} ${entry.padEnd(18)} ${String(size).padStart(6)} B gzip (budget ${budget})  ${modules.join(', ')}`);
}
if (failed) {
  console.error('\nA size budget was exceeded: trim the code or raise the budget on purpose.');
  process.exit(1);
}
