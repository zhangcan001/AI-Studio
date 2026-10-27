// @vitest-environment node
import { expect, it } from 'vitest';
import { readFileSync, writeFileSync, copyFileSync, mkdirSync, mkdtempSync, rmSync } from 'node:fs';
import { join, dirname, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { execFileSync } from 'node:child_process';
import { STORYBOARD_WORKBENCH_BATCH_BASE, STORYBOARD_WORKBENCH_BATCH_PROOF, STORYBOARD_WORKBENCH_BATCH_CHANGED, STORYBOARD_WORKBENCH_BATCH_ADDED, STORYBOARD_WORKBENCH_BATCH_CONTRACTS } from '../storyboard-workbench-batch-successor-guard.mjs';
import { successorChainParentReader, SUCCESSOR_CHAIN_BASE, SUCCESSOR_CHAIN_PROOF, SUCCESSOR_CHAIN_CHANGED, SUCCESSOR_CHAIN_ADDED, SUCCESSOR_CHAIN_CONTRACTS } from '../successor-chain-successor-guard.mjs';
const PARENT_PROOF = 'docs/architecture/package-import-lock-successor.json';
const ui = 'src/features/create/StoryboardWorkbench.tsx';
const facade = 'src-tauri/src/application/product/creation_facade/context.rs';
it('projects through the successor-chain tip with a clean boundary', () => {
  const checked = successorChainParentReader('.');
  expect(checked.violations).toEqual([]);
}, 120000);
it('preserves the frozen package-import-lock proof bytes via the tip', () => {
  const checked = successorChainParentReader('.');
  expect(checked.violations).toEqual([]);
  expect(readFileSync(PARENT_PROOF)).toEqual(execFileSync('git', ['show', `${STORYBOARD_WORKBENCH_BATCH_BASE}:${PARENT_PROOF}`]));
}, 120000);
it('loads the storyboard through one workbench IPC instead of per-shot creation.get', () => {
  expect(readFileSync(ui, 'utf8')).toContain('creation.workbench');
  expect(readFileSync(ui, 'utf8')).not.toMatch(/creation\.get\(projectId, summary\.id/);
  expect(readFileSync(facade, 'utf8')).toContain('pub async fn workbench(');
  expect(successorChainParentReader('.').violations).toEqual([]);
}, 120000);
it('rejects forged proof identity, contracts, scope, hashes and undeclared changes', () => {
  const proof = JSON.parse(readFileSync(STORYBOARD_WORKBENCH_BATCH_PROOF, 'utf8'));
  for (const override of [
    ...['checkpoint', 'parentHead', 'planningParentHead'].map(key => ({ [STORYBOARD_WORKBENCH_BATCH_PROOF]: JSON.stringify({ ...proof, [key]: 'forged' }) })),
    ...Object.entries(STORYBOARD_WORKBENCH_BATCH_CONTRACTS).map(([key, value]) => ({ [STORYBOARD_WORKBENCH_BATCH_PROOF]: JSON.stringify({ ...proof, contracts: { ...proof.contracts, [key]: !value } }) })),
    { [STORYBOARD_WORKBENCH_BATCH_PROOF]: JSON.stringify({ ...proof, files: {} }) },
    ...['beforeHash', 'afterHash'].map(key => ({ [STORYBOARD_WORKBENCH_BATCH_PROOF]: JSON.stringify({ ...proof, files: { ...proof.files, [ui]: { ...proof.files[ui], [key]: '0'.repeat(64) } } }) })),
    ...[...STORYBOARD_WORKBENCH_BATCH_CHANGED, ...STORYBOARD_WORKBENCH_BATCH_ADDED, PARENT_PROOF].map(p => ({ [p]: readFileSync(p, 'utf8') + '\n// forged' })),
    { 'src-tauri/src/application/production_package_service.rs': 'changed import' },
    { 'src-tauri/migrations/051_unapproved.sql': 'SELECT 1;' },
  ]) expect(successorChainParentReader('.', override).violations.length).toBeGreaterThan(0);
  expect(successorChainParentReader('.').violations).toEqual([]);
}, 120000);
it('invalidates warm-cache fixtures on mutation, deletion and ignored additions; restoration passes', () => {
  const root = mkdtempSync(join(tmpdir(), 'storyboard-workbench-batch-proof-'));
  try {
    const paths = execFileSync('git', ['ls-files', '--cached', '--others', '--exclude-standard'], { encoding: 'utf8' }).trim().split(/\r?\n/).filter(p => p === 'vite.config.ts' || /^(\.github\/workflows\/|src\/|src-tauri\/|scripts\/|docs\/architecture\/)/.test(p));
    for (const p of paths) { mkdirSync(dirname(join(root, p)), { recursive: true }); copyFileSync(p, join(root, p)); }
    execFileSync('git', ['init', '-q', root]);
    const objects = resolve(execFileSync('git', ['rev-parse', '--git-path', 'objects'], { encoding: 'utf8' }).trim());
    writeFileSync(join(root, '.git/objects/info/alternates'), objects.replaceAll('\\', '/') + '\n');
    const validate = () => successorChainParentReader(root).violations;
    expect(validate()).toEqual([]);
    for (const p of [STORYBOARD_WORKBENCH_BATCH_PROOF, PARENT_PROOF, ui, ...STORYBOARD_WORKBENCH_BATCH_ADDED]) {
      const file = join(root, p), original = readFileSync(file);
      try {
        writeFileSync(file, Buffer.concat([original, Buffer.from('\nmutation')])); expect(validate().length).toBeGreaterThan(0);
        rmSync(file); expect(validate().length).toBeGreaterThan(0);
      } finally { writeFileSync(file, original); }
      expect(validate()).toEqual([]);
    }
  } finally { rmSync(root, { recursive: true, force: true }); }
}, 120000);
