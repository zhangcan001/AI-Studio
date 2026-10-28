// @vitest-environment node
// One workspace copy shared by every ci-*-fix successor-tip check (previously one copy per file).
import { afterAll, describe, expect, it } from 'vitest';
import { readFileSync, writeFileSync, copyFileSync, mkdirSync, mkdtempSync, rmSync } from 'node:fs';
import { join, dirname, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { execFileSync } from 'node:child_process';
import { ciAssertFixParentReader, CI_ASSERT_FIX_BASE, CI_ASSERT_FIX_PROOF, CI_ASSERT_FIX_CHANGED, CI_ASSERT_FIX_ADDED, CI_ASSERT_FIX_CONTRACTS } from '../ci-assert-fix-successor-guard.mjs';
import { ciBoundaryFixParentReader, CI_BOUNDARY_FIX_BASE, CI_BOUNDARY_FIX_PROOF, CI_BOUNDARY_FIX_CHANGED, CI_BOUNDARY_FIX_ADDED, CI_BOUNDARY_FIX_CONTRACTS } from '../ci-boundary-fix-successor-guard.mjs';
import { ciChainFixParentReader, CI_CHAIN_FIX_BASE, CI_CHAIN_FIX_PROOF, CI_CHAIN_FIX_CHANGED, CI_CHAIN_FIX_ADDED, CI_CHAIN_FIX_CONTRACTS } from '../ci-chain-fix-successor-guard.mjs';
import { ciClosedTipFixParentReader, CI_CLOSED_TIP_FIX_BASE, CI_CLOSED_TIP_FIX_PROOF, CI_CLOSED_TIP_FIX_CHANGED, CI_CLOSED_TIP_FIX_ADDED, CI_CLOSED_TIP_FIX_CONTRACTS } from '../ci-closed-tip-fix-successor-guard.mjs';
import { ciGreenFixParentReader, CI_GREEN_FIX_BASE, CI_GREEN_FIX_PROOF, CI_GREEN_FIX_CHANGED, CI_GREEN_FIX_ADDED, CI_GREEN_FIX_CONTRACTS } from '../ci-green-fix-successor-guard.mjs';
import { ciH3BudgetFixParentReader, CI_H3_BUDGET_FIX_BASE, CI_H3_BUDGET_FIX_PROOF, CI_H3_BUDGET_FIX_CHANGED, CI_H3_BUDGET_FIX_ADDED, CI_H3_BUDGET_FIX_CONTRACTS } from '../ci-h3-budget-fix-successor-guard.mjs';
import { ciPhase1FixParentReader, CI_PHASE1_FIX_BASE, CI_PHASE1_FIX_PROOF, CI_PHASE1_FIX_CHANGED, CI_PHASE1_FIX_ADDED, CI_PHASE1_FIX_CONTRACTS } from '../ci-phase1-fix-successor-guard.mjs';
import { ciPhase5HFixParentReader, CI_PHASE5H_FIX_BASE, CI_PHASE5H_FIX_PROOF, CI_PHASE5H_FIX_CHANGED, CI_PHASE5H_FIX_ADDED, CI_PHASE5H_FIX_CONTRACTS } from '../ci-phase5h-fix-successor-guard.mjs';
import { ciPhase6AFixParentReader, CI_PHASE6A_FIX_BASE, CI_PHASE6A_FIX_PROOF, CI_PHASE6A_FIX_CHANGED, CI_PHASE6A_FIX_ADDED, CI_PHASE6A_FIX_CONTRACTS } from '../ci-phase6a-fix-successor-guard.mjs';
import { ciProbeFixParentReader, CI_PROBE_FIX_BASE, CI_PROBE_FIX_PROOF, CI_PROBE_FIX_CHANGED, CI_PROBE_FIX_ADDED, CI_PROBE_FIX_CONTRACTS } from '../ci-probe-fix-successor-guard.mjs';
import { ciQueueFixParentReader, CI_QUEUE_FIX_BASE, CI_QUEUE_FIX_PROOF, CI_QUEUE_FIX_CHANGED, CI_QUEUE_FIX_ADDED, CI_QUEUE_FIX_CONTRACTS } from '../ci-queue-fix-successor-guard.mjs';
import { ciScopeFixParentReader, CI_SCOPE_FIX_BASE, CI_SCOPE_FIX_PROOF, CI_SCOPE_FIX_CHANGED, CI_SCOPE_FIX_ADDED, CI_SCOPE_FIX_CONTRACTS } from '../ci-scope-fix-successor-guard.mjs';
import { ciShardFixParentReader, CI_SHARD_FIX_BASE, CI_SHARD_FIX_PROOF, CI_SHARD_FIX_CHANGED, CI_SHARD_FIX_ADDED, CI_SHARD_FIX_CONTRACTS } from '../ci-shard-fix-successor-guard.mjs';
import { ciStableFixParentReader, CI_STABLE_FIX_BASE, CI_STABLE_FIX_PROOF, CI_STABLE_FIX_CHANGED, CI_STABLE_FIX_ADDED, CI_STABLE_FIX_CONTRACTS } from '../ci-stable-fix-successor-guard.mjs';
import { ciStaleFixParentReader, CI_STALE_FIX_BASE, CI_STALE_FIX_PROOF, CI_STALE_FIX_CHANGED, CI_STALE_FIX_ADDED, CI_STALE_FIX_CONTRACTS } from '../ci-stale-fix-successor-guard.mjs';
import { ciConsolidateFixParentReader, CI_CONSOLIDATE_FIX_BASE, CI_CONSOLIDATE_FIX_PROOF, CI_CONSOLIDATE_FIX_CHANGED, CI_CONSOLIDATE_FIX_ADDED, CI_CONSOLIDATE_FIX_CONTRACTS } from '../ci-consolidate-fix-successor-guard.mjs';

const CONSOLIDATED_TIPS = ['ci-assert-fix', 'ci-boundary-fix', 'ci-chain-fix', 'ci-closed-tip-fix', 'ci-green-fix', 'ci-h3-budget-fix', 'ci-phase1-fix', 'ci-phase5h-fix', 'ci-phase6a-fix', 'ci-probe-fix', 'ci-queue-fix', 'ci-scope-fix', 'ci-shard-fix', 'ci-stable-fix', 'ci-stale-fix'];
const consolidatedRead = p => ciConsolidateFixParentReader('.').read(p);
let shared;
function sharedRoot() {
  if (shared) return shared;
  const root = mkdtempSync(join(tmpdir(), 'ci-chain-consolidated-proof-'));
  const paths = execFileSync('git', ['ls-files', '--cached', '--others', '--exclude-standard'], { encoding: 'utf8' }).trim().split(/\r?\n/).filter(p => p === 'vite.config.ts' || /^(\.github\/workflows\/|src\/|src-tauri\/|scripts\/|docs\/architecture\/)/.test(p));
  for (const p of paths) { mkdirSync(dirname(join(root, p)), { recursive: true }); copyFileSync(p, join(root, p)); }
  execFileSync('git', ['init', '-q', root]);
  const objects = resolve(execFileSync('git', ['rev-parse', '--git-path', 'objects'], { encoding: 'utf8' }).trim());
  writeFileSync(join(root, '.git/objects/info/alternates'), objects.replaceAll('\\', '/') + '\n');
  return shared = root;
}
afterAll(() => { if (shared) rmSync(shared, { recursive: true, force: true }); });

describe('ci-assert-fix', () => {
const PARENT_PROOF = 'docs/architecture/ci-green-fix-successor.json';
const phase5c = 'src/app/UiPhase5CReviewBoundary.test.ts';
it('validates the standalone ci-assert-fix tip and its declared scope', () => {
  const checked = ciAssertFixParentReader('.');
  expect(checked.violations).toEqual([]);
  for (const p of CI_ASSERT_FIX_CHANGED) expect(checked.changedPaths).toContain(p);
  for (const p of [...CI_ASSERT_FIX_ADDED, CI_ASSERT_FIX_PROOF]) expect(checked.addedPaths).toContain(p);
}, 120000);
it('projects changed files to the closed parent tip and preserves its proof bytes', () => {
  const checked = ciAssertFixParentReader('.');
  expect(checked.violations).toEqual([]);
  for (const p of CI_ASSERT_FIX_CHANGED) {
    expect(checked.read(p)).toBe(execFileSync('git', ['show', `${CI_ASSERT_FIX_BASE}:${p}`], { encoding: 'utf8' }).replaceAll('\r\n', '\n'));
  }
  expect(readFileSync(PARENT_PROOF)).toEqual(execFileSync('git', ['show', `${CI_ASSERT_FIX_BASE}:${PARENT_PROOF}`]));
}, 120000);
it('restores real Phase5C live-mutate freshness and keeps timeoutsRaised false', () => {
  const src = readFileSync(phase5c, 'utf8');
  expect(src).toContain("writeFileSync(p, bytes.toString() + '\\n')");
  expect(src).toContain('ui-phase5c-review-inventory:${dir}');
  expect(src).toContain('rmSync(PHASE5C_MANIFEST)');
  expect(src).not.toContain("uiPhase5CParentReader('.', { [p]: readFileSync(p, 'utf8')");
  expect(CI_ASSERT_FIX_CONTRACTS.timeoutsRaised).toBe(false);
  expect(CI_ASSERT_FIX_CONTRACTS.guardsRelaxed).toBe(false);
  expect(ciAssertFixParentReader('.').violations).toEqual([]);
}, 120000);
it('rejects forged proof identity, contracts, scope, hashes and undeclared changes', () => {
  const proof = JSON.parse(readFileSync(CI_ASSERT_FIX_PROOF, 'utf8'));
  for (const override of [
    ...['checkpoint', 'parentHead', 'planningParentHead'].map(key => ({ [CI_ASSERT_FIX_PROOF]: JSON.stringify({ ...proof, [key]: 'forged' }) })),
    ...Object.entries(CI_ASSERT_FIX_CONTRACTS).map(([key, value]) => ({ [CI_ASSERT_FIX_PROOF]: JSON.stringify({ ...proof, contracts: { ...proof.contracts, [key]: !value } }) })),
    { [CI_ASSERT_FIX_PROOF]: JSON.stringify({ ...proof, files: {} }) },
    ...['beforeHash', 'afterHash'].map(key => ({ [CI_ASSERT_FIX_PROOF]: JSON.stringify({ ...proof, files: { ...proof.files, [phase5c]: { ...proof.files[phase5c], [key]: '0'.repeat(64) } } }) })),
    ...[...CI_ASSERT_FIX_CHANGED, ...CI_ASSERT_FIX_ADDED, PARENT_PROOF].map(p => ({ [p]: readFileSync(p, 'utf8') + '\n// forged' })),
    { 'src-tauri/src/application/production_package_service.rs': 'changed import' },
    { 'src-tauri/migrations/051_unapproved.sql': 'SELECT 1;' },
  ]) expect(ciAssertFixParentReader('.', override).violations.length).toBeGreaterThan(0);
  expect(ciAssertFixParentReader('.').violations).toEqual([]);
}, 120000);
it('invalidates warm-cache fixtures on live mutations, deletion and illegal files; restoration passes', () => {
  const root = sharedRoot();
  {
    const validate = () => ciAssertFixParentReader(root).violations;
    expect(validate()).toEqual([]);
    for (const p of [CI_ASSERT_FIX_PROOF, PARENT_PROOF, phase5c]) {
      const file = join(root, p), original = readFileSync(file);
      try {
        writeFileSync(file, Buffer.concat([original, Buffer.from('\nmutation')])); expect(validate().length).toBeGreaterThan(0);
        rmSync(file); expect(validate().length).toBeGreaterThan(0);
      } finally { writeFileSync(file, original); }
      expect(validate()).toEqual([]);
    }
  }
}, 120000);
});

describe('ci-boundary-fix', () => {
const PARENT_PROOF = 'docs/architecture/ci-phase1-fix-successor.json';
it('validates the standalone ci-boundary-fix tip and its declared scope', () => {
  const checked = ciBoundaryFixParentReader('.');
  expect(checked.violations).toEqual([]);
  for (const p of CI_BOUNDARY_FIX_CHANGED) expect(checked.changedPaths).toContain(p);
  for (const p of [...CI_BOUNDARY_FIX_ADDED, CI_BOUNDARY_FIX_PROOF]) expect(checked.addedPaths).toContain(p);
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
  const root = sharedRoot();
  {
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
  }
}, 120000);
});

describe('ci-chain-fix', () => {
const PARENT_PROOF = 'docs/architecture/ci-scope-fix-successor.json';
it('validates the standalone ci-chain-fix tip and its declared scope', () => {
  const checked = ciChainFixParentReader('.');
  expect(checked.violations).toEqual([]);
  for (const p of CI_CHAIN_FIX_CHANGED) expect(checked.changedPaths).toContain(p);
  for (const p of [...CI_CHAIN_FIX_ADDED, CI_CHAIN_FIX_PROOF]) expect(checked.addedPaths).toContain(p);
}, 120000);
it('projects changed files to the closed parent tip and preserves its proof bytes', () => {
  const checked = ciChainFixParentReader('.');
  expect(checked.violations).toEqual([]);
  for (const p of CI_CHAIN_FIX_CHANGED) {
    expect(checked.read(p)).toBe(execFileSync('git', ['show', `${CI_CHAIN_FIX_BASE}:${p}`], { encoding: 'utf8' }).replaceAll('\r\n', '\n'));
  }
  expect(readFileSync(PARENT_PROOF)).toEqual(execFileSync('git', ['show', `${CI_CHAIN_FIX_BASE}:${PARENT_PROOF}`]));
}, 120000);
it('keeps tip-jump ScopeSince needle and tip-leaf warm-cache without raising timeouts', () => {
  expect(readFileSync('scripts/testing/scope-since-chain.test.mjs', 'utf8')).toContain('...scopeSinceChainScopeSince(root, REVIEW_HARDENING_BASE)');
  expect(readFileSync('scripts/testing/batch-create-reason.test.mjs', 'utf8')).toContain('ciChainFixParentReader');
  expect(CI_CHAIN_FIX_CONTRACTS.timeoutsRaised).toBe(false);
  expect(CI_CHAIN_FIX_CONTRACTS.guardsRelaxed).toBe(false);
  expect(ciChainFixParentReader('.').violations).toEqual([]);
}, 120000);
it('rejects forged proof identity, contracts, scope, hashes and undeclared changes', () => {
  const proof = JSON.parse(readFileSync(CI_CHAIN_FIX_PROOF, 'utf8'));
  const sample = CI_CHAIN_FIX_CHANGED[0];
  for (const override of [
    ...['checkpoint', 'parentHead', 'planningParentHead'].map(key => ({ [CI_CHAIN_FIX_PROOF]: JSON.stringify({ ...proof, [key]: 'forged' }) })),
    ...Object.entries(CI_CHAIN_FIX_CONTRACTS).map(([key, value]) => ({ [CI_CHAIN_FIX_PROOF]: JSON.stringify({ ...proof, contracts: { ...proof.contracts, [key]: !value } }) })),
    { [CI_CHAIN_FIX_PROOF]: JSON.stringify({ ...proof, files: {} }) },
    ...['beforeHash', 'afterHash'].map(key => ({ [CI_CHAIN_FIX_PROOF]: JSON.stringify({ ...proof, files: { ...proof.files, [sample]: { ...proof.files[sample], [key]: '0'.repeat(64) } } }) })),
    ...[...CI_CHAIN_FIX_CHANGED, ...CI_CHAIN_FIX_ADDED, PARENT_PROOF].map(p => ({ [p]: readFileSync(p, 'utf8') + '\n// forged' })),
    { 'src-tauri/migrations/051_unapproved.sql': 'SELECT 1;' },
  ]) expect(ciChainFixParentReader('.', override).violations.length).toBeGreaterThan(0);
  expect(ciChainFixParentReader('.').violations).toEqual([]);
}, 120000);
it('invalidates warm-cache fixtures on live mutations, deletion and illegal files; restoration passes', () => {
  const root = sharedRoot();
  {
    const validate = () => ciChainFixParentReader(root).violations;
    expect(validate()).toEqual([]);
    for (const p of [CI_CHAIN_FIX_PROOF, PARENT_PROOF, 'scripts/testing/scope-since-chain.test.mjs']) {
      const file = join(root, p), original = readFileSync(file);
      try {
        writeFileSync(file, Buffer.concat([original, Buffer.from('\nmutation')])); expect(validate().length).toBeGreaterThan(0);
        rmSync(file); expect(validate().length).toBeGreaterThan(0);
      } finally { writeFileSync(file, original); }
      expect(validate()).toEqual([]);
    }
  }
}, 120000);
});

describe('ci-closed-tip-fix', () => {
const PARENT_PROOF = 'docs/architecture/ci-assert-fix-successor.json';
it('validates the standalone ci-closed-tip-fix tip and its declared scope', () => {
  const checked = ciClosedTipFixParentReader('.');
  expect(checked.violations).toEqual([]);
  for (const p of CI_CLOSED_TIP_FIX_CHANGED) expect(checked.changedPaths).toContain(p);
  for (const p of [...CI_CLOSED_TIP_FIX_ADDED, CI_CLOSED_TIP_FIX_PROOF]) expect(checked.addedPaths).toContain(p);
}, 120000);
it('projects changed files to the closed parent tip and preserves its proof bytes', () => {
  const checked = ciClosedTipFixParentReader('.');
  expect(checked.violations).toEqual([]);
  for (const p of CI_CLOSED_TIP_FIX_CHANGED) {
    expect(checked.read(p)).toBe(execFileSync('git', ['show', `${CI_CLOSED_TIP_FIX_BASE}:${p}`], { encoding: 'utf8' }).replaceAll('\r\n', '\n'));
  }
  expect(readFileSync(PARENT_PROOF)).toEqual(execFileSync('git', ['show', `${CI_CLOSED_TIP_FIX_BASE}:${PARENT_PROOF}`]));
}, 120000);
it('keeps successor-aware closed-tip scope asserts and runtime drift fail-closed without raising timeouts', () => {
  expect(consolidatedRead('scripts/testing/ci-green-fix.test.mjs')).toContain('toContain(p)');
  expect(readFileSync('scripts/testing/folder-batch-import.test.mjs', 'utf8')).toContain('Buffer.concat([original, Buffer.from');
  expect(CI_CLOSED_TIP_FIX_CONTRACTS.timeoutsRaised).toBe(false);
  expect(CI_CLOSED_TIP_FIX_CONTRACTS.guardsRelaxed).toBe(false);
  expect(ciClosedTipFixParentReader('.').violations).toEqual([]);
}, 120000);
it('rejects forged proof identity, contracts, scope, hashes and undeclared changes', () => {
  const proof = JSON.parse(readFileSync(CI_CLOSED_TIP_FIX_PROOF, 'utf8'));
  const sample = CI_CLOSED_TIP_FIX_CHANGED[0];
  for (const override of [
    ...['checkpoint', 'parentHead', 'planningParentHead'].map(key => ({ [CI_CLOSED_TIP_FIX_PROOF]: JSON.stringify({ ...proof, [key]: 'forged' }) })),
    ...Object.entries(CI_CLOSED_TIP_FIX_CONTRACTS).map(([key, value]) => ({ [CI_CLOSED_TIP_FIX_PROOF]: JSON.stringify({ ...proof, contracts: { ...proof.contracts, [key]: !value } }) })),
    { [CI_CLOSED_TIP_FIX_PROOF]: JSON.stringify({ ...proof, files: {} }) },
    ...['beforeHash', 'afterHash'].map(key => ({ [CI_CLOSED_TIP_FIX_PROOF]: JSON.stringify({ ...proof, files: { ...proof.files, [sample]: { ...proof.files[sample], [key]: '0'.repeat(64) } } }) })),
    ...[...CI_CLOSED_TIP_FIX_CHANGED, ...CI_CLOSED_TIP_FIX_ADDED, PARENT_PROOF].map(p => ({ [p]: readFileSync(p, 'utf8') + '\n// forged' })),
    { 'src-tauri/migrations/051_unapproved.sql': 'SELECT 1;' },
  ]) expect(ciClosedTipFixParentReader('.', override).violations.length).toBeGreaterThan(0);
  expect(ciClosedTipFixParentReader('.').violations).toEqual([]);
}, 120000);
it('invalidates warm-cache fixtures on live mutations, deletion and illegal files; restoration passes', () => {
  const root = sharedRoot();
  {
    const validate = () => ciClosedTipFixParentReader(root).violations;
    expect(validate()).toEqual([]);
    for (const p of [CI_CLOSED_TIP_FIX_PROOF, PARENT_PROOF, 'scripts/testing/folder-batch-import.test.mjs']) {
      const file = join(root, p), original = readFileSync(file);
      try {
        writeFileSync(file, Buffer.concat([original, Buffer.from('\nmutation')])); expect(validate().length).toBeGreaterThan(0);
        rmSync(file); expect(validate().length).toBeGreaterThan(0);
      } finally { writeFileSync(file, original); }
      expect(validate()).toEqual([]);
    }
  }
}, 120000);
});

describe('ci-green-fix', () => {
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
  expect(readFileSync('src/app/UiPhase5CReviewBoundary.test.ts', 'utf8')).toContain("writeFileSync(p, bytes.toString() + '\\n')");
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
  const root = sharedRoot();
  {
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
  }
}, 120000);
});

