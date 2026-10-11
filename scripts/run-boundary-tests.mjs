#!/usr/bin/env node
// Dedicated Boundary runner matching CI intent:
// - Expand src/app/*Boundary*.test.ts explicitly (Linux-safe).
// - Run ONE vitest process per file with maxWorkers=1 so a tinypool
//   ERR_IPC_CHANNEL_CLOSED teardown after a long chain cannot fail the job
//   after green assertions (Phase5J per-file worker pattern).
import { spawnSync } from 'node:child_process';
import { readdirSync } from 'node:fs';
import { join } from 'node:path';

const files = readdirSync('src/app')
  .filter((name) => /Boundary\.test\.tsx?$/.test(name))
  .map((name) => join('src/app', name))
  .sort();
if (!files.length) {
  console.error('No Boundary test files found under src/app');
  process.exit(1);
}

let failed = 0;
const failures = [];
for (const file of files) {
  console.log(`\n=== Boundary file ${file} ===`);
  const result = spawnSync(
    'pnpm',
    ['exec', 'vitest', 'run', '--maxWorkers=1', '--reporter=dot', file],
    { stdio: 'inherit', shell: process.platform === 'win32' },
  );
  const code = result.status ?? 1;
  if (code !== 0) {
    failed += 1;
    failures.push(`${file} (exit ${code})`);
  }
}

if (failed) {
  console.error(`\nBoundary job failed: ${failed}/${files.length} files`);
  for (const f of failures) console.error(' -', f);
  process.exit(1);
}
console.log(`\nBoundary job passed: ${files.length}/${files.length} files`);
process.exit(0);
