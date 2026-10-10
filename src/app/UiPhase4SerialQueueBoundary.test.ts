// @vitest-environment node
import { expect, it } from 'vitest';
// @ts-expect-error Node-only successor checkpoint.
import { uiPhase4SerialQueueParentReader, SERIAL_QUEUE_EXISTING, SERIAL_QUEUE_MANIFEST, SERIAL_QUEUE_PARENT, SERIAL_QUEUE_INVARIANTS } from '../../scripts/ui-phase4-serial-queue-successor-guard.mjs';
// @ts-expect-error Node-only immutable Git proof.
import { execFileSync } from 'node:child_process';
// @ts-expect-error Node-only evidence.
import { readFileSync } from 'node:fs';

it('validates an exact serial-queue successor and projects all prior byte checkpoints', () => {
  const verified = uiPhase4SerialQueueParentReader('.');
  expect(verified.violations).toEqual([]);
  const proof = JSON.parse(readFileSync(SERIAL_QUEUE_MANIFEST, 'utf8'));
  expect(proof.parentHead).toBe(SERIAL_QUEUE_PARENT);
  expect(proof.invariants).toEqual(SERIAL_QUEUE_INVARIANTS);
  for (const path of SERIAL_QUEUE_EXISTING) {
    const prior = execFileSync('git', ['show', `${SERIAL_QUEUE_PARENT}:${path}`], { encoding: 'utf8' }).replaceAll('\r\n', '\n');
    expect(verified.read(path)).toBe(prior);
  }
});

it('rejects forged parent, allowance scope, invariants and source hashes', () => {
  const proof = JSON.parse(readFileSync(SERIAL_QUEUE_MANIFEST, 'utf8'));
  for (const mutate of [
    (p: any) => { p.parentHead = '0'.repeat(40); },
    (p: any) => { p.invariants.extraTaskExecutorAdded = true; },
    (p: any) => { p.paths['src/services/tauriClient.ts'] = { afterHash: '0'.repeat(64) }; },
    (p: any) => { p.paths['src-tauri/src/application/production_queue_service.rs'].afterHash = '0'.repeat(64); },
    (p: any) => { p.paths['src-tauri/migrations/044_deferred_direct_generation.sql'].afterHash = '0'.repeat(64); },
  ]) {
    const forged = structuredClone(proof);
    mutate(forged);
    const denied = uiPhase4SerialQueueParentReader('.', forged);
    expect(denied.violations.length).toBeGreaterThan(0);
    expect(denied.addedPaths).toEqual([]);
  }
});
