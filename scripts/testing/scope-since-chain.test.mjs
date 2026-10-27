// @vitest-environment node
import { expect, it } from 'vitest';
import { readFileSync, writeFileSync, copyFileSync, mkdirSync, mkdtempSync, rmSync } from 'node:fs';
import { join, dirname, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { execFileSync } from 'node:child_process';
import { scopeSinceChainParentReader, SCOPE_SINCE_CHAIN_BASE, SCOPE_SINCE_CHAIN_PROOF, SCOPE_SINCE_CHAIN_CHANGED, SCOPE_SINCE_CHAIN_ADDED, SCOPE_SINCE_CHAIN_CONTRACTS } from '../scope-since-chain-successor-guard.mjs';
const PARENT_PROOF = 'docs/architecture/successor-chain-successor.json';
const chainGuard = 'scripts/review-hardening-successor-guard.mjs';
it('validates the standalone scope-since-chain tip and its declared scope', () => {
  const checked = scopeSinceChainParentReader('.');
  expect(checked.violations).toEqual([]);
  expect(checked.changedPaths).toEqual(SCOPE_SINCE_CHAIN_CHANGED);
  expect(checked.addedPaths).toEqual([...SCOPE_SINCE_CHAIN_ADDED, SCOPE_SINCE_CHAIN_PROOF]);
}, 120000);
it('projects changed files to the closed parent tip and preserves its proof bytes', () => {
  const checked = scopeSinceChainParentReader('.');
  expect(checked.violations).toEqual([]);
  for (const p of SCOPE_SINCE_CHAIN_CHANGED) {
    expect(checked.read(p)).toBe(execFileSync('git', ['show', `${SCOPE_SINCE_CHAIN_BASE}:${p}`], { encoding: 'utf8' }).replaceAll('\r\n', '\n'));
  }
  expect(readFileSync(PARENT_PROOF)).toEqual(execFileSync('git', ['show', `${SCOPE_SINCE_CHAIN_BASE}:${PARENT_PROOF}`]));
}, 120000);
it('extends review-hardening ScopeSince through cancel…successor-chain so older probes see tip files', () => {
  const src = readFileSync(chainGuard, 'utf8');
  expect(src).toContain('cancelCorrectnessScopeSince');
  expect(src).toContain('...cancelCorrectnessScopeSince(root, CANCEL_CORRECTNESS_BASE)');
  expect(scopeSinceChainParentReader('.').violations).toEqual([]);
}, 120000);
it('rejects forged proof identity, contracts, scope, hashes and undeclared changes', () => {
  const proof = JSON.parse(readFileSync(SCOPE_SINCE_CHAIN_PROOF, 'utf8'));
  for (const override of [
    ...['checkpoint', 'parentHead', 'planningParentHead'].map(key => ({ [SCOPE_SINCE_CHAIN_PROOF]: JSON.stringify({ ...proof, [key]: 'forged' }) })),
    ...Object.entries(SCOPE_SINCE_CHAIN_CONTRACTS).map(([key, value]) => ({ [SCOPE_SINCE_CHAIN_PROOF]: JSON.stringify({ ...proof, contracts: { ...proof.contracts, [key]: !value } }) })),
    { [SCOPE_SINCE_CHAIN_PROOF]: JSON.stringify({ ...proof, files: {} }) },
    ...['beforeHash', 'afterHash'].map(key => ({ [SCOPE_SINCE_CHAIN_PROOF]: JSON.stringify({ ...proof, files: { ...proof.files, [chainGuard]: { ...proof.files[chainGuard], [key]: '0'.repeat(64) } } }) })),
    ...[...SCOPE_SINCE_CHAIN_CHANGED, ...SCOPE_SINCE_CHAIN_ADDED, PARENT_PROOF].map(p => ({ [p]: readFileSync(p, 'utf8') + '\n// forged' })),
    { 'src-tauri/src/application/production_package_service.rs': 'changed import' },
    { 'src-tauri/migrations/051_unapproved.sql': 'SELECT 1;' },
  ]) expect(scopeSinceChainParentReader('.', override).violations.length).toBeGreaterThan(0);
  expect(scopeSinceChainParentReader('.').violations).toEqual([]);
}, 120000);
it('invalidates warm-cache fixtures on live mutations, deletion and illegal files; restoration passes', () => {
  const root = mkdtempSync(join(tmpdir(), 'scope-since-chain-proof-'));
  try {
    const paths = execFileSync('git', ['ls-files', '--cached', '--others', '--exclude-standard'], { encoding: 'utf8' }).trim().split(/\r?\n/).filter(p => p === 'vite.config.ts' || /^(\.github\/workflows\/|src\/|src-tauri\/|scripts\/|docs\/architecture\/)/.test(p));
    for (const p of paths) { mkdirSync(dirname(join(root, p)), { recursive: true }); copyFileSync(p, join(root, p)); }
    execFileSync('git', ['init', '-q', root]);
    const objects = resolve(execFileSync('git', ['rev-parse', '--git-path', 'objects'], { encoding: 'utf8' }).trim());
    writeFileSync(join(root, '.git/objects/info/alternates'), objects.replaceAll('\\', '/') + '\n');
    const validate = () => scopeSinceChainParentReader(root).violations;
    expect(validate()).toEqual([]);
    for (const p of [SCOPE_SINCE_CHAIN_PROOF, PARENT_PROOF, chainGuard]) {
      const file = join(root, p), original = readFileSync(file);
      try {
        writeFileSync(file, Buffer.concat([original, Buffer.from('\nmutation')])); expect(validate().length).toBeGreaterThan(0);
        rmSync(file); expect(validate().length).toBeGreaterThan(0);
      } finally { writeFileSync(file, original); }
      expect(validate()).toEqual([]);
    }
  } finally { rmSync(root, { recursive: true, force: true }); }
}, 120000);
