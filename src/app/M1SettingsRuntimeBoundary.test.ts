// @vitest-environment node
import { expect, it } from 'vitest';
// @ts-expect-error Node-only guard fixture.
import { readFileSync } from 'node:fs';
// @ts-expect-error Node-only successor validation.
import { m1SettingsParentReader } from '../../scripts/m1-settings-runtime-boundary-guard.mjs';
// @ts-expect-error Node-only complete historical successor chain.
import { m1ParentReader } from '../../scripts/m1-readiness-boundary-guard.mjs';
it('M1-3 freezes exact scoped settings/tool/navigation bytes without changing operational authority', () => {
  const proof = JSON.parse(readFileSync('docs/architecture/m1-3-settings-runtime.json', 'utf8'));
  expect(m1SettingsParentReader('.').violations).toEqual([]); expect(m1ParentReader('.').violations).toEqual([]);
  expect(m1SettingsParentReader('.').read('src/app/App.tsx')).not.toContain('onOpenToolHub=');
  const app = readFileSync('src/app/App.tsx', 'utf8');
  expect(app).toContain('onOpenProjectGenerators={(projectId) => void navigate({ kind: "project-settings", projectId, section: "generators" })}');
  expect(app).toContain('onOpenToolHub={() => void navigate({ kind: "system-settings", section: "advanced-tools", returnTo: route })}');
  for (const mutate of [
    (p: typeof proof) => { p.parentHead = '0'.repeat(40); },
    (p: typeof proof) => { p.paths['src/services/ipc.ts'] = { beforeHash: null, afterHash: '0'.repeat(64) }; },
    (p: typeof proof) => { p.paths['src/app/App.tsx'].beforeHash = '0'.repeat(64); },
    (p: typeof proof) => { p.paths['src/features/tools/LocalToolHub.tsx'].afterHash = '0'.repeat(64); },
    (p: typeof proof) => { p.frontend.untouchedAggregateHash = '0'.repeat(64); },
    (p: typeof proof) => { p.scripts.afterAggregateHash = '0'.repeat(64); },
    (p: typeof proof) => { p.backend.afterFiles++; },
    (p: typeof proof) => { p.invariants.comfySettingsAuthorityChanged = true; },
    (p: typeof proof) => { p.invariants.toolHubAuthorityChanged = true; },
    (p: typeof proof) => { p.invariants.automaticToolDiscovery = true; },
    (p: typeof proof) => { p.invariants.automaticToolHealthPolling = true; },
    (p: typeof proof) => { p.invariants.processLaunchAdded = true; },
  ]) {
    const invalid = structuredClone(proof); mutate(invalid); const gate = m1SettingsParentReader('.', invalid);
    expect(gate.violations.length).toBeGreaterThan(0); expect(gate.read('src/app/App.tsx')).toContain('onOpenToolHub=');
  }
}, 30000);
