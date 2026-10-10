// Phase4 serial-queue successor: validate the current implementation before
// projecting the exact committed predecessor into immutable historical guards.
// Historical manifests, runtime packages and previous proof bytes are not edited.
import { cachedBoundary } from './boundary-validation-cache.mjs';
import { readinessLifecycleParentFacts } from './readiness-lifecycle-successor-guard.mjs';
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

export const SERIAL_QUEUE_PARENT = '6c7399d061b253897cd4ff3108d4a5ea7ee9b0ef';
export const SERIAL_QUEUE_MANIFEST = 'docs/architecture/ui-phase4-serial-queue-successor.json';
export const SERIAL_QUEUE_EXISTING = [
  'scripts/minimax-video-phase2-successor-guard.mjs',
  'scripts/minimax-video-v2-ui-phase3-successor-guard.mjs',
  'scripts/queue-lifecycle-repair-successor-guard.mjs',
  'scripts/ui-phase4-native-ui-bugfix-successor-guard.mjs',
  'src-tauri/src/application/ports/mod.rs',
  'src-tauri/src/application/ports/production_queue_repository.rs',
  'src-tauri/src/application/product/creation_facade/submission.rs',
  'src-tauri/src/application/product/run_facade.rs',
  'src-tauri/src/application/production_queue_service.rs',
  'src-tauri/src/application/production_start_admission_service.rs',
  'src-tauri/src/commands/product.rs',
  'src-tauri/src/infrastructure/database/dev048_consistency_e2e.rs',
  'src-tauri/src/infrastructure/database/pool.rs',
  'src-tauri/src/infrastructure/database/repositories/production_queue.rs',
  'src-tauri/src/infrastructure/database/repositories/shot_video_input/tests.rs',
  'src-tauri/src/lib.rs',
  'src-tauri/tests/dev035_structure_e2e.rs',
  'src-tauri/tests/dev040_safety.rs',
  'src-tauri/tests/dev049_context_resolver.rs',
  'src-tauri/tests/dev052_runtime_integration.rs',
  'src-tauri/tests/dev055_release_compatibility.rs',
  'src-tauri/tests/product_library_contract.rs',
  'src-tauri/tests/project_database_fixture.rs',
  'src-tauri/tests/support/creation_submission_contract.rs',
  'src/features/create/CreatePage.test.tsx',
  'src/features/create/CreateResults.tsx',
  'src/features/create/CreateVideoWorkspace.test.tsx',
  'src/product/types.ts',
].sort();
export const SERIAL_QUEUE_ADDED = [
  'scripts/ui-phase4-serial-queue-successor-guard.mjs',
  'src/app/UiPhase4SerialQueueBoundary.test.ts',
  'src-tauri/migrations/044_deferred_direct_generation.sql',
].sort();
export const SERIAL_QUEUE_FILES = [...SERIAL_QUEUE_EXISTING, ...SERIAL_QUEUE_ADDED, SERIAL_QUEUE_MANIFEST];
export const SERIAL_QUEUE_INVARIANTS = {
  runtimePackagesChanged: false,
  historicalProofsRewritten: false,
  backupFormatChanged: false,
  extraTaskExecutorAdded: false,
  officialQueueAndStartAdmissionPreserved: true,
  maxConcurrentVideoGeneration: 1,
  deferredRequestsDurable: true,
  phase5Started: false,
};
const hash = b => createHash('sha256').update(b).digest('hex');
const normalized = s => s.replaceAll('\r\n', '\n');
function validate(root, override) {
  const violations = [], fail = k => violations.push('serial-queue-successor-' + k);
  const live = new Map(), disk = p => {
    if (!live.has(p)) live.set(p, readFileSync(join(root, p)));
    return live.get(p);
  };
  // Git normalizes source text blobs to LF while Windows checkout may use
  // CRLF. Keep the SQL migration byte-exact; historical Runtime Package and
  // frozen proof validators retain their independent raw-byte requirements.
  const read = p => normalized(disk(p).toString('utf8'));
  const canonicalHash = p => p === 'src-tauri/migrations/044_deferred_direct_generation.sql'
    ? hash(disk(p)) : hash(Buffer.from(read(p), 'utf8'));
  const denied = () => ({ violations, addedPaths: [], afterHashes: {}, read });
  let proof, parent;
  try {
    proof = override ?? JSON.parse(read(SERIAL_QUEUE_MANIFEST));
    parent = readinessLifecycleParentFacts(root, SERIAL_QUEUE_PARENT);
  } catch { fail('missing-evidence'); return denied(); }
  const scope = [...SERIAL_QUEUE_EXISTING, ...SERIAL_QUEUE_ADDED].sort();
  if (proof.schemaVersion !== 1 || proof.checkpoint !== 'UI_PHASE4_SERIAL_QUEUE_SUCCESSOR'
      || proof.parentHead !== SERIAL_QUEUE_PARENT
      || JSON.stringify(proof.invariants) !== JSON.stringify(SERIAL_QUEUE_INVARIANTS)
      || JSON.stringify(Object.keys(proof.paths ?? {}).sort()) !== JSON.stringify(scope)) fail('contract');
  if (violations.length) return denied();
  for (const p of SERIAL_QUEUE_EXISTING) {
    try {
      if (!parent.paths.includes(p) || proof.paths[p]?.afterHash !== canonicalHash(p)) fail('path:' + p);
    } catch { fail('missing:' + p); }
  }
  for (const p of SERIAL_QUEUE_ADDED) {
    try {
      if (parent.paths.includes(p) || proof.paths[p]?.afterHash !== canonicalHash(p)) fail('addition:' + p);
    } catch { fail('missing:' + p); }
  }
  if (violations.length) return denied();
  return {
    violations, addedPaths: [...SERIAL_QUEUE_ADDED, SERIAL_QUEUE_MANIFEST],
    // Current successor validates Git-canonical text bytes above; historical
    // readers hash the same LF-normalized source without altering their proofs.
    afterHashes: Object.fromEntries(scope.map(p => [p, canonicalHash(p)])),
    read: p => SERIAL_QUEUE_EXISTING.includes(p) ? parent.read(p) : read(p),
  };
}
export function uiPhase4SerialQueueParentReader(root, override) {
  return cachedBoundary(root, 'uiPhase4SerialQueueParentReader', override,
    () => validate(root, override), SERIAL_QUEUE_MANIFEST);
}
