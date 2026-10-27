// @vitest-environment node
import { expect, it } from 'vitest';
import { readFileSync, writeFileSync, readdirSync, rmSync } from 'node:fs';
import { join } from 'node:path';
import { uiPhase5AParentReader, PHASE5A_MANIFEST, PHASE5A_PARENT, PHASE5A_EXISTING, phase5AParentFacts } from '../../scripts/ui-phase5a-storyboard-successor-guard.mjs';
import { uiPhase4SerialQueueParentReader } from '../../scripts/ui-phase4-serial-queue-successor-guard.mjs';

it('validates the actual Phase5A parent and replays frozen Phase4 without rewriting historical proofs', () => {
  const reader = uiPhase5AParentReader('.');
  expect(reader.violations).toEqual([]);
  const proof = JSON.parse(readFileSync(PHASE5A_MANIFEST, 'utf8'));
  expect(proof.parentHead).toBe(PHASE5A_PARENT);
  expect(proof.planningParentHead).toBe(PHASE5A_PARENT);
  expect(proof.phaseCommitsBeforeCheckpoint).toEqual([]);
  expect(proof.database).toEqual({ migrationBefore: 44, migrationAfter: 44, newTables: 0, backupBefore: 21, backupAfter: 21 });
  for (const path of PHASE5A_EXISTING) expect(reader.read(path), path).toBe(phase5AParentFacts('.').read(path));
  expect(uiPhase4SerialQueueParentReader('.').violations).toEqual([]);
}, 90000);

it('rejects forged planning parent, scope, hashes, schema and execution invariants', () => {
  const proof = JSON.parse(readFileSync(PHASE5A_MANIFEST, 'utf8'));
  for (const mutate of [
    (p: any) => { p.planningParentHead = '0'.repeat(40); },
    (p: any) => { p.paths['src/services/tauriClient.ts'] = { afterHash: '0'.repeat(64) }; },
    (p: any) => { p.paths['src/features/shots/ShotWorkspace.tsx'].beforeHash = '0'.repeat(64); },
    (p: any) => { p.paths['src/features/shots/ShotWorkspace.tsx'].afterHash = '0'.repeat(64); },
    (p: any) => { p.database.migrationAfter = 45; },
    (p: any) => { p.invariants.preparationStartsGeneration = true; },
  ]) {
    const forged = structuredClone(proof); mutate(forged);
    const rejected = uiPhase5AParentReader('.', forged);
    expect(rejected.violations.length).toBeGreaterThan(0);
    expect(rejected.addedPaths).toEqual([]);
    expect(rejected.afterHashes).toEqual({});
  }
}, 90000);

it('rejects live edited source/config with parent fail-closed', () => {
  // Live working-tree mutate — tip-inflated FixtureFiles blow the 90s Boundary budget.
  expect(uiPhase5AParentReader('.').violations).toEqual([]);
  for (const p of ['src/features/shots/ShotWorkspace.tsx', 'src/app/ShellHost.tsx', 'vite.config.ts']) {
    const original = readFileSync(p);
    try {
      writeFileSync(p, readFileSync(p, 'utf8') + '\n');
      const rejected = uiPhase5AParentReader('.');
      expect(rejected.violations.length, p).toBeGreaterThan(0);
      expect(rejected.addedPaths, p).toEqual([]);
      expect(uiPhase4SerialQueueParentReader('.').violations.length, p).toBeGreaterThan(0);
    } finally { writeFileSync(p, original); }
    expect(uiPhase5AParentReader('.').violations).toEqual([]);
  }
}, 90000);

it('rejects historical proof and exact Runtime bytes drift', () => {
  const runtime = readdirSync('src-tauri/runtime_packages', { withFileTypes: true })
    .filter((e: { isDirectory(): boolean }) => e.isDirectory())
    .map((e: { name: string }) => join('src-tauri/runtime_packages', e.name, 'workflow_api.json'))
    .find((p: string) => { try { readFileSync(p); return true; } catch { return false; } })!;
  for (const p of ['docs/architecture/ui-phase4-serial-queue-successor.json', PHASE5A_MANIFEST, runtime]) {
    const original = readFileSync(p);
    try {
      writeFileSync(p, readFileSync(p, 'utf8') + '\n');
      const rejected = uiPhase5AParentReader('.');
      expect(rejected.violations.length, p).toBeGreaterThan(0);
      expect(rejected.addedPaths, p).toEqual([]);
      expect(uiPhase4SerialQueueParentReader('.').violations.length, p).toBeGreaterThan(0);
    } finally { writeFileSync(p, original); }
    expect(uiPhase5AParentReader('.').violations).toEqual([]);
  }
}, 90000);

it('invalidates missing evidence and illegal queue authority additions', () => {
  const proofBytes = readFileSync(PHASE5A_MANIFEST);
  try {
    rmSync(PHASE5A_MANIFEST); expect(uiPhase5AParentReader('.').violations.length).toBeGreaterThan(0);
  } finally { writeFileSync(PHASE5A_MANIFEST, proofBytes); }
  const extra = 'src/IllegalQueueOwner.js';
  try {
    writeFileSync(extra, 'export const illegalOwner = true;');
    expect(uiPhase5AParentReader('.').violations).toContain('ui-phase5a-storyboard-inventory:src');
  } finally { rmSync(extra); }
  expect(uiPhase5AParentReader('.').violations).toEqual([]);
}, 90000);
