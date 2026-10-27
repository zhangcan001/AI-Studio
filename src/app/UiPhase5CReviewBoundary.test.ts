// @vitest-environment node
import { expect, it } from 'vitest';
import { readFileSync, readdirSync } from 'node:fs';
import { uiPhase5CParentReader, PHASE5C_MANIFEST, PHASE5C_PARENT, PHASE5C_EXISTING, phase5CParentFacts } from '../../scripts/ui-phase5c-review-successor-guard.mjs';
import { uiPhase5BParentReader } from '../../scripts/ui-phase5b-mixed-video-successor-guard.mjs';

it('validates Phase5C live changes before projecting the exact frozen Phase5B checkpoint and historical chain', () => {
  const reader = uiPhase5CParentReader('.'); expect(reader.violations).toEqual([]);
  const proof = JSON.parse(readFileSync(PHASE5C_MANIFEST, 'utf8'));
  expect(proof.parentHead).toBe(PHASE5C_PARENT);
  expect(proof.planningParentHead).toBe('14c9ebbf09b402faf2c6b1fca6864401a946bc7a');
  expect(proof.invariants.asyncOwnerFenced).toBe(true);
  expect(proof.invariants.phase5CStarted).toBe(true);
  for (const p of PHASE5C_EXISTING) expect(reader.read(p)).toBe(phase5CParentFacts('.').read(p));
  expect(uiPhase5BParentReader('.').violations).toEqual([]);
}, 90000);

it('rejects forged parent, scope, blob verification and freshness declarations without projection', () => {
  const proof = JSON.parse(readFileSync(PHASE5C_MANIFEST, 'utf8'));
  for (const mutate of [
    (p: any) => { p.parentHead = '0'.repeat(40); },
    (p: any) => { p.paths['src/features/production/ProductionReviewInbox.tsx'].afterHash = '0'.repeat(64); },
    (p: any) => { p.paths['src/features/production/ProductionReviewInbox.tsx'].beforeHash = '0'.repeat(64); },
    (p: any) => { p.invariants.asyncOwnerFenced = false; },
    (p: any) => { p.invariants.candidateSelectionDistinct = false; },
    (p: any) => { p.invariants.realGpuExecuted = true; },
    (p: any) => { p.paths['src/services/tauriClient.ts'] = { beforeHash: null, afterHash: '0'.repeat(64) }; },
  ]) {
    const invalid = structuredClone(proof); mutate(invalid);
    const denied = uiPhase5CParentReader('.', invalid);
    expect(denied.violations.length).toBeGreaterThan(0); expect(denied.addedPaths).toEqual([]); expect(denied.afterHashes).toEqual({});
  }
}, 90000);

it('keeps live freshness for review source and shell bytes with parent fail-closed checked once', () => {
  let parentChecked = false;
  for (const p of ['src/features/production/ProductionReviewInbox.tsx', 'src/app/ShellHost.tsx']) {
    const denied = uiPhase5CParentReader('.', { [p]: readFileSync(p, 'utf8') + '\n' });
    expect(denied.violations.length, p).toBeGreaterThan(0); expect(denied.addedPaths).toEqual([]);
    if (!parentChecked) { expect(uiPhase5BParentReader('.', { [p]: readFileSync(p, 'utf8') + '\n' }).violations.length, p).toBeGreaterThan(0); parentChecked = true; }
  }
  expect(uiPhase5CParentReader('.').violations).toEqual([]);
}, 90000);

it('keeps live freshness for frozen proofs, Runtime raw bytes and manifest deletion', () => {
  const runtime = readdirSync('src-tauri/runtime_packages', { withFileTypes: true })
    .filter(e => e.isDirectory())
    .map(e => `src-tauri/runtime_packages/${e.name}/workflow_api.json`)
    .find(p => { try { readFileSync(p); return true; } catch { return false; } })!;
  for (const p of [PHASE5C_MANIFEST, runtime, 'docs/architecture/ui-phase5b-mixed-video-successor.json']) {
    const denied = uiPhase5CParentReader('.', { [p]: readFileSync(p, 'utf8') + '\n' });
    expect(denied.violations.length, p).toBeGreaterThan(0);
    expect(denied.addedPaths).toEqual([]);
  }
  expect(uiPhase5CParentReader('.').violations).toEqual([]);
}, 90000);

it('keeps live freshness for illegal additions across owned inventories', () => {
  for (const p of ['src/IllegalOwner.js', '.github/illegal.yml', 'docs/IllegalOwner.js', 'src-tauri/examples/IllegalOwner.rs']) {
    const denied = uiPhase5CParentReader('.', { [p]: 'unreviewed bytes' });
    expect(denied.violations.length, p).toBeGreaterThan(0);
  }
  expect(uiPhase5CParentReader('.').violations).toEqual([]);
}, 90000);