describe('ci-h3-budget-fix', () => {
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
  const root = sharedRoot();
  {
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
  }
}, 120000);
});

describe('ci-phase1-fix', () => {
const PARENT_PROOF = 'docs/architecture/ci-queue-fix-successor.json';
it('validates the standalone ci-phase1-fix tip and its declared scope', () => {
  const checked = ciPhase1FixParentReader('.');
  expect(checked.violations).toEqual([]);
  for (const p of CI_PHASE1_FIX_CHANGED) expect(checked.changedPaths).toContain(p);
  for (const p of [...CI_PHASE1_FIX_ADDED, CI_PHASE1_FIX_PROOF]) expect(checked.addedPaths).toContain(p);
}, 120000);
it('projects changed files to the closed parent tip and preserves its proof bytes', () => {
  const checked = ciPhase1FixParentReader('.');
  expect(checked.violations).toEqual([]);
  for (const p of CI_PHASE1_FIX_CHANGED) {
    expect(checked.read(p)).toBe(execFileSync('git', ['show', `${CI_PHASE1_FIX_BASE}:${p}`], { encoding: 'utf8' }).replaceAll('\r\n', '\n'));
  }
  expect(readFileSync(PARENT_PROOF)).toEqual(execFileSync('git', ['show', `${CI_PHASE1_FIX_BASE}:${PARENT_PROOF}`]));
}, 120000);
it('keeps Phase1 live-mutate drift asserts without Phase2 fixture copy or raised timeouts', () => {
  const src = readFileSync('src/app/MiniMaxVideoPhase1Boundary.test.ts', 'utf8');
  expect(src).toContain('before.toString()');
  expect(src).toContain('# drift');
  expect(src).not.toContain('minimaxVideoPhase2FixtureFiles');
  expect(CI_PHASE1_FIX_CONTRACTS.timeoutsRaised).toBe(false);
  expect(CI_PHASE1_FIX_CONTRACTS.guardsRelaxed).toBe(false);
  expect(ciPhase1FixParentReader('.').violations).toEqual([]);
}, 120000);
it('rejects forged proof identity, contracts, scope, hashes and undeclared changes', () => {
  const proof = JSON.parse(readFileSync(CI_PHASE1_FIX_PROOF, 'utf8'));
  const sample = CI_PHASE1_FIX_CHANGED[0];
  for (const override of [
    ...['checkpoint', 'parentHead', 'planningParentHead'].map(key => ({ [CI_PHASE1_FIX_PROOF]: JSON.stringify({ ...proof, [key]: 'forged' }) })),
    ...Object.entries(CI_PHASE1_FIX_CONTRACTS).map(([key, value]) => ({ [CI_PHASE1_FIX_PROOF]: JSON.stringify({ ...proof, contracts: { ...proof.contracts, [key]: !value } }) })),
    { [CI_PHASE1_FIX_PROOF]: JSON.stringify({ ...proof, files: {} }) },
    ...['beforeHash', 'afterHash'].map(key => ({ [CI_PHASE1_FIX_PROOF]: JSON.stringify({ ...proof, files: { ...proof.files, [sample]: { ...proof.files[sample], [key]: '0'.repeat(64) } } }) })),
    ...[...CI_PHASE1_FIX_CHANGED, ...CI_PHASE1_FIX_ADDED, PARENT_PROOF].map(p => ({ [p]: readFileSync(p, 'utf8') + '\n// forged' })),
    { 'src-tauri/migrations/051_unapproved.sql': 'SELECT 1;' },
  ]) expect(ciPhase1FixParentReader('.', override).violations.length).toBeGreaterThan(0);
  expect(ciPhase1FixParentReader('.').violations).toEqual([]);
}, 120000);
it('invalidates warm-cache fixtures on live mutations, deletion and illegal files; restoration passes', () => {
  const root = sharedRoot();
  {
    const validate = () => ciPhase1FixParentReader(root).violations;
    expect(validate()).toEqual([]);
    for (const p of [CI_PHASE1_FIX_PROOF, PARENT_PROOF, 'src/app/MiniMaxVideoPhase1Boundary.test.ts']) {
      const file = join(root, p), original = readFileSync(file);
      try {
        writeFileSync(file, Buffer.concat([original, Buffer.from('\nmutation')])); expect(validate().length).toBeGreaterThan(0);
        rmSync(file); expect(validate().length).toBeGreaterThan(0);
      } finally { writeFileSync(file, original); }
      expect(validate()).toEqual([]);
    }
  }
}, 120000);
});

