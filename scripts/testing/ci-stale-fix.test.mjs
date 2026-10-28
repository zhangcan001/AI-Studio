// @vitest-environment node
import { expect, it } from 'vitest';
import { readFileSync, writeFileSync, copyFileSync, mkdirSync, mkdtempSync, rmSync } from 'node:fs';
import { join, dirname, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { execFileSync } from 'node:child_process';
import { ciStaleFixParentReader, CI_STALE_FIX_BASE, CI_STALE_FIX_PROOF, CI_STALE_FIX_CHANGED, CI_STALE_FIX_ADDED, CI_STALE_FIX_CONTRACTS } from '../ci-stale-fix-successor-guard.mjs';
const PARENT_PROOF = 'docs/architecture/ci-phase6a-fix-successor.json';
it('validates the standalone ci-stale-fix tip and its declared scope', () => {
  const checked = ciStaleFixParentReader('.');
  expect(checked.violations).toEqual([]);
  for (const p of CI_STALE_FIX_CHANGED) expect(checked.changedPaths).toContain(p);
  for (const p of [...CI_STALE_FIX_ADDED, CI_STALE_FIX_PROOF]) expect(checked.addedPaths).toContain(p);
}, 120000);
it('projects changed files to the closed parent tip and preserves its proof bytes', () => {
  const checked = ciStaleFixParentReader('.');
  expect(checked.violations).toEqual([]);
  for (const p of CI_STALE_FIX_CHANGED) {
    expect(checked.read(p)).toBe(execFileSync('git', ['show', `${CI_STALE_FIX_BASE}:${p}`], { encoding: 'utf8' }).replaceAll('\r\n', '\n'));
  }
  expect(readFileSync(PARENT_PROOF)).toEqual(execFileSync('git', ['show', `${CI_STALE_FIX_BASE}:${PARENT_PROOF}`]));
}, 120000);
it('keeps stale-binding 5s product it and Phase2 per-path live-mutate without raised timeouts', () => {
  const src = readFileSync('scripts/testing/stale-binding-migration.test.mjs', 'utf8');
  const product = src.match(/it\('moves only same-workflow PRODUCT selections[\s\S]*?\n\}\);/)?.[0] ?? '';
  expect(product.length).toBeGreaterThan(0);
  expect(product).not.toContain('ciAssertFixParentReader');
  expect(product).not.toMatch(/,\s*\d{4,}\s*\)/);
  expect(src).not.toContain('ciAssertFixParentReader');
  const phase2 = readFileSync('src/app/MiniMaxVideoPhase2Boundary.test.ts', 'utf8');
  expect(phase2).toContain('it.each([');
  expect(phase2).toContain('rejects individual undeclared source, design and migration drift');
  expect(phase2).not.toContain('minimaxVideoPhase3FixtureFiles');
  expect(phase2).toContain('Live working-tree mutate');
  expect(CI_STALE_FIX_CONTRACTS.timeoutsRaised).toBe(false);
  expect(CI_STALE_FIX_CONTRACTS.guardsRelaxed).toBe(false);
  expect(ciStaleFixParentReader('.').violations).toEqual([]);
}, 120000);
it('rejects forged proof identity, contracts, scope, hashes and undeclared changes', () => {
  const proof = JSON.parse(readFileSync(CI_STALE_FIX_PROOF, 'utf8'));
  const sample = CI_STALE_FIX_CHANGED[0];
  for (const override of [
    ...['checkpoint', 'parentHead', 'planningParentHead'].map(key => ({ [CI_STALE_FIX_PROOF]: JSON.stringify({ ...proof, [key]: 'forged' }) })),
    ...Object.entries(CI_STALE_FIX_CONTRACTS).map(([key, value]) => ({ [CI_STALE_FIX_PROOF]: JSON.stringify({ ...proof, contracts: { ...proof.contracts, [key]: !value } }) })),
    { [CI_STALE_FIX_PROOF]: JSON.stringify({ ...proof, files: {} }) },
    ...['beforeHash', 'afterHash'].map(key => ({ [CI_STALE_FIX_PROOF]: JSON.stringify({ ...proof, files: { ...proof.files, [sample]: { ...proof.files[sample], [key]: '0'.repeat(64) } } }) })),
    ...[...CI_STALE_FIX_CHANGED, ...CI_STALE_FIX_ADDED, PARENT_PROOF].map(p => ({ [p]: readFileSync(p, 'utf8') + '\n// forged' })),
    { 'src-tauri/migrations/051_unapproved.sql': 'SELECT 1;' },
  ]) expect(ciStaleFixParentReader('.', override).violations.length).toBeGreaterThan(0);
  expect(ciStaleFixParentReader('.').violations).toEqual([]);
}, 120000);
it('invalidates warm-cache fixtures on live mutations, deletion and illegal files; restoration passes', () => {
  const root = mkdtempSync(join(tmpdir(), 'ci-stale-fix-proof-'));
  try {
    const paths = execFileSync('git', ['ls-files', '--cached', '--others', '--exclude-standard'], { encoding: 'utf8' }).trim().split(/\r?\n/).filter(p => p === 'vite.config.ts' || /^(\.github\/workflows\/|src\/|src-tauri\/|scripts\/|docs\/architecture\/)/.test(p));
    for (const p of paths) { mkdirSync(dirname(join(root, p)), { recursive: true }); copyFileSync(p, join(root, p)); }
    execFileSync('git', ['init', '-q', root]);
    const objects = resolve(execFileSync('git', ['rev-parse', '--git-path', 'objects'], { encoding: 'utf8' }).trim());
    writeFileSync(join(root, '.git/objects/info/alternates'), objects.replaceAll('\\', '/') + '\n');
    const validate = () => ciStaleFixParentReader(root).violations;
    expect(validate()).toEqual([]);
    for (const p of [CI_STALE_FIX_PROOF, PARENT_PROOF, 'scripts/testing/stale-binding-migration.test.mjs']) {
      const file = join(root, p), original = readFileSync(file);
      try {
        writeFileSync(file, Buffer.concat([original, Buffer.from('\nmutation')])); expect(validate().length).toBeGreaterThan(0);
        rmSync(file); expect(validate().length).toBeGreaterThan(0);
      } finally { writeFileSync(file, original); }
      expect(validate()).toEqual([]);
    }
  } finally { rmSync(root, { recursive: true, force: true }); }
}, 120000);
