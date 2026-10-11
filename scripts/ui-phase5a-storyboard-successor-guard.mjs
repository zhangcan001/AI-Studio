// Validate Phase5A live bytes before replaying the immutable Phase4 checkpoint.
import { cachedBoundary, immutableGit } from './boundary-validation-cache.mjs';
import { createHash } from 'node:crypto';
import { readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';

export const PHASE5A_PARENT = 'a770c2090d3fde190d4dde775495164c1ecd5a05';
export const PHASE5A_MANIFEST = 'docs/architecture/ui-phase5a-storyboard-successor.json';
export const PHASE5A_EXISTING = [
  'scripts/boundary-validation-cache.mjs',
  'scripts/minimax-video-v2-ui-phase3-successor-guard.mjs',
  'scripts/ui-phase4-serial-queue-successor-guard.mjs',
  'src/app/UiPhase4SerialQueueBoundary.test.ts',
  'src/features/shots/ProjectProductionPipeline.tsx',
  'src/features/shots/ProjectWorkflowFormalProductionUat.test.tsx',
  'src/features/shots/EpisodeProductionPanel.tsx',
  'src/features/shots/EpisodeProductionPanel.test.tsx',
  'src/features/shots/SeriesProductionPanel.tsx',
  'src/features/shots/SeriesProductionPanel.test.tsx',
  'src/features/shots/ProjectStructureTree.tsx',
  'src/features/shots/ProjectStructureTree.test.tsx',
  'src/features/shots/SceneProductionPanel.tsx',
  'src/features/shots/SceneProductionPreparation.tsx',
  'src/features/shots/SceneProductionPreparation.test.tsx',
  'src/features/shots/ShotBulkConfigPanel.tsx',
  'src/features/shots/ShotCreationWorkspace.tsx',
  'src/features/shots/ShotInspector.tsx',
  'src/features/shots/ShotInspector.test.tsx',
  'src/features/shots/ShotWorkspace.tsx',
  'vite.config.ts',
].sort();
export const PHASE5A_ADDED = [
  'scripts/ui-phase5a-storyboard-successor-guard.mjs',
  'src/app/UiPhase5AStoryboardBoundary.test.ts',
  'src/features/shots/ShotBulkConfigPanel.phase5.test.tsx',
].sort();
export const PHASE5A_FILES = [...PHASE5A_EXISTING, ...PHASE5A_ADDED, PHASE5A_MANIFEST];
export const PHASE5A_INVARIANTS = {
  schemaChanged: false, backupFormatChanged: false, backendChanged: false,
  queueAuthorityChanged: false, storeAuthorityChanged: false,
  runtimePackagesChanged: false, historicalProofsRewritten: false,
  typedIpcPreserved: true, exactRecipeInputIdentity: true,
  freshPreflightAndFrozenAdmission: true, preparationStartsGeneration: false,
  imageGenerationReopened: false, automaticHumanApproval: false,
  videoFirstStoryboard: true, unassignedShotsPreserved: true,
  phase5AStarted: true, phase5BStarted: false, phase6Started: false,
};
export const PHASE5A_DATABASE = { migrationBefore: 44, migrationAfter: 44, newTables: 0, backupBefore: 21, backupAfter: 21 };
export const PHASE5A_GROUPS = [
  ['backend', 'src-tauri/src', /\.rs$/], ['tests', 'src-tauri/tests', /\.rs$/],
  ['frontend', 'src', /\.tsx?$/], ['styles', 'src', /\.css$/],
  ['scripts', 'scripts', /\.(mjs|ts)$/], ['migrations', 'src-tauri/migrations', /\.sql$/],
  ['architecture', 'docs/architecture', /\.json$/], ['packages', 'src-tauri/runtime_packages', /\.(yaml|json)$/],
  ['domainDocs', 'docs/architecture', /\.md$/],
];
const directories = [...new Set(PHASE5A_GROUPS.map(([, dir]) => dir))];
const configs = ['CONTEXT.md', 'README.md', 'package.json', 'pnpm-lock.yaml', 'vite.config.ts',
  'src-tauri/Cargo.toml', 'src-tauri/Cargo.lock', 'src-tauri/tauri.conf.json', 'src-tauri/build.rs',
  '.github/workflows/ci.yml', 'docs/AI_STUDIO_2_1_CLOSEOUT.md', 'docs/RELEASE_NOTES_v2.1.0-personal.md'];
const normalize = text => text.replaceAll('\r\n', '\n');
export const phase5AHash = text => createHash('sha256').update(normalize(text)).digest('hex');
const rawHash = bytes => createHash('sha256').update(bytes).digest('hex');
export const phase5AAggregate = (paths, read) => phase5AHash(paths.map(p => `${p}\n${read(p)}`).join('\n'));
const parents = new Map();
export function phase5AParentFacts(root) {
  if (parents.has(root)) return parents.get(root);
  const paths = immutableGit('git', ['ls-tree', '-r', '--name-only', PHASE5A_PARENT], { cwd: root, encoding: 'utf8' })
    .trim().split(/\r?\n/).filter(p => configs.includes(p) || directories.some(dir => p.startsWith(dir + '/'))).sort();
  const bytes = immutableGit('git', ['cat-file', '--batch'], { cwd: root,
    input: paths.map(p => `${PHASE5A_PARENT}:${p}\n`).join(''), maxBuffer: 64 * 1024 * 1024 });
  const blobs = new Map(); let offset = 0;
  for (const p of paths) {
    const end = bytes.indexOf(10, offset), match = /^[a-f0-9]+ blob (\d+)$/.exec(bytes.subarray(offset, end).toString());
    if (!match) throw Error('Phase5A immutable parent missing');
    const start = end + 1, length = Number(match[1]);
    if (bytes[start + length] !== 10) throw Error('Phase5A immutable blob boundary');
    blobs.set(p, bytes.subarray(start, start + length)); offset = start + length + 1;
  }
  if (offset !== bytes.length) throw Error('Phase5A immutable trailing bytes');
  const facts = { paths, read: p => normalize(blobs.get(p).toString('utf8')), raw: p => blobs.get(p) };
  parents.set(root, facts); return facts;
}
function files(root, dir) {
  return readdirSync(join(root, dir), { withFileTypes: true }).flatMap(entry => {
    const p = `${dir}/${entry.name}`;
    if (entry.isSymbolicLink()) throw Error('Phase5A symbolic source');
    return entry.isDirectory() ? files(root, p) : [p];
  }).sort();
}
export const phase5AFixtureFiles = root => [...new Set([...phase5AParentFacts(root).paths, ...PHASE5A_FILES])];
function validate(root, override) {
  const violations = [], fail = name => violations.push('ui-phase5a-storyboard-' + name);
  const live = new Map(), read = p => {
    if (!live.has(p)) live.set(p, normalize(readFileSync(join(root, p), 'utf8')));
    return live.get(p);
  };
  const denied = () => ({ violations, addedPaths: [], afterHashes: {}, read });
  let proof, parent;
  try { proof = override ?? JSON.parse(read(PHASE5A_MANIFEST)); parent = phase5AParentFacts(root); }
  catch { fail('missing-evidence'); return denied(); }
  const scope = [...PHASE5A_EXISTING, ...PHASE5A_ADDED].sort();
  const proofKeys = ['schemaVersion', 'checkpoint', 'parentHead', 'planningParentHead',
    'phaseCommitsBeforeCheckpoint', 'invariants', 'database', 'paths', ...PHASE5A_GROUPS.map(([name]) => name)].sort();
  if (JSON.stringify(Object.keys(proof).sort()) !== JSON.stringify(proofKeys)
      || read(PHASE5A_MANIFEST) !== JSON.stringify(JSON.parse(read(PHASE5A_MANIFEST)), null, 2) + '\n') fail('proof-bytes');
  if (proof.schemaVersion !== 1 || proof.checkpoint !== 'UI_PHASE5A_STORYBOARD_SUCCESSOR'
      || proof.parentHead !== PHASE5A_PARENT || proof.planningParentHead !== PHASE5A_PARENT
      || JSON.stringify(proof.phaseCommitsBeforeCheckpoint) !== '[]') fail('header');
  if (JSON.stringify(Object.keys(proof.paths ?? {}).sort()) !== JSON.stringify(scope)) fail('scope');
  if (JSON.stringify(proof.invariants) !== JSON.stringify(PHASE5A_INVARIANTS)
      || JSON.stringify(proof.database) !== JSON.stringify(PHASE5A_DATABASE)) fail('contract');
  if (violations.length) return denied();
  try {
    for (const p of PHASE5A_EXISTING) {
      if (!parent.paths.includes(p) || proof.paths[p]?.beforeHash !== phase5AHash(parent.read(p))
          || proof.paths[p]?.afterHash !== phase5AHash(read(p))) fail('path:' + p);
    }
    for (const p of PHASE5A_ADDED) {
      if (parent.paths.includes(p) || proof.paths[p]?.beforeHash !== null
          || proof.paths[p]?.afterHash !== phase5AHash(read(p))) fail('addition:' + p);
    }
    for (const dir of directories) {
      const expected = [...parent.paths.filter(p => p.startsWith(dir + '/')),
        ...PHASE5A_ADDED.filter(p => p.startsWith(dir + '/')),
        ...[PHASE5A_MANIFEST].filter(p => p.startsWith(dir + '/'))].sort();
      if (JSON.stringify(files(root, dir)) !== JSON.stringify(expected)) fail('inventory:' + dir);
    }
    // Every untouched file is protected, including historical proof JSON and
    // uncommon file extensions. No broad new-source allowance is authorized.
    for (const p of parent.paths.filter(p => !PHASE5A_EXISTING.includes(p))) {
      if (read(p) !== parent.read(p)) fail('untouched:' + p);
      if (p.startsWith('src-tauri/migrations/') && rawHash(readFileSync(join(root, p))) !== rawHash(parent.raw(p))) fail('migration-bytes:' + p);
    }
    for (const [name, dir, pattern] of PHASE5A_GROUPS) {
      const base = parent.paths.filter(p => p.startsWith(dir + '/') && pattern.test(p));
      const current = [...base, ...PHASE5A_ADDED.filter(p => p.startsWith(dir + '/') && pattern.test(p))].sort();
      const untouched = base.filter(p => !PHASE5A_EXISTING.includes(p)), record = proof[name];
      if (record?.beforeFiles !== base.length || record?.afterFiles !== current.length
          || record?.beforeAggregateHash !== phase5AAggregate(base, parent.read)
          || record?.untouchedAggregateHash !== phase5AAggregate(untouched, parent.read)
          || record?.afterAggregateHash !== phase5AAggregate(current, read)) fail('aggregate:' + name);
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
  return { violations, addedPaths: [...PHASE5A_ADDED, PHASE5A_MANIFEST],
    afterHashes: Object.fromEntries(scope.map(p => [p, proof.paths[p].afterHash])),
    read: p => PHASE5A_EXISTING.includes(p) ? parent.read(p) : read(p) };
}
export function uiPhase5AParentReader(root, override) {
  return cachedBoundary(root, 'uiPhase5AParentReader', override, () => validate(root, override), PHASE5A_MANIFEST);
}
