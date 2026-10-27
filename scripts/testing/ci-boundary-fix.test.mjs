// @vitest-environment node
import { expect, it } from 'vitest';
import { readFileSync, writeFileSync, copyFileSync, mkdirSync, mkdtempSync, rmSync } from 'node:fs';
import { join, dirname, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { execFileSync } from 'node:child_process';
import { ciBoundaryFixParentReader, CI_BOUNDARY_FIX_BASE, CI_BOUNDARY_FIX_PROOF, CI_BOUNDARY_FIX_CHANGED, CI_BOUNDARY_FIX_ADDED, CI_BOUNDARY_FIX_CONTRACTS } from '../ci-boundary-fix-successor-guard.mjs';
const PARENT_PROOF = 'docs/architecture/ci-phase1-fix-successor.json';
it('validates the standalone ci-boundary-fix tip and its declared scope', () => {
  const checked = ciBoundaryFixParentReader('.');
  expect(checked.violations).toEqual([]);
  expect(checked.changedPaths).toEqual(CI_BOUNDARY_FIX_CHANGED);
  expect(checked.addedPaths).toEqual([...CI_BOUNDARY_FIX_ADDED, CI_BOUNDARY_FIX_PROOF]);
}, 120000);
it('projects changed files to the closed parent tip and preserves its proof bytes', () => {
  const checked = ciBoundaryFixParentReader('.');
  expect(checked.violations).toEqual([]);
  for (const p of CI_BOUNDARY_FIX_CHANGED) {
    expect(checked.read(p)).toBe(execFileSync('git', ['show', `${CI_BOUNDARY_FIX_BASE}:${p}`], { encoding: 'utf8' }).replaceAll('\r\n', '\n'));
  }
  expect(readFileSync(PARENT_PROOF)).toEqual(execFileSync('git', ['show', `${CI_BOUNDARY_FIX_BASE}:${PARENT_PROOF}`]));
}, 120000);
it('keeps Readiness/5B/5G live-mutate without Phase2 fixture copy or raised timeouts', () => {
  for (const f of ['src/app/ReadinessLifecycleBoundary.test.ts','src/app/UiPhase5BMixedVideoBoundary.test.ts','src/app/UiPhase5GCiTimeoutBoundary.test.ts']) {
    const src = readFileSync(f, 'utf8');
    expect(src).not.toContain('minimaxVideoPhase2FixtureFiles');
    expect(src).not.toContain('phase5BFixtureFiles');
    expect(src).not.toContain('phase5GFixtureFiles');
  }
  expect(CI_BOUNDARY_FIX_CONTRACTS.timeoutsRaised).toBe(false);
  expect(CI_BOUNDARY_FIX_CONTRACTS.guardsRelaxed).toBe(false);
  expect(ciBoundaryFixParentReader('.').violations).toEqual([]);
}, 120000);
it('rejects forged proof identity, contracts, scope, hashes and undeclared changes', () => {
  const proof = JSON.parse(readFileSync(CI_BOUNDARY_FIX_PROOF, 'utf8'));
  const sample = CI_BOUNDARY_FIX_CHANGED[0];
  for (const override of [
    ...['checkpoint', 'parentHead', 'planningParentHead'].map(key => ({ [CI_BOUNDARY_FIX_PROOF]: JSON.stringify({ ...proof, [key]: 'forged' }) })),
    ...Object.entries(CI_BOUNDARY_FIX_CONTRACTS).map(([key, value]) => ({ [CI_BOUNDARY_FIX_PROOF]: JSON.stringify({ ...proof, contracts: { ...proof.contracts, [key]: !value } }) })),
    { [CI_BOUNDARY_FIX_PROOF]: JSON.stringify({ ...proof, files: {} }) },
    ...['beforeHash', 'afterHash'].map(key => ({ [CI_BOUNDARY_FIX_PROOF]: JSON.stringify({ ...proof, files: { ...proof.files, [sample]: { ...proof.files[sample], [key]: '0'.repeat(64) } } }) })),
    ...[...CI_BOUNDARY_FIX_CHANGED, ...CI_BOUNDARY_FIX_ADDED, PARENT_PROOF].map(p => ({ [p]: readFileSync(p, 'utf8') + '\n// forged' })),
    { 'src-tauri/migrations/051_unapproved.sql': 'SELECT 1;' },
  ]) expect(ciBoundaryFixParentReader('.', override).violations.length).toBeGreaterThan(0);
  expect(ciBoundaryFixParentReader('.').violations).toEqual([]);
}, 120000);
it('invalidates warm-cache fixtures on live mutations, deletion and illegal files; restoration passes', () => {
  const root = mkdtempSync(join(tmpdir(), 'ci-boundary-fix-proof-'));
  try {
    const paths = execFileSync('git', ['ls-files', '--cached', '--others', '--exclude-standard'], { encoding: 'utf8' }).trim().split(/\r?\n/).filter(p => p === 'vite.config.ts' || /^(\.github\/workflows\/|src\/|src-tauri\/|scripts\/|docs\/architecture\/)/.test(p));
    for (const p of paths) { mkdirSync(dirname(join(root, p)), { recursive: true }); copyFileSync(p, join(root, p)); }
    execFileSync('git', ['init', '-q', root]);
    const objects = resolve(execFileSync('git', ['rev-parse', '--git-path', 'objects'], { encoding: 'utf8' }).trim());
    writeFileSync(join(root, '.git/objects/info/alternates'), objects.replaceAll('\\', '/') + '\n');
    const validate = () => ciBoundaryFixParentReader(root).violations;
    expect(validate()).toEqual([]);
    for (const p of [CI_BOUNDARY_FIX_PROOF, PARENT_PROOF, 'src/app/ReadinessLifecycleBoundary.test.ts']) {
      const file = join(root, p), original = readFileSync(file);
      try {
        writeFileSync(file, Buffer.concat([original, Buffer.from('\nmutation')])); expect(validate().length).toBeGreaterThan(0);
        rmSync(file); expect(validate().length).toBeGreaterThan(0);
      } finally { writeFileSync(file, original); }
      expect(validate()).toEqual([]);
    }
  } finally { rmSync(root, { recursive: true, force: true }); }
}, 120000);
