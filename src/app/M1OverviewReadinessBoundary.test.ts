// @vitest-environment node
import { expect, it } from 'vitest';
// @ts-expect-error Node-only architecture guard.
import { readFileSync } from 'node:fs';
// @ts-expect-error Node-only successor validation.
import { m1OverviewParentReader } from '../../scripts/m1-overview-readiness-boundary-guard.mjs';
// @ts-expect-error Node-only historical successor validation.
import { m1ParentReader } from '../../scripts/m1-readiness-boundary-guard.mjs';

it('M1-2 validates exact parent/live/untouched bytes before M1-1 projection', () => {
  const proof = JSON.parse(readFileSync('docs/architecture/m1-2-overview-readiness.json', 'utf8'));
  expect(m1OverviewParentReader('.').violations).toEqual([]);
  expect(m1ParentReader('.').violations).toEqual([]);
  expect(m1OverviewParentReader('.').read('src/app/v3/ProjectOverviewPage.tsx')).not.toContain('runtimeReadinessPresentation');
  for (const mutate of [
    (p: typeof proof) => { p.parentHead = '0'.repeat(40); },
    (p: typeof proof) => { p.paths['src/services/ipc.ts'] = { beforeHash: null, afterHash: '0'.repeat(64) }; },
    (p: typeof proof) => { p.paths['src/app/v3/ProjectOverviewPage.tsx'].beforeHash = '0'.repeat(64); },
    (p: typeof proof) => { p.paths['src/app/v3/ProjectOverviewPage.tsx'].afterHash = '0'.repeat(64); },
    (p: typeof proof) => { p.frontend.untouchedAggregateHash = '0'.repeat(64); },
    (p: typeof proof) => { p.scripts.afterAggregateHash = '0'.repeat(64); },
    (p: typeof proof) => { p.backend.afterFiles++; },
    (p: typeof proof) => { p.styles.afterAggregateHash = '0'.repeat(64); },
    (p: typeof proof) => { p.invariants.projectCommandCenterAuthorityChanged = true; },
    (p: typeof proof) => { p.invariants.newReadinessAuthority = true; },
    (p: typeof proof) => { p.invariants.newPollingOwner = true; },
  ]) {
    const invalid = structuredClone(proof); mutate(invalid);
    const gate = m1OverviewParentReader('.', invalid);
    expect(gate.violations.length).toBeGreaterThan(0);
    expect(gate.read('src/app/v3/ProjectOverviewPage.tsx')).toContain('runtimeReadinessPresentation');
  }
}, 30000);
