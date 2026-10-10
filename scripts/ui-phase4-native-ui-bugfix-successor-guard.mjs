// Validate the Phase4 Native UI bug fixes before projecting any published checkpoint.
import { cachedBoundary } from './boundary-validation-cache.mjs';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { readinessLifecycleParentFacts, readinessLifecycleHash } from './readiness-lifecycle-successor-guard.mjs';

export const NATIVE_UI_BUGFIX_PARENT = 'd321080844b71af493c0eb1f39ad7f973820c691';
export const NATIVE_UI_BUGFIX_MANIFEST = 'docs/architecture/ui-phase4-native-ui-bugfix.json';
export const NATIVE_UI_BUGFIX_EXISTING = [
  'src/features/create/CreateController.ts',
  'src/features/create/CreateVideoWorkspace.test.tsx',
  'scripts/ui-phase4-native-import-repair-successor-guard.mjs',
  'scripts/minimax-video-v2-ui-phase3-successor-guard.mjs',
].sort();
export const NATIVE_UI_BUGFIX_ADDED = [
  'scripts/ui-phase4-native-ui-bugfix-successor-guard.mjs',
  'src/app/UiPhase4NativeUiBugfixBoundary.test.ts',
].sort();
export const NATIVE_UI_BUGFIX_FILES = [...NATIVE_UI_BUGFIX_EXISTING, ...NATIVE_UI_BUGFIX_ADDED, NATIVE_UI_BUGFIX_MANIFEST];
export const NATIVE_UI_BUGFIX_INVARIANTS = {
  schemaChanged: false, backupFormatChanged: false, queueAuthorityChanged: false,
  storeAuthorityChanged: false, runtimePackagesChanged: false,
  historicalProofsRewritten: false, nativeImportProofPreserved: true,
  userInterfaceBugfixOnly: true, phase5Started: false,
};
const normalized = s => s.replaceAll('\r\n', '\n');
function validate(root, override) {
  const violations = [], fail = name => violations.push('ui-phase4-native-ui-bugfix-' + name);
  const live = new Map(), disk = p => {
    if (!live.has(p)) live.set(p, normalized(readFileSync(join(root, p), 'utf8')));
    return live.get(p);
  };
  const rejected = () => ({ violations, addedPaths: [], afterHashes: {}, read: disk });
  let proof, parent;
  try {
    proof = override ?? JSON.parse(disk(NATIVE_UI_BUGFIX_MANIFEST));
    parent = readinessLifecycleParentFacts(root, NATIVE_UI_BUGFIX_PARENT);
  } catch { fail('missing-evidence'); return rejected(); }
  const paths = [...NATIVE_UI_BUGFIX_EXISTING, ...NATIVE_UI_BUGFIX_ADDED].sort();
  if (proof.schemaVersion !== 1 || proof.checkpoint !== 'MINIMAX_VIDEO_UI_PHASE4_NATIVE_UI_BUGFIX'
      || proof.parentHead !== NATIVE_UI_BUGFIX_PARENT
      || JSON.stringify(proof.invariants) !== JSON.stringify(NATIVE_UI_BUGFIX_INVARIANTS)
      || JSON.stringify(Object.keys(proof.paths ?? {}).sort()) !== JSON.stringify(paths)) fail('contract');
  if (violations.length) return rejected();
  try {
    for (const p of NATIVE_UI_BUGFIX_EXISTING) {
      if (!parent.paths.includes(p) || proof.paths[p]?.beforeHash !== readinessLifecycleHash(parent.read(p))
          || proof.paths[p]?.afterHash !== readinessLifecycleHash(disk(p))) fail('path:' + p);
    }
    for (const p of NATIVE_UI_BUGFIX_ADDED) {
      if (parent.paths.includes(p) || proof.paths[p]?.beforeHash !== null
          || proof.paths[p]?.afterHash !== readinessLifecycleHash(disk(p))) fail('addition:' + p);
    }
  } catch { fail('missing-evidence'); }
  if (violations.length) return rejected();
  return {
    violations,
    addedPaths: [...NATIVE_UI_BUGFIX_ADDED, NATIVE_UI_BUGFIX_MANIFEST],
    afterHashes: Object.fromEntries(paths.map(p => [p, proof.paths[p].afterHash])),
    read: p => NATIVE_UI_BUGFIX_EXISTING.includes(p) ? parent.read(p) : disk(p),
  };
}
export function uiPhase4NativeUiBugfixParentReader(root, override) {
  return cachedBoundary(root, 'uiPhase4NativeUiBugfixParentReader', override,
    () => validate(root, override), NATIVE_UI_BUGFIX_MANIFEST);
}
