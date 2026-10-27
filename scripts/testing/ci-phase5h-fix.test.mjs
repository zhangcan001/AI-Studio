// @vitest-environment node
import { expect, it } from 'vitest';
import { readFileSync, writeFileSync, copyFileSync, mkdirSync, mkdtempSync, rmSync } from 'node:fs';
import { join, dirname, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { execFileSync } from 'node:child_process';
import { ciPhase5HFixParentReader, CI_PHASE5H_FIX_BASE, CI_PHASE5H_FIX_PROOF, CI_PHASE5H_FIX_CHANGED, CI_PHASE5H_FIX_ADDED, CI_PHASE5H_FIX_CONTRACTS } from '../ci-phase5h-fix-successor-guard.mjs';
const PARENT_PROOF = 'docs/architecture/ci-boundary-fix-successor.json';
it('validates the standalone ci-phase5h-fix tip and its declared scope', () => {
  const checked = ciPhase5HFixParentReader('.');
  expect(checked.violations).toEqual([]);
  for (const p of CI_PHASE5H_FIX_CHANGED) expect(checked.changedPaths).toContain(p);
  for (const p of [...CI_PHASE5H_FIX_ADDED, CI_PHASE5H_FIX_PROOF]) expect(checked.addedPaths).toContain(p);
}, 120000);
it('projects changed files to the closed parent tip and preserves its proof bytes', () => {
  const checked = ciPhase5HFixParentReader('.');
  expect(checked.violations).toEqual([]);
  for (const p of CI_PHASE5H_FIX_CHANGED) {
    expect(checked.read(p)).toBe(execFileSync('git', ['show', `${CI_PHASE5H_FIX_BASE}:${p}`], { encoding: 'utf8' }).replaceAll('\r\n', '\n'));
  }
  expect(readFileSync(PARENT_PROOF)).toEqual(execFileSync('git', ['show', `${CI_PHASE5H_FIX_BASE}:${PARENT_PROOF}`]));
}, 120000);
it('keeps Phase5A/Ci/F/H/I/J live-mutate without FixtureFiles copy or raised timeouts', () => {
  for (const [f, needle] of [
    ['src/app/UiPhase5AStoryboardBoundary.test.ts', 'phase5AFixtureFiles'],
    ['src/app/UiPhase5CiRepairBoundary.test.ts', 'phase5CiFixtureFiles'],
    ['src/app/UiPhase5FCiMemoryBoundary.test.ts', 'phase5FFixtureFiles'],
    ['src/app/UiPhase5HCiTimeoutBudgetBoundary.test.ts', 'phase5HFixtureFiles'],
    ['src/app/UiPhase5ICiWorkerTeardownBoundary.test.ts', 'phase5IFixtureFiles'],
    ['src/app/UiPhase5JCiPerFileWorkerBoundary.test.ts', 'phase5JFixtureFiles'],
  ]) {
    const src = readFileSync(f, 'utf8');
    expect(src).not.toContain(needle);
    expect(src).toContain('Live working-tree mutate');
  }
  expect(CI_PHASE5H_FIX_CONTRACTS.timeoutsRaised).toBe(false);
  expect(CI_PHASE5H_FIX_CONTRACTS.guardsRelaxed).toBe(false);
  expect(ciPhase5HFixParentReader('.').violations).toEqual([]);
}, 120000);
it('rejects forged proof identity, contracts, scope, hashes and undeclared changes', () => {
  const proof = JSON.parse(readFileSync(CI_PHASE5H_FIX_PROOF, 'utf8'));
  const sample = CI_PHASE5H_FIX_CHANGED[0];
  for (const override of [
    ...['checkpoint', 'parentHead', 'planningParentHead'].map(key => ({ [CI_PHASE5H_FIX_PROOF]: JSON.stringify({ ...proof, [key]: 'forged' }) })),
    ...Object.entries(CI_PHASE5H_FIX_CONTRACTS).map(([key, value]) => ({ [CI_PHASE5H_FIX_PROOF]: JSON.stringify({ ...proof, contracts: { ...proof.contracts, [key]: !value } }) })),
    { [CI_PHASE5H_FIX_PROOF]: JSON.stringify({ ...proof, files: {} }) },
    ...['beforeHash', 'afterHash'].map(key => ({ [CI_PHASE5H_FIX_PROOF]: JSON.stringify({ ...proof, files: { ...proof.files, [sample]: { ...proof.files[sample], [key]: '0'.repeat(64) } } }) })),
    ...[...CI_PHASE5H_FIX_CHANGED, ...CI_PHASE5H_FIX_ADDED, PARENT_PROOF].map(p => ({ [p]: readFileSync(p, 'utf8') + '\n// forged' })),
    { 'src-tauri/migrations/051_unapproved.sql': 'SELECT 1;' },
  ]) expect(ciPhase5HFixParentReader('.', override).violations.length).toBeGreaterThan(0);
  expect(ciPhase5HFixParentReader('.').violations).toEqual([]);
}, 120000);
it('invalidates warm-cache fixtures on live mutations, deletion and illegal files; restoration passes', () => {
  const root = mkdtempSync(join(tmpdir(), 'ci-phase5h-fix-proof-'));
  try {
    const paths = execFileSync('git', ['ls-files', '--cached', '--others', '--exclude-standard'], { encoding: 'utf8' }).trim().split(/\r?\n/).filter(p => p === 'vite.config.ts' || /^(\.github\/workflows\/|src\/|src-tauri\/|scripts\/|docs\/architecture\/)/.test(p));
    for (const p of paths) { mkdirSync(dirname(join(root, p)), { recursive: true }); copyFileSync(p, join(root, p)); }
    execFileSync('git', ['init', '-q', root]);
    const objects = resolve(execFileSync('git', ['rev-parse', '--git-path', 'objects'], { encoding: 'utf8' }).trim());
    writeFileSync(join(root, '.git/objects/info/alternates'), objects.replaceAll('\\', '/') + '\n');
    const validate = () => ciPhase5HFixParentReader(root).violations;
    expect(validate()).toEqual([]);
    for (const p of [CI_PHASE5H_FIX_PROOF, PARENT_PROOF, 'src/app/UiPhase5HCiTimeoutBudgetBoundary.test.ts']) {
      const file = join(root, p), original = readFileSync(file);
      try {
        writeFileSync(file, Buffer.concat([original, Buffer.from('\nmutation')])); expect(validate().length).toBeGreaterThan(0);
        rmSync(file); expect(validate().length).toBeGreaterThan(0);
      } finally { writeFileSync(file, original); }
      expect(validate()).toEqual([]);
    }
  } finally { rmSync(root, { recursive: true, force: true }); }
}, 120000);
