// M1-1 successor validates live bytes before historical guards read their parent.
// Historical manifests remain immutable; no authority or transport exemption.
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';
import { m1OverviewParentReader } from './m1-overview-readiness-boundary-guard.mjs';

export const M1_PARENT = '3feefee642eb88c496bf9bcb603e95c4bb1eb03d';
const existing = [
  'src/app/App.tsx', 'src/app/BackendBoundary.test.ts', 'src/app/StyleBoundary.test.ts',
  'src/features/create/CreateController.ts', 'src/features/create/CreateInputs.tsx',
  'src/features/create/CreatePage.tsx', 'src/features/create/CreateResults.tsx',
  'src/features/create/CreatePage.test.tsx',
  'src-tauri/src/application/product/creation_facade/submission.rs',
  'src-tauri/tests/support/creation_submission_contract.rs',
  'scripts/phase14-release-guard.mjs', 'scripts/phase13-observability-guard.mjs',
  'scripts/style-boundary-guard.mjs',
].sort();
const added = ['src/features/create/createReadinessAction.ts', 'src/app/M1ReadinessBoundary.test.ts',
  'scripts/m1-readiness-boundary-guard.mjs'].sort();
const text = s => s.replaceAll('\r\n', '\n');
const hash = s => createHash('sha256').update(text(s)).digest('hex');
const disk = (root, p) => text(readFileSync(join(root, p), 'utf8'));
const files = (root, folder, pattern) => readdirSync(join(root, folder), { withFileTypes: true }).flatMap(e => {
  const p = `${folder}/${e.name}`;
  return e.isDirectory() ? files(root, p, pattern) : pattern.test(p) ? [p] : [];
}).sort();
const aggregate = (paths, read) => hash(paths.map(p => `${p}\n${read(p)}`).join('\n'));
const immutable = new Map();
function parentBlobs(root, paths) {
  const cacheKey = JSON.stringify([root, paths]);
  if (immutable.has(cacheKey)) return immutable.get(cacheKey);
  const bytes = execFileSync('git', ['cat-file', '--batch'], { cwd: root,
    input: paths.map(p => `${M1_PARENT}:${p}\n`).join(''), maxBuffer: 64 * 1024 * 1024 });
  const result = new Map(); let offset = 0;
  for (const p of paths) {
    const end = bytes.indexOf(10, offset), match = /^[a-f0-9]+ blob (\d+)$/.exec(bytes.subarray(offset, end).toString());
    if (!match) throw Error(`M1 parent unavailable: ${p}`);
    const start = end + 1, length = Number(match[1]);
    if (bytes[start + length] !== 10) throw Error('Invalid M1 parent blob boundary');
    result.set(p, text(bytes.subarray(start, start + length).toString('utf8'))); offset = start + length + 1;
  }
  if (offset !== bytes.length) throw Error('Unexpected M1 parent blob bytes');
  immutable.set(cacheKey, result); return result;
}

export function m1ParentReader(root, override) {
  // Validate the live successor first; historical M1-1 consumes only its proven parent.
  const successor = m1OverviewParentReader(root);
  const disk = (_root, p) => successor.read(p);
  const projectedFiles = (root, folder, pattern) => files(root, folder, pattern).filter(p => !successor.addedPaths.includes(p));
  const review = override ?? JSON.parse(disk(root, 'docs/architecture/m1-1-readiness.json'));
  const violations = [...successor.violations], fail = s => violations.push(`m1-${s}`);
  if (review.checkpoint !== 'M1_1_CREATE_READINESS_ACTION_ROUTING' || review.parentHead !== M1_PARENT || review.schemaVersion !== 1) fail('invalid-header');
  if (JSON.stringify(Object.keys(review.paths ?? {}).sort()) !== JSON.stringify([...existing, ...added].sort())) fail('path-set');
  const listed = execFileSync('git', ['ls-tree', '-r', '--name-only', M1_PARENT, 'src', 'src-tauri/src', 'scripts', 'src-tauri/tests/support/creation_submission_contract.rs'],
    { cwd: root, encoding: 'utf8' }).trim().split(/\r?\n/).filter(p => /\.(tsx?|rs|css|mjs)$/.test(p)).sort();
  const blobs = parentBlobs(root, listed), before = p => blobs.get(p);
  for (const p of existing) if (review.paths?.[p]?.beforeHash !== hash(before(p)) || review.paths?.[p]?.afterHash !== hash(disk(root, p))) fail(`path-drift:${p}`);
  for (const p of added) if (blobs.has(p) || review.paths?.[p]?.beforeHash !== null || review.paths?.[p]?.afterHash !== hash(disk(root, p))) fail(`addition-drift:${p}`);
  for (const [name, folder, pattern] of [['backend', 'src-tauri/src', /\.rs$/], ['frontend', 'src', /\.tsx?$/], ['styles', 'src', /\.css$/], ['scripts', 'scripts', /\.mjs$/]]) {
    const base = listed.filter(p => p.startsWith(`${folder}/`) && pattern.test(p)), current = projectedFiles(root, folder, pattern);
    const expected = [...base, ...added.filter(p => p.startsWith(`${folder}/`) && pattern.test(p))].sort();
    const unchanged = base.filter(p => !existing.includes(p));
    const proof = review[name];
    if (JSON.stringify(current) !== JSON.stringify(expected) || proof?.beforeFiles !== base.length || proof?.afterFiles !== current.length) fail(`${name}-file-set`);
    if (proof?.beforeAggregateHash !== aggregate(base, before) || proof?.afterAggregateHash !== aggregate(current, p => disk(root, p)) ||
      proof?.untouchedAggregateHash !== aggregate(unchanged, before) || proof?.untouchedAggregateHash !== aggregate(unchanged, p => disk(root, p))) fail(`${name}-aggregate`);
  }
  for (const flag of ['queueAuthorityChanged', 'taskStateMachineChanged', 'workflowEngineChanged', 'bindingOccChanged', 'draftAuthorityChanged', 'schemaChanged', 'backupFormatChanged', 'remoteTelemetry']) if (review.invariants?.[flag] !== false) fail(`invariant:${flag}`);
  return { violations, addedPaths: [...added, ...successor.addedPaths], afterHashes: { ...Object.fromEntries(Object.entries(review.paths ?? {}).map(([p, proof]) => [p, proof.afterHash])), ...successor.afterHashes },
    backendAggregateSha256: violations.length ? undefined : successor.backendAggregateSha256 ?? review.backend.afterAggregateHash,
    read: p => violations.length || !existing.includes(p) ? disk(root, p) : before(p) };
}
