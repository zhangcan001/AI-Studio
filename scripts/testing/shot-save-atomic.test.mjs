// @vitest-environment node
import { expect, it } from 'vitest';
import { readFileSync, writeFileSync, copyFileSync, mkdirSync, mkdtempSync, rmSync } from 'node:fs';
import { join, dirname, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { execFileSync } from 'node:child_process';
import { packageImportLockParentReader } from '../package-import-lock-successor-guard.mjs';
import { SHOT_SAVE_ATOMIC_BASE, SHOT_SAVE_ATOMIC_PROOF } from '../shot-save-atomic-successor-guard.mjs';
import { PROMPT_SECTIONS_WITHOUT_AT_PROOF as PARENT_PROOF } from '../prompt-sections-without-at-successor-guard.mjs';

const source = 'src-tauri/src/application/shot_service.rs';
const command = 'src-tauri/src/commands/product.rs';

it('keeps shot update and creation shot update on one transaction each', () => {
  const svc = readFileSync(source, 'utf8');
  expect(svc).toContain('update_shot_with_stage_prompts');
  expect(command && readFileSync(command, 'utf8')).toContain('update_shot_with_fields');
  expect(packageImportLockParentReader('.').violations).toEqual([]);
}, 120000);

it('preserves the frozen prompt-sections-without-at proof bytes', () => {
  const checked = packageImportLockParentReader('.');
  expect(checked.violations).toEqual([]);
  expect(checked.read(source)).toBe(execFileSync('git', ['show', `${SHOT_SAVE_ATOMIC_BASE}:${source}`], { encoding: 'utf8' }).replaceAll('\r\n', '\n'));
  expect(readFileSync(PARENT_PROOF)).toEqual(execFileSync('git', ['show', `${SHOT_SAVE_ATOMIC_BASE}:${PARENT_PROOF}`]));
}, 120000);

it('rejects forged proof identity/hash, source drift, migrations and undeclared additions', () => {
  const proof = JSON.parse(readFileSync(SHOT_SAVE_ATOMIC_PROOF, 'utf8'));
  for (const override of [
    { [SHOT_SAVE_ATOMIC_PROOF]: JSON.stringify({ ...proof, parentHead: 'forged' }) },
    { [SHOT_SAVE_ATOMIC_PROOF]: JSON.stringify({ ...proof, contracts: { ...proof.contracts, guardsRelaxed: true } }) },
    { [SHOT_SAVE_ATOMIC_PROOF]: JSON.stringify({ ...proof, files: { ...proof.files, [source]: { ...proof.files[source], afterHash: '0'.repeat(64) } } }) },
    { [source]: readFileSync(source, 'utf8') + '\n// forged' },
    { 'src/features/create/StoryboardShotRows.tsx': 'changed frontend' },
    { 'src/unapproved-shot-save.ts': 'unapproved' },
    { 'src-tauri/migrations/051_unapproved.sql': 'SELECT 1;' },
  ]) expect(packageImportLockParentReader('.', override).violations.length).toBeGreaterThan(0);
  expect(packageImportLockParentReader('.').violations).toEqual([]);
}, 120000);

it('invalidates warm-cache fixtures on live mutations, deletion and illegal files; restoration passes', () => {
  const root = mkdtempSync(join(tmpdir(), 'shot-save-atomic-proof-'));
  try {
    const paths = execFileSync('git', ['ls-files', '--cached', '--others', '--exclude-standard'], { encoding: 'utf8' }).trim().split(/\r?\n/).filter(p => p === 'vite.config.ts' || /^(\.github\/workflows\/|src\/|src-tauri\/|scripts\/|docs\/architecture\/)/.test(p));
    for (const p of paths) { mkdirSync(dirname(join(root, p)), { recursive: true }); copyFileSync(p, join(root, p)); }
    execFileSync('git', ['init', '-q', root]);
    const objects = resolve(execFileSync('git', ['rev-parse', '--git-path', 'objects'], { encoding: 'utf8' }).trim());
    writeFileSync(join(root, '.git/objects/info/alternates'), objects.replaceAll('\\', '/') + '\n');
    const validate = () => packageImportLockParentReader(root).violations;
    expect(validate()).toEqual([]);
    for (const p of [SHOT_SAVE_ATOMIC_PROOF, PARENT_PROOF, source]) {
      const file = join(root, p), original = readFileSync(file);
      try {
        writeFileSync(file, Buffer.concat([original, Buffer.from('\nmutation')])); expect(validate().length).toBeGreaterThan(0);
        rmSync(file); expect(validate().length).toBeGreaterThan(0);
      } finally { writeFileSync(file, original); }
      expect(validate()).toEqual([]);
    }
  } finally { rmSync(root, { recursive: true, force: true }); }
}, 120000);
