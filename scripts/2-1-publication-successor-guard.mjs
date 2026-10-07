import { cachedBoundary, immutableGit as execFileSync } from './boundary-validation-cache.mjs';
// Validate published evidence before projecting immutable pre-publication history.
import { createHash } from 'node:crypto';
import { existsSync, readFileSync, readdirSync, statSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { h3RepairParentReader } from './h3-generation-repair-successor-guard.mjs';

export const PUBLICATION_PARENT = '130cbbfaf67e1627fca7e447de322cf0f5f5ae85';
export const RELEASE_TAG_HEAD = '5af3f20273e722466c82b91ede4970cd83e0bcb8';
const releaseTag = 'v2.1.0-personal';
const installerSource = '7739d314a19719e43e74fe6a2daec7d0b10097c6';
const installerSha = '857D7A04A1184C4F86D15FF1258B11CEEDADDDD664F97C8858AD044256B4CD32';
const docs = ['README.md', 'docs/AI_STUDIO_2_1_CLOSEOUT.md', 'docs/RELEASE_NOTES_v2.1.0-personal.md'].sort();
const integration = 'scripts/2-1-rc-backup-asset-version-repair-guard.mjs';
const existing = [...docs, integration].sort();
const added = ['scripts/2-1-publication-successor-guard.mjs', 'src/app/AIStudio21PublicationBoundary.test.ts'].sort();
const manifest = 'docs/architecture/2-1-publication-successor.json';
const groups = [
  ['backend', 'src-tauri/src', /\.rs$/],
  ['tests', 'src-tauri/tests', /\.rs$/],
  ['frontend', 'src', /\.tsx?$/],
  ['styles', 'src', /\.css$/],
  ['scripts', 'scripts', /\.(?:mjs|ts)$/],
  ['migrations', 'src-tauri/migrations', /\.sql$/],
  ['architecture', 'docs/architecture', /\.json$/],
];
const configs = ['package.json', 'pnpm-lock.yaml', 'src-tauri/Cargo.toml', 'src-tauri/Cargo.lock',
  'src-tauri/tauri.conf.json', 'src-tauri/build.rs', '.github/workflows/ci.yml'];
const normalize = s => s.replaceAll('\r\n', '\n');
const hash = s => createHash('sha256').update(normalize(s)).digest('hex');
const aggregate = (paths, read) => hash(paths.map(p => `${p}\n${read(p)}`).join('\n'));
const files = (root, dir, pattern) => readdirSync(join(root, dir), {withFileTypes: true}).flatMap(e => {
  const p = `${dir}/${e.name}`;
  return e.isDirectory() ? files(root, p, pattern) : pattern.test(p) ? [p] : [];
}).sort();
const immutable = new Map();
const reviewedAggregates = new Map();
const peeledObjects = new Map();
function liveTagTarget(root) {
  const dotGit = resolve(root, '.git');
  const gitDir = statSync(dotGit).isDirectory() ? dotGit :
    resolve(root, readFileSync(dotGit, 'utf8').trim().replace(/^gitdir: /, ''));
  const common = existsSync(join(gitDir, 'commondir')) ?
    resolve(gitDir, readFileSync(join(gitDir, 'commondir'), 'utf8').trim()) : gitDir;
  const ref = `refs/tags/${releaseTag}`, loose = join(common, ref);
  let oid;
  if (existsSync(loose)) oid = readFileSync(loose, 'utf8').trim();
  else if (existsSync(join(common, 'packed-refs'))) oid = readFileSync(join(common, 'packed-refs'), 'utf8')
    .split(/\r?\n/).find(line => line.endsWith(` ${ref}`))?.split(' ')[0];
  if (!/^[a-f0-9]{40}$/.test(oid ?? '')) throw Error('Publication tag missing');
  // Read the mutable ref every time. Only peeling the immutable object is reused.
  const key = JSON.stringify([common, oid]);
  if (!peeledObjects.has(key)) peeledObjects.set(key, execFileSync('git',
    ['rev-list', '-n', '1', oid], {cwd: root, encoding: 'utf8', stdio: ['ignore','pipe','pipe']}).trim());
  return peeledObjects.get(key);
}
function parentObjects(root) {
  if (immutable.has(root)) return immutable.get(root);
  const git = args => execFileSync('git', args, {cwd: root, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024}).trim();
  const all = git(['ls-tree', '-r', '--name-only', PUBLICATION_PARENT]).split(/\r?\n/).sort();
  const paths = all.filter(p => existing.includes(p) || configs.includes(p) ||
    groups.some(([,dir,pattern]) => p.startsWith(`${dir}/`) && pattern.test(p)));
  const requests = paths.map(p => [`parent:${p}`, PUBLICATION_PARENT, p])
    .concat(docs.map(p => [`release:${p}`, RELEASE_TAG_HEAD, p]));
  const bytes = execFileSync('git', ['cat-file', '--batch'], {cwd: root,
    input: requests.map(([,head,p]) => `${head}:${p}\n`).join(''), maxBuffer: 64 * 1024 * 1024});
  const blobs = new Map(); let offset = 0;
  for (const [key] of requests) {
    const end = bytes.indexOf(10, offset), m = /^[a-f0-9]+ blob (\d+)$/.exec(bytes.subarray(offset, end).toString());
    if (!m) throw Error('Publication parent blob unavailable');
    const start = end + 1, length = Number(m[1]);
    if (bytes[start + length] !== 10) throw Error('Publication parent blob boundary');
    blobs.set(key, normalize(bytes.subarray(start, start + length).toString('utf8')));
    offset = start + length + 1;
  }
  if (offset !== bytes.length) throw Error('Publication parent trailing bytes');
  const read = p => blobs.get(`parent:${p}`);
  const fixed = Object.fromEntries(groups.map(([name,dir,pattern]) => {
    const base = paths.filter(p => p.startsWith(`${dir}/`) && pattern.test(p));
    const untouched = base.filter(p => !existing.includes(p));
    const beforeHash = aggregate(base, read);
    return [name, {base, untouched, beforeHash, untouchedHash: untouched.length === base.length ? beforeHash : aggregate(untouched, read)}];
  }));
  const parentIsRelease = git(['rev-parse', `${PUBLICATION_PARENT}^`]) === RELEASE_TAG_HEAD;
  const docsOnly = JSON.stringify(git(['diff', '--name-only', RELEASE_TAG_HEAD, PUBLICATION_PARENT]).split(/\r?\n/).sort()) === JSON.stringify(docs);
  const result = {all, paths, read, fixed, parentIsRelease, docsOnly,
    historical: p => blobs.get(`release:${p}`)};
  immutable.set(root, result); return result;
}
function publicationParentReaderUncached(root, override) {
  const successor = h3RepairParentReader(root);
  // No cross-call live cache: edited docs, tag refs and source must fail immediately.
  const live = new Map();
  const disk = p => {
    if (!live.has(p)) live.set(p, normalize(successor.read(p)));
    return live.get(p);
  };
  const violations = [...successor.violations], fail = s => violations.push(`publication-${s}`);
  const rejected = () => ({violations, addedPaths: [], afterHashes: {}, read: disk});
  let proof, facts;
  try {
    proof = override ?? JSON.parse(disk(manifest));
    facts = parentObjects(root);
    const tag = liveTagTarget(root);
    if (tag !== RELEASE_TAG_HEAD) fail('tag-target');
  } catch { fail('missing-git-or-proof-evidence'); return rejected(); }
  if (proof.schemaVersion !== 1 || proof.checkpoint !== 'AI_STUDIO_2_1_PUBLICATION_SUCCESSOR' ||
    proof.parentHead !== PUBLICATION_PARENT || proof.releaseTag !== releaseTag ||
    proof.releaseTagHead !== RELEASE_TAG_HEAD) fail('header');
  if (!facts.parentIsRelease || !facts.docsOnly) fail('publication-docs-only-parent');
  if (proof.productVersion !== '2.1.0-personal' || proof.installerSourceHead !== installerSource ||
    proof.installerSha256 !== installerSha) fail('installer-provenance');
  const evidence = proof.releaseEvidence;
  if (evidence?.releaseId !== 404490006 || evidence?.tagCiRun !== 37434078940 ||
    evidence?.sourceCiRun !== 37416738731 ||
    evidence?.assetName !== 'AI-Studio-2.1.0-personal-windows-x64-setup.exe' ||
    evidence?.assetSize !== 13055486 || evidence?.downloadedEqualsAccepted !== true) fail('release-evidence');
  if (JSON.stringify(Object.keys(proof.paths ?? {}).sort()) !== JSON.stringify([...existing, ...added].sort())) fail('scope');
  for (const flag of ['businessRuntimeChanged','schemaChanged','backupFormatChanged','queueAuthorityChanged',
    'taskStateMachineChanged','workflowEngineChanged','bindingOccChanged','installerChanged',
    'releaseTagMoved','releaseAssetsReplaced','remoteTelemetry']) if (proof.invariants?.[flag] !== false) fail(`invariant:${flag}`);
  // Preserve this boundary's precise diagnostics when an outer successor fails.
  // Neither boundary exposes a historical projection on any rejection.
  if (violations.some(v => v.startsWith('publication-'))) return rejected();
  for (const p of existing) {
    if (proof.paths[p]?.beforeHash !== hash(facts.read(p)) || proof.paths[p]?.afterHash !== hash(disk(p))) fail(`path:${p}`);
  }
  for (const p of docs) if (disk(p) !== facts.read(p) || proof.paths[p].beforeHash !== proof.paths[p].afterHash) fail(`published-doc-drift:${p}`);
  for (const p of added) if (facts.all.includes(p) || proof.paths[p]?.beforeHash !== null || proof.paths[p]?.afterHash !== hash(disk(p))) fail(`addition:${p}`);
  const inventories = new Map();
  for (const [name,dir,pattern] of groups) {
    const fixed = facts.fixed[name], r = proof[name];
    if (!inventories.has(dir)) inventories.set(dir, files(root, dir, /./));
    const current = inventories.get(dir).filter(p => p !== manifest && !successor.addedPaths.includes(p) && pattern.test(p));
    const expected = [...fixed.base, ...added.filter(p => p.startsWith(`${dir}/`) && pattern.test(p))].sort();
    if (JSON.stringify(current) !== JSON.stringify(expected) || r?.beforeFiles !== fixed.base.length || r?.afterFiles !== current.length) fail(`${name}-files`);
    // Exact fresh byte equality proves unchanged groups without hashing identical
    // multi-megabyte text again. Only immutable Git facts are cached across calls.
    const untouchedMatch = fixed.untouched.every(p => disk(p) === facts.read(p));
    const wholeGroupUnchanged = untouchedMatch && fixed.untouched.length === fixed.base.length &&
      JSON.stringify(current) === JSON.stringify(fixed.base);
    const reviewed = [...existing, ...added].filter(p => current.includes(p));
    const key = JSON.stringify([root, name, reviewed.map(p => [p, proof.paths[p].afterHash])]);
    // Path hashes were checked above; fresh untouched byte equality is checked
    // on every call. The expected aggregate is then content-addressed evidence,
    // never a cached acceptance result or stale working-tree read.
    const reviewedMatch = reviewed.every(p => hash(disk(p)) === proof.paths[p].afterHash);
    let afterHash = fixed.beforeHash;
    if (!wholeGroupUnchanged) {
      if (!untouchedMatch || !reviewedMatch) afterHash = aggregate(current, disk);
      else {
        if (!reviewedAggregates.has(key)) reviewedAggregates.set(key, aggregate(current, disk));
        afterHash = reviewedAggregates.get(key);
      }
    }
    if (r?.beforeAggregateHash !== fixed.beforeHash || r?.untouchedAggregateHash !== fixed.untouchedHash ||
      r?.afterAggregateHash !== afterHash) fail(`${name}-aggregate`);
    if (!untouchedMatch) fail(`${name}-untouched-bytes`);
  }
  for (const p of configs) if (disk(p) !== facts.read(p)) fail(`frozen-config:${p}`);
  const readme = disk(docs.find(p => p === 'README.md'));
  const notes = disk('docs/RELEASE_NOTES_v2.1.0-personal.md');
  const closeout = disk('docs/AI_STUDIO_2_1_CLOSEOUT.md').split('## Final Publication —')[1] ?? '';
  if (!readme.includes('Stable release: [`v2.1.0-personal`]') ||
    !readme.includes('Current product: `2.1.0-personal` **released**.') ||
    !readme.includes('Previous validated 2.0 release: `v2.0.0-personal-r4`.') ||
    readme.includes('NOT YET PUBLISHED')) fail('readme-published');
  if (!notes.startsWith('# AI Studio 2.1.0 Personal Edition\n') ||
    !notes.includes('Git tag: `v2.1.0-personal`.') || !notes.includes('GitHub Release: [published]') ||
    !notes.includes('Public stable release: **v2.1.0-personal**.') ||
    !notes.includes(RELEASE_TAG_HEAD) || notes.includes('NOT YET PUBLISHED')) fail('notes-published');
  for (const token of ['37416738731','37434078940','completed / success',releaseTag,RELEASE_TAG_HEAD,
    installerSource,installerSha,'P0=NONE; P1=NONE','AI_STUDIO_2_1_RELEASED=YES','PUBLIC_STABLE_RELEASE=v2.1.0-personal']) {
    if (!closeout.includes(token)) fail('closeout-publication');
  }
  if (violations.length) return rejected();
  return {violations, addedPaths: [...added, ...successor.addedPaths], backendAggregateSha256: successor.backendAggregateSha256, afterHashes: {[integration]: proof.paths[integration].afterHash, ...successor.afterHashes},
    read: p => docs.includes(p) ? facts.historical(p) : p === integration ? facts.read(p) : disk(p)};
}

export function publicationParentReader(root, override) {
  return cachedBoundary(root, "publicationParentReader", override, () => publicationParentReaderUncached(root, override), 'docs/architecture/2-1-publication-successor.json');
}
