// Release-only successor: validate real live bytes before projecting the exact
// parent for historical Phase13 checks. No historical manifest is rewritten.
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';
import { m1ParentReader } from './m1-readiness-boundary-guard.mjs';

export const RELEASE_PARENT = '52ba662d3e4f0a5e14fe30028be6fcc5778ac1eb';
const rustPath = 'src-tauri/src/application/project_backup_service.rs';
const guardPath = 'scripts/phase13-observability-guard.mjs';
const ownPath = 'scripts/phase14-release-guard.mjs';
const workflowPath = '.github/workflows/ci.yml';
const normalize = s => s.replaceAll('\r\n', '\n');
const hash = s => createHash('sha256').update(normalize(s)).digest('hex');
const live = (root, path) => readFileSync(join(root, path), 'utf8');

const files = (root, dir) => readdirSync(join(root, dir), { withFileTypes: true }).flatMap(e => {
  const p = `${dir}/${e.name}`;
  return e.isDirectory() ? files(root, p) : p.endsWith('.rs') ? [p] : [];
}).sort();
const aggregate = (paths, read) => hash(paths.map(p => `${p}\n${normalize(read(p))}`).join('\n'));

export function releaseParentReader(root, reviewOverride) {
  const checkpoint = m1ParentReader(root);
  const live = (_root, path) => checkpoint.read(path);
  const proofPath = join(root, 'docs/architecture/phase14-release.json');
  if (!existsSync(proofPath) && !reviewOverride) return { violations: [], read: path => live(root, path) };
  const proof = reviewOverride ?? JSON.parse(readFileSync(proofPath, 'utf8'));
  const violations = [...checkpoint.violations];
  const fail = s => violations.push(`phase14-${s}`);
  if (proof.phase !== 14 || proof.schemaVersion !== 1 || proof.parentHead !== RELEASE_PARENT ||
      proof.reason !== 'release-compatibility-and-validation-resource-boundary') fail('invalid-release-proof');
  if (JSON.stringify(Object.keys(proof.paths ?? {}).sort()) !== JSON.stringify([rustPath, guardPath, ownPath, workflowPath].sort())) fail('reviewed-path-set');
  const basePaths = execFileSync('git', ['ls-tree', '-r', '--name-only', RELEASE_PARENT, 'src-tauri/src'],
    { cwd: root, encoding: 'utf8' }).trim().split(/\r?\n/).filter(p => p.endsWith('.rs')).sort();
  // Batch immutable reads; never cache working-tree data.
  const bytes = execFileSync('git', ['cat-file', '--batch'], { cwd: root,
    input: [...basePaths, guardPath, workflowPath].map(p => `${RELEASE_PARENT}:${p}\n`).join(''), maxBuffer: 64 * 1024 * 1024 });
  const blobs = new Map(); let offset = 0;
  for (const path of [...basePaths, guardPath, workflowPath]) {
    const end = bytes.indexOf(10, offset);
    const m = /^[a-f0-9]+ blob (\d+)$/.exec(bytes.subarray(offset, end).toString());
    if (!m) throw Error('Release parent blob unavailable');
    const length = Number(m[1]), start = end + 1;
    blobs.set(path, bytes.subarray(start, start + length).toString('utf8'));
    offset = start + length + 1;
  }
  if (offset !== bytes.length) fail('parent-blob-boundary');
  const parent = (_root, path) => blobs.get(path);
  // Exclude only additions already validated by the complete successor chain.
  const livePaths = files(root, 'src-tauri/src').filter(path => !checkpoint.addedPaths?.includes(path));
  if (JSON.stringify(basePaths) !== JSON.stringify(livePaths) || proof.backend.files !== livePaths.length) fail('backend-file-set');
  for (const path of [rustPath, guardPath, workflowPath]) {
    const p = proof.paths?.[path];
    if (p?.beforeHash !== hash(parent(root, path)) || p?.afterHash !== hash(live(root, path))) fail(`reviewed-path-drift:${path}`);
  }
  // Only constrain frontend worker contention; preserve every test, timeout,
  // Rust command/thread setting and all other workflow bytes.
  const oldWorkflow = normalize(parent(root, workflowPath));
  if (normalize(live(root, workflowPath)) !== oldWorkflow.replace(
      'run: pnpm test\n', 'run: pnpm test --maxWorkers=1\n')) fail('workflow-resource-scope');
  if (proof.paths?.[ownPath]?.beforeHash !== null || proof.paths?.[ownPath]?.afterHash !== hash(live(root, ownPath))) fail('new-guard-drift');
  const untouched = basePaths.filter(p => p !== rustPath);
  if (aggregate(basePaths, p => blobs.get(p)) !== proof.backend.beforeAggregateHash ||
      aggregate(livePaths, p => live(root, p)) !== proof.backend.afterAggregateHash ||
      aggregate(untouched, p => blobs.get(p)) !== proof.backend.untouchedAggregateHash ||
      aggregate(untouched, p => live(root, p)) !== proof.backend.untouchedAggregateHash) fail('backend-aggregate-drift');
  for (const flag of ['queueAuthorityChanged', 'taskStateMachineChanged', 'bindingAuthorityChanged',
    'workflowEngineChanged', 'schemaChanged', 'backupFormatChanged', 'remoteTelemetry']) {
    if (proof.invariants?.[flag] !== false) fail(`authority:${flag}`);
  }
  return { violations, addedPaths: checkpoint.addedPaths, backendAggregateSha256: violations.length ? undefined : checkpoint.backendAggregateSha256 ?? proof.backend.afterAggregateHash, read: path => {
    // A rejected proof must never expose even a validated successor projection.
    if (violations.length) return readFileSync(join(root, path), 'utf8');
    if (![rustPath, guardPath, workflowPath].includes(path)) return live(root, path);
    return path === rustPath ? blobs.get(path) : parent(root, path);
  } };
}
