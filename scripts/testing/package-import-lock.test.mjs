// @vitest-environment node
import { expect, it } from 'vitest';
import { readFileSync, writeFileSync, copyFileSync, mkdirSync, mkdtempSync, rmSync } from 'node:fs';
import { join, dirname, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { execFileSync } from 'node:child_process';
import { packageImportLockParentReader, PACKAGE_IMPORT_LOCK_BASE, PACKAGE_IMPORT_LOCK_PROOF, PACKAGE_IMPORT_LOCK_CHANGED, PACKAGE_IMPORT_LOCK_ADDED, PACKAGE_IMPORT_LOCK_CONTRACTS } from '../package-import-lock-successor-guard.mjs';
const PARENT_PROOF = 'docs/architecture/shot-save-atomic-successor.json';
const source = 'src-tauri/src/application/production_package_service.rs';
it('validates the standalone package-import-lock tip and its declared scope', () => {
  const checked = packageImportLockParentReader('.');
  expect(checked.violations).toEqual([]);
  expect(checked.changedPaths).toEqual(PACKAGE_IMPORT_LOCK_CHANGED);
  expect(checked.addedPaths).toEqual([...PACKAGE_IMPORT_LOCK_ADDED, PACKAGE_IMPORT_LOCK_PROOF]);
}, 120000);
it('projects changed files to the closed parent tip and preserves its proof bytes', () => {
  const checked = packageImportLockParentReader('.');
  expect(checked.violations).toEqual([]);
  for (const p of PACKAGE_IMPORT_LOCK_CHANGED) {
    expect(checked.read(p)).toBe(execFileSync('git', ['show', `${PACKAGE_IMPORT_LOCK_BASE}:${p}`], { encoding: 'utf8' }).replaceAll('\r\n', '\n'));
  }
  expect(readFileSync(PARENT_PROOF)).toEqual(execFileSync('git', ['show', `${PACKAGE_IMPORT_LOCK_BASE}:${PARENT_PROOF}`]));
}, 120000);
it('serializes create_batches with an in-process import gate and adds no migration', () => {
  const src = readFileSync(source, 'utf8');
  expect(src).toContain('import_gate');
  expect(src).toContain('let _import_guard = self.import_gate.lock().await');
  expect(src).toContain('already_created');
  expect(packageImportLockParentReader('.').violations).toEqual([]);
}, 120000);
it('rejects forged proof identity, contracts, scope, hashes and undeclared changes', () => {
  const proof = JSON.parse(readFileSync(PACKAGE_IMPORT_LOCK_PROOF, 'utf8'));
  for (const override of [
    ...['checkpoint', 'parentHead', 'planningParentHead'].map(key => ({ [PACKAGE_IMPORT_LOCK_PROOF]: JSON.stringify({ ...proof, [key]: 'forged' }) })),
    ...Object.entries(PACKAGE_IMPORT_LOCK_CONTRACTS).map(([key, value]) => ({ [PACKAGE_IMPORT_LOCK_PROOF]: JSON.stringify({ ...proof, contracts: { ...proof.contracts, [key]: !value } }) })),
    { [PACKAGE_IMPORT_LOCK_PROOF]: JSON.stringify({ ...proof, files: {} }) },
    ...['beforeHash', 'afterHash'].map(key => ({ [PACKAGE_IMPORT_LOCK_PROOF]: JSON.stringify({ ...proof, files: { ...proof.files, [source]: { ...proof.files[source], [key]: '0'.repeat(64) } } }) })),
    ...[...PACKAGE_IMPORT_LOCK_CHANGED, ...PACKAGE_IMPORT_LOCK_ADDED, PARENT_PROOF].map(p => ({ [p]: readFileSync(p, 'utf8') + '\n// forged' })),
    { 'src/features/create/StoryboardWorkbench.tsx': 'changed frontend' },
    { 'src-tauri/migrations/051_unapproved.sql': 'SELECT 1;' },
  ]) expect(packageImportLockParentReader('.', override).violations.length).toBeGreaterThan(0);
  expect(packageImportLockParentReader('.').violations).toEqual([]);
}, 120000);
it('invalidates warm-cache fixtures on mutation, deletion and ignored additions; restoration passes', () => {
  const root = mkdtempSync(join(tmpdir(), 'package-import-lock-proof-'));
  try {
    const paths = execFileSync('git', ['ls-files', '--cached', '--others', '--exclude-standard'], { encoding: 'utf8' }).trim().split(/\r?\n/).filter(p => p === 'vite.config.ts' || /^(\.github\/workflows\/|src\/|src-tauri\/|scripts\/|docs\/architecture\/)/.test(p));
    for (const p of paths) { mkdirSync(dirname(join(root, p)), { recursive: true }); copyFileSync(p, join(root, p)); }
    execFileSync('git', ['init', '-q', root]);
    const objects = resolve(execFileSync('git', ['rev-parse', '--git-path', 'objects'], { encoding: 'utf8' }).trim());
    writeFileSync(join(root, '.git/objects/info/alternates'), objects.replaceAll('\\', '/') + '\n');
    const validate = () => packageImportLockParentReader(root).violations;
    expect(validate()).toEqual([]);
    for (const p of [PACKAGE_IMPORT_LOCK_PROOF, PARENT_PROOF, source, ...PACKAGE_IMPORT_LOCK_ADDED]) {
      const file = join(root, p), original = readFileSync(file);
      try {
        writeFileSync(file, Buffer.concat([original, Buffer.from('\nmutation')])); expect(validate().length).toBeGreaterThan(0);
        rmSync(file); expect(validate().length).toBeGreaterThan(0);
      } finally { writeFileSync(file, original); }
      expect(validate()).toEqual([]);
    }
  } finally { rmSync(root, { recursive: true, force: true }); }
}, 120000);
