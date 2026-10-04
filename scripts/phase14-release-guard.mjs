// Release-only successor: validate real live bytes before projecting the exact
// parent for historical Phase13 checks. No historical manifest is rewritten.
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';

export const RELEASE_PARENT = '52ba662d3e4f0a5e14fe30028be6fcc5778ac1eb';
const rustPath = 'src-tauri/src/application/project_backup_service.rs';
const guardPath = 'scripts/phase13-observability-guard.mjs';
const ownPath = 'scripts/phase14-release-guard.mjs';
const normalize = s => s.replaceAll('\r\n', '\n');
const hash = s => createHash('sha256').update(normalize(s)).digest('hex');
const live = (root, path) => readFileSync(join(root, path), 'utf8');
const parent = (root, path) => execFileSync('git', ['show', `${RELEASE_PARENT}:${path}`],
  { cwd: root, encoding: 'utf8', maxBuffer: 16 * 1024 * 1024 });
const files = (root, dir) => readdirSync(join(root, dir), { withFileTypes: true }).flatMap(e => {
  const p = `${dir}/${e.name}`;
  return e.isDirectory() ? files(root, p) : p.endsWith('.rs') ? [p] : [];
}).sort();
const aggregate = (paths, read) => hash(paths.map(p => `${p}\n${normalize(read(p))}`).join('\n'));

export function releaseParentReader(root, reviewOverride) {
  const proofPath = join(root, 'docs/architecture/phase14-release.json');
  if (!existsSync(proofPath) && !reviewOverride) return { violations: [], read: path => live(root, path) };
  const proof = reviewOverride ?? JSON.parse(readFileSync(proofPath, 'utf8'));
  const violations = [];
  const fail = s => violations.push(`phase14-${s}`);
  if (proof.phase !== 14 || proof.schemaVersion !== 1 || proof.parentHead !== RELEASE_PARENT ||
      proof.reason !== 'restore-report-empty-list-serialization') fail('invalid-release-proof');
  if (JSON.stringify(Object.keys(proof.paths ?? {}).sort()) !== JSON.stringify([rustPath, guardPath, ownPath].sort())) fail('reviewed-path-set');
  for (const path of [rustPath, guardPath]) {
    const p = proof.paths?.[path];
    if (p?.beforeHash !== hash(parent(root, path)) || p?.afterHash !== hash(live(root, path))) fail(`reviewed-path-drift:${path}`);
  }
  if (proof.paths?.[ownPath]?.beforeHash !== null || proof.paths?.[ownPath]?.afterHash !== hash(live(root, ownPath))) fail('new-guard-drift');
  const basePaths = execFileSync('git', ['ls-tree', '-r', '--name-only', RELEASE_PARENT, 'src-tauri/src'],
    { cwd: root, encoding: 'utf8' }).trim().split(/\r?\n/).filter(p => p.endsWith('.rs')).sort();
  const livePaths = files(root, 'src-tauri/src');
  if (JSON.stringify(basePaths) !== JSON.stringify(livePaths) || proof.backend.files !== livePaths.length) fail('backend-file-set');
  // Batch immutable reads; never cache working-tree data.
  const bytes = execFileSync('git', ['cat-file', '--batch'], { cwd: root,
    input: basePaths.map(p => `${RELEASE_PARENT}:${p}\n`).join(''), maxBuffer: 64 * 1024 * 1024 });
  const blobs = new Map(); let offset = 0;
  for (const path of basePaths) {
    const end = bytes.indexOf(10, offset);
    const m = /^[a-f0-9]+ blob (\d+)$/.exec(bytes.subarray(offset, end).toString());
    if (!m) throw Error('Release parent blob unavailable');
    const length = Number(m[1]), start = end + 1;
    blobs.set(path, bytes.subarray(start, start + length).toString('utf8'));
    offset = start + length + 1;
  }
  if (offset !== bytes.length) fail('parent-blob-boundary');
  const untouched = basePaths.filter(p => p !== rustPath);
  if (aggregate(basePaths, p => blobs.get(p)) !== proof.backend.beforeAggregateHash ||
      aggregate(livePaths, p => live(root, p)) !== proof.backend.afterAggregateHash ||
      aggregate(untouched, p => blobs.get(p)) !== proof.backend.untouchedAggregateHash ||
      aggregate(untouched, p => live(root, p)) !== proof.backend.untouchedAggregateHash) fail('backend-aggregate-drift');
  for (const flag of ['queueAuthorityChanged', 'taskStateMachineChanged', 'bindingAuthorityChanged',
    'workflowEngineChanged', 'schemaChanged', 'backupFormatChanged', 'remoteTelemetry']) {
    if (proof.invariants?.[flag] !== false) fail(`authority:${flag}`);
  }
  return { violations, backendAggregateSha256: violations.length ? undefined : proof.backend.afterAggregateHash, read: path => {
    if (violations.length || ![rustPath, guardPath].includes(path)) return live(root, path);
    return path === rustPath ? blobs.get(path) : parent(root, path);
  } };
}
