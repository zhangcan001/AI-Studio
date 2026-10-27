// @vitest-environment node
import { expect, it } from 'vitest';
import { readFileSync, writeFileSync, copyFileSync, mkdirSync, mkdtempSync, rmSync } from 'node:fs';
import { join, dirname, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { execFileSync } from 'node:child_process';
import { ciGreenFixParentReader, CI_GREEN_FIX_BASE, CI_GREEN_FIX_PROOF, CI_GREEN_FIX_CHANGED, CI_GREEN_FIX_ADDED, CI_GREEN_FIX_CONTRACTS } from '../ci-green-fix-successor-guard.mjs';
const PARENT_PROOF = 'docs/architecture/ci-probe-fix-successor.json';
const createPage = 'src/features/create/CreatePage.test.tsx';
it('validates the standalone ci-green-fix tip and its declared scope', () => {
  const checked = ciGreenFixParentReader('.');
  expect(checked.violations).toEqual([]);
  for (const p of CI_GREEN_FIX_CHANGED) expect(checked.changedPaths).toContain(p);
  for (const p of [...CI_GREEN_FIX_ADDED, CI_GREEN_FIX_PROOF]) expect(checked.addedPaths).toContain(p);
}, 120000);
it('projects changed files to the closed parent tip and preserves its proof bytes', () => {
  const checked = ciGreenFixParentReader('.');
  expect(checked.violations).toEqual([]);
  for (const p of CI_GREEN_FIX_CHANGED) {
    expect(checked.read(p)).toBe(execFileSync('git', ['show', `${CI_GREEN_FIX_BASE}:${p}`], { encoding: 'utf8' }).replaceAll('\r\n', '\n'));
  }
  expect(readFileSync(PARENT_PROOF)).toEqual(execFileSync('git', ['show', `${CI_GREEN_FIX_BASE}:${PARENT_PROOF}`]));
}, 120000);
it('keeps CreatePage workbench shot names and real Phase5C live-mutate freshness without raising timeouts', () => {
  expect(readFileSync(createPage, 'utf8')).toContain('shots.find(s => s.id === shotId)');
  expect(readFileSync('src/app/UiPhase5CReviewBoundary.test.ts', 'utf8')).toContain("writeFileSync(p, bytes.toString() + '\n')");
  expect(readFileSync('src/app/UiPhase5CReviewBoundary.test.ts', 'utf8')).toContain('ui-phase5c-review-inventory:${dir}');
  expect(readFileSync('scripts/testing/stale-binding-migration.test.mjs', 'utf8')).toContain('frozenTip');
  expect(CI_GREEN_FIX_CONTRACTS.timeoutsRaised).toBe(false);
  expect(ciGreenFixParentReader('.').violations).toEqual([]);
}, 120000);
it('rejects forged proof identity, contracts, scope, hashes and undeclared changes', () => {
  const proof = JSON.parse(readFileSync(CI_GREEN_FIX_PROOF, 'utf8'));
  for (const override of [
    ...['checkpoint', 'parentHead', 'planningParentHead'].map(key => ({ [CI_GREEN_FIX_PROOF]: JSON.stringify({ ...proof, [key]: 'forged' }) })),
    ...Object.entries(CI_GREEN_FIX_CONTRACTS).map(([key, value]) => ({ [CI_GREEN_FIX_PROOF]: JSON.stringify({ ...proof, contracts: { ...proof.contracts, [key]: !value } }) })),
    { [CI_GREEN_FIX_PROOF]: JSON.stringify({ ...proof, files: {} }) },
    ...['beforeHash', 'afterHash'].map(key => ({ [CI_GREEN_FIX_PROOF]: JSON.stringify({ ...proof, files: { ...proof.files, [createPage]: { ...proof.files[createPage], [key]: '0'.repeat(64) } } }) })),
    ...[...CI_GREEN_FIX_CHANGED, ...CI_GREEN_FIX_ADDED, PARENT_PROOF].map(p => ({ [p]: readFileSync(p, 'utf8') + '\n// forged' })),
    { 'src-tauri/src/application/production_package_service.rs': 'changed import' },
    { 'src-tauri/migrations/051_unapproved.sql': 'SELECT 1;' },
  ]) expect(ciGreenFixParentReader('.', override).violations.length).toBeGreaterThan(0);
  expect(ciGreenFixParentReader('.').violations).toEqual([]);
}, 120000);
it('invalidates warm-cache fixtures on live mutations, deletion and illegal files; restoration passes', () => {
  const root = mkdtempSync(join(tmpdir(), 'ci-green-fix-proof-'));
  try {
    const paths = execFileSync('git', ['ls-files', '--cached', '--others', '--exclude-standard'], { encoding: 'utf8' }).trim().split(/\r?\n/).filter(p => p === 'vite.config.ts' || /^(\.github\/workflows\/|src\/|src-tauri\/|scripts\/|docs\/architecture\/)/.test(p));
    for (const p of paths) { mkdirSync(dirname(join(root, p)), { recursive: true }); copyFileSync(p, join(root, p)); }
    execFileSync('git', ['init', '-q', root]);
    const objects = resolve(execFileSync('git', ['rev-parse', '--git-path', 'objects'], { encoding: 'utf8' }).trim());
    writeFileSync(join(root, '.git/objects/info/alternates'), objects.replaceAll('\\', '/') + '\n');
    const validate = () => ciGreenFixParentReader(root).violations;
    expect(validate()).toEqual([]);
    for (const p of [CI_GREEN_FIX_PROOF, PARENT_PROOF, createPage]) {
      const file = join(root, p), original = readFileSync(file);
      try {
        writeFileSync(file, Buffer.concat([original, Buffer.from('\nmutation')])); expect(validate().length).toBeGreaterThan(0);
        rmSync(file); expect(validate().length).toBeGreaterThan(0);
      } finally { writeFileSync(file, original); }
      expect(validate()).toEqual([]);
    }
  } finally { rmSync(root, { recursive: true, force: true }); }
}, 120000);