describe('ci-phase5h-fix', () => {
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
  const root = sharedRoot();
  {
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
  }
}, 120000);
});

describe('ci-phase6a-fix', () => {
const PARENT_PROOF = 'docs/architecture/ci-phase5h-fix-successor.json';
it('validates the standalone ci-phase6a-fix tip and its declared scope', () => {
  const checked = ciPhase6AFixParentReader('.');
  expect(checked.violations).toEqual([]);
  for (const p of CI_PHASE6A_FIX_CHANGED) expect(checked.changedPaths).toContain(p);
  for (const p of [...CI_PHASE6A_FIX_ADDED, CI_PHASE6A_FIX_PROOF]) expect(checked.addedPaths).toContain(p);
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
  const root = sharedRoot();
  {
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
  }
}, 120000);
});

describe('ci-probe-fix', () => {
const PARENT_PROOF = 'docs/architecture/scope-since-chain-successor.json';
const safety = 'src/features/create/StoryboardSafety.test.tsx';
it('validates the standalone ci-probe-fix tip and its declared scope', () => {
  const checked = ciProbeFixParentReader('.');
  expect(checked.violations).toEqual([]);
  // Successor tips extend changed/addedPaths; this closed tip still owns its declared scope.
  for (const p of CI_PROBE_FIX_CHANGED) expect(checked.changedPaths).toContain(p);
  for (const p of [...CI_PROBE_FIX_ADDED, CI_PROBE_FIX_PROOF]) expect(checked.addedPaths).toContain(p);
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
  const root = sharedRoot();
  {
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
  }
}, 120000);
});

