import { cachedBoundary } from './boundary-validation-cache.mjs';
import { readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';
import { createHash } from 'node:crypto';
import { READINESS_LIFECYCLE_GROUPS, readinessLifecycleParentFacts, readinessLifecycleHash, readinessLifecycleAggregate, MINIMAX_VIDEO_PHASE1_MANIFEST } from './readiness-lifecycle-successor-guard.mjs';
import { minimaxVideoPhase3ParentReader, minimaxVideoPhase3FixtureFiles, MINIMAX_VIDEO_PHASE3_EXISTING, MINIMAX_VIDEO_PHASE3_PARENT } from './minimax-video-phase3-successor-guard.mjs';

// One reviewed stage, directly based on the final Phase1 commit. Never repin an old proof.
export const MINIMAX_VIDEO_PHASE2_PARENT = '8a0d037443dcc66f804379357da13c5e6bc92f74';
export const MINIMAX_VIDEO_PHASE2_MANIFEST = 'docs/architecture/minimax-video-phase2.json';
export const MINIMAX_VIDEO_PHASE2_EXISTING = [
  'CONTEXT.md',
  'scripts/2-1-rc-backup-asset-version-repair-guard.mjs',
  'scripts/ai-studio-2-1-closeout-boundary-guard.mjs',
  'scripts/boundary-validation-cache.mjs',
  'scripts/m4-readonly-media-integrity-boundary-guard.mjs',
  'scripts/phase13-observability-guard.mjs',
  'scripts/readiness-lifecycle-successor-guard.mjs',
  'src-tauri/src/app_state.rs',
  'src-tauri/src/application/asset_deletion_service.rs',
  'src-tauri/src/application/generation_catalog_service.rs',
  'src-tauri/src/application/generation_input_preparer.rs',
  'src-tauri/src/application/generation_service.rs',
  'src-tauri/src/application/media_probe.rs',
  'src-tauri/src/application/mod.rs',
  'src-tauri/src/application/ports/asset_deletion_repository.rs',
  'src-tauri/src/application/ports/asset_repository.rs',
  'src-tauri/src/application/ports/mod.rs',
  'src-tauri/src/application/product/creation_facade.rs',
  'src-tauri/src/application/product/mod.rs',
  'src-tauri/src/application/production_queue_service.rs',
  'src-tauri/src/application/project_backup_service.rs',
  'src-tauri/src/application/shot_batch_service.rs',
  'src-tauri/src/application/shot_context_resolver.rs',
  'src-tauri/src/application/shot_readiness_evaluator.rs',
  'src-tauri/src/application/shot_readiness_service.rs',
  'src-tauri/src/application/shot_service.rs',
  'src-tauri/src/application/shot_service/creation.rs',
  'src-tauri/src/application/shot_workflow_compatibility.rs',
  'src-tauri/src/application/source_asset_import_service.rs',
  'src-tauri/src/commands/asset.rs',
  'src-tauri/src/commands/product.rs',
  'src-tauri/src/commands/shot.rs',
  'src-tauri/src/domain/shot_context.rs',
  'src-tauri/src/error.rs',
  'src-tauri/src/infrastructure/database/pool.rs',
  'src-tauri/src/infrastructure/database/repositories/asset.rs',
  'src-tauri/src/infrastructure/database/repositories/asset_deletion.rs',
  'src-tauri/src/infrastructure/database/repositories/mod.rs',
  'src-tauri/src/infrastructure/database/repositories/project_backup.rs',
  'src-tauri/src/lib.rs',
  'src-tauri/tests/dev049_context_resolver.rs',
  'src-tauri/tests/dev050_readiness.rs',
  'src-tauri/tests/dev052_runtime_integration.rs',
  'src-tauri/tests/support/minimax_video_policy_regressions.rs',
  'src/app/AIStudio21PublicationBoundary.test.ts',
  'src/app/BackendBoundary.test.ts',
  'src/app/H3GenerationRepairBoundary.test.ts',
  'src/app/H3ReleaseCloseoutPhase1Boundary.test.ts',
  'src/app/MiniMaxVideoPhase1Boundary.test.ts',
  'src/app/Phase7Retirement.test.tsx',
  'src/app/QueueLifecycleRepairBoundary.test.ts',
  'src/app/ReadinessLifecycleBoundary.test.ts',
  'src/features/assets/AssetLibrary.tsx',
  'src/features/create/CreateController.ts',
  'src/features/create/CreateInputs.tsx',
  'src/features/create/CreatePage.tsx',
  'src/features/create/CreateResults.tsx',
  'src/features/library/LibraryPage.test.tsx',
  'src/features/runtime/shotWorkflowCompatibility.test.ts',
  'src/features/runtime/shotWorkflowCompatibility.ts',
  'src/features/shots/ShotInspector.tsx',
  'src/features/shots/ShotWorkspace.tsx',
  'src/features/shots/shotProductionState.test.ts',
  'src/features/shots/shotProductionState.ts',
  'src/features/workflow-lab/WorkflowLabBoundary.test.tsx',
  'src/product/client.test.ts',
  'src/product/client.ts',
  'src/product/errors.ts',
  'src/product/transport.ts',
  'src/product/types.ts',
  'src/services/tauriClient.ts',
  'src/types/generation.ts',
  'src/types/shot.ts',
];
export const MINIMAX_VIDEO_PHASE2_ADDED = [
  'docs/architecture/minimax-video-phase2-design.md',
  'docs/handoffs/minimax-video-phase2-handoff.md',
  'scripts/minimax-video-phase2-successor-guard.mjs',
  'src-tauri/migrations/043_shot_video_inputs.sql',
  'src-tauri/src/application/ports/shot_video_input_repository.rs',
  'src-tauri/src/application/product/video_inputs.rs',
  'src-tauri/src/application/project_backup_service/video_inputs.rs',
  'src-tauri/src/application/shot_video_input_service.rs',
  'src-tauri/src/application/source_asset_import_service/source_paths.rs',
  'src-tauri/src/infrastructure/database/repositories/project_backup/video_inputs.rs',
  'src-tauri/src/infrastructure/database/repositories/shot_video_input.rs',
  'src-tauri/src/infrastructure/database/repositories/shot_video_input/tests.rs',
  'src-tauri/tests/support/minimax_video_input_regressions.rs',
  'src/app/MiniMaxVideoPhase2Boundary.test.ts',
  'src/features/create/usePersistentVideoInputs.test.tsx',
  'src/features/create/usePersistentVideoInputs.ts',
  'src/features/shots/ShotVideoInputsPanel.test.tsx',
  'src/features/shots/ShotVideoInputsPanel.tsx',
  'src/types/shotVideoInput.ts',
];
export const MINIMAX_VIDEO_PHASE2_FILES = [...MINIMAX_VIDEO_PHASE2_EXISTING, ...MINIMAX_VIDEO_PHASE2_ADDED, MINIMAX_VIDEO_PHASE2_MANIFEST];
// Owned negative fixtures need the complete governed parent view, including
// older domain design docs, not just files changed by this checkpoint.
export const minimaxVideoPhase2FixtureFiles = root => [...readinessLifecycleParentFacts(root,MINIMAX_VIDEO_PHASE2_PARENT).paths,...MINIMAX_VIDEO_PHASE2_FILES,...minimaxVideoPhase3FixtureFiles(root)];
export const minimaxVideoPhase2Groups = () => [...READINESS_LIFECYCLE_GROUPS, ['domainDocs', 'docs/architecture', /\.md$/]];
export const MINIMAX_VIDEO_PHASE2_INVARIANTS = {
  schemaChanged: true, backupFormatChanged: true, queueAuthorityChanged: false,
  storeAuthorityChanged: false, uiV2Changed: false, historicalDataRewritten: false,
  immutablePackagesPreserved: true, phase1AdmissionPreserved: true,
  exactRecipeInputIdentity: true, inputOccRequired: true, inputResultSemanticsSeparated: true,
  sharedH3ConstraintsPreserved: true, pureSourceImport: true,
};
// Two explicitly frozen, literal-byte baselines. Never auto-select by live
// bytes or normalize packages. CI/fresh Git checkouts use the default 'git';
// this pre-existing CRLF checkout uses an explicitly selected local profile.
export const MINIMAX_VIDEO_PHASE2_RUNTIME_BYTES = {
  git: '30c02a776b62d0723272d1f41f5d4e612b54e0e01b898189ff77b570e760eccf',
  checkout: 'bb5561badae002819a55966d9975295d030878d6a1447c378335dc8128e37b2e',
};
const shaBytes = bytes => createHash('sha256').update(bytes).digest('hex');
const rawAggregate = (paths, read) => shaBytes(JSON.stringify(paths.map(p=>[p,shaBytes(read(p))])));
const configs = ['package.json','pnpm-lock.yaml','src-tauri/Cargo.toml','src-tauri/Cargo.lock',
  'src-tauri/tauri.conf.json','src-tauri/build.rs','.github/workflows/ci.yml','README.md',
  'docs/AI_STUDIO_2_1_CLOSEOUT.md','docs/RELEASE_NOTES_v2.1.0-personal.md'];
const normalize = s => s.replaceAll('\r\n','\n');
const files = (root, dir, pattern) => readdirSync(join(root,dir),{withFileTypes:true}).flatMap(e => {
  const p = `${dir}/${e.name}`; return e.isDirectory() ? files(root,p,pattern) : pattern.test(p) ? [p] : [];
}).sort();

function validate(root, override) {
  const successor = minimaxVideoPhase3ParentReader(root);
  const violations = [...successor.violations], fail = s => violations.push(`minimax-video-phase2-${s}`);
  const rawDisk = p => normalize(readFileSync(join(root, p), 'utf8'));
  // Files owned by Phase 3 stay on that checkpoint's parent bytes even when a
  // later probe is rejected. Everything else stays live so a deleted or drifted
  // file still produces this checkpoint's evidence name.
  let phase3Facts;
  const live = new Map(), disk = p => {
    if (live.has(p)) return live.get(p);
    const value = MINIMAX_VIDEO_PHASE3_EXISTING.includes(p)
      ? (phase3Facts ??= readinessLifecycleParentFacts(root, MINIMAX_VIDEO_PHASE3_PARENT)).read(p)
      : (successor.violations.length ? rawDisk(p) : normalize(successor.read(p)));
    live.set(p, value);
    return value;
  };
  const rejected = () => ({ violations, addedPaths: [], afterHashes: {}, backendAggregateSha256: undefined, read: p => normalize(readFileSync(join(root,p),'utf8')) });
  let proof, facts;
  try { proof = override ?? JSON.parse(disk(MINIMAX_VIDEO_PHASE2_MANIFEST)); facts = readinessLifecycleParentFacts(root, MINIMAX_VIDEO_PHASE2_PARENT); }
  catch { fail('missing-evidence'); return rejected(); }
  if (proof.schemaVersion !== 1 || proof.checkpoint !== 'MINIMAX_VIDEO_PHASE_2'
    || proof.parentHead !== MINIMAX_VIDEO_PHASE2_PARENT || proof.planningParentHead !== MINIMAX_VIDEO_PHASE2_PARENT
    || JSON.stringify(proof.phaseCommitsBeforeCheckpoint) !== '[]') fail('header');
  const scope = [...MINIMAX_VIDEO_PHASE2_EXISTING,...MINIMAX_VIDEO_PHASE2_ADDED].sort();
  if (JSON.stringify(Object.keys(proof.paths ?? {}).sort()) !== JSON.stringify(scope)) fail('scope');
  if (Object.keys(proof.invariants ?? {}).length !== Object.keys(MINIMAX_VIDEO_PHASE2_INVARIANTS).length
    || Object.entries(MINIMAX_VIDEO_PHASE2_INVARIANTS).some(([k,v]) => proof.invariants?.[k] !== v)) fail('contract');
  if (proof.database?.migrationBefore !== 42 || proof.database?.migrationAfter !== 43 || proof.database?.newTables !== 3
    || proof.database?.backupBefore !== 20 || proof.database?.backupAfter !== 21) fail('database-contract');
  const runtimeProfile = process.env.AI_STUDIO_PHASE2_RUNTIME_BYTE_PROFILE ?? 'git';
  if (!Object.hasOwn(MINIMAX_VIDEO_PHASE2_RUNTIME_BYTES,runtimeProfile)
    || JSON.stringify(proof.runtimeByteBaselines) !== JSON.stringify(MINIMAX_VIDEO_PHASE2_RUNTIME_BYTES)) fail('runtime-byte-baseline');
  try {
    if (proof.phase1ProofHash !== readinessLifecycleHash(facts.read(MINIMAX_VIDEO_PHASE1_MANIFEST))
      || disk(MINIMAX_VIDEO_PHASE1_MANIFEST) !== facts.read(MINIMAX_VIDEO_PHASE1_MANIFEST)) fail('historical-proof');
    for (const p of MINIMAX_VIDEO_PHASE2_EXISTING)
      if (proof.paths[p]?.beforeHash !== readinessLifecycleHash(facts.read(p)) || proof.paths[p]?.afterHash !== readinessLifecycleHash(disk(p))) fail(`path:${p}`);
    for (const p of MINIMAX_VIDEO_PHASE2_ADDED)
      if (facts.paths.includes(p) || proof.paths[p]?.beforeHash !== null || proof.paths[p]?.afterHash !== readinessLifecycleHash(disk(p))) fail(`addition:${p}`);
    for (const [name,dir,pattern] of minimaxVideoPhase2Groups()) {
      const base = facts.paths.filter(p => p.startsWith(dir+'/') && pattern.test(p));
      const untouched = base.filter(p => !MINIMAX_VIDEO_PHASE2_EXISTING.includes(p));
      const current = files(root,dir,pattern).filter(p => p !== MINIMAX_VIDEO_PHASE2_MANIFEST && !successor.addedPaths.includes(p));
      const expected = [...base,...MINIMAX_VIDEO_PHASE2_ADDED.filter(p => p.startsWith(dir+'/') && pattern.test(p))].sort(), r = proof[name];
      if (JSON.stringify(current) !== JSON.stringify(expected) || r?.beforeFiles !== base.length || r?.afterFiles !== current.length) fail(`${name}-files`);
      // Visit every required file: early byte drift must not hide later missing evidence.
      let untouchedDrift = false;
      for (const p of untouched) {
        if (disk(p) !== facts.read(p)) untouchedDrift = true;
      }
      if (untouchedDrift) fail(`${name}-untouched-bytes`);
      // Runtime Package preservation is literal bytes, including newline-only drift.
      if (name === 'packages' && (
        JSON.stringify(files(root,dir,/./)) !== JSON.stringify(base)
        || rawAggregate(base,facts.raw) !== MINIMAX_VIDEO_PHASE2_RUNTIME_BYTES.git
        || rawAggregate(base,p=>readFileSync(join(root,p))) !== MINIMAX_VIDEO_PHASE2_RUNTIME_BYTES[runtimeProfile]
      )) fail('runtime-package-bytes');
      if (violations.length) continue;
      if (r?.beforeAggregateHash !== readinessLifecycleAggregate(base,facts.read)
        || r?.untouchedAggregateHash !== readinessLifecycleAggregate(untouched,facts.read)
        || r?.afterAggregateHash !== readinessLifecycleAggregate(current,disk)) fail(`${name}-aggregate`);
    }
    for (const p of configs) if (disk(p) !== facts.read(p)) fail(`frozen:${p}`);
  } catch { fail('missing-evidence'); return rejected(); }
  if (violations.length) return rejected();
  return { violations, addedPaths: [...MINIMAX_VIDEO_PHASE2_ADDED,MINIMAX_VIDEO_PHASE2_MANIFEST,...successor.addedPaths],
    afterHashes: { ...Object.fromEntries(scope.map(p => [p,proof.paths[p].afterHash])), ...successor.afterHashes },
    backendAggregateSha256: proof.backend.afterAggregateHash,
    // Only after validating live bytes do historical readers receive Phase1-final bytes.
    read: p => MINIMAX_VIDEO_PHASE2_EXISTING.includes(p) ? facts.read(p) : disk(p) };
}
export function minimaxVideoPhase2ParentReader(root, override) {
  return cachedBoundary(root,'minimaxVideoPhase2ParentReader',override,() => validate(root,override),MINIMAX_VIDEO_PHASE2_MANIFEST);
}
