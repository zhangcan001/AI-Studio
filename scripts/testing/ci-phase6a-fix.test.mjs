// @vitest-environment node
import { expect, it } from 'vitest';
import { readFileSync, writeFileSync, copyFileSync, mkdirSync, mkdtempSync, rmSync } from 'node:fs';
import { join, dirname, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { execFileSync } from 'node:child_process';
import { ciPhase6AFixParentReader, CI_PHASE6A_FIX_BASE, CI_PHASE6A_FIX_PROOF, CI_PHASE6A_FIX_CHANGED, CI_PHASE6A_FIX_ADDED, CI_PHASE6A_FIX_CONTRACTS } from '../ci-phase6a-fix-successor-guard.mjs';
const PARENT_PROOF = 'docs/architecture/ci-phase5h-fix-successor.json';
it('validates the standalone ci-phase6a-fix tip and its declared scope', () => {
  const checked = ciPhase6AFixParentReader('.');
  expect(checked.violations).toEqual([]);
  expect(checked.changedPaths).toEqual(CI_PHASE6A_FIX_CHANGED);
  expect(checked.addedPaths).toEqual([...CI_PHASE6A_FIX_ADDED, CI_PHASE6A_FIX_PROOF]);
}, 120000);
it('projects changed files to the closed parent tip and preserves its proof bytes', () => {
  const checked = ciPhase6AFixParentReader('.');
  expect(checked.violations).toEqual([]);
  for (const p of CI_PHASE6A_FIX_CHANGED) {
    expect(checked.read(p)).toBe(execFileSync('git', ['show', `${CI_PHASE6A_FIX_BASE}:${p}`], { encoding: 'utf8' }).replaceAll('\r\n', '\n'));
  }
  expect(readFileSync(PARENT_PROOF)).toEqual(execFileSync('git', ['show', `${CI_PHASE6A_FIX_BASE}:${PARENT_PROOF}`]));
}, 120000);
it('keeps Phase2/5A/6A live-mutate and Phase7C 90s needle without raised timeouts', () => {
  for (const [f, needle] of [
    ['src/app/MiniMaxVideoPhase2Boundary.test.ts', 'minimaxVideoPhase3FixtureFiles'],
    ['src/app/UiPhase5AStoryboardBoundary.test.ts', 'phase5AFixtureFiles'],
    ['src/app/UiPhase6ALibrarySourceBoundary.test.ts', 'phase6AFixtureFiles'],
  ]) {
    const src = readFileSync(f, 'utf8');
    expect(src).not.toContain(needle);
    expect(src).toContain('Live working-tree mutate');
  }
  expect(readFileSync('src/app/UiPhase5AStoryboardBoundary.test.ts', 'utf8')).toContain('parent fail-closed checked once');
  const seven = readFileSync('src/app/UiPhase7CCiBudgetSuccessorBoundary.test.ts', 'utf8');
  expect(seven).toContain('90000');
  expect(seven).not.toMatch(/UiPhase5CiRepairBoundary[\s\S]*300000/);
  expect(CI_PHASE6A_FIX_CONTRACTS.timeoutsRaised).toBe(false);
  expect(CI_PHASE6A_FIX_CONTRACTS.guardsRelaxed).toBe(false);
  expect(ciPhase6AFixParentReader('.').violations).toEqual([]);
}, 120000);
it('rejects forged proof identity, contracts, scope, hashes and undeclared changes', () => {
  const proof = JSON.parse(readFileSync(CI_PHASE6A_FIX_PROOF, 'utf8'));
  const sample = CI_PHASE6A_FIX_CHANGED[0];
  for (const override of [
    ...['checkpoint', 'parentHead', 'planningParentHead'].map(key => ({ [CI_PHASE6A_FIX_PROOF]: JSON.stringify({ ...proof, [key]: 'forged' }) })),
    ...Object.entries(CI_PHASE6A_FIX_CONTRACTS).map(([key, value]) => ({ [CI_PHASE6A_FIX_PROOF]: JSON.stringify({ ...proof, contracts: { ...proof.contracts, [key]: !value } }) })),
    { [CI_PHASE6A_FIX_PROOF]: JSON.stringify({ ...proof, files: {} }) },
    ...['beforeHash', 'afterHash'].map(key => ({ [CI_PHASE6A_FIX_PROOF]: JSON.stringify({ ...proof, files: { ...proof.files, [sample]: { ...proof.files[sample], [key]: '0'.repeat(64) } } }) })),
    ...[...CI_PHASE6A_FIX_CHANGED, ...CI_PHASE6A_FIX_ADDED, PARENT_PROOF].map(p => ({ [p]: readFileSync(p, 'utf8') + '\n// forged' })),
    { 'src-tauri/migrations/051_unapproved.sql': 'SELECT 1;' },
  ]) expect(ciPhase6AFixParentReader('.', override).violations.length).toBeGreaterThan(0);
  expect(ciPhase6AFixParentReader('.').violations).toEqual([]);
}, 120000);
it('invalidates warm-cache fixtures on live mutations, deletion and illegal files; restoration passes', () => {
  const root = mkdtempSync(join(tmpdir(), 'ci-phase6a-fix-proof-'));
  try {
    const paths = execFileSync('git', ['ls-files', '--cached', '--others', '--exclude-standard'], { encoding: 'utf8' }).trim().split(/\r?\n/).filter(p => p === 'vite.config.ts' || /^(\.github\/workflows\/|src\/|src-tauri\/|scripts\/|docs\/architecture\/)/.test(p));
    for (const p of paths) { mkdirSync(dirname(join(root, p)), { recursive: true }); copyFileSync(p, join(root, p)); }
    execFileSync('git', ['init', '-q', root]);
    const objects = resolve(execFileSync('git', ['rev-parse', '--git-path', 'objects'], { encoding: 'utf8' }).trim());
    writeFileSync(join(root, '.git/objects/info/alternates'), objects.replaceAll('\\', '/') + '\n');
    const validate = () => ciPhase6AFixParentReader(root).violations;
    expect(validate()).toEqual([]);
    for (const p of [CI_PHASE6A_FIX_PROOF, PARENT_PROOF, 'src/app/UiPhase6ALibrarySourceBoundary.test.ts']) {
      const file = join(root, p), original = readFileSync(file);
      try {
        writeFileSync(file, Buffer.concat([original, Buffer.from('\nmutation')])); expect(validate().length).toBeGreaterThan(0);
        rmSync(file); expect(validate().length).toBeGreaterThan(0);
      } finally { writeFileSync(file, original); }
      expect(validate()).toEqual([]);
    }
  } finally { rmSync(root, { recursive: true, force: true }); }
}, 120000);
