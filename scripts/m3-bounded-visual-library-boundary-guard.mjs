import {m4MediaParentReader} from './m4-readonly-media-integrity-boundary-guard.mjs';
// M3-2 successor validates live bytes before historical guards read their parent.
// Historical manifests remain immutable; no authority or transport exemption.
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';

export const M3_VISUAL_PARENT = '8842a2750e4402ca8e45a6e4efc4571c3e34a425';
const ADDED_COMMAND = {"name": "product_library_thumbnail_get", "signature": "#[tauri::command(rename_all = \"camelCase\")] pub async fn product_library_thumbnail_get( state: State<'_, AppState>, project_id: String, resource: ResourceRef, ) -> Result<Vec<u8>, ProductError>"};
const existing = ["src/app/BackendBoundary.test.ts", "src/app/M2CreateLibraryReuseBoundary.test.ts", "src/app/StyleBoundary.test.ts", "scripts/style-boundary-guard.mjs", "src/product/errors.ts", "src/features/library/LibraryController.ts", "src/features/library/LibraryPage.tsx", "src/features/library/LibraryPage.css", "src/features/library/LibraryPage.test.tsx", "src/features/library/LibraryFindability.test.tsx", "src/features/library/LibraryScale.observed.test.tsx", "src/product/client.ts", "src/product/transport.ts", "src/product/client.test.ts", "src-tauri/src/application/product/library_facade/detail.rs", "src-tauri/src/commands/product.rs", "src-tauri/src/lib.rs", "src-tauri/tests/product_library_contract.rs", "src/features/runs/RunsPage.tsx", "src/features/runs/RunsPage.test.tsx", "scripts/m3-library-findability-boundary-guard.mjs", "src/app/M3LibraryFindabilityBoundary.test.ts"].sort();
const added = ["docs/architecture/m3-2-library-scale-candidate.json", "src/features/library/LibraryThumbnail.tsx", "src/features/library/LibraryThumbnail.test.tsx", "src/features/library/LibraryPagination.test.tsx", "src-tauri/tests/support/library_thumbnail_contract.rs", "scripts/m3-bounded-visual-library-boundary-guard.mjs", "src/app/M3BoundedVisualLibraryBoundary.test.ts", "docs/architecture/m3-2-library-targets.json"].sort();
const text = s => s.replaceAll('\r\n', '\n');
const hash = s => createHash('sha256').update(text(s)).digest('hex');
const disk = (root, p) => text(readFileSync(join(root, p), 'utf8'));
const files = (root, folder, pattern) => readdirSync(join(root, folder), { withFileTypes: true }).flatMap(e => {
  const p = `${folder}/${e.name}`;
  return e.isDirectory() ? files(root, p, pattern) : pattern.test(p) ? [p] : [];
}).sort();
const aggregate = (paths, read) => hash(paths.map(p => `${p}\n${read(p)}`).join('\n'));
const immutable = new Map();
const parentFileSets = new Map();
const frozenArtifacts = ['docs/architecture/m3-1-library-findability.json','docs/architecture/m3-1-library-scale-baseline.json','docs/architecture/m3-1-library-scale-candidate.json'];
function parentBlobs(root, paths) {
  const cacheKey = JSON.stringify([root, paths]);
  if (immutable.has(cacheKey)) return immutable.get(cacheKey);
  const bytes = execFileSync('git', ['cat-file', '--batch'], { cwd: root,
    input: paths.map(p => `${M3_VISUAL_PARENT}:${p}\n`).join(''), maxBuffer: 64 * 1024 * 1024 });
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

export function visualCandidateViolations(baseline,candidate) {
  const violations=[];
  for(const dataset of [1000,10000])for(const category of ['images','prompts'])for(const pages of [1,5,20]){
    const match=r=>r.dataset===dataset&&r.category===category&&r.pages===pages;
    const before=baseline.frontend.observations.find(match),rows=candidate.observations?.filter(match)??[];
    if(rows.length!==1||rows[0].productListCalls!==pages||rows[0].domItems>30||!Number.isFinite(rows[0].p95Ms)||rows[0].p95Ms>Math.max(before.p95Ms*3,500))violations.push(`candidate:${dataset}:${category}:${pages}`);
  }
  const t=candidate.thumbnailObservation;
  if(!t||t.visibleEligibleCards!==5||t.thumbnailRequestCount!==5||t.firstTwoRequests!==2||t.offscreenRequestCount!==0||t.fullMediaReads!==0||t.objectUrlsCreated!==t.objectUrlsRevoked)violations.push('candidate-thumbnail-ownership');
  if(candidate.budgetAdjusted!==false||candidate.m3_1ArtifactsModified!==false)violations.push('candidate-budget-widened');
  return violations;
}

export function m3VisualParentReader(root, override) {
  // Memoize reads only within this synchronous validation; no live cache survives
  // a call or replaces a fresh fail-closed read. Parent blobs alone persist.
  const successor=m4MediaParentReader(root);
  const projectedFiles=(root,folder,pattern)=>files(root,folder,pattern).filter(p=>!successor.addedPaths.includes(p));
  const live = new Map();
  const disk = (_root,p) => { if (!live.has(p)) live.set(p,successor.read(p)); return live.get(p); };
  const review = override ?? JSON.parse(disk(root, 'docs/architecture/m3-2-bounded-visual-library.json'));
  const violations = [...successor.violations], fail = s => violations.push(`m3-2-${s}`);
  if (review.checkpoint !== 'M3_2_BOUNDED_VISUAL_LIBRARY' || review.parentHead !== M3_VISUAL_PARENT || review.schemaVersion !== 1) fail('invalid-header');
  if (JSON.stringify(Object.keys(review.paths ?? {}).sort()) !== JSON.stringify([...existing, ...added].sort())) fail('path-set');
  for (const flag of ["libraryAuthorityChanged", "assetBrowseAuthorityChanged", "promptLibraryAuthorityChanged", "thumbnailStorageAuthorityChanged", "thumbnailGenerationAuthorityAdded", "newCacheAuthority", "newStore", "newRouter", "newPollingOwner", "runsAuthorityChanged", "taskHistoryAuthorityChanged", "mixedRunsPaginationAdded", "queueAuthorityChanged", "taskStateMachineChanged", "workflowEngineChanged", "bindingOccChanged", "schemaChanged", "backupFormatChanged", "remoteTelemetry"]) if (review.invariants?.[flag] !== false) fail(`invariant:${flag}`);
  // Invalid proof shape/invariants fail closed without any historical projection.
  if (violations.length) return { violations, addedPaths: [...added,...successor.addedPaths], addedCommandSignatures: [], afterHashes: {}, backendAggregateSha256: undefined, read: p => successor.read(p) };
  // Only immutable Git objects/file sets are reused. Every live byte and full
  // untouched aggregate is still read and validated anew on every invocation.
  if (!parentFileSets.has(root)) parentFileSets.set(root, execFileSync('git', ['ls-tree', '-r', '--name-only', M3_VISUAL_PARENT, 'src', 'src-tauri/src', 'scripts', 'src-tauri/tests/product_library_contract.rs', 'src-tauri/examples/phase12_query_profile.rs'],
    { cwd: root, encoding: 'utf8' }).trim().split(/\r?\n/).filter(p => /\.(tsx?|rs|css|mjs)$/.test(p)).sort());
  const listed = parentFileSets.get(root);
  const blobs = parentBlobs(root, [...listed,...frozenArtifacts]), before = p => blobs.get(p);
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

  const commands=disk(root,'src-tauri/src/commands/product.rs');
  const signature=[...commands.matchAll(/#\[tauri::command[^\]]*\]\s*pub\s+async\s+fn\s+(\w+)[\s\S]*?(?=\{)/g)].find(m=>m[1]===ADDED_COMMAND.name);
  if(!signature||signature[0].replace(/\s+/g,' ').trim()!==ADDED_COMMAND.signature||!commands.includes('.thumbnail_get(&project_id, &resource)'))fail('readonly-thumbnail-command');
  const detail=disk(root,'src-tauri/src/application/product/library_facade/detail.rs');
  const adapter=detail.slice(detail.indexOf('pub async fn thumbnail_get'),detail.indexOf('pub async fn image_get'));
  if(!adapter.includes('.read_thumbnail(project_id, id)')||/read_image|write|thumbnail_path|storage_path/.test(adapter))fail('managed-thumbnail-only');
  const controller=disk(root,'src/features/library/LibraryController.ts');
  if((controller.match(/productClient.library.list\(/g)||[]).length!==1||/pageCount|items.push/.test(controller)||!controller.includes('starts:[null]')||!controller.includes('if(!navigating.current)void refresh({tags:false});},5000)'))fail('bounded-page-owner');
  const thumbnail=disk(root,'src/features/library/LibraryThumbnail.tsx');
  if(!thumbnail.includes('IntersectionObserver')||!thumbnail.includes('active < 4')||!thumbnail.includes('URL.revokeObjectURL')||!thumbnail.includes('if (!live) return;'))fail('visible-thumbnail-lifecycle');
  for(const p of ['src/features/library/LibraryController.ts','src/features/library/LibraryPage.tsx','src/features/library/LibraryThumbnail.tsx'])if(/library\.(?:imageGet|mediaUrl)|services\/(?:tauriClient|ipc)|@tauri-apps|\binvoke\s*\(/.test(disk(root,p)))fail(`list-full-media-or-raw-transport:${p}`);
  const runs=disk(root,'src/features/runs/RunsPage.tsx');
  if(!runs.includes('查看完整任务历史')||!runs.includes('section: "advanced-tasks"')||/TaskHistory|taskHistoryPage/.test(runs))fail('existing-history-navigation');
  const targets=JSON.parse(disk(root,'docs/architecture/m3-2-library-targets.json'));
  if(!targets.recordedBeforeProductImplementation||targets.targets?.page20CumulativeListCallsMax!==20||targets.targets?.pageDomCardsMax!==30||targets.targets?.offscreenThumbnailRequests!==0||targets.targets?.fullMediaBytesForList!==0||targets.raiseTargetsAfterFailure!==false)fail('fixed-targets');
  for(const p of frozenArtifacts) if(disk(root,p)!==before(p))fail('immutable-artifact:'+p);
  violations.push(...visualCandidateViolations(JSON.parse(disk(root,'docs/architecture/m3-1-library-scale-baseline.json')),JSON.parse(disk(root,'docs/architecture/m3-2-library-scale-candidate.json'))));
  return { addedCommandSignatures: violations.length ? [] : [ADDED_COMMAND,...successor.addedCommandSignatures], violations, addedPaths: [...added,...successor.addedPaths], afterHashes: {...Object.fromEntries(Object.entries(review.paths ?? {}).map(([p, proof]) => [p, proof.afterHash])),...successor.afterHashes},
    backendAggregateSha256: violations.length ? undefined : successor.backendAggregateSha256 ?? review.backend.afterAggregateHash,
    read: p => violations.length || !existing.includes(p) ? successor.read(p) : before(p) };
}
