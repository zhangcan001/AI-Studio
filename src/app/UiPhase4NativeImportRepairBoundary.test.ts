// @vitest-environment node
import { expect, it } from 'vitest';
// @ts-expect-error Node-only proof fixture.
import { readFileSync } from 'node:fs';
// @ts-expect-error Immutable Git snapshot source.
import { readinessLifecycleParentFacts } from '../../scripts/readiness-lifecycle-successor-guard.mjs';
// @ts-expect-error Native repair successor.
import { uiPhase4NativeImportRepairParentReader, NATIVE_REPAIR_MANIFEST, NATIVE_REPAIR_PARENT, NATIVE_REPAIR_INVARIANTS } from '../../scripts/ui-phase4-native-import-repair-successor-guard.mjs';
// @ts-expect-error Frozen checkpoint reader.
import { uiPhase4CiRepairParentReader } from '../../scripts/ui-phase4-ci-repair-successor-guard.mjs';
// @ts-expect-error Immutable Phase4 UI reader.
import { uiPhase4ParentReader } from '../../scripts/minimax-video-v2-ui-phase4-successor-guard.mjs';

it('validates the reviewed Native repair before projecting unchanged Phase4 historical proofs', () => {
  const result = uiPhase4NativeImportRepairParentReader('.');
  expect(result.violations).toEqual([]);
  const parent = readinessLifecycleParentFacts('.', NATIVE_REPAIR_PARENT);
  expect(result.read('src-tauri/src/application/media_probe.rs'))
    .toBe(parent.read('src-tauri/src/application/media_probe.rs'));
  expect(result.read('src-tauri/Cargo.toml')).toBe(parent.read('src-tauri/Cargo.toml'));
  expect(uiPhase4CiRepairParentReader('.').violations).toEqual([]);
  expect(uiPhase4ParentReader('.').violations).toEqual([]);
  const manifest = JSON.parse(readFileSync(NATIVE_REPAIR_MANIFEST, 'utf8'));
  expect(manifest.invariants).toEqual(NATIVE_REPAIR_INVARIANTS);
  expect(manifest.invariants.nativeMixedImportAccepted).toBe(false);
}, 30000);

it('rejects altered Native repair hashes, parent, or claimed acceptance', () => {
  const manifest = JSON.parse(readFileSync(NATIVE_REPAIR_MANIFEST, 'utf8'));
  const variants = [
    (p: any) => { p.parentHead = '0'.repeat(40); },
    (p: any) => { p.paths['src-tauri/src/application/media_probe.rs'].afterHash = '0'.repeat(64); },
    (p: any) => { p.paths['src-tauri/Cargo.lock'].beforeHash = '0'.repeat(64); },
    (p: any) => { p.paths['src/features/create/CreatePage.tsx'] = { beforeHash: null, afterHash: '0'.repeat(64) }; },
    (p: any) => { p.invariants.nativeMixedImportAccepted = true; },
  ];
  for (const mutate of variants) {
    const altered = structuredClone(manifest);
    mutate(altered);
    const result = uiPhase4NativeImportRepairParentReader('.', altered);
    expect(result.violations.length).toBeGreaterThan(0);
    expect(result.addedPaths).toEqual([]);
  }
}, 30000);
