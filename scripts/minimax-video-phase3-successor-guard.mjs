import { cachedBoundary } from './boundary-validation-cache.mjs';
import { readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';
import { READINESS_LIFECYCLE_GROUPS, readinessLifecycleParentFacts, readinessLifecycleHash, readinessLifecycleAggregate, MINIMAX_VIDEO_PHASE1_MANIFEST } from './readiness-lifecycle-successor-guard.mjs';

// Reviewed fix on the Phase 2 checkpoint. Do not repin minimax-video-phase2.json.
export const MINIMAX_VIDEO_PHASE3_PARENT = 'b7d7c5bd08b3cf0bdb67e702e74bbe2b3fae35fd';
export const MINIMAX_VIDEO_PHASE3_MANIFEST = 'docs/architecture/minimax-video-phase3.json';
export const MINIMAX_VIDEO_PHASE2_HISTORICAL_MANIFEST = 'docs/architecture/minimax-video-phase2.json';
export const MINIMAX_VIDEO_PHASE3_EXISTING = [
  'scripts/minimax-video-phase2-successor-guard.mjs',
  'src-tauri/src/application/ordered_reference_binding.rs',
  'src-tauri/src/application/product/video_inputs.rs',
  'src-tauri/src/application/production_orchestrator_service.rs',
  'src-tauri/src/application/production_queue_service.rs',
  'src-tauri/src/application/shot_batch_service.rs',
  'src-tauri/src/application/shot_readiness_evaluator.rs',
  'src-tauri/src/application/shot_video_input_service.rs',
  'src-tauri/src/commands/shot.rs',
  'src-tauri/src/infrastructure/database/dev048_consistency_e2e.rs',
  'src-tauri/tests/dev040_safety.rs',
  'src-tauri/tests/dev035_structure_e2e.rs',
  'src-tauri/tests/dev049_context_resolver.rs',
  'src-tauri/tests/dev050_readiness.rs',
  'src-tauri/tests/dev055_release_compatibility.rs',
  'src-tauri/tests/product_library_contract.rs',
  'src-tauri/tests/project_database_fixture.rs',
  'src-tauri/tests/dev100_authoring_removal.rs',
  'src/app/MiniMaxVideoPhase2Boundary.test.ts',
  'src/app/Phase7Retirement.test.tsx',
  'src/app/BackendBoundary.test.ts',
  'src/features/shots/ShotVideoInputsPanel.tsx',
  'src/product/errors.ts',
];
export const MINIMAX_VIDEO_PHASE3_ADDED = [
  'scripts/minimax-video-phase3-successor-guard.mjs',
  'src/app/MiniMaxVideoPhase3Boundary.test.ts',
];
export const MINIMAX_VIDEO_PHASE3_FILES = [...MINIMAX_VIDEO_PHASE3_EXISTING, ...MINIMAX_VIDEO_PHASE3_ADDED, MINIMAX_VIDEO_PHASE3_MANIFEST];
export const minimaxVideoPhase3FixtureFiles = root => [...readinessLifecycleParentFacts(root, MINIMAX_VIDEO_PHASE3_PARENT).paths, ...MINIMAX_VIDEO_PHASE3_FILES];
export const minimaxVideoPhase3Groups = () => [...READINESS_LIFECYCLE_GROUPS, ['domainDocs', 'docs/architecture', /\.md$/]];
export const MINIMAX_VIDEO_PHASE3_INVARIANTS = {
  schemaChanged: false, backupFormatChanged: false, queueAuthorityChanged: false,
  storeAuthorityChanged: false, uiV2Changed: false, historicalDataRewritten: false,
  immutablePackagesPreserved: true, phase2ProofPreserved: true, productGatesAligned: true,
};
export const MINIMAX_VIDEO_PHASE3_DATABASE = {
  migrationBefore: 43, migrationAfter: 43, newTables: 0, backupBefore: 21, backupAfter: 21,
};
const normalize = s => s.replaceAll('\r\n', '\n');
const files = (root, dir, pattern) => readdirSync(join(root, dir), { withFileTypes: true }).flatMap(e => {
  const p = `${dir}/${e.name}`; return e.isDirectory() ? files(root, p, pattern) : pattern.test(p) ? [p] : [];
}).sort();
const configs = ['package.json', 'pnpm-lock.yaml', 'src-tauri/Cargo.toml', 'src-tauri/Cargo.lock',
  'src-tauri/tauri.conf.json', 'src-tauri/build.rs', '.github/workflows/ci.yml', 'README.md',
  'docs/AI_STUDIO_2_1_CLOSEOUT.md', 'docs/RELEASE_NOTES_v2.1.0-personal.md'];

function validate(root, override) {
  const violations = [], fail = s => violations.push(`minimax-video-phase3-${s}`);
  const live = new Map(), disk = p => { if (!live.has(p)) live.set(p, normalize(readFileSync(join(root, p), 'utf8'))); return live.get(p); };
  const rejected = () => ({ violations, addedPaths: [], afterHashes: {}, backendAggregateSha256: undefined, read: disk });
  let proof, facts;
  try { proof = override ?? JSON.parse(disk(MINIMAX_VIDEO_PHASE3_MANIFEST)); facts = readinessLifecycleParentFacts(root, MINIMAX_VIDEO_PHASE3_PARENT); }
  catch { fail('missing-evidence'); return rejected(); }
  if (proof.schemaVersion !== 1 || proof.checkpoint !== 'MINIMAX_VIDEO_PHASE_3'
    || proof.parentHead !== MINIMAX_VIDEO_PHASE3_PARENT || proof.planningParentHead !== MINIMAX_VIDEO_PHASE3_PARENT
    || JSON.stringify(proof.phaseCommitsBeforeCheckpoint) !== '[]') fail('header');
  const scope = [...MINIMAX_VIDEO_PHASE3_EXISTING, ...MINIMAX_VIDEO_PHASE3_ADDED].sort();
  if (JSON.stringify(Object.keys(proof.paths ?? {}).sort()) !== JSON.stringify(scope)) fail('scope');
  if (Object.keys(proof.invariants ?? {}).length !== Object.keys(MINIMAX_VIDEO_PHASE3_INVARIANTS).length
    || Object.entries(MINIMAX_VIDEO_PHASE3_INVARIANTS).some(([k, v]) => proof.invariants?.[k] !== v)) fail('contract');
  const database = MINIMAX_VIDEO_PHASE3_DATABASE;
  if (Object.keys(proof.database ?? {}).length !== Object.keys(database).length
    || Object.entries(database).some(([k, v]) => proof.database?.[k] !== v)) fail('database-contract');
  if (violations.length) return rejected();
  try {
    if (disk(MINIMAX_VIDEO_PHASE1_MANIFEST) !== facts.read(MINIMAX_VIDEO_PHASE1_MANIFEST)
      || disk(MINIMAX_VIDEO_PHASE2_HISTORICAL_MANIFEST) !== facts.read(MINIMAX_VIDEO_PHASE2_HISTORICAL_MANIFEST)) fail('historical-proof');
    for (const p of MINIMAX_VIDEO_PHASE3_EXISTING)
      if (proof.paths[p]?.beforeHash !== readinessLifecycleHash(facts.read(p)) || proof.paths[p]?.afterHash !== readinessLifecycleHash(disk(p))) fail(`path:${p}`);
    for (const p of MINIMAX_VIDEO_PHASE3_ADDED)
      if (facts.paths.includes(p) || proof.paths[p]?.beforeHash !== null || proof.paths[p]?.afterHash !== readinessLifecycleHash(disk(p))) fail(`addition:${p}`);
    for (const [name, dir, pattern] of minimaxVideoPhase3Groups()) {
      const base = facts.paths.filter(p => p.startsWith(dir + '/') && pattern.test(p));
      const untouched = base.filter(p => !MINIMAX_VIDEO_PHASE3_EXISTING.includes(p));
      const current = files(root, dir, pattern).filter(p => p !== MINIMAX_VIDEO_PHASE3_MANIFEST);
      const expected = [...base, ...MINIMAX_VIDEO_PHASE3_ADDED.filter(p => p.startsWith(dir + '/') && pattern.test(p))].sort();
      const record = proof[name];
      if (JSON.stringify(current) !== JSON.stringify(expected) || record?.beforeFiles !== base.length || record?.afterFiles !== current.length) fail(`${name}-files`);
      if (untouched.some(p => disk(p) !== facts.read(p))) fail(`${name}-untouched-bytes`);
      if (violations.length) continue;
      if (record?.beforeAggregateHash !== readinessLifecycleAggregate(base, facts.read)
        || record?.untouchedAggregateHash !== readinessLifecycleAggregate(untouched, facts.read)
        || record?.afterAggregateHash !== readinessLifecycleAggregate(current, disk)) fail(`${name}-aggregate`);
    }
    for (const p of configs) if (disk(p) !== facts.read(p)) fail(`frozen:${p}`);
  } catch { fail('missing-evidence'); return rejected(); }
  if (violations.length) return rejected();
  return {
    violations, addedPaths: [...MINIMAX_VIDEO_PHASE3_ADDED, MINIMAX_VIDEO_PHASE3_MANIFEST],
    afterHashes: Object.fromEntries(scope.map(p => [p, proof.paths[p].afterHash])),
    backendAggregateSha256: proof.backend.afterAggregateHash,
    read: p => MINIMAX_VIDEO_PHASE3_EXISTING.includes(p) ? facts.read(p) : disk(p),
  };
}
export function minimaxVideoPhase3ParentReader(root, override) {
  return cachedBoundary(root, 'minimaxVideoPhase3ParentReader', override, () => validate(root, override), MINIMAX_VIDEO_PHASE3_MANIFEST);
}
