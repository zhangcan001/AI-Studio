// Static discovery is a measurement queue, never evidence of a bottleneck.
import { readFileSync, writeFileSync } from 'node:fs';
const candidates = [
  ['src-tauri/src/lib.rs', 'run_application', 'startup', 'startup', 'Bootstrap/composition', 'Native launch and existing database-ready logs'],
  ['src-tauri/src/infrastructure/database/pool.rs', 'initialize', 'query', 'startup', 'Connection/pragmas/migration check', 'Existing logs plus real migrated fixture benchmark'],
  ['src/app/App.tsx', 'App', 'frontend', 'startup/project-switch', 'Root subscriptions and state fan-out', 'React commit observation and scoped IPC counts'],
  ['src/app/v3/AppShellV3.tsx', 'AppShellV3', 'frontend', 'route-switch', 'Navigation renders/listener lifetime', 'React commits and five real transitions'],
  ['src/app/routes/resumeAdapter.ts', 'writeRouteResume', 'filesystem', 'resume', 'localStorage serialization', 'Real process restart; do not substitute navigation'],
  ['src/product/client.ts', 'productClient', 'IPC', 'all', 'Command frequency/payload', 'CDP actual POST requests; Phase11 preflight classifier'],
  ['src/product/transport.ts', 'productRequest', 'IPC', 'all', 'IPC latency/serialization', 'CDP completion timestamps and encoded response bytes'],
  ['src/features/create/CreateController.ts', 'useCreateController', 'frontend', 'create', 'Preparation/readiness/submission', 'Native readiness separately from generation; typed controlled guards'],
  ['src/features/runs/RunsController.ts', 'refresh', 'IPC', 'runs', 'List then detail/results; polling', 'Native action counts/elapsed; controlled dependency tests'],
  ['src/features/runs/RunsPage.tsx', 'RunsPage', 'frontend', 'runs-large', 'List rendering/date formatting', 'Representative 50 recent-task dataset and commits'],
  ['src/features/library/LibraryController.ts', 'refresh', 'IPC', 'library', 'Page reload/detail fan-out', 'Native initial/selection/edit IPC counts and transitions'],
  ['src/features/library/LibraryPage.tsx', 'LibraryPage', 'frontend', 'library-large', 'Card render/filter/selection', 'Representative isolated dataset/commits'],
  ['src/features/workflows/useWorkflowLabController.ts', 'useWorkflowLabController', 'IPC', 'workflow-lab', 'Metadata/binding/workspace loads', 'Native command timings; file/OCC paths separately'],
  ['src-tauri/src/application/product/run_facade/workspace.rs', 'ProductRunFacade::list', 'query', 'runs-large', 'Per-run projection and task lookups', 'Real SQL/repository counters, not source-call counts'],
  ['src-tauri/src/application/product/run_facade.rs', 'ProductRunFacade::get', 'query', 'runs', 'Resume plan/result enrichment', 'Real SQLite query count and elapsed time'],
  ['src-tauri/src/application/product/library_facade/list.rs', 'LibraryServices::list', 'query', 'library', 'Read-model composition', 'Real category query counts and elapsed time'],
  ['src-tauri/src/application/product/project_facade.rs', 'ProductProjectFacade', 'query', 'project-open/project-switch', 'Overview composition', 'Native IPC and real repository queries'],
  ['src-tauri/src/infrastructure/database/repositories/workflow_registry.rs', 'SqliteWorkflowRegistryRepository', 'query', 'workflow-lab', 'Version/registry lookup', 'Real migrated SQLite with representative registry'],
  ['src-tauri/src/application/workflow_registry_service.rs', 'WorkflowRegistryService', 'filesystem', 'workflow-lab', 'Workspace assembly and artifact reads', 'Existing diagnostic spans plus controlled file counters'],
  ['src-tauri/src/application/production_queue_service.rs', 'ProductionQueueService', 'runtime', 'runs/create', 'Queue read/probe/poll lifecycle', 'Measure probes; preserve execution authority'],
  ['src-tauri/src/application/comfy_preflight_service.rs', 'ComfyPreflightService', 'runtime', 'create', 'External schema/runtime probe', 'Separate local admission from external runtime delay'],
  ['src-tauri/src/infrastructure/comfy/client.rs', 'ComfyUIClient', 'runtime', 'create', 'HTTP/poll/serialization', 'No real generation required for local performance baseline'],
];

const rows = candidates.map(([path, symbol, area, critical_path, suspected_cost, measurement_method]) => {
  const source = readFileSync(path, 'utf8');
  return { path, symbol, area, critical_path, suspected_cost, measurement_method,
    classification: 'STATIC_CANDIDATE', baseline: 'NOT_MEASURED', decision: 'MEASURE_BEFORE_OPTIMIZATION',
    optimization: null, after: 'NOT_MEASURED', risk: 'Preserve authority, scope, freshness, errors and ordering',
    tests: 'Existing Phase11 behavior map; add count guard only after measured hotspot',
    staticSignals: { effects: (source.match(/\buseEffect\(/g) || []).length,
      memoization: (source.match(/\b(?:useMemo|useCallback|memo)\(/g) || []).length,
      derivations: (source.match(/\.(?:sort|filter|map|reduce)\(/g) || []).length,
      serialization: (source.match(/JSON\.(?:parse|stringify)|serde_json::/g) || []).length,
      storage: (source.match(/localStorage/g) || []).length,
      polling: (source.match(/setInterval|setTimeout|tokio::time|interval\(/g) || []).length } };
});
const output = { phase: 12, status: 'INVENTORY_COMPLETE_MEASUREMENT_PENDING',
  baselineHead: '0bb137303bbcf4baa98a109e7558d5899ef236fc', baselineCi: 37107002128,
  criticalPaths: ['startup', 'project-open', 'project-switch', 'resume', 'create', 'runs', 'library', 'workflow-lab'],
  environment: { buildMode: 'DEBUG', machine: 'Same local Windows machine', dataset: 'Owned isolated Phase11 fixture copy; never real project data',
    warmupRuns: 1, measuredRuns: 5, coldOsCacheControlled: false },
  candidates: rows, blockedUnknown: [], measurements: {}, optimizations: [],
  limitations: ['Static counts are NOT measured query/render counts', 'OS cold cache cannot be controlled safely',
    'CDP wire bytes are not decoded DTO bytes', 'No heap-leak absence claim without lifetime evidence'],
  freeze: { databaseVersion: 42, backupVersion: 20, schemaChange: false, ipcBreakingChange: false,
    routeChange: false, nextPhaseStarted: false } };
writeFileSync('docs/architecture/phase12-performance.json', JSON.stringify(output, null, 2) + '\n');
console.log(JSON.stringify({ candidates: rows.length, paths: output.criticalPaths.length,
  byArea: Object.fromEntries([...new Set(rows.map(r => r.area))].map(area => [area, rows.filter(r => r.area === area).length])) }));
