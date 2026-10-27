// @vitest-environment node
import { expect, it } from 'vitest';
import { readFileSync, writeFileSync, copyFileSync, mkdirSync, mkdtempSync, rmSync } from 'node:fs';
import { join, dirname, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { execFileSync } from 'node:child_process';
import { deleteIntegrityParentReader, DELETE_INTEGRITY_BASE, DELETE_INTEGRITY_PROOF, DELETE_INTEGRITY_CHANGED, DELETE_INTEGRITY_ADDED, DELETE_INTEGRITY_CONTRACTS } from '../delete-integrity-successor-guard.mjs';
import { CANCEL_CORRECTNESS_PROOF as PARENT_PROOF } from '../cancel-correctness-successor-guard.mjs';

const shot = 'src-tauri/src/infrastructure/database/repositories/shot.rs';

it('validates the standalone delete-integrity tip and its declared scope', () => {
  const checked = deleteIntegrityParentReader('.');
  expect(checked.violations).toEqual([]);
  expect(checked.changedPaths).toEqual(DELETE_INTEGRITY_CHANGED);
  expect(checked.addedPaths).toEqual([...DELETE_INTEGRITY_ADDED, DELETE_INTEGRITY_PROOF]);
}, 120000);

it('projects changed files to the closed cancel-correctness tip and preserves its proof bytes', () => {
  const checked = deleteIntegrityParentReader('.');
  expect(checked.violations).toEqual([]);
  for (const p of DELETE_INTEGRITY_CHANGED) {
    expect(checked.read(p)).toBe(execFileSync('git', ['show', `${DELETE_INTEGRITY_BASE}:${p}`], { encoding: 'utf8' }).replaceAll('\r\n', '\n'));
  }
  expect(readFileSync(PARENT_PROOF)).toEqual(execFileSync('git', ['show', `${DELETE_INTEGRITY_BASE}:${PARENT_PROOF}`]));
}, 120000);

it('rejects forged proof identity, contracts, scope, hashes and undeclared changes', () => {
  const proof = JSON.parse(readFileSync(DELETE_INTEGRITY_PROOF, 'utf8'));
  for (const override of [
    ...['checkpoint', 'parentHead', 'planningParentHead'].map(key => ({ [DELETE_INTEGRITY_PROOF]: JSON.stringify({ ...proof, [key]: 'forged' }) })),
    ...Object.entries(DELETE_INTEGRITY_CONTRACTS).map(([key, value]) => ({ [DELETE_INTEGRITY_PROOF]: JSON.stringify({ ...proof, contracts: { ...proof.contracts, [key]: !value } }) })),
    { [DELETE_INTEGRITY_PROOF]: JSON.stringify({ ...proof, files: {} }) },
    ...['beforeHash', 'afterHash'].map(key => ({ [DELETE_INTEGRITY_PROOF]: JSON.stringify({ ...proof, files: { ...proof.files, [shot]: { ...proof.files[shot], [key]: '0'.repeat(64) } } }) })),
    ...[...DELETE_INTEGRITY_CHANGED, ...DELETE_INTEGRITY_ADDED, PARENT_PROOF].map(p => ({ [p]: readFileSync(p, 'utf8') + '\n// forged' })),
    { 'src-tauri/src/application/shot_video_input_service.rs': 'changed compiler' },
    { 'src-tauri/runtime_packages/minimax_h3_fl2va_i2v_quality_2_2_0/recipe.yaml': 'changed runtime' },
    { 'src/unapproved-delete.ts': 'unapproved' },
    { 'src-tauri/src/unapproved_delete.rs': 'unapproved' },
    { 'src-tauri/migrations/051_unapproved.sql': 'SELECT 1;' },
  ]) expect(deleteIntegrityParentReader('.', override).violations.length).toBeGreaterThan(0);
  expect(deleteIntegrityParentReader('.').violations).toEqual([]);
}, 120000);

it('invalidates warm-cache fixtures on mutation, deletion and ignored additions; restoration passes', () => {
  const root = mkdtempSync(join(tmpdir(), 'delete-integrity-proof-'));
  try {
    const paths = execFileSync('git', ['ls-files', '--cached', '--others', '--exclude-standard'], { encoding: 'utf8' }).trim().split(/\r?\n/).filter(p => p === 'vite.config.ts' || /^(\.github\/workflows\/|src\/|src-tauri\/|scripts\/|docs\/architecture\/)/.test(p));
    for (const p of paths) { mkdirSync(dirname(join(root, p)), { recursive: true }); copyFileSync(p, join(root, p)); }
    execFileSync('git', ['init', '-q', root]);
    const objects = resolve(execFileSync('git', ['rev-parse', '--git-path', 'objects'], { encoding: 'utf8' }).trim());
    writeFileSync(join(root, '.git/objects/info/alternates'), objects.replaceAll('\\', '/') + '\n');
    const validate = () => deleteIntegrityParentReader(root).violations;
    expect(validate()).toEqual([]);
    for (const p of [DELETE_INTEGRITY_PROOF, PARENT_PROOF, shot, ...DELETE_INTEGRITY_ADDED]) {
      const file = join(root, p), original = readFileSync(file);
      try {
        writeFileSync(file, Buffer.concat([original, Buffer.from('\nmutation')])); expect(validate().length).toBeGreaterThan(0);
        rmSync(file); expect(validate().length).toBeGreaterThan(0);
      } finally { writeFileSync(file, original); }
      expect(validate()).toEqual([]);
    }
    writeFileSync(join(root, '.git/info/exclude'), 'src/unapproved-delete.ts\n');
    const illegal = join(root, 'src/unapproved-delete.ts');
    try { writeFileSync(illegal, 'unapproved'); expect(validate()).toContain('delete-integrity-illegal-file'); }
    finally { rmSync(illegal); }
    expect(validate()).toEqual([]);
  } finally { rmSync(root, { recursive: true, force: true }); }
}, 120000);