describe('ci-queue-fix', () => {
const PARENT_PROOF = 'docs/architecture/ci-chain-fix-successor.json';
it('validates the standalone ci-queue-fix tip and its declared scope', () => {
  const checked = ciQueueFixParentReader('.');
  expect(checked.violations).toEqual([]);
  for (const p of CI_QUEUE_FIX_CHANGED) expect(checked.changedPaths).toContain(p);
  for (const p of [...CI_QUEUE_FIX_ADDED, CI_QUEUE_FIX_PROOF]) expect(checked.addedPaths).toContain(p);
}, 120000);
it('projects changed files to the closed parent tip and preserves its proof bytes', () => {
  const checked = ciQueueFixParentReader('.');
  expect(checked.violations).toEqual([]);
  for (const p of CI_QUEUE_FIX_CHANGED) {
    expect(checked.read(p)).toBe(execFileSync('git', ['show', `${CI_QUEUE_FIX_BASE}:${p}`], { encoding: 'utf8' }).replaceAll('\r\n', '\n'));
  }
  expect(readFileSync(PARENT_PROOF)).toEqual(execFileSync('git', ['show', `${CI_QUEUE_FIX_BASE}:${PARENT_PROOF}`]));
}, 120000);
it('keeps QueueLifecycle live-mutate drift asserts without Phase2 fixture copy or raised timeouts', () => {
  const src = readFileSync('src/app/QueueLifecycleRepairBoundary.test.ts', 'utf8');
  expect(src).toContain('original.toString()');
  expect(src).toContain('# drift');
  expect(src).not.toContain('minimaxVideoPhase2FixtureFiles');
  expect(readFileSync('src/app/H3GenerationRepairBoundary.test.ts', 'utf8')).not.toContain('Buffer.concat');
  expect(CI_QUEUE_FIX_CONTRACTS.timeoutsRaised).toBe(false);
  expect(CI_QUEUE_FIX_CONTRACTS.guardsRelaxed).toBe(false);
  expect(ciQueueFixParentReader('.').violations).toEqual([]);
}, 120000);
it('rejects forged proof identity, contracts, scope, hashes and undeclared changes', () => {
  const proof = JSON.parse(readFileSync(CI_QUEUE_FIX_PROOF, 'utf8'));
  const sample = CI_QUEUE_FIX_CHANGED[0];
  for (const override of [
    ...['checkpoint', 'parentHead', 'planningParentHead'].map(key => ({ [CI_QUEUE_FIX_PROOF]: JSON.stringify({ ...proof, [key]: 'forged' }) })),
    ...Object.entries(CI_QUEUE_FIX_CONTRACTS).map(([key, value]) => ({ [CI_QUEUE_FIX_PROOF]: JSON.stringify({ ...proof, contracts: { ...proof.contracts, [key]: !value } }) })),
    { [CI_QUEUE_FIX_PROOF]: JSON.stringify({ ...proof, files: {} }) },
    ...['beforeHash', 'afterHash'].map(key => ({ [CI_QUEUE_FIX_PROOF]: JSON.stringify({ ...proof, files: { ...proof.files, [sample]: { ...proof.files[sample], [key]: '0'.repeat(64) } } }) })),
    ...[...CI_QUEUE_FIX_CHANGED, ...CI_QUEUE_FIX_ADDED, PARENT_PROOF].map(p => ({ [p]: readFileSync(p, 'utf8') + '\n// forged' })),
    { 'src-tauri/migrations/051_unapproved.sql': 'SELECT 1;' },
  ]) expect(ciQueueFixParentReader('.', override).violations.length).toBeGreaterThan(0);
  expect(ciQueueFixParentReader('.').violations).toEqual([]);
}, 120000);
it('invalidates warm-cache fixtures on live mutations, deletion and illegal files; restoration passes', () => {
  const root = sharedRoot();
  {
    const validate = () => ciQueueFixParentReader(root).violations;
    expect(validate()).toEqual([]);
    for (const p of [CI_QUEUE_FIX_PROOF, PARENT_PROOF, 'src/app/QueueLifecycleRepairBoundary.test.ts']) {
      const file = join(root, p), original = readFileSync(file);
      try {
        writeFileSync(file, Buffer.concat([original, Buffer.from('\nmutation')])); expect(validate().length).toBeGreaterThan(0);
        rmSync(file); expect(validate().length).toBeGreaterThan(0);
      } finally { writeFileSync(file, original); }
      expect(validate()).toEqual([]);
    }
  }
}, 120000);
});

