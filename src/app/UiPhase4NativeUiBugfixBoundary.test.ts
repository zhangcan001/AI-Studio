// @vitest-environment node
import { expect, it } from 'vitest';
// @ts-expect-error Node-only immutable source.
import { readFileSync } from 'node:fs';
// @ts-expect-error Current bugfix successor.
import { uiPhase4NativeUiBugfixParentReader, NATIVE_UI_BUGFIX_INVARIANTS, NATIVE_UI_BUGFIX_MANIFEST, NATIVE_UI_BUGFIX_PARENT } from '../../scripts/ui-phase4-native-ui-bugfix-successor-guard.mjs';
// @ts-expect-error Historical Native import successor.
import { uiPhase4NativeImportRepairParentReader } from '../../scripts/ui-phase4-native-import-repair-successor-guard.mjs';
// @ts-expect-error Frozen UI Phase4 reader.
import { uiPhase4ParentReader } from '../../scripts/minimax-video-v2-ui-phase4-successor-guard.mjs';
// @ts-expect-error Git parent facts.
import { readinessLifecycleParentFacts } from '../../scripts/readiness-lifecycle-successor-guard.mjs';

it('accepts exactly the two reviewed Native UI fixes and preserves original Phase4 checkpoint bytes', () => {
  const result = uiPhase4NativeUiBugfixParentReader('.');
  expect(result.violations).toEqual([]);
  const parent = readinessLifecycleParentFacts('.', NATIVE_UI_BUGFIX_PARENT);
  expect(result.read('src/features/create/CreateController.ts')).toBe(parent.read('src/features/create/CreateController.ts'));
  expect(result.read('scripts/ui-phase4-native-import-repair-successor-guard.mjs'))
    .toBe(parent.read('scripts/ui-phase4-native-import-repair-successor-guard.mjs'));
  expect(uiPhase4NativeImportRepairParentReader('.').violations).toEqual([]);
  expect(uiPhase4ParentReader('.').violations).toEqual([]);
  const proof = JSON.parse(readFileSync(NATIVE_UI_BUGFIX_MANIFEST, 'utf8'));
  expect(proof.invariants).toEqual(NATIVE_UI_BUGFIX_INVARIANTS);
}, 30000);

it('rejects forged post-image, new addition, parent, or invariant while leaving published proof immutable', () => {
  const manifest = JSON.parse(readFileSync(NATIVE_UI_BUGFIX_MANIFEST, 'utf8'));
  const variants = [
    (p: any) => { p.parentHead = '0'.repeat(40); },
    (p: any) => { p.paths['src/features/create/CreateController.ts'].afterHash = '0'.repeat(64); },
    (p: any) => { p.paths['scripts/ui-phase4-native-import-repair-successor-guard.mjs'].beforeHash = '0'.repeat(64); },
    (p: any) => { p.paths['src/app/UiPhase4NativeUiBugfixBoundary.test.ts'].afterHash = '0'.repeat(64); },
    (p: any) => { p.invariants.historicalProofsRewritten = true; },
  ];
  for (const mutate of variants) {
    const changed = structuredClone(manifest);
    mutate(changed);
    const result = uiPhase4NativeUiBugfixParentReader('.', changed);
    expect(result.violations.length).toBeGreaterThan(0);
    expect(result.addedPaths).toEqual([]);
  }
}, 30000);
