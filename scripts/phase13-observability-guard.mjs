import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync, readdirSync } from 'node:fs';
import { join, relative } from 'node:path';
import { releaseParentReader } from './phase14-release-guard.mjs';

export const PHASE13_BASELINE = '2795ffddf964e88f3c99af2ed3f284120da4b368';
const EXPECTED_RUST_EXISTING = [
  'src-tauri/src/application/diagnostics_service.rs',
  'src-tauri/src/commands/diagnostics.rs',
  'src-tauri/src/infrastructure/logging.rs',
  'src-tauri/src/lib.rs',
  'src-tauri/src/application/ports/task_repository.rs',
  'src-tauri/src/application/ports/mod.rs',
  'src-tauri/src/infrastructure/database/repositories/task.rs',
];
const EXPECTED_RUST_ADDED = ['src-tauri/src/application/diagnostics_service/execution.rs'];
const EXPECTED_TS_EXISTING = [
  'src/app/App.tsx', 'src/app/BackendBoundary.test.ts', 'src/app/Phase7Retirement.test.tsx',
  'src/app/routes/resumeAdapter.ts', 'src/app/routes/types.ts',
  'src/features/production/ProductionAuditCenter.tsx', 'src/features/runs/RunDetail.tsx',
  'src/features/settings/RepairJobsStatusSection.tsx', 'src/features/settings/SettingsWorkspace.tsx',
  'src/features/tasks/TaskHistory.tsx', 'src/services/tauriClient.ts', 'src/types/diagnostics.ts',
  'src/features/workflow-lab/WorkflowLabPage.tsx',
  'src/app/StyleBoundary.test.ts',
  'src/features/settings/RepairJobsStatusSection.test.tsx',
  'src/features/workflow-lab/WorkflowLabBoundary.test.tsx',
];
const EXPECTED_TS_ADDED = [
  'src/features/settings/DiagnosticsExecutionPanel.test.tsx',
  'src/features/settings/DiagnosticsExecutionPanel.tsx', 'src/services/diagnosticsClient.ts',
];
const EXPECTED_GUARD_SCRIPTS = [
  'scripts/backend-boundary-guard.mjs', 'scripts/dev088-architecture-guard.mjs',
  'scripts/phase13-observability-guard.mjs', 'scripts/style-boundary-guard.mjs',
];

const normalized = value => value.replaceAll('\r\n', '\n');
const digest = value => createHash('sha256').update(value).digest('hex');
const sourceDigest = value => digest(normalized(value));
const rootOf = root => root.replaceAll('\\', '/');
// Cache only immutable commit blobs during this guard process. Live files are
// always reread, so drift/negative probes cannot be hidden by this optimization.
const immutableBlobs = new Map();
const git = (root, ...args) => {
  const key = args[0] === 'show' && /^[a-f0-9]{40}:/.test(args[1] ?? '') ? JSON.stringify([root, ...args]) : undefined;
  if (key && immutableBlobs.has(key)) return immutableBlobs.get(key);
  const value = execFileSync('git', args, { cwd: root, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] }).replaceAll('\r\n', '\n');
  if (key) immutableBlobs.set(key, value);
  return value;
};
function primeCommitBlobs(root, commit, paths) {
  if (!/^[a-f0-9]{40}$/.test(commit)) throw Error('Successor requires an immutable commit');
  const pending = paths.filter(path => !immutableBlobs.has(JSON.stringify([root, 'show', `${commit}:${path}`])));
  if (!pending.length) return;
  // One read-only batch avoids hundreds of process launches on Windows CI.
  const bytes = execFileSync('git', ['cat-file', '--batch'], { cwd: root,
    input: pending.map(path => `${commit}:${path}\n`).join(''), maxBuffer: 64 * 1024 * 1024 });
  let offset = 0;
  for (const path of pending) {
    const end = bytes.indexOf(10, offset);
    const header = bytes.subarray(offset, end).toString('utf8');
    const match = /^[a-f0-9]{40,64} blob (\d+)$/.exec(header);
    if (!match) throw Error(`Historical source blob unavailable: ${path}`);
    const size = Number(match[1]), start = end + 1;
    if (start + size >= bytes.length || bytes[start + size] !== 10) throw Error('Invalid historical blob boundary');
    immutableBlobs.set(JSON.stringify([root, 'show', `${commit}:${path}`]), normalized(bytes.subarray(start, start + size).toString('utf8')));
    offset = start + size + 1;
  }
  if (offset !== bytes.length) throw Error('Unexpected historical blob output');
}