describe('ci-scope-fix', () => {
const PARENT_PROOF = 'docs/architecture/ci-h3-budget-fix-successor.json';
it('validates the standalone ci-scope-fix tip and its declared scope', () => {
  const checked = ciScopeFixParentReader('.');
  expect(checked.violations).toEqual([]);
  for (const p of CI_SCOPE_FIX_CHANGED) expect(checked.changedPaths).toContain(p);
  for (const p of [...CI_SCOPE_FIX_ADDED, CI_SCOPE_FIX_PROOF]) expect(checked.addedPaths).toContain(p);
}, 120000);
it('projects changed files to the closed parent tip and preserves its proof bytes', () => {
  const checked = ciScopeFixParentReader('.');
  expect(checked.violations).toEqual([]);
  for (const p of CI_SCOPE_FIX_CHANGED) {
    expect(checked.read(p)).toBe(execFileSync('git', ['show', `${CI_SCOPE_FIX_BASE}:${p}`], { encoding: 'utf8' }).replaceAll('\r\n', '\n'));
  }
  expect(readFileSync(PARENT_PROOF)).toEqual(execFileSync('git', ['show', `${CI_SCOPE_FIX_BASE}:${PARENT_PROOF}`]));
}, 120000);
it('makes closed-tip scope asserts successor-aware and keeps Veda tip-leaf under 5s', () => {
  expect(consolidatedRead('scripts/testing/ci-assert-fix.test.mjs')).toContain('toContain(p)');
  expect(readFileSync('scripts/testing/veda-removed-white-flash.test.mjs', 'utf8')).toContain('ciScopeFixParentReader');
  expect(CI_SCOPE_FIX_CONTRACTS.timeoutsRaised).toBe(false);
  expect(CI_SCOPE_FIX_CONTRACTS.guardsRelaxed).toBe(false);
  expect(ciScopeFixParentReader('.').violations).toEqual([]);
}, 120000);
it('rejects forged proof identity, contracts, scope, hashes and undeclared changes', () => {
  const proof = JSON.parse(readFileSync(CI_SCOPE_FIX_PROOF, 'utf8'));
  const sample = CI_SCOPE_FIX_CHANGED[0];
  for (const override of [
    ...['checkpoint', 'parentHead', 'planningParentHead'].map(key => ({ [CI_SCOPE_FIX_PROOF]: JSON.stringify({ ...proof, [key]: 'forged' }) })),
    ...Object.entries(CI_SCOPE_FIX_CONTRACTS).map(([key, value]) => ({ [CI_SCOPE_FIX_PROOF]: JSON.stringify({ ...proof, contracts: { ...proof.contracts, [key]: !value } }) })),
    { [CI_SCOPE_FIX_PROOF]: JSON.stringify({ ...proof, files: {} }) },
    ...['beforeHash', 'afterHash'].map(key => ({ [CI_SCOPE_FIX_PROOF]: JSON.stringify({ ...proof, files: { ...proof.files, [sample]: { ...proof.files[sample], [key]: '0'.repeat(64) } } }) })),
    ...[...CI_SCOPE_FIX_CHANGED, ...CI_SCOPE_FIX_ADDED, PARENT_PROOF].map(p => ({ [p]: readFileSync(p, 'utf8') + '\n// forged' })),
    { 'src-tauri/migrations/051_unapproved.sql': 'SELECT 1;' },
  ]) expect(ciScopeFixParentReader('.', override).violations.length).toBeGreaterThan(0);
  expect(ciScopeFixParentReader('.').violations).toEqual([]);
}, 120000);
it('invalidates warm-cache fixtures on live mutations, deletion and illegal files; restoration passes', () => {
  const root = sharedRoot();
  {
    const validate = () => ciScopeFixParentReader(root).violations;
    expect(validate()).toEqual([]);
    for (const p of [CI_SCOPE_FIX_PROOF, PARENT_PROOF, 'scripts/testing/veda-removed-white-flash.test.mjs']) {
      const file = join(root, p), original = readFileSync(file);
      try {
        writeFileSync(file, Buffer.concat([original, Buffer.from('\nmutation')])); expect(validate().length).toBeGreaterThan(0);
        rmSync(file); expect(validate().length).toBeGreaterThan(0);
      } finally { writeFileSync(file, original); }
      expect(validate()).toEqual([]);
    }
  }
}, 120000);
});

