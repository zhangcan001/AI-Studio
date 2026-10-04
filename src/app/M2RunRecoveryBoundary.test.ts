// @vitest-environment node
import { expect, it } from 'vitest';
// @ts-expect-error Node-only guard fixture.
import { readFileSync } from 'node:fs';
// @ts-expect-error Node-only successor validation.
import { m2RecoveryParentReader } from '../../scripts/m2-run-recovery-boundary-guard.mjs';
// @ts-expect-error Node-only complete M1 historical chain.
import { m1ParentReader } from '../../scripts/m1-readiness-boundary-guard.mjs';
it('validates scoped M2 recovery presentation before frozen M1 projection and fails closed on drift', () => {
  const proof = JSON.parse(readFileSync('docs/architecture/m2-1-run-recovery.json', 'utf8'));
  expect(m2RecoveryParentReader('.').violations).toEqual([]);
  expect(m1ParentReader('.').violations).toEqual([]);
  expect(m2RecoveryParentReader('.').read('src/features/runs/RunDetail.tsx')).not.toContain('runRecoveryPresentation');
  for (const mutate of [
    (p: typeof proof) => { p.parentHead = '0'.repeat(40); },
    (p: typeof proof) => { p.paths['src/product/types.ts'] = { beforeHash: null, afterHash: '0'.repeat(64) }; },
    (p: typeof proof) => { p.paths['src/features/runs/RunDetail.tsx'].beforeHash = '0'.repeat(64); },
    (p: typeof proof) => { p.paths['src/features/runs/runRecoveryPresentation.ts'].afterHash = '0'.repeat(64); },
    (p: typeof proof) => { p.frontend.untouchedAggregateHash = '0'.repeat(64); },
    (p: typeof proof) => { p.scripts.afterAggregateHash = '0'.repeat(64); },
    (p: typeof proof) => { p.backend.afterFiles++; },
    (p: typeof proof) => { p.invariants.runRetryAuthorityChanged = true; },
    (p: typeof proof) => { p.invariants.reviewAuthorityChanged = true; },
    (p: typeof proof) => { p.invariants.shotSelectionAuthorityChanged = true; },
    (p: typeof proof) => { p.invariants.newRecoveryStore = true; },
    (p: typeof proof) => { p.invariants.newPollingOwner = true; },
  ]) {
    const invalid = structuredClone(proof); mutate(invalid); const gate = m2RecoveryParentReader('.', invalid);
    expect(gate.violations.length).toBeGreaterThan(0);
    expect(gate.read('src/features/runs/RunDetail.tsx')).toContain('runRecoveryPresentation');
  }
}, 30000);
