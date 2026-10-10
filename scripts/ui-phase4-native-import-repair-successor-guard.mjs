// Validate an explicit Phase4 Native import repair before replaying frozen UI proofs.
import { cachedBoundary } from './boundary-validation-cache.mjs';
import { uiPhase4NativeUiBugfixParentReader } from './ui-phase4-native-ui-bugfix-successor-guard.mjs';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { readinessLifecycleParentFacts, readinessLifecycleHash } from './readiness-lifecycle-successor-guard.mjs';

export const NATIVE_REPAIR_PARENT = 'b2718afd2d1a1293d78b97a8bb2291900d739239';
export const NATIVE_REPAIR_MANIFEST = 'docs/architecture/ui-phase4-native-import-repair.json';
export const NATIVE_REPAIR_EXISTING = [
  'src-tauri/Cargo.toml', 'src-tauri/Cargo.lock',
  'src-tauri/src/application/media_probe.rs',
  'src-tauri/src/application/source_asset_import_service.rs',
  'src-tauri/src/infrastructure/filesystem/asset_store.rs',
  'scripts/ui-phase4-ci-repair-successor-guard.mjs',
  'scripts/minimax-video-v2-ui-phase3-successor-guard.mjs',
].sort();
export const NATIVE_REPAIR_ADDED = [
  'scripts/ui-phase4-native-import-repair-successor-guard.mjs',
  'src/app/UiPhase4NativeImportRepairBoundary.test.ts',
].sort();
export const NATIVE_REPAIR_FILES = [...NATIVE_REPAIR_EXISTING, ...NATIVE_REPAIR_ADDED, NATIVE_REPAIR_MANIFEST];
export const NATIVE_REPAIR_INVARIANTS = {
  schemaChanged: false,
  backupFormatChanged: false,
  queueAuthorityChanged: false,
  studioStoreAuthorityChanged: false,
  runtimePackagesChanged: false,
  historicalProofsRewritten: false,
  nativeMixedImportAccepted: false,
};
const normalize = s => s.replaceAll('\r\n', '\n');

function validate(root, override) {
  const successor = uiPhase4NativeUiBugfixParentReader(root);
  const violations = [...successor.violations], fail = label => violations.push('ui-phase4-native-repair-' + label);
  const live = new Map(), disk = p => {
    if (!live.has(p)) live.set(p, normalize(successor.read(p)));
    return live.get(p);
  };
  const rejected = () => ({ violations, addedPaths: [], afterHashes: {}, read: disk });
  let proof, parent;
  try {
    proof = override ?? JSON.parse(disk(NATIVE_REPAIR_MANIFEST));
    parent = readinessLifecycleParentFacts(root, NATIVE_REPAIR_PARENT);
  } catch { fail('missing-evidence'); return rejected(); }
  const paths = [...NATIVE_REPAIR_EXISTING, ...NATIVE_REPAIR_ADDED].sort();
  if (proof.schemaVersion !== 1 || proof.checkpoint !== 'MINIMAX_VIDEO_UI_PHASE4_NATIVE_IMPORT_REPAIR'
    || proof.parentHead !== NATIVE_REPAIR_PARENT
    || JSON.stringify(proof.invariants) !== JSON.stringify(NATIVE_REPAIR_INVARIANTS)) fail('contract');
  if (JSON.stringify(Object.keys(proof.paths ?? {}).sort()) !== JSON.stringify(paths)) fail('scope');
  if (violations.length) return rejected();
  try {
    for (const p of NATIVE_REPAIR_EXISTING) {
      if (!parent.paths.includes(p)
        || proof.paths[p]?.beforeHash !== readinessLifecycleHash(parent.read(p))
        || proof.paths[p]?.afterHash !== readinessLifecycleHash(disk(p))) fail('path:' + p);
    }
    for (const p of NATIVE_REPAIR_ADDED) {
      if (parent.paths.includes(p) || proof.paths[p]?.beforeHash !== null
        || proof.paths[p]?.afterHash !== readinessLifecycleHash(disk(p))) fail('addition:' + p);
    }
  } catch { fail('missing-evidence'); }
  if (violations.length) return rejected();
  return {
    violations,
    addedPaths: [...NATIVE_REPAIR_ADDED, NATIVE_REPAIR_MANIFEST, ...successor.addedPaths],
    afterHashes: { ...Object.fromEntries(paths.map(p => [p, proof.paths[p].afterHash])), ...successor.afterHashes },
    read: p => NATIVE_REPAIR_EXISTING.includes(p) ? parent.read(p) : disk(p),
  };
}
export function uiPhase4NativeImportRepairParentReader(root, override) {
  return cachedBoundary(root, 'uiPhase4NativeImportRepairParentReader', override,
    () => validate(root, override), NATIVE_REPAIR_MANIFEST);
}
