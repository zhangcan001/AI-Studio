#!/usr/bin/env node
// Dedicated Boundary runner matching CI intent:
// - Expand app + script Boundary files explicitly (Windows/Linux-safe).
// - Run ONE vitest process per file with maxWorkers=1 so a tinypool
//   ERR_IPC_CHANNEL_CLOSED teardown after a long chain cannot fail the job
//   after green assertions (Phase5J per-file worker pattern).
import { spawnSync } from 'node:child_process';
import { readdirSync } from 'node:fs';
const walk = dir => readdirSync(dir, { withFileTypes: true }).flatMap(entry => {
  const path = `${dir}/${entry.name}`;
  if (entry.isSymbolicLink()) throw Error(`Symbolic test path: ${path}`);
  return entry.isDirectory() ? walk(path) : [path];
});
const inventory = [
  ...walk('src/app').filter(path => /Boundary.*\.test\.tsx?$/.test(path)),
  ...walk('scripts/testing').filter(path => /[Bb]oundary.*\.test\.tsx?$/.test(path)
    || path === 'scripts/testing/immutable-git-cache.test.ts'),
].sort();
const args = process.argv.slice(2);
const shards = args.filter(arg => arg.startsWith('--shard='));
const shard = /^(\d+)\/(\d+)$/.exec(shards[0]?.slice('--shard='.length) ?? '1/1');
if (shards.length > 1 || args.some(arg => arg !== '--list' && !arg.startsWith('--shard='))
    || !shard || Number(shard[1]) < 1 || Number(shard[1]) > Number(shard[2])) {
  console.error('Usage: run-boundary-tests.mjs [--list] [--shard=index/count]');
  process.exit(1);
}
const files = inventory.filter((_, index) => index % Number(shard[2]) === Number(shard[1]) - 1);
if (!files.length) {
  console.error('No Boundary test files found for this shard');
  process.exit(1);
}
if (args.includes('--list')) {
  console.log(JSON.stringify(files));
  process.exit(0);
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
