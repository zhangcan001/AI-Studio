// @vitest-environment node
import { expect, it } from 'vitest';
// @ts-expect-error Test-only Node helper.
import { readFileSync } from 'node:fs';
// @ts-expect-error Build-time successor validation, not application code.
import { m1ParentReader } from '../../scripts/m1-readiness-boundary-guard.mjs';
import { preservesCreateDraftForSettings } from './App';
import type { AppRoute } from './routes/types';

it('M1-1 bypasses discard only for an exact scoped Create return with existing saved state', () => {
  const current = { kind: 'create', projectId: 'p', shotId: 's', stage: 'image' } as const;
  const next = { kind: 'system-settings', section: 'general', returnTo: current } as const;
  expect(preservesCreateDraftForSettings(current, next, 'p:s:image')).toBe(true);
  expect(preservesCreateDraftForSettings(current, { ...next, section: 'advanced-workflows' }, 'p:s:image')).toBe(true);
  for (const route of [
    { ...next, section: 'advanced-tools' }, { kind: 'project-list' },
    { ...next, returnTo: { ...current, projectId: 'other' } },
    { ...next, returnTo: { ...current, shotId: 'other' } },
    { ...next, returnTo: { ...current, stage: 'video' } },
    { ...next, returnTo: { ...current, surface: 'batch' } },
  ] satisfies AppRoute[]) expect(preservesCreateDraftForSettings(current, route, 'p:s:image')).toBe(false);
  expect(preservesCreateDraftForSettings(current, next)).toBe(false);
  expect(preservesCreateDraftForSettings(current, next, 'other:s:image')).toBe(false);
});

it('M1-1 advances only reviewed live bytes and preserves parent/untouched aggregates', () => {
  const proof = JSON.parse(readFileSync('docs/architecture/m1-1-readiness.json', 'utf8'));
  const current = m1ParentReader('.');
  expect(current.violations).toEqual([]);
  expect(current.read('src/features/create/CreateResults.tsx')).toContain('focus(issue.details.field)');
  for (const mutate of [
    (p: typeof proof) => { p.parentHead = '0'.repeat(40); },
    (p: typeof proof) => { p.paths['src/services/ipc.ts'] = { beforeHash: null, afterHash: '0'.repeat(64) }; },
    (p: typeof proof) => { p.paths['src/app/App.tsx'].beforeHash = '0'.repeat(64); },
    (p: typeof proof) => { p.paths['src/app/App.tsx'].afterHash = '0'.repeat(64); },
    (p: typeof proof) => { p.backend.untouchedAggregateHash = '0'.repeat(64); },
    (p: typeof proof) => { p.frontend.afterFiles++; },
    (p: typeof proof) => { p.styles.afterAggregateHash = '0'.repeat(64); },
    (p: typeof proof) => { p.scripts.afterAggregateHash = '0'.repeat(64); },
    (p: typeof proof) => { p.invariants.queueAuthorityChanged = true; },
  ]) {
    const invalid = structuredClone(proof); mutate(invalid);
    const gate = m1ParentReader('.', invalid);
    expect(gate.violations.length).toBeGreaterThan(0);
    // Fail closed: no parent projection may hide live drift when proof is invalid.
    expect(gate.read('src/features/create/CreateResults.tsx')).not.toContain('focus(issue.details.field)');
  }
}, 30000);
