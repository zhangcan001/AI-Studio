// @vitest-environment node
import { expect, it } from 'vitest';
import { readFileSync, writeFileSync, readdirSync, rmSync } from 'node:fs';
import { join } from 'node:path';
import { uiPhase5BParentReader, PHASE5B_MANIFEST, PHASE5B_PARENT, PHASE5B_EXISTING, phase5BParentFacts } from '../../scripts/ui-phase5b-mixed-video-successor-guard.mjs';
import { uiPhase5AParentReader } from '../../scripts/ui-phase5a-storyboard-successor-guard.mjs';

it('validates Phase5B live changes before projecting the exact frozen CI checkpoint and Phase5A history', () => {
  const reader = uiPhase5BParentReader('.'); expect(reader.violations).toEqual([]);
  const proof = JSON.parse(readFileSync(PHASE5B_MANIFEST, 'utf8'));
  expect(proof.parentHead).toBe(PHASE5B_PARENT);
  expect(proof.planningParentHead).toBe('0f288edf207873bcdbada0a67bc5dbdf4a24b2ce');
  expect(proof.invariants.hierarchyScopeRevalidated).toBe(true);
  expect(proof.invariants.phase5BStarted).toBe(true);
  for (const p of PHASE5B_EXISTING) expect(reader.read(p)).toBe(phase5BParentFacts('.').read(p));
  expect(uiPhase5AParentReader('.').violations).toEqual([]);
}, 90000);

it('rejects forged parent, scope, blob verification and freshness declarations without projection', () => {
  const proof = JSON.parse(readFileSync(PHASE5B_MANIFEST, 'utf8'));
  for (const mutate of [
    (p: any) => { p.parentHead = '0'.repeat(40); },
    (p: any) => { p.paths['src-tauri/src/application/production_preparation_service.rs'].afterHash = '0'.repeat(64); },
    (p: any) => { p.paths['src-tauri/src/application/production_preparation_service.rs'].beforeHash = '0'.repeat(64); },
    (p: any) => { p.invariants.hierarchyScopeRevalidated = false; },
    (p: any) => { p.invariants.explicitQueueStart = false; },
    (p: any) => { p.paths['src/services/tauriClient.ts'] = { beforeHash: null, afterHash: '0'.repeat(64) }; },
  ]) {
    const invalid = structuredClone(proof); mutate(invalid);
    const denied = uiPhase5BParentReader('.', invalid);
    expect(denied.violations.length).toBeGreaterThan(0); expect(denied.addedPaths).toEqual([]); expect(denied.afterHashes).toEqual({});
  }
}, 90000);

it('keeps live freshness for source and shell bytes with parent fail-closed checked once', () => {
  // Live working-tree mutate — tip-inflated FixtureFiles blow the 90s Boundary budget.
  let parentChecked = false;
  for (const p of ['src-tauri/src/application/production_preparation_service.rs', 'src/app/ShellHost.tsx']) {
    const bytes = readFileSync(p);
    try {
      writeFileSync(p, bytes.toString() + '\n');
      const denied = uiPhase5BParentReader('.');
      expect(denied.violations.length, p).toBeGreaterThan(0); expect(denied.addedPaths).toEqual([]);
      if (!parentChecked) { expect(uiPhase5AParentReader('.').violations.length, p).toBeGreaterThan(0); parentChecked = true; }
    } finally { writeFileSync(p, bytes); }
    expect(uiPhase5BParentReader('.').violations, p).toEqual([]);
  }
}, 90000);

it('keeps live freshness for frozen proofs, Runtime raw bytes and manifest deletion', () => {
  const runtime = readdirSync('src-tauri/runtime_packages', { withFileTypes: true })
    .filter((e: { isDirectory(): boolean; name: string }) => e.isDirectory())
    .map((e: { name: string }) => join('src-tauri/runtime_packages', e.name, 'workflow_api.json'))
    .find((p: string) => { try { readFileSync(p); return true; } catch { return false; } })!;
  let parentChecked = false;
  for (const p of ['docs/architecture/ui-phase5a-storyboard-successor.json', 'docs/architecture/ui-phase5-ci-repair-successor.json', PHASE5B_MANIFEST, runtime]) {
    const bytes = readFileSync(p);
    try {
      writeFileSync(p, bytes.toString() + '\n');
      const denied = uiPhase5BParentReader('.');
      expect(denied.violations.length, p).toBeGreaterThan(0); expect(denied.addedPaths).toEqual([]);
      if (!parentChecked) { expect(uiPhase5AParentReader('.').violations.length, p).toBeGreaterThan(0); parentChecked = true; }
    } finally { writeFileSync(p, bytes); }
    expect(uiPhase5BParentReader('.').violations, p).toEqual([]);
  }
  const bytes = readFileSync(PHASE5B_MANIFEST);
  try {
    rmSync(PHASE5B_MANIFEST); expect(uiPhase5BParentReader('.').violations.length).toBeGreaterThan(0);
  } finally { writeFileSync(PHASE5B_MANIFEST, bytes); }
  expect(uiPhase5BParentReader('.').violations).toEqual([]);
}, 90000);

it('keeps live freshness for illegal additions across owned inventories', () => {
  for (const [dir, name] of [['src', 'IllegalOwner.js'], ['.github', 'illegal.yml'], ['docs', 'IllegalOwner.js'], ['src-tauri/examples', 'IllegalOwner.rs']]) {
    const extra = join(dir, name);
    try {
      writeFileSync(extra, 'unreviewed bytes');
      expect(uiPhase5BParentReader('.').violations).toContain(`ui-phase5b-mixed-video-inventory:${dir}`);
    } finally { rmSync(extra); }
    expect(uiPhase5BParentReader('.').violations).toEqual([]);
  }
}, 90000);
