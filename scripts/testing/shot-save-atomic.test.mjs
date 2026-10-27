// @vitest-environment node
import { expect, it } from 'vitest';
import { readFileSync, writeFileSync, copyFileSync, mkdirSync, mkdtempSync, rmSync } from 'node:fs';
import { join, dirname, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { execFileSync } from 'node:child_process';
import { shotSaveAtomicParentReader, SHOT_SAVE_ATOMIC_BASE, SHOT_SAVE_ATOMIC_PROOF, SHOT_SAVE_ATOMIC_CHANGED, SHOT_SAVE_ATOMIC_ADDED, SHOT_SAVE_ATOMIC_CONTRACTS } from '../shot-save-atomic-successor-guard.mjs';
const PARENT_PROOF = 'docs/architecture/prompt-sections-without-at-successor.json';
const source = 'src-tauri/src/application/shot_service.rs';
const command = 'src-tauri/src/commands/product.rs';

it('validates the standalone shot-save-atomic tip and its declared scope', () => {
  const checked = shotSaveAtomicParentReader('.');
  expect(checked.violations).toEqual([]);
  expect(checked.changedPaths).toEqual(SHOT_SAVE_ATOMIC_CHANGED);
  expect(checked.addedPaths).toEqual([...SHOT_SAVE_ATOMIC_ADDED, SHOT_SAVE_ATOMIC_PROOF]);
}, 120000);

it('projects changed files to the closed parent tip and preserves its proof bytes', () => {
  const checked = shotSaveAtomicParentReader('.');
  expect(checked.violations).toEqual([]);
  for (const p of SHOT_SAVE_ATOMIC_CHANGED) {
    expect(checked.read(p)).toBe(execFileSync('git', ['show', `${SHOT_SAVE_ATOMIC_BASE}:${p}`], { encoding: 'utf8' }).replaceAll('\r\n', '\n'));
  }
  expect(readFileSync(PARENT_PROOF)).toEqual(execFileSync('git', ['show', `${SHOT_SAVE_ATOMIC_BASE}:${PARENT_PROOF}`]));
}, 120000);

it('keeps shot update and creation shot update on one transaction each', () => {
  const svc = readFileSync(source, 'utf8');
  expect(svc).toContain('update_shot_with_stage_prompts');
  expect(svc).not.toMatch(/repository\.update\(&shot\)[\s\S]{0,400}update_stage_prompts_atomic/);
  const cmd = readFileSync(command, 'utf8');
  expect(cmd).toContain('update_shot_with_fields');
  expect(cmd).not.toMatch(/update_shot\(&project_id, update\)\.await\?;[\s\S]{0,120}save_shot_fields/);
  expect(shotSaveAtomicParentReader('.').violations).toEqual([]);
}, 120000);

it('rejects forged proof identity, contracts, scope, hashes and undeclared changes', () => {
  const proof = JSON.parse(readFileSync(SHOT_SAVE_ATOMIC_PROOF, 'utf8'));
  for (const override of [
    ...['checkpoint', 'parentHead', 'planningParentHead'].map(key => ({ [SHOT_SAVE_ATOMIC_PROOF]: JSON.stringify({ ...proof, [key]: 'forged' }) })),
    ...Object.entries(SHOT_SAVE_ATOMIC_CONTRACTS).map(([key, value]) => ({ [SHOT_SAVE_ATOMIC_PROOF]: JSON.stringify({ ...proof, contracts: { ...proof.contracts, [key]: !value } }) })),
    { [SHOT_SAVE_ATOMIC_PROOF]: JSON.stringify({ ...proof, files: {} }) },
    ...['beforeHash', 'afterHash'].map(key => ({ [SHOT_SAVE_ATOMIC_PROOF]: JSON.stringify({ ...proof, files: { ...proof.files, [source]: { ...proof.files[source], [key]: '0'.repeat(64) } } }) })),
    ...[...SHOT_SAVE_ATOMIC_CHANGED, ...SHOT_SAVE_ATOMIC_ADDED, PARENT_PROOF].map(p => ({ [p]: readFileSync(p, 'utf8') + '\n// forged' })),
    { 'src/features/create/StoryboardShotRows.tsx': 'changed frontend' },
    { 'src-tauri/runtime_packages/minimax_h3_fl2va_i2v_quality_2_2_0/recipe.yaml': 'changed runtime' },
    { 'src/unapproved-shot-save-atomic.ts': 'unapproved' },
    { 'src-tauri/migrations/051_unapproved.sql': 'SELECT 1;' },
  ]) expect(shotSaveAtomicParentReader('.', override).violations.length).toBeGreaterThan(0);
  expect(shotSaveAtomicParentReader('.').violations).toEqual([]);
}, 120000);

it('invalidates warm-cache fixtures on mutation, deletion and ignored additions; restoration passes', () => {
  const root = mkdtempSync(join(tmpdir(), 'shot-save-atomic-proof-'));
  try {
    const paths = execFileSync('git', ['ls-files', '--cached', '--others', '--exclude-standard'], { encoding: 'utf8' }).trim().split(/\r?\n/).filter(p => p === 'vite.config.ts' || /^(\.github\/workflows\/|src\/|src-tauri\/|scripts\/|docs\/architecture\/)/.test(p));
    for (const p of paths) { mkdirSync(dirname(join(root, p)), { recursive: true }); copyFileSync(p, join(root, p)); }
    execFileSync('git', ['init', '-q', root]);
    const objects = resolve(execFileSync('git', ['rev-parse', '--git-path', 'objects'], { encoding: 'utf8' }).trim());
    writeFileSync(join(root, '.git/objects/info/alternates'), objects.replaceAll('\\', '/') + '\n');
    const validate = () => shotSaveAtomicParentReader(root).violations;
    expect(validate()).toEqual([]);
    for (const p of [SHOT_SAVE_ATOMIC_PROOF, PARENT_PROOF, source, ...SHOT_SAVE_ATOMIC_ADDED]) {
      const file = join(root, p), original = readFileSync(file);
      try {
        writeFileSync(file, Buffer.concat([original, Buffer.from('\nmutation')])); expect(validate().length).toBeGreaterThan(0);
        rmSync(file); expect(validate().length).toBeGreaterThan(0);
      } finally { writeFileSync(file, original); }
      expect(validate()).toEqual([]);
    }
    writeFileSync(join(root, '.git/info/exclude'), 'src/unapproved-shot-save-atomic.ts\n');
    const illegal = join(root, 'src/unapproved-shot-save-atomic.ts');
    try { writeFileSync(illegal, 'unapproved'); expect(validate()).toContain('shot-save-atomic-illegal-file'); }
    finally { rmSync(illegal); }
    expect(validate()).toEqual([]);
  } finally { rmSync(root, { recursive: true, force: true }); }
}, 120000);
