// @vitest-environment node
import { expect, it } from 'vitest';
import { readFileSync, writeFileSync, copyFileSync, mkdirSync, mkdtempSync, rmSync } from 'node:fs';
import { join, dirname, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { execFileSync } from 'node:child_process';
import { ciProbeFixParentReader, CI_PROBE_FIX_BASE, CI_PROBE_FIX_PROOF, CI_PROBE_FIX_CHANGED, CI_PROBE_FIX_ADDED, CI_PROBE_FIX_CONTRACTS } from '../ci-probe-fix-successor-guard.mjs';
const PARENT_PROOF = 'docs/architecture/scope-since-chain-successor.json';
const safety = 'src/features/create/StoryboardSafety.test.tsx';
it('validates the standalone ci-probe-fix tip and its declared scope', () => {
  const checked = ciProbeFixParentReader('.');
  expect(checked.violations).toEqual([]);
  expect(checked.changedPaths).toEqual(CI_PROBE_FIX_CHANGED);
  expect(checked.addedPaths).toEqual([...CI_PROBE_FIX_ADDED, CI_PROBE_FIX_PROOF]);
}, 120000);
it('projects changed files to the closed parent tip and preserves its proof bytes', () => {
  const checked = ciProbeFixParentReader('.');
  expect(checked.violations).toEqual([]);
  for (const p of CI_PROBE_FIX_CHANGED) {
    expect(checked.read(p)).toBe(execFileSync('git', ['show', `${CI_PROBE_FIX_BASE}:${p}`], { encoding: 'utf8' }).replaceAll('\r\n', '\n'));
  }
  expect(readFileSync(PARENT_PROOF)).toEqual(execFileSync('git', ['show', `${CI_PROBE_FIX_BASE}:${PARENT_PROOF}`]));
}, 120000);
it('mocks creation.workbench in Safety/CreatePage and drops closed-tip read projections', () => {
  expect(readFileSync(safety, 'utf8')).toContain('api.workbench');
  expect(readFileSync(safety, 'utf8')).toContain('workbenchFrom');
  expect(readFileSync('src/features/create/CreatePage.test.tsx', 'utf8')).toContain('api.workbench');
  expect(readFileSync('scripts/review-hardening-successor-guard.mjs', 'utf8')).toContain('scopeSinceChainScopeSince(root, REVIEW_HARDENING_BASE)');
  expect(CI_PROBE_FIX_CONTRACTS.timeoutsRaised).toBe(false);
  expect(ciProbeFixParentReader('.').violations).toEqual([]);
}, 120000);
it('rejects forged proof identity, contracts, scope, hashes and undeclared changes', () => {
  const proof = JSON.parse(readFileSync(CI_PROBE_FIX_PROOF, 'utf8'));
  for (const override of [
    ...['checkpoint', 'parentHead', 'planningParentHead'].map(key => ({ [CI_PROBE_FIX_PROOF]: JSON.stringify({ ...proof, [key]: 'forged' }) })),
    ...Object.entries(CI_PROBE_FIX_CONTRACTS).map(([key, value]) => ({ [CI_PROBE_FIX_PROOF]: JSON.stringify({ ...proof, contracts: { ...proof.contracts, [key]: !value } }) })),
    { [CI_PROBE_FIX_PROOF]: JSON.stringify({ ...proof, files: {} }) },
    ...['beforeHash', 'afterHash'].map(key => ({ [CI_PROBE_FIX_PROOF]: JSON.stringify({ ...proof, files: { ...proof.files, [safety]: { ...proof.files[safety], [key]: '0'.repeat(64) } } }) })),
    ...[...CI_PROBE_FIX_CHANGED, ...CI_PROBE_FIX_ADDED, PARENT_PROOF].map(p => ({ [p]: readFileSync(p, 'utf8') + '\n// forged' })),
    { 'src-tauri/src/application/production_package_service.rs': 'changed import' },
    { 'src-tauri/migrations/051_unapproved.sql': 'SELECT 1;' },
  ]) expect(ciProbeFixParentReader('.', override).violations.length).toBeGreaterThan(0);
  expect(ciProbeFixParentReader('.').violations).toEqual([]);
}, 120000);
it('invalidates warm-cache fixtures on live mutations, deletion and illegal files; restoration passes', () => {
  const root = mkdtempSync(join(tmpdir(), 'ci-probe-fix-proof-'));
  try {
    const paths = execFileSync('git', ['ls-files', '--cached', '--others', '--exclude-standard'], { encoding: 'utf8' }).trim().split(/\r?\n/).filter(p => p === 'vite.config.ts' || /^(\.github\/workflows\/|src\/|src-tauri\/|scripts\/|docs\/architecture\/)/.test(p));
    for (const p of paths) { mkdirSync(dirname(join(root, p)), { recursive: true }); copyFileSync(p, join(root, p)); }
    execFileSync('git', ['init', '-q', root]);
    const objects = resolve(execFileSync('git', ['rev-parse', '--git-path', 'objects'], { encoding: 'utf8' }).trim());
    writeFileSync(join(root, '.git/objects/info/alternates'), objects.replaceAll('\\', '/') + '\n');
    const validate = () => ciProbeFixParentReader(root).violations;
    expect(validate()).toEqual([]);
    for (const p of [CI_PROBE_FIX_PROOF, PARENT_PROOF, safety]) {
      const file = join(root, p), original = readFileSync(file);
      try {
        writeFileSync(file, Buffer.concat([original, Buffer.from('\nmutation')])); expect(validate().length).toBeGreaterThan(0);
        rmSync(file); expect(validate().length).toBeGreaterThan(0);
      } finally { writeFileSync(file, original); }
      expect(validate()).toEqual([]);
    }
  } finally { rmSync(root, { recursive: true, force: true }); }
}, 120000);
