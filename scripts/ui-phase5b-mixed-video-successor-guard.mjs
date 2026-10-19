// Validate bounded mixed-mode/hierarchy Phase5B before replaying frozen CI/Phase5A bytes.
import { uiPhase5CParentReader, phase5CFixtureFiles } from './ui-phase5c-review-successor-guard.mjs';
import { cachedBoundary, immutableGit, immutableGitStoreIdentity } from './boundary-validation-cache.mjs';
import { createHash } from 'node:crypto';
import { readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';

export const PHASE5B_PARENT = '0f288edf207873bcdbada0a67bc5dbdf4a24b2ce';
export const PHASE5B_MANIFEST = 'docs/architecture/ui-phase5b-mixed-video-successor.json';
export const PHASE5B_EXISTING = ["src/features/shots/ShotBulkConfigPanel.tsx", "src/features/shots/ShotWorkspace.tsx", "src/features/shots/SeriesProductionPanel.tsx", "src/features/shots/EpisodeProductionPanel.tsx", "src/features/shots/SceneProductionPreparation.tsx", "src/features/shots/SceneProductionPreparation.test.tsx", "src/features/studio/ProductionQueuePanel.tsx", "src/types/seriesProduction.ts", "src/types/episodeProduction.ts", "src/types/productionPreparation.ts", "src-tauri/src/application/production_preparation_service.rs", "src-tauri/src/commands/production_preparation.rs", "src-tauri/src/domain/production_preparation.rs", "src-tauri/tests/support/minimax_video_input_regressions.rs", "src-tauri/tests/support/minimax_video_policy_regressions.rs", "src-tauri/tests/dev061b_queue_recovery.rs", "src-tauri/tests/dev052_production_preparation.rs", "scripts/ui-phase5-ci-repair-successor-guard.mjs"].sort();
export const PHASE5B_ADDED = ["src/features/shots/videoPreparationScope.ts", "src/features/shots/ShotBulkConfigPanel.phase5b.test.tsx", "src/features/studio/ProductionQueuePanel.phase5b.test.tsx", "scripts/ui-phase5b-mixed-video-successor-guard.mjs", "src/app/UiPhase5BMixedVideoBoundary.test.ts"].sort();
export const PHASE5B_FILES = [...PHASE5B_EXISTING, ...PHASE5B_ADDED, PHASE5B_MANIFEST];
export const PHASE5B_INVARIANTS = { schemaChanged: false, backupFormatChanged: false, backendChanged: true, queueAuthorityChanged: false, storeAuthorityChanged: false, runtimePackagesChanged: false, historicalProofsRewritten: false, hierarchyScopeRevalidated: true, mixedFourModeFrozenInputs: true, freshPreflightPreserved: true, explicitQueueStart: true, nonPreemptivePause: true, localFullGatePolicy: true, nativeGpuExecuted: false, phase5BStarted: true, phase5CStarted: false, phase6Started: false };
export const PHASE5B_DATABASE = { migrationBefore: 44, migrationAfter: 44, newTables: 0, backupBefore: 21, backupAfter: 21 };
export const PHASE5B_GROUPS = [
  ['backend', 'src-tauri/src', /\.rs$/], ['tests', 'src-tauri/tests', /\.rs$/],
  ['frontend', 'src', /\.tsx?$/], ['styles', 'src', /\.css$/],
  ['scripts', 'scripts', /\.(mjs|ts)$/], ['migrations', 'src-tauri/migrations', /\.sql$/],
  ['architecture', 'docs/architecture', /\.json$/], ['packages', 'src-tauri/runtime_packages', /\.(yaml|json)$/],
  ['domainDocs', 'docs/architecture', /\.md$/],
];
const directories = [...new Set([...PHASE5B_GROUPS.map(([, dir]) => dir), 'src-tauri/examples', 'docs', '.github'])];
const configs = ['CONTEXT.md', 'README.md', 'package.json', 'pnpm-lock.yaml', 'vite.config.ts',
  'src-tauri/Cargo.toml', 'src-tauri/Cargo.lock', 'src-tauri/tauri.conf.json', 'src-tauri/build.rs',
  '.github/workflows/ci.yml', 'docs/AI_STUDIO_2_1_CLOSEOUT.md', 'docs/RELEASE_NOTES_v2.1.0-personal.md'];
const normalize = text => text.replaceAll('\r\n', '\n');
export const phase5BHash = text => createHash('sha256').update(normalize(text)).digest('hex');
const rawHash = bytes => createHash('sha256').update(bytes).digest('hex');
export const phase5BAggregate = (paths, read) => phase5BHash(paths.map(p => `${p}\n${read(p)}`).join('\n'));
// Immutable parent aggregates are pure functions of verified parent blobs.
const parentAggregates=new WeakMap();
const parentAggregate=(paths,parent)=>{let memo=parentAggregates.get(parent);if(!memo)parentAggregates.set(parent,memo=new Map());const key=paths.join('\n');if(!memo.has(key))memo.set(key,phase5BAggregate(paths,parent.read));return memo.get(key);};
const parents = new Map();
export function phase5BParentFacts(root) {
  // Immutable parent facts depend only on the verified object store, not the
  // fixture root; owned fixtures sharing one store share one parsed snapshot.
  const store = immutableGitStoreIdentity(root);
  if (parents.has(store)) return parents.get(store);
  const paths = immutableGit('git', ['ls-tree', '-r', '--name-only', PHASE5B_PARENT], { cwd: root, encoding: 'utf8' })
    .trim().split(/\r?\n/).filter(p => configs.includes(p) || directories.some(dir => p.startsWith(dir + '/'))).sort();
  const bytes = immutableGit('git', ['cat-file', '--batch'], { cwd: root,
    input: paths.map(p => `${PHASE5B_PARENT}:${p}\n`).join(''), maxBuffer: 64 * 1024 * 1024 });
  const blobs = new Map(); let offset = 0;
  for (const p of paths) {
    const end = bytes.indexOf(10, offset), match = /^[a-f0-9]+ blob (\d+)$/.exec(bytes.subarray(offset, end).toString());
    if (!match) throw Error('Phase5B immutable parent missing');
    const start = end + 1, length = Number(match[1]);
    if (bytes[start + length] !== 10) throw Error('Phase5B immutable blob boundary');
    blobs.set(p, bytes.subarray(start, start + length)); offset = start + length + 1;
  }
  if (offset !== bytes.length) throw Error('Phase5B immutable trailing bytes');
  const facts = { paths, read: (texts => p => { if (!texts.has(p)) texts.set(p, normalize(blobs.get(p).toString('utf8'))); return texts.get(p); })(new Map()), raw: p => blobs.get(p) };
  parents.set(store, facts); return facts;
}
function files(root, dir) {
  return readdirSync(join(root, dir), { withFileTypes: true }).flatMap(entry => {
    const p = `${dir}/${entry.name}`;
    if (entry.isSymbolicLink()) throw Error('Phase5B symbolic source');
    return entry.isDirectory() ? files(root, p) : [p];
  }).sort();
}
export const phase5BFixtureFiles = root => [...new Set([...phase5BParentFacts(root).paths, ...PHASE5B_FILES, ...phase5CFixtureFiles(root)])];
function validate(root, override) {
  const successor = uiPhase5CParentReader(root);
  const violations = [...successor.violations], fail = name => violations.push('ui-phase5b-mixed-video-' + name);
  // Preserve the predecessor's inventory diagnostic without accepting a denied
  // child projection or changing any frozen proof/negative-test contract.
  for (const violation of successor.violations) {
    if (violation.startsWith('ui-phase5c-review-inventory:'))
      fail(violation.slice('ui-phase5c-review-'.length));
  }
  const live = new Map(), read = p => {
    if (!live.has(p)) live.set(p, normalize(successor.read(p)));
    return live.get(p);
  };
  const denied = () => ({ violations, addedPaths: [], afterHashes: {}, read });
  let proof, parent;
  try { proof = override ?? JSON.parse(read(PHASE5B_MANIFEST)); parent = phase5BParentFacts(root); }
  catch { fail('missing-evidence'); return denied(); }
  const scope = [...PHASE5B_EXISTING, ...PHASE5B_ADDED].sort();
  const proofKeys = ['schemaVersion', 'checkpoint', 'parentHead', 'planningParentHead',
    'phaseCommitsBeforeCheckpoint', 'invariants', 'database', 'paths', ...PHASE5B_GROUPS.map(([name]) => name)].sort();
  if (JSON.stringify(Object.keys(proof).sort()) !== JSON.stringify(proofKeys)
      || read(PHASE5B_MANIFEST) !== JSON.stringify(JSON.parse(read(PHASE5B_MANIFEST)), null, 2) + '\n') fail('proof-bytes');
  if (proof.schemaVersion !== 1 || proof.checkpoint !== 'UI_PHASE5B_MIXED_VIDEO_SUCCESSOR'
      || proof.parentHead !== PHASE5B_PARENT || proof.planningParentHead !== PHASE5B_PARENT
      || JSON.stringify(proof.phaseCommitsBeforeCheckpoint) !== '[]') fail('header');
  if (JSON.stringify(Object.keys(proof.paths ?? {}).sort()) !== JSON.stringify(scope)) fail('scope');
  if (JSON.stringify(proof.invariants) !== JSON.stringify(PHASE5B_INVARIANTS)
      || JSON.stringify(proof.database) !== JSON.stringify(PHASE5B_DATABASE)) fail('contract');
  if (violations.length) return denied();
  try {
    for (const p of PHASE5B_EXISTING) {
      if (!parent.paths.includes(p) || proof.paths[p]?.beforeHash !== phase5BHash(parent.read(p))
          || proof.paths[p]?.afterHash !== phase5BHash(read(p))) fail('path:' + p);
    }
    for (const p of PHASE5B_ADDED) {
      if (parent.paths.includes(p) || proof.paths[p]?.beforeHash !== null
          || proof.paths[p]?.afterHash !== phase5BHash(read(p))) fail('addition:' + p);
    }
    for (const dir of directories) {
      const expected = [...parent.paths.filter(p => p.startsWith(dir + '/')),
        ...PHASE5B_ADDED.filter(p => p.startsWith(dir + '/')),
        ...successor.addedPaths.filter(p => p.startsWith(dir + '/')),
        ...[PHASE5B_MANIFEST].filter(p => p.startsWith(dir + '/'))].sort();
      if (JSON.stringify(files(root, dir)) !== JSON.stringify(expected)) fail('inventory:' + dir);
    }
    // Every untouched file is protected, including historical proof JSON and
    // uncommon file extensions. No broad new-source allowance is authorized.
    for (const p of parent.paths.filter(p => !PHASE5B_EXISTING.includes(p))) {
      if (read(p) !== parent.read(p)) fail('untouched:' + p);
      if (p.startsWith('src-tauri/migrations/') && rawHash(readFileSync(join(root, p))) !== rawHash(parent.raw(p))) fail('migration-bytes:' + p);
    }
    for (const [name, dir, pattern] of PHASE5B_GROUPS) {
      const base = parent.paths.filter(p => p.startsWith(dir + '/') && pattern.test(p));
      const current = [...base, ...PHASE5B_ADDED.filter(p => p.startsWith(dir + '/') && pattern.test(p))].sort();
      const untouched = base.filter(p => !PHASE5B_EXISTING.includes(p)), record = proof[name];
      if (record?.beforeFiles !== base.length || record?.afterFiles !== current.length
          || record?.beforeAggregateHash !== parentAggregate(base,parent)
          || record?.untouchedAggregateHash !== parentAggregate(untouched,parent)
          || record?.afterAggregateHash !== phase5BAggregate(current, read)) fail('aggregate:' + name);
    }
    // Use the existing explicit Git/Windows raw-byte baselines, never infer
    // acceptance from current bytes and never normalize Runtime Packages.
    const baselines = JSON.parse(parent.read('docs/architecture/minimax-video-phase2.json')).runtimeByteBaselines;
    const profile = process.env.AI_STUDIO_PHASE2_RUNTIME_BYTE_PROFILE ?? 'git';
    const packages = parent.paths.filter(p => p.startsWith('src-tauri/runtime_packages/'));
    const rawAggregate = readBytes => rawHash(JSON.stringify(packages.map(p => [p, rawHash(readBytes(p))])));
    if (!Object.hasOwn(baselines, profile) || rawAggregate(parent.raw) !== baselines.git
        || rawAggregate(p => readFileSync(join(root, p))) !== baselines[profile]) fail('runtime-package-bytes');
  } catch { fail('missing-evidence'); }
  if (violations.length) return denied();
  return { violations, addedPaths: [...PHASE5B_ADDED, PHASE5B_MANIFEST, ...successor.addedPaths],
    afterHashes: { ...Object.fromEntries(scope.map(p => [p, proof.paths[p].afterHash])), ...successor.afterHashes },
    read: p => PHASE5B_EXISTING.includes(p) ? parent.read(p) : read(p) };
}
export function uiPhase5BParentReader(root, override) {
  return cachedBoundary(root, 'uiPhase5BParentReader', override, () => validate(root, override), PHASE5B_MANIFEST);
}
