// @vitest-environment node
import { expect, it } from 'vitest';
import { readFileSync, writeFileSync, copyFileSync, mkdirSync, mkdtempSync, rmSync } from 'node:fs';
import { join, dirname, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { execFileSync } from 'node:child_process';
import { ciShardFixParentReader, CI_SHARD_FIX_BASE, CI_SHARD_FIX_PROOF, CI_SHARD_FIX_CHANGED, CI_SHARD_FIX_ADDED, CI_SHARD_FIX_CONTRACTS } from '../ci-shard-fix-successor-guard.mjs';
const PARENT_PROOF = 'docs/architecture/ci-stale-fix-successor.json';
it('validates the standalone ci-shard-fix tip and its declared scope', () => {
  const checked = ciShardFixParentReader('.');
  expect(checked.violations).toEqual([]);
  expect(checked.changedPaths).toEqual(CI_SHARD_FIX_CHANGED);
  expect(checked.addedPaths).toEqual([...CI_SHARD_FIX_ADDED, CI_SHARD_FIX_PROOF]);
}, 120000);
it('projects changed files to the closed parent tip and preserves its proof bytes', () => {
  const checked = ciShardFixParentReader('.');
  expect(checked.violations).toEqual([]);
  for (const p of CI_SHARD_FIX_CHANGED) {
    expect(checked.read(p)).toBe(execFileSync('git', ['show', `${CI_SHARD_FIX_BASE}:${p}`], { encoding: 'utf8' }).replaceAll('\r\n', '\n'));
  }
  expect(readFileSync(PARENT_PROOF)).toEqual(execFileSync('git', ['show', `${CI_SHARD_FIX_BASE}:${PARENT_PROOF}`]));
}, 120000);
it('keeps Phase5E/5B live-mutate and Veda 5s product it without FixtureFiles copy or raised timeouts', () => {
  const src = readFileSync('src/app/UiPhase5ECiShardBoundary.test.ts', 'utf8');
  expect(src).not.toContain('phase5EFixtureFiles');
  expect(src).toContain('Live working-tree mutate');
  const veda = readFileSync('scripts/testing/veda-removed-white-flash.test.mjs', 'utf8');
  const product = veda.match(/it\('ships Veda reference 1\.3\.0[\s\S]*?\n\}\);/)?.[0] ?? '';
  expect(product.length).toBeGreaterThan(0);
  expect(product).not.toContain('ParentReader(');
  expect(veda).toMatch(/const tip = ciScopeFixParentReader\('\.'\);\s*expect\(tip\.violations\)\.toEqual\(\[\]\);/);
  const fiveB = readFileSync('src/app/UiPhase5BMixedVideoBoundary.test.ts', 'utf8');
  expect(fiveB).toContain('keeps live freshness for frozen proofs, Runtime raw bytes: %s');
  expect(fiveB).toContain('keeps live freshness for manifest deletion');
  expect(CI_SHARD_FIX_CONTRACTS.timeoutsRaised).toBe(false);
  expect(CI_SHARD_FIX_CONTRACTS.guardsRelaxed).toBe(false);
  expect(ciShardFixParentReader('.').violations).toEqual([]);
}, 120000);
it('rejects forged proof identity, contracts, scope, hashes and undeclared changes', () => {
  const proof = JSON.parse(readFileSync(CI_SHARD_FIX_PROOF, 'utf8'));
  const sample = CI_SHARD_FIX_CHANGED[0];
  for (const override of [
    ...['checkpoint', 'parentHead', 'planningParentHead'].map(key => ({ [CI_SHARD_FIX_PROOF]: JSON.stringify({ ...proof, [key]: 'forged' }) })),
    ...Object.entries(CI_SHARD_FIX_CONTRACTS).map(([key, value]) => ({ [CI_SHARD_FIX_PROOF]: JSON.stringify({ ...proof, contracts: { ...proof.contracts, [key]: !value } }) })),
    { [CI_SHARD_FIX_PROOF]: JSON.stringify({ ...proof, files: {} }) },
    ...['beforeHash', 'afterHash'].map(key => ({ [CI_SHARD_FIX_PROOF]: JSON.stringify({ ...proof, files: { ...proof.files, [sample]: { ...proof.files[sample], [key]: '0'.repeat(64) } } }) })),
    ...[...CI_SHARD_FIX_CHANGED, ...CI_SHARD_FIX_ADDED, PARENT_PROOF].map(p => ({ [p]: readFileSync(p, 'utf8') + '\n// forged' })),
    { 'src-tauri/migrations/051_unapproved.sql': 'SELECT 1;' },
  ]) expect(ciShardFixParentReader('.', override).violations.length).toBeGreaterThan(0);
  expect(ciShardFixParentReader('.').violations).toEqual([]);
}, 120000);
it('invalidates warm-cache fixtures on live mutations, deletion and illegal files; restoration passes', () => {
  const root = mkdtempSync(join(tmpdir(), 'ci-shard-fix-proof-'));
  try {
    const paths = execFileSync('git', ['ls-files', '--cached', '--others', '--exclude-standard'], { encoding: 'utf8' }).trim().split(/\r?\n/).filter(p => p === 'vite.config.ts' || /^(\.github\/workflows\/|src\/|src-tauri\/|scripts\/|docs\/architecture\/)/.test(p));
    for (const p of paths) { mkdirSync(dirname(join(root, p)), { recursive: true }); copyFileSync(p, join(root, p)); }
    execFileSync('git', ['init', '-q', root]);
    const objects = resolve(execFileSync('git', ['rev-parse', '--git-path', 'objects'], { encoding: 'utf8' }).trim());
    writeFileSync(join(root, '.git/objects/info/alternates'), objects.replaceAll('\\', '/') + '\n');
    const validate = () => ciShardFixParentReader(root).violations;
    expect(validate()).toEqual([]);
    for (const p of [CI_SHARD_FIX_PROOF, PARENT_PROOF, 'src/app/UiPhase5ECiShardBoundary.test.ts']) {
      const file = join(root, p), original = readFileSync(file);
      try {
        writeFileSync(file, Buffer.concat([original, Buffer.from('\nmutation')])); expect(validate().length).toBeGreaterThan(0);
        rmSync(file); expect(validate().length).toBeGreaterThan(0);
      } finally { writeFileSync(file, original); }
      expect(validate()).toEqual([]);
    }
  } finally { rmSync(root, { recursive: true, force: true }); }
}, 120000);