describe('ci-shard-fix', () => {
const PARENT_PROOF = 'docs/architecture/ci-stale-fix-successor.json';
it('validates the standalone ci-shard-fix tip and its declared scope', () => {
  const checked = ciShardFixParentReader('.');
  expect(checked.violations).toEqual([]);
  for (const p of CI_SHARD_FIX_CHANGED) expect(checked.changedPaths).toContain(p);
  for (const p of [...CI_SHARD_FIX_ADDED, CI_SHARD_FIX_PROOF]) expect(checked.addedPaths).toContain(p);
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
  const root = sharedRoot();
  {
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
  }
}, 120000);
});

describe('ci-stable-fix', () => {
const PARENT_PROOF = 'docs/architecture/ci-closed-tip-fix-successor.json';
it('validates the standalone ci-stable-fix tip and its declared scope', () => {
  const checked = ciStableFixParentReader('.');
  expect(checked.violations).toEqual([]);
  for (const p of CI_STABLE_FIX_CHANGED) expect(checked.changedPaths).toContain(p);
  for (const p of [...CI_STABLE_FIX_ADDED, CI_STABLE_FIX_PROOF]) expect(checked.addedPaths).toContain(p);
}, 120000);
it('projects changed files to the closed parent tip and preserves its proof bytes', () => {
  const checked = ciStableFixParentReader('.');
  expect(checked.violations).toEqual([]);
  for (const p of CI_STABLE_FIX_CHANGED) {
    expect(checked.read(p)).toBe(execFileSync('git', ['show', `${CI_STABLE_FIX_BASE}:${p}`], { encoding: 'utf8' }).replaceAll('\r\n', '\n'));
  }
  expect(readFileSync(PARENT_PROOF)).toEqual(execFileSync('git', ['show', `${CI_STABLE_FIX_BASE}:${PARENT_PROOF}`]));
}, 120000);
it('fixes green Phase5C needle escape and FolderBatch page waitFor without raising timeouts', () => {
  const green = consolidatedRead('scripts/testing/ci-green-fix.test.mjs');
  expect(green).toContain("UiPhase5CReviewBoundary.test.ts");
  expect(green).toContain("bytes.toString()");
  expect(readFileSync('src/app/UiPhase5CReviewBoundary.test.ts', 'utf8')).toContain("writeFileSync(p, bytes.toString() + '\\n')");
  expect(readFileSync('src/features/shots/FolderBatchImportPanel.test.tsx', 'utf8')).toContain('waitFor(() => expect(screen.getByRole("button",{name:"编辑导入镜头 51"}))');
  expect(CI_STABLE_FIX_CONTRACTS.timeoutsRaised).toBe(false);
  expect(CI_STABLE_FIX_CONTRACTS.guardsRelaxed).toBe(false);
  expect(ciStableFixParentReader('.').violations).toEqual([]);
}, 120000);
it('rejects forged proof identity, contracts, scope, hashes and undeclared changes', () => {
  const proof = JSON.parse(readFileSync(CI_STABLE_FIX_PROOF, 'utf8'));
  const sample = CI_STABLE_FIX_CHANGED[0];
  for (const override of [
    ...['checkpoint', 'parentHead', 'planningParentHead'].map(key => ({ [CI_STABLE_FIX_PROOF]: JSON.stringify({ ...proof, [key]: 'forged' }) })),
    ...Object.entries(CI_STABLE_FIX_CONTRACTS).map(([key, value]) => ({ [CI_STABLE_FIX_PROOF]: JSON.stringify({ ...proof, contracts: { ...proof.contracts, [key]: !value } }) })),
    { [CI_STABLE_FIX_PROOF]: JSON.stringify({ ...proof, files: {} }) },
    ...['beforeHash', 'afterHash'].map(key => ({ [CI_STABLE_FIX_PROOF]: JSON.stringify({ ...proof, files: { ...proof.files, [sample]: { ...proof.files[sample], [key]: '0'.repeat(64) } } }) })),
    ...[...CI_STABLE_FIX_CHANGED, ...CI_STABLE_FIX_ADDED, PARENT_PROOF].map(p => ({ [p]: readFileSync(p, 'utf8') + '\n// forged' })),
    { 'src-tauri/migrations/051_unapproved.sql': 'SELECT 1;' },
  ]) expect(ciStableFixParentReader('.', override).violations.length).toBeGreaterThan(0);
  expect(ciStableFixParentReader('.').violations).toEqual([]);
}, 120000);
it('invalidates warm-cache fixtures on live mutations, deletion and illegal files; restoration passes', () => {
  const root = sharedRoot();
  {
    const validate = () => ciStableFixParentReader(root).violations;
    expect(validate()).toEqual([]);
    for (const p of [CI_STABLE_FIX_PROOF, PARENT_PROOF, 'scripts/testing/ci-green-fix.test.mjs']) {
      const file = join(root, p), original = readFileSync(file);
      try {
        writeFileSync(file, Buffer.concat([original, Buffer.from('\nmutation')])); expect(validate().length).toBeGreaterThan(0);
        rmSync(file); expect(validate().length).toBeGreaterThan(0);
      } finally { writeFileSync(file, original); }
      expect(validate()).toEqual([]);
    }
  }
}, 120000);
});

