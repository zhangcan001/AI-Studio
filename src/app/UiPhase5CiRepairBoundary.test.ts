// @vitest-environment node
import { expect, it } from 'vitest';
import { readFileSync, writeFileSync, readdirSync, rmSync } from 'node:fs';
import { join } from 'node:path';
import { uiPhase5CiParentReader, PHASE5_CI_MANIFEST, PHASE5_CI_PARENT, PHASE5_CI_EXISTING, phase5CiParentFacts } from '../../scripts/ui-phase5-ci-repair-successor-guard.mjs';
import { uiPhase5AParentReader } from '../../scripts/ui-phase5a-storyboard-successor-guard.mjs';

it('validates CI-only live changes before projecting the exact frozen Phase5A checkpoint', () => {
  const reader = uiPhase5CiParentReader('.'); expect(reader.violations).toEqual([]);
  const proof = JSON.parse(readFileSync(PHASE5_CI_MANIFEST, 'utf8'));
  expect(proof.parentHead).toBe(PHASE5_CI_PARENT);
  expect(proof.planningParentHead).toBe('ad8db6e20bf6f84b81ce9bc0c788f32669dc552a');
  expect(proof.invariants.freshLiveIdentityPreserved).toBe(true);
  expect(proof.invariants.phase5BProductStarted).toBe(false);
  for (const p of PHASE5_CI_EXISTING) expect(reader.read(p)).toBe(phase5CiParentFacts('.').read(p));
  expect(uiPhase5AParentReader('.').violations).toEqual([]);
}, 90000);

it('rejects forged parent, scope, blob verification and freshness declarations without projection', () => {
  const proof = JSON.parse(readFileSync(PHASE5_CI_MANIFEST, 'utf8'));
  for (const mutate of [
    (p: any) => { p.parentHead = '0'.repeat(40); },
    (p: any) => { p.paths['scripts/boundary-validation-cache.mjs'].afterHash = '0'.repeat(64); },
    (p: any) => { p.paths['scripts/boundary-validation-cache.mjs'].beforeHash = '0'.repeat(64); },
    (p: any) => { p.invariants.freshLiveIdentityPreserved = false; },
    (p: any) => { p.invariants.gitBlobIdentityVerified = false; },
    (p: any) => { p.paths['src/services/tauriClient.ts'] = { beforeHash: null, afterHash: '0'.repeat(64) }; },
  ]) {
    const invalid = structuredClone(proof); mutate(invalid);
    const denied = uiPhase5CiParentReader('.', invalid);
    expect(denied.violations.length).toBeGreaterThan(0); expect(denied.addedPaths).toEqual([]); expect(denied.afterHashes).toEqual({});
  }
}, 90000);

it('keeps live freshness for CI repair source and shell bytes with parent fail-closed checked once', () => {
  // Live working-tree mutate — tip-inflated FixtureFiles blow the 90s Boundary budget.
  let parentChecked = false;
  for (const p of ['scripts/boundary-validation-cache.mjs', 'src/app/ShellHost.tsx']) {
    const bytes = readFileSync(p);
    try {
      writeFileSync(p, bytes.toString() + '\n');
      const denied = uiPhase5CiParentReader('.');
      expect(denied.violations.length, p).toBeGreaterThan(0); expect(denied.addedPaths).toEqual([]);
      if (!parentChecked) { expect(uiPhase5AParentReader('.').violations.length, p).toBeGreaterThan(0); parentChecked = true; }
    } finally { writeFileSync(p, bytes); }
    expect(uiPhase5CiParentReader('.').violations, p).toEqual([]);
  }
}, 90000);

it('keeps live freshness for frozen proofs, Runtime raw bytes and manifest deletion', () => {
  const runtime = readdirSync('src-tauri/runtime_packages', { withFileTypes: true })
    .filter((e: { isDirectory(): boolean }) => e.isDirectory())
    .map((e: { name: string }) => join('src-tauri/runtime_packages', e.name, 'workflow_api.json'))
    .find((p: string) => { try { readFileSync(p); return true; } catch { return false; } })!;
  let parentChecked = false;
  for (const p of ['docs/architecture/ui-phase5a-storyboard-successor.json', PHASE5_CI_MANIFEST, runtime]) {
    const bytes = readFileSync(p);
    try {
      writeFileSync(p, bytes.toString() + '\n');
      const denied = uiPhase5CiParentReader('.');
      expect(denied.violations.length, p).toBeGreaterThan(0); expect(denied.addedPaths).toEqual([]);
      if (!parentChecked) { expect(uiPhase5AParentReader('.').violations.length, p).toBeGreaterThan(0); parentChecked = true; }
    } finally { writeFileSync(p, bytes); }
    expect(uiPhase5CiParentReader('.').violations, p).toEqual([]);
  }
  const bytes = readFileSync(PHASE5_CI_MANIFEST);
  try {
    rmSync(PHASE5_CI_MANIFEST); expect(uiPhase5CiParentReader('.').violations.length).toBeGreaterThan(0);
  } finally { writeFileSync(PHASE5_CI_MANIFEST, bytes); }
  expect(uiPhase5CiParentReader('.').violations).toEqual([]);
}, 90000);

it('keeps live freshness for illegal additions across owned inventories', () => {
  for (const [dir, name] of [['src', 'IllegalOwner.js'], ['.github', 'illegal.yml'], ['docs', 'IllegalOwner.js'], ['src-tauri/examples', 'IllegalOwner.rs']]) {
    const extra = join(dir, name);
    try {
      writeFileSync(extra, 'unreviewed bytes');
      expect(uiPhase5CiParentReader('.').violations).toContain(`ui-phase5-ci-repair-inventory:${dir}`);
    } finally { rmSync(extra); }
    expect(uiPhase5CiParentReader('.').violations).toEqual([]);
  }
}, 90000);
