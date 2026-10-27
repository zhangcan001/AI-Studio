// @vitest-environment node
import { expect, it } from 'vitest';
import { readFileSync, writeFileSync, copyFileSync, mkdirSync, mkdtempSync, rmSync } from 'node:fs';
import { join, dirname, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { execFileSync } from 'node:child_process';
import { successorChainParentReader } from '../successor-chain-successor-guard.mjs';
import { PACKAGE_IMPORT_LOCK_BASE, PACKAGE_IMPORT_LOCK_PROOF } from '../package-import-lock-successor-guard.mjs';
import { SHOT_SAVE_ATOMIC_PROOF as PARENT_PROOF } from '../shot-save-atomic-successor-guard.mjs';

const source = 'src-tauri/src/application/production_package_service.rs';

it('serializes create_batches with an in-process import gate and adds no migration', () => {
  const src = readFileSync(source, 'utf8');
  expect(src).toContain('import_gate');
  expect(src).toContain('let _import_guard = self.import_gate.lock().await');
  expect(successorChainParentReader('.').violations).toEqual([]);
}, 120000);

it('preserves the frozen shot-save-atomic proof bytes', () => {
  const checked = successorChainParentReader('.');
  expect(checked.violations).toEqual([]);
  expect(checked.read(source)).toBe(execFileSync('git', ['show', `${PACKAGE_IMPORT_LOCK_BASE}:${source}`], { encoding: 'utf8' }).replaceAll('\r\n', '\n'));
  expect(readFileSync(PARENT_PROOF)).toEqual(execFileSync('git', ['show', `${PACKAGE_IMPORT_LOCK_BASE}:${PARENT_PROOF}`]));
}, 120000);

it('rejects forged proof identity/hash, source drift, migrations and undeclared additions', () => {
  const proof = JSON.parse(readFileSync(PACKAGE_IMPORT_LOCK_PROOF, 'utf8'));
  for (const override of [
    { [PACKAGE_IMPORT_LOCK_PROOF]: JSON.stringify({ ...proof, parentHead: 'forged' }) },
    { [PACKAGE_IMPORT_LOCK_PROOF]: JSON.stringify({ ...proof, contracts: { ...proof.contracts, guardsRelaxed: true } }) },
    { [PACKAGE_IMPORT_LOCK_PROOF]: JSON.stringify({ ...proof, files: { ...proof.files, [source]: { ...proof.files[source], afterHash: '0'.repeat(64) } } }) },
    { [source]: readFileSync(source, 'utf8') + '\n// forged' },
    { 'src/features/create/StoryboardWorkbench.tsx': 'changed frontend' },
    { 'src/unapproved-import-lock.ts': 'unapproved' },
    { 'src-tauri/migrations/051_unapproved.sql': 'SELECT 1;' },
  ]) expect(successorChainParentReader('.', override).violations.length).toBeGreaterThan(0);
  expect(successorChainParentReader('.').violations).toEqual([]);
}, 120000);

it('invalidates warm-cache fixtures on live mutations, deletion and illegal files; restoration passes', () => {
  const root = mkdtempSync(join(tmpdir(), 'package-import-lock-proof-'));
  try {
    const paths = execFileSync('git', ['ls-files', '--cached', '--others', '--exclude-standard'], { encoding: 'utf8' }).trim().split(/\r?\n/).filter(p => p === 'vite.config.ts' || /^(\.github\/workflows\/|src\/|src-tauri\/|scripts\/|docs\/architecture\/)/.test(p));
    for (const p of paths) { mkdirSync(dirname(join(root, p)), { recursive: true }); copyFileSync(p, join(root, p)); }
    execFileSync('git', ['init', '-q', root]);
    const objects = resolve(execFileSync('git', ['rev-parse', '--git-path', 'objects'], { encoding: 'utf8' }).trim());
    writeFileSync(join(root, '.git/objects/info/alternates'), objects.replaceAll('\\', '/') + '\n');
    const validate = () => successorChainParentReader(root).violations;
    expect(validate()).toEqual([]);
    for (const p of [PACKAGE_IMPORT_LOCK_PROOF, PARENT_PROOF, source]) {
      const file = join(root, p), original = readFileSync(file);
      try {
        writeFileSync(file, Buffer.concat([original, Buffer.from('\nmutation')])); expect(validate().length).toBeGreaterThan(0);
        rmSync(file); expect(validate().length).toBeGreaterThan(0);
      } finally { writeFileSync(file, original); }
      expect(validate()).toEqual([]);
    }
  } finally { rmSync(root, { recursive: true, force: true }); }
}, 120000);