function sourceFiles(root, folder, extensionPattern) {
  const start = join(root, folder);
  const walk = dir => readdirSync(dir, { withFileTypes: true }).flatMap(entry => {
    const path = join(dir, entry.name);
    return entry.isDirectory() ? walk(path) : extensionPattern.test(entry.name) ? [rootOf(relative(root, path))] : [];
  });
  return walk(start).sort();
}

function aggregate(paths, read) {
  return digest(paths.map(path => `${path}\n${normalized(read(path))}`).join('\n'));
}

export function phase13Boundary(root, review, phase12Review, phase8Snapshot) {
  const violations = [];
  const release = releaseParentReader(root);
  violations.push(...release.violations);
  const frozenRead = path => release.read(path);
  const fail = message => violations.push(message);
  if (review?.phase !== 13 || review?.baselineHead !== PHASE13_BASELINE || review?.schemaVersion !== 1) fail('invalid-phase13-review-header');
  if (review?.migration?.maxVersion !== 42 || review.migration.tableCount !== 71 || review.migration.migration043 !== false || review.migration.backupVersion !== 20) fail('phase13-schema-boundary-changed');

  const baseRust = git(root, 'ls-tree', '-r', '--name-only', PHASE13_BASELINE, 'src-tauri/src').trim().split('\n').filter(path => path.endsWith('.rs')).sort();
  primeCommitBlobs(root, PHASE13_BASELINE, baseRust);
  const liveRust = sourceFiles(root, 'src-tauri/src', /\.rs$/);
  const phase12Proof = phase12Review?.backendOptimization2?.sourceFreeze;
  const phase12Seam = phase12Review?.backendOptimization2;
  if (phase12Seam?.status !== 'MEASURED_VERIFIED' || !Number.isFinite(phase12Seam.baseline?.samples) || !Number.isFinite(phase12Seam.after?.samples) || phase12Seam.baseline.samples < 5 || phase12Seam.after.samples < 5) fail('phase12-measurement-proof-invalid');
  const historicalPaths = Object.keys(phase12Proof?.paths || {}).sort();
  const expectedHistoricalPaths = ['src-tauri/src/application/product/run_facade.rs', 'src-tauri/src/application/product/run_facade/workspace.rs'].sort();
  const baseRead = path => git(root, 'show', `${PHASE13_BASELINE}:${path}`);
  if (phase12Review?.phase !== 12 || phase8Snapshot?.sha256 !== phase12Proof?.beforeAggregateHash || phase8Snapshot?.files !== baseRust.length || phase12Proof?.files !== baseRust.length ||
      JSON.stringify(historicalPaths) !== JSON.stringify(expectedHistoricalPaths)) fail('phase12-historical-freeze-not-preserved');
  const phase12Base = aggregate(baseRust, baseRead);
  if (phase12Base !== phase12Proof?.afterAggregateHash || phase12Proof?.afterAggregateHash !== '22bf6637d39d861004a2853bd6d11f97a5158dc02ccbd88e25a72fb72823b222') fail('phase12-successor-not-anchored-to-baseline');
  const historicalUntouched = aggregate(baseRust.filter(path => !expectedHistoricalPaths.includes(path)), baseRead);
  if (historicalUntouched !== phase12Proof?.untouchedAggregateHash) fail('phase12-untouched-backend-changed');
  for (const path of expectedHistoricalPaths) {
    const proof = phase12Proof?.paths?.[path];
    const phase12StartingPoint = phase12Review?.baselineHead;
    if (!proof || !phase12StartingPoint || digest(git(root, 'show', `${phase12StartingPoint}:${path}`)) !== proof.beforeHash || sourceDigest(baseRead(path)) !== proof.afterHash) fail(`phase12-reviewed-source-drift:${path}`);
  }

  const backendReview = review?.backend;
  if (JSON.stringify([...(backendReview?.existingPaths || [])].sort()) !== JSON.stringify([...EXPECTED_RUST_EXISTING].sort()) ||
      JSON.stringify([...(backendReview?.addedPaths || [])].sort()) !== JSON.stringify([...EXPECTED_RUST_ADDED].sort())) fail('phase13-backend-scope-mismatch');
  const backendExisting = backendReview?.existingPaths || [];
  const backendAdded = backendReview?.addedPaths || [];
  const backendAllowed = new Set([...backendExisting, ...backendAdded]);
  const backendBaseSet = new Set(baseRust);
  const expectedLiveRust = [...baseRust, ...backendAdded].sort();
  if (backendReview?.beforeFiles !== baseRust.length || backendReview?.afterFiles !== liveRust.length || backendReview?.beforeAggregateSha256 !== phase12Base) fail('phase13-backend-parent-or-file-count-changed');
  if (JSON.stringify(liveRust) !== JSON.stringify(expectedLiveRust)) fail('phase13-backend-file-set-changed');
  for (const path of backendExisting) {
    if (!backendBaseSet.has(path) || sourceDigest(baseRead(path)) !== backendReview.beforeHashes?.[path] || sourceDigest(frozenRead(path)) !== backendReview.afterHashes?.[path]) fail(`phase13-backend-path-drift:${path}`);
  }
  for (const path of backendAdded) {
    if (backendBaseSet.has(path) || !existsSync(join(root, path)) || backendReview.beforeHashes?.[path] !== null || sourceDigest(frozenRead(path)) !== backendReview.afterHashes?.[path]) fail(`phase13-backend-addition-drift:${path}`);
  }
  const untouchedBackend = aggregate(baseRust.filter(path => !backendAllowed.has(path)), path => frozenRead(path));
  if (backendReview.untouchedAggregateSha256 !== aggregate(baseRust.filter(path => !backendAllowed.has(path)), baseRead)) fail('phase13-untouched-backend-parent-proof-changed');
  if (untouchedBackend !== backendReview.untouchedAggregateSha256) fail('phase13-untouched-backend-changed');
  if (aggregate(liveRust, path => frozenRead(path)) !== backendReview.aggregateSha256) fail('phase13-backend-aggregate-changed');

  const baseTs = git(root, 'ls-tree', '-r', '--name-only', PHASE13_BASELINE, 'src').trim().split('\n').filter(path => /\.tsx?$/.test(path)).sort();
  primeCommitBlobs(root, PHASE13_BASELINE, baseTs);
  const liveTs = sourceFiles(root, 'src', /\.tsx?$/);
  const frontendReview = review?.frontend;
  if (JSON.stringify([...(frontendReview?.existingPaths || [])].sort()) !== JSON.stringify([...EXPECTED_TS_EXISTING].sort()) ||
      JSON.stringify([...(frontendReview?.addedPaths || [])].sort()) !== JSON.stringify([...EXPECTED_TS_ADDED].sort())) fail('phase13-frontend-scope-mismatch');
  const frontendExisting = frontendReview?.existingPaths || [];
  const frontendAdded = frontendReview?.addedPaths || [];
  const frontendAllowed = new Set([...frontendExisting, ...frontendAdded]);
  if (JSON.stringify(liveTs) !== JSON.stringify([...baseTs, ...frontendAdded].sort())) fail('phase13-frontend-file-set-changed');
  for (const path of frontendExisting) {
    if (!baseTs.includes(path) || sourceDigest(baseRead(path)) !== frontendReview.beforeHashes?.[path] || sourceDigest(frozenRead(path)) !== frontendReview.afterHashes?.[path]) fail(`phase13-frontend-path-drift:${path}`);
  }
  for (const path of frontendAdded) {
    if (baseTs.includes(path) || frontendReview.beforeHashes?.[path] !== null || !existsSync(join(root, path)) || sourceDigest(frozenRead(path)) !== frontendReview.afterHashes?.[path]) fail(`phase13-frontend-addition-drift:${path}`);
  }
  if (frontendReview?.beforeFiles !== baseTs.length || frontendReview?.afterFiles !== liveTs.length || frontendReview?.beforeAggregateSha256 !== aggregate(baseTs, baseRead)) fail('phase13-frontend-parent-or-file-count-changed');
  const untouchedFrontend = aggregate(baseTs.filter(path => !frontendAllowed.has(path)), path => frozenRead(path));
  if (frontendReview.untouchedAggregateSha256 !== aggregate(baseTs.filter(path => !frontendAllowed.has(path)), baseRead)) fail('phase13-untouched-frontend-parent-proof-changed');
  if (untouchedFrontend !== frontendReview.untouchedAggregateSha256) fail('phase13-untouched-frontend-changed');
  if (aggregate(liveTs, path => frozenRead(path)) !== frontendReview.aggregateSha256) fail('phase13-frontend-aggregate-changed');

  const guardReview = review?.guardScripts || {};
  if (JSON.stringify(Object.keys(guardReview).sort()) !== JSON.stringify([...EXPECTED_GUARD_SCRIPTS].sort())) fail('phase13-guard-script-scope-mismatch');
  for (const path of EXPECTED_GUARD_SCRIPTS) {
    const proof = guardReview[path];
    const baselinePath = git(root, 'ls-tree', '-r', '--name-only', PHASE13_BASELINE, path).trim();
    if (!proof || proof.afterHash !== sourceDigest(frozenRead(path))) fail(`phase13-guard-script-drift:${path}`);
    if (baselinePath === path && proof.beforeHash !== sourceDigest(git(root, 'show', `${PHASE13_BASELINE}:${path}`))) fail(`phase13-guard-script-before-hash-invalid:${path}`);
    if (!baselinePath && proof.beforeHash !== null) fail(`phase13-new-guard-parent-hash-invalid:${path}`);
    if (baselinePath && proof.beforeHash === null) fail(`phase13-guard-script-parent-missing:${path}`);
  }
  const workflowPath = '.github/workflows/ci.yml';
  const workflowReview = review?.validationWorkflow;
  const workflowBefore = git(root, 'show', `${PHASE13_BASELINE}:${workflowPath}`);
  if (!workflowReview || workflowReview.path !== workflowPath ||
      workflowReview.beforeHash !== sourceDigest(workflowBefore) ||
      workflowReview.afterHash !== sourceDigest(readFileSync(join(root, workflowPath), 'utf8'))) fail('phase13-validation-workflow-drift');

  if (review?.privacy?.databaseExport !== false || review?.privacy?.promptExport !== false || review?.privacy?.absolutePathExport !== false || review?.privacy?.assetBytesExport !== false || review?.privacy?.workflowSourceExport !== false || review?.privacy?.recipeSourceExport !== false || review?.privacy?.remoteUpload !== false) fail('phase13-privacy-invariants-missing');
  const migrations = readdirSync(join(root, 'src-tauri/migrations'));
  if (migrations.some(name => /^043/.test(name)) || !migrations.some(name => /^042/.test(name))) fail('phase13-migration-boundary-changed');
  const baselineMigrations = git(root, 'ls-tree', '-r', '--name-only', PHASE13_BASELINE, 'src-tauri/migrations').trim().split('\n').filter(path => path.endsWith('.sql')).sort();
  const liveMigrations = sourceFiles(root, 'src-tauri/migrations', /\.sql$/);
  if (JSON.stringify(baselineMigrations) !== JSON.stringify(liveMigrations) || aggregate(liveMigrations, path => frozenRead(path)) !== aggregate(baselineMigrations, baseRead)) fail('phase13-historical-migrations-changed');
  for (const key of ['newObservabilityAuthority', 'newTelemetryRepository', 'newMetricsDatabase', 'newExecutionAuthority', 'phase12ProfilerPromoted']) if (review?.authority?.[key] !== false) fail(`phase13-authority-invariant:${key}`);
  if (!readFileSync(join(root, 'src-tauri/src/application/project_backup_service.rs'), 'utf8').includes('const BACKUP_VERSION: u32 = 20;')) fail('phase13-backup-version-changed');
  if (review?.commands?.countAdded !== 3 || !Array.isArray(review.commands.addedNames) || review.commands.addedNames.length !== 3) fail('phase13-typed-command-review-missing');
  return { violations, backendFiles: liveRust.length, frontendFiles: liveTs.length, backendAggregateSha256: release.backendAggregateSha256 ?? backendReview?.aggregateSha256, frontendAggregateSha256: frontendReview?.aggregateSha256 };
}
