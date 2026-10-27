// @vitest-environment node
import { expect, it } from 'vitest';
import { readFileSync, writeFileSync, copyFileSync, mkdirSync, mkdtempSync, rmSync } from 'node:fs';
import { join, dirname, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { execFileSync } from 'node:child_process';
import { ciH3BudgetFixParentReader, CI_H3_BUDGET_FIX_BASE, CI_H3_BUDGET_FIX_PROOF, CI_H3_BUDGET_FIX_CHANGED, CI_H3_BUDGET_FIX_ADDED, CI_H3_BUDGET_FIX_CONTRACTS } from '../ci-h3-budget-fix-successor-guard.mjs';
const PARENT_PROOF = 'docs/architecture/ci-stable-fix-successor.json';
const h3 = 'src/app/H3GenerationRepairBoundary.test.ts';
it('validates the standalone ci-h3-budget-fix tip and its declared scope', () => {
  const checked = ciH3BudgetFixParentReader('.');
  expect(checked.violations).toEqual([]);
  for (const p of CI_H3_BUDGET_FIX_CHANGED) expect(checked.changedPaths).toContain(p);
  for (const p of [...CI_H3_BUDGET_FIX_ADDED, CI_H3_BUDGET_FIX_PROOF]) expect(checked.addedPaths).toContain(p);
}, 120000);
it('projects changed files to the closed parent tip and preserves its proof bytes', () => {
  const checked = ciH3BudgetFixParentReader('.');
  expect(checked.violations).toEqual([]);
  for (const p of CI_H3_BUDGET_FIX_CHANGED) {
    expect(checked.read(p)).toBe(execFileSync('git', ['show', `${CI_H3_BUDGET_FIX_BASE}:${p}`], { encoding: 'utf8' }).replaceAll('\r\n', '\n'));
  }
  expect(readFileSync(PARENT_PROOF)).toEqual(execFileSync('git', ['show', `${CI_H3_BUDGET_FIX_BASE}:${PARENT_PROOF}`]));
}, 120000);
it('keeps H3 live-mutate drift asserts without fixture copy or raised timeouts', () => {
  const src = readFileSync(h3, 'utf8');
  expect(src).toContain("h3RepairParentReader('.')");
  expect(src).toContain('minimax-video-phase2-backend-files');
  expect(src).not.toContain('minimaxVideoPhase2FixtureFiles');
  expect(CI_H3_BUDGET_FIX_CONTRACTS.timeoutsRaised).toBe(false);
  expect(CI_H3_BUDGET_FIX_CONTRACTS.guardsRelaxed).toBe(false);
  expect(ciH3BudgetFixParentReader('.').violations).toEqual([]);
}, 120000);
it('rejects forged proof identity, contracts, scope, hashes and undeclared changes', () => {
  const proof = JSON.parse(readFileSync(CI_H3_BUDGET_FIX_PROOF, 'utf8'));
  const sample = CI_H3_BUDGET_FIX_CHANGED[0];
  for (const override of [
    ...['checkpoint', 'parentHead', 'planningParentHead'].map(key => ({ [CI_H3_BUDGET_FIX_PROOF]: JSON.stringify({ ...proof, [key]: 'forged' }) })),
    ...Object.entries(CI_H3_BUDGET_FIX_CONTRACTS).map(([key, value]) => ({ [CI_H3_BUDGET_FIX_PROOF]: JSON.stringify({ ...proof, contracts: { ...proof.contracts, [key]: !value } }) })),
    { [CI_H3_BUDGET_FIX_PROOF]: JSON.stringify({ ...proof, files: {} }) },
    ...['beforeHash', 'afterHash'].map(key => ({ [CI_H3_BUDGET_FIX_PROOF]: JSON.stringify({ ...proof, files: { ...proof.files, [sample]: { ...proof.files[sample], [key]: '0'.repeat(64) } } }) })),
    ...[...CI_H3_BUDGET_FIX_CHANGED, ...CI_H3_BUDGET_FIX_ADDED, PARENT_PROOF].map(p => ({ [p]: readFileSync(p, 'utf8') + '\n// forged' })),
    { 'src-tauri/migrations/051_unapproved.sql': 'SELECT 1;' },
  ]) expect(ciH3BudgetFixParentReader('.', override).violations.length).toBeGreaterThan(0);
  expect(ciH3BudgetFixParentReader('.').violations).toEqual([]);
}, 120000);
it('invalidates warm-cache fixtures on live mutations, deletion and illegal files; restoration passes', () => {
  const root = mkdtempSync(join(tmpdir(), 'ci-h3-budget-fix-proof-'));
  try {
    const paths = execFileSync('git', ['ls-files', '--cached', '--others', '--exclude-standard'], { encoding: 'utf8' }).trim().split(/\r?\n/).filter(p => p === 'vite.config.ts' || /^(\.github\/workflows\/|src\/|src-tauri\/|scripts\/|docs\/architecture\/)/.test(p));
    for (const p of paths) { mkdirSync(dirname(join(root, p)), { recursive: true }); copyFileSync(p, join(root, p)); }
    execFileSync('git', ['init', '-q', root]);
    const objects = resolve(execFileSync('git', ['rev-parse', '--git-path', 'objects'], { encoding: 'utf8' }).trim());
    writeFileSync(join(root, '.git/objects/info/alternates'), objects.replaceAll('\\', '/') + '\n');
    const validate = () => ciH3BudgetFixParentReader(root).violations;
    expect(validate()).toEqual([]);
    for (const p of [CI_H3_BUDGET_FIX_PROOF, PARENT_PROOF, h3]) {
      const file = join(root, p), original = readFileSync(file);
      try {
        writeFileSync(file, Buffer.concat([original, Buffer.from('\nmutation')])); expect(validate().length).toBeGreaterThan(0);
        rmSync(file); expect(validate().length).toBeGreaterThan(0);
      } finally { writeFileSync(file, original); }
      expect(validate()).toEqual([]);
    }
  } finally { rmSync(root, { recursive: true, force: true }); }
}, 120000);
