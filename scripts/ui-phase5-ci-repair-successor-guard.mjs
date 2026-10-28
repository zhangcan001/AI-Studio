// Validate the CI-only immutable-blob optimization before replaying frozen Phase5A.
import { cachedBoundary, immutableGit, immutableGitStoreIdentity } from './boundary-validation-cache.mjs';
import { uiPhase5BParentReader, phase5BFixtureFiles } from './ui-phase5b-mixed-video-successor-guard.mjs';
import { createHash } from 'node:crypto';
import { readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';

export const PHASE5_CI_PARENT = 'ad8db6e20bf6f84b81ce9bc0c788f32669dc552a';
export const PHASE5_CI_MANIFEST = 'docs/architecture/ui-phase5-ci-repair-successor.json';
export const PHASE5_CI_EXISTING = ['.github/workflows/ci.yml', 'scripts/boundary-validation-cache.mjs', 'scripts/ui-phase5a-storyboard-successor-guard.mjs'].sort();
export const PHASE5_CI_ADDED = ['scripts/ui-phase5-ci-repair-successor-guard.mjs', 'scripts/testing/immutable-git-cache.test.ts', 'src/app/UiPhase5CiRepairBoundary.test.ts'].sort();
export const PHASE5_CI_FILES = [...PHASE5_CI_EXISTING, ...PHASE5_CI_ADDED, PHASE5_CI_MANIFEST];
export const PHASE5_CI_INVARIANTS = { schemaChanged: false, backupFormatChanged: false, backendChanged: false, queueAuthorityChanged: false, storeAuthorityChanged: false, runtimePackagesChanged: false, historicalProofsRewritten: false, freshLiveIdentityPreserved: true, gitBlobIdentityVerified: true, negativeProbesPreserved: true, globalTestTimeoutRaised: false, frontendJobBudgetMinutes: 20, phase5BProductStarted: false, phase5CStarted: false, phase6Started: false };
export const PHASE5_CI_DATABASE = { migrationBefore: 44, migrationAfter: 44, newTables: 0, backupBefore: 21, backupAfter: 21 };
export const PHASE5_CI_GROUPS = [
  ['backend', 'src-tauri/src', /\.rs$/], ['tests', 'src-tauri/tests', /\.rs$/],
  ['frontend', 'src', /\.tsx?$/], ['styles', 'src', /\.css$/],
  ['scripts', 'scripts', /\.(mjs|ts)$/], ['migrations', 'src-tauri/migrations', /\.sql$/],
  ['architecture', 'docs/architecture', /\.json$/], ['packages', 'src-tauri/runtime_packages', /\.(yaml|json)$/],
  ['domainDocs', 'docs/architecture', /\.md$/],
];
const directories = [...new Set([...PHASE5_CI_GROUPS.map(([, dir]) => dir), 'src-tauri/examples', 'docs', '.github'])];
const configs = ['CONTEXT.md', 'README.md', 'package.json', 'pnpm-lock.yaml', 'vite.config.ts',
  'src-tauri/Cargo.toml', 'src-tauri/Cargo.lock', 'src-tauri/tauri.conf.json', 'src-tauri/build.rs',
  '.github/workflows/ci.yml', 'docs/AI_STUDIO_2_1_CLOSEOUT.md', 'docs/RELEASE_NOTES_v2.1.0-personal.md'];
const normalize = text => text.replaceAll('\r\n', '\n');
export const phase5CiHash = text => createHash('sha256').update(normalize(text)).digest('hex');
const rawHash = bytes => createHash('sha256').update(bytes).digest('hex');
export const phase5CiAggregate = (paths, read) => phase5CiHash(paths.map(p => `${p}\n${read(p)}`).join('\n'));
// Immutable parent aggregates are pure functions of verified parent blobs.
const parentAggregates=new WeakMap();
const parentAggregate=(paths,parent)=>{let memo=parentAggregates.get(parent);if(!memo)parentAggregates.set(parent,memo=new Map());const key=paths.join('\n');if(!memo.has(key))memo.set(key,phase5CiAggregate(paths,parent.read));return memo.get(key);};
const parents = new Map();
export function phase5CiParentFacts(root) {
  // Immutable parent facts depend only on the verified object store, not the
  // fixture root; owned fixtures sharing one store share one parsed snapshot.
  const store = immutableGitStoreIdentity(root);
  if (parents.has(store)) return parents.get(store);
  const paths = immutableGit('git', ['ls-tree', '-r', '--name-only', PHASE5_CI_PARENT], { cwd: root, encoding: 'utf8' })
    .trim().split(/\r?\n/).filter(p => configs.includes(p) || directories.some(dir => p.startsWith(dir + '/'))).sort();
  const bytes = immutableGit('git', ['cat-file', '--batch'], { cwd: root,
    input: paths.map(p => `${PHASE5_CI_PARENT}:${p}\n`).join(''), maxBuffer: 64 * 1024 * 1024 });
  const blobs = new Map(); let offset = 0;
  for (const p of paths) {
    const end = bytes.indexOf(10, offset), match = /^[a-f0-9]+ blob (\d+)$/.exec(bytes.subarray(offset, end).toString());
    if (!match) throw Error('Phase5Ci immutable parent missing');
    const start = end + 1, length = Number(match[1]);
    if (bytes[start + length] !== 10) throw Error('Phase5Ci immutable blob boundary');
    blobs.set(p, Buffer.from(bytes.subarray(start, start + length))); offset = start + length + 1;
  }
  if (offset !== bytes.length) throw Error('Phase5Ci immutable trailing bytes');
  const facts = { paths, read: (texts => p => { if (!texts.has(p)) texts.set(p, normalize(blobs.get(p).toString('utf8'))); return texts.get(p); })(new Map()), raw: p => blobs.get(p) };
  parents.set(store, facts); return facts;
}
function files(root, dir) {
  return readdirSync(join(root, dir), { withFileTypes: true }).flatMap(entry => {
    const p = `${dir}/${entry.name}`;
    if (entry.isSymbolicLink()) throw Error('Phase5Ci symbolic source');
    return entry.isDirectory() ? files(root, p) : [p];
  }).sort();
}
export const phase5CiFixtureFiles = root => [...new Set([...phase5CiParentFacts(root).paths, ...PHASE5_CI_FILES, ...phase5BFixtureFiles(root)])];
function validate(root, override) {
  const successor = uiPhase5BParentReader(root);
  const violations = [...successor.violations], fail = name => violations.push('ui-phase5-ci-repair-' + name);
  const live = new Map(), read = p => {
    if (!live.has(p)) live.set(p, normalize(successor.read(p)));
    return live.get(p);
  };
  const denied = () => ({ violations, addedPaths: [], afterHashes: {}, read });
  let proof, parent;
  try { proof = override ?? JSON.parse(read(PHASE5_CI_MANIFEST)); parent = phase5CiParentFacts(root); }
  catch { fail('missing-evidence'); return denied(); }
  const scope = [...PHASE5_CI_EXISTING, ...PHASE5_CI_ADDED].sort();
  const proofKeys = ['schemaVersion', 'checkpoint', 'parentHead', 'planningParentHead',
    'phaseCommitsBeforeCheckpoint', 'invariants', 'database', 'paths', ...PHASE5_CI_GROUPS.map(([name]) => name)].sort();
  if (JSON.stringify(Object.keys(proof).sort()) !== JSON.stringify(proofKeys)
      || read(PHASE5_CI_MANIFEST) !== JSON.stringify(JSON.parse(read(PHASE5_CI_MANIFEST)), null, 2) + '\n') fail('proof-bytes');
  if (proof.schemaVersion !== 1 || proof.checkpoint !== 'UI_PHASE5_CI_REPAIR_SUCCESSOR'
      || proof.parentHead !== PHASE5_CI_PARENT || proof.planningParentHead !== PHASE5_CI_PARENT
      || JSON.stringify(proof.phaseCommitsBeforeCheckpoint) !== '[]') fail('header');
  if (JSON.stringify(Object.keys(proof.paths ?? {}).sort()) !== JSON.stringify(scope)) fail('scope');
  if (JSON.stringify(proof.invariants) !== JSON.stringify(PHASE5_CI_INVARIANTS)
      || JSON.stringify(proof.database) !== JSON.stringify(PHASE5_CI_DATABASE)) fail('contract');
  if (violations.length > successor.violations.length) return denied();
  try {
    for (const p of PHASE5_CI_EXISTING) {
      if (!parent.paths.includes(p) || proof.paths[p]?.beforeHash !== phase5CiHash(parent.read(p))
          || proof.paths[p]?.afterHash !== phase5CiHash(read(p))) fail('path:' + p);
    }
    for (const p of PHASE5_CI_ADDED) {
      if (parent.paths.includes(p) || proof.paths[p]?.beforeHash !== null
          || proof.paths[p]?.afterHash !== phase5CiHash(read(p))) fail('addition:' + p);
    }
    for (const dir of directories) {
      const expected = [...parent.paths.filter(p => p.startsWith(dir + '/')),
        ...PHASE5_CI_ADDED.filter(p => p.startsWith(dir + '/')),
        ...[PHASE5_CI_MANIFEST].filter(p => p.startsWith(dir + '/')),
        ...successor.addedPaths.filter(p => p.startsWith(dir + '/'))].sort();
      if (JSON.stringify(files(root, dir)) !== JSON.stringify(expected)) fail('inventory:' + dir);
    }
    // Every untouched file is protected, including historical proof JSON and
    // uncommon file extensions. No broad new-source allowance is authorized.
    for (const p of parent.paths.filter(p => !PHASE5_CI_EXISTING.includes(p))) {
      if (read(p) !== parent.read(p)) fail('untouched:' + p);
      if (p.startsWith('src-tauri/migrations/') && rawHash(readFileSync(join(root, p))) !== rawHash(parent.raw(p))) fail('migration-bytes:' + p);
    }
    for (const [name, dir, pattern] of PHASE5_CI_GROUPS) {
      const base = parent.paths.filter(p => p.startsWith(dir + '/') && pattern.test(p));
      const current = [...base, ...PHASE5_CI_ADDED.filter(p => p.startsWith(dir + '/') && pattern.test(p))].sort();
      const untouched = base.filter(p => !PHASE5_CI_EXISTING.includes(p)), record = proof[name];
      if (record?.beforeFiles !== base.length || record?.afterFiles !== current.length
          || record?.beforeAggregateHash !== parentAggregate(base,parent)
          || record?.untouchedAggregateHash !== parentAggregate(untouched,parent)
          || record?.afterAggregateHash !== phase5CiAggregate(current, read)) fail('aggregate:' + name);
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
  return { violations, addedPaths: [...PHASE5_CI_ADDED, PHASE5_CI_MANIFEST, ...successor.addedPaths],
    afterHashes: { ...Object.fromEntries(scope.map(p => [p, proof.paths[p].afterHash])), ...successor.afterHashes },
    read: p => PHASE5_CI_EXISTING.includes(p) ? parent.read(p) : read(p) };
}
export function uiPhase5CiParentReader(root, override) {
  return cachedBoundary(root, 'uiPhase5CiParentReader', override, () => validate(root, override), PHASE5_CI_MANIFEST);
}