describe('ci-stale-fix', () => {
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
  const root = sharedRoot();
  {
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
  }
}, 120000);
});

describe('ci-consolidate-fix', () => {
const PARENT_PROOF = 'docs/architecture/ci-shard-fix-successor.json';
it('validates the standalone ci-consolidate-fix tip and its declared scope', () => {
  const checked = ciConsolidateFixParentReader('.');
  expect(checked.violations).toEqual([]);
  expect(checked.changedPaths).toEqual(CI_CONSOLIDATE_FIX_CHANGED);
  expect(checked.addedPaths).toEqual([...CI_CONSOLIDATE_FIX_ADDED, CI_CONSOLIDATE_FIX_PROOF]);
}, 120000);
it('projects changed files to the closed parent tip and preserves its proof bytes', () => {
  const checked = ciConsolidateFixParentReader('.');
  expect(checked.violations).toEqual([]);
  for (const p of CI_CONSOLIDATE_FIX_CHANGED) {
    expect(checked.read(p)).toBe(execFileSync('git', ['show', `${CI_CONSOLIDATE_FIX_BASE}:${p}`], { encoding: 'utf8' }).replaceAll('\r\n', '\n'));
  }
  expect(readFileSync(PARENT_PROOF)).toEqual(execFileSync('git', ['show', `${CI_CONSOLIDATE_FIX_BASE}:${PARENT_PROOF}`]));
}, 120000);
it('consolidates every ci-*-fix tip into one single-copy test with all assertions and no raised timeouts', () => {
  const self = readFileSync('scripts/testing/ci-chain-consolidated.test.mjs', 'utf8');
  for (const name of CONSOLIDATED_TIPS) {
    const original = consolidatedRead(`scripts/testing/${name}.test.mjs`);
    expect(self).toContain(`describe('${name}', () => {`);
    const stub = readFileSync(`scripts/testing/${name}.test.mjs`, 'utf8');
    expect(stub).toContain(`describe('${name}', () => {`);
    expect(stub).not.toContain('mkdtempSync');
    const block = self.split(`describe('${name}', () => {`)[1].split('\n});\n')[0];
    expect((block.match(/^it\(/gm) ?? []).length).toBe((original.match(/^it\(/gm) ?? []).length);
    expect((block.match(/expect\(/g) ?? []).length).toBeGreaterThanOrEqual((original.match(/expect\(/g) ?? []).length);
  }
  expect((self.match(/mkdtempSync\(/g) ?? []).length).toBe(1);
  expect(self).not.toMatch(/,\s*(?!120000|90000)\d{5,}\s*\)/);
  const fiveG = readFileSync('src/app/UiPhase5GCiTimeoutBoundary.test.ts', 'utf8');
  expect(fiveG).toContain("'keeps live freshness for frozen proofs, Runtime raw bytes: %s'");
  expect(fiveG).toContain("it('keeps live freshness for manifest deletion'");
  expect(CI_CONSOLIDATE_FIX_CONTRACTS.timeoutsRaised).toBe(false);
  expect(CI_CONSOLIDATE_FIX_CONTRACTS.guardsRelaxed).toBe(false);
  expect(ciConsolidateFixParentReader('.').violations).toEqual([]);
}, 120000);
it('rejects forged proof identity, contracts, scope, hashes and undeclared changes', () => {
  const proof = JSON.parse(readFileSync(CI_CONSOLIDATE_FIX_PROOF, 'utf8'));
  const sample = CI_CONSOLIDATE_FIX_CHANGED[0];
  for (const override of [
    ...['checkpoint', 'parentHead', 'planningParentHead'].map(key => ({ [CI_CONSOLIDATE_FIX_PROOF]: JSON.stringify({ ...proof, [key]: 'forged' }) })),
    ...Object.entries(CI_CONSOLIDATE_FIX_CONTRACTS).map(([key, value]) => ({ [CI_CONSOLIDATE_FIX_PROOF]: JSON.stringify({ ...proof, contracts: { ...proof.contracts, [key]: !value } }) })),
    { [CI_CONSOLIDATE_FIX_PROOF]: JSON.stringify({ ...proof, files: {} }) },
    ...['beforeHash', 'afterHash'].map(key => ({ [CI_CONSOLIDATE_FIX_PROOF]: JSON.stringify({ ...proof, files: { ...proof.files, [sample]: { ...proof.files[sample], [key]: '0'.repeat(64) } } }) })),
    ...[...CI_CONSOLIDATE_FIX_CHANGED, ...CI_CONSOLIDATE_FIX_ADDED, PARENT_PROOF].map(p => ({ [p]: readFileSync(p, 'utf8') + '\n// forged' })),
    { 'src-tauri/migrations/051_unapproved.sql': 'SELECT 1;' },
  ]) expect(ciConsolidateFixParentReader('.', override).violations.length).toBeGreaterThan(0);
  expect(ciConsolidateFixParentReader('.').violations).toEqual([]);
}, 120000);
it('invalidates warm-cache fixtures on live mutations, deletion and illegal files; restoration passes', () => {
  const root = sharedRoot();
  const validate = () => ciConsolidateFixParentReader(root).violations;
  expect(validate()).toEqual([]);
  for (const p of [CI_CONSOLIDATE_FIX_PROOF, PARENT_PROOF, 'scripts/testing/ci-assert-fix.test.mjs']) {
    const file = join(root, p), original = readFileSync(file);
    try {
      writeFileSync(file, Buffer.concat([original, Buffer.from('\nmutation')])); expect(validate().length).toBeGreaterThan(0);
      rmSync(file); expect(validate().length).toBeGreaterThan(0);
    } finally { writeFileSync(file, original); }
    expect(validate()).toEqual([]);
  }
}, 120000);
});
