import {m3VisualParentReader} from './m3-bounded-visual-library-boundary-guard.mjs';
// M3-1 successor validates live bytes before historical guards read their parent.
// Historical manifests remain immutable; no authority or transport exemption.
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';

export const M3_LIBRARY_PARENT = 'ca01163fc4359daae9a3aa9dc47a0f4a328dc353';
const ADDED_COMMAND = {"name": "product_library_tags_list", "signature": "#[tauri::command(rename_all = \"camelCase\")] pub async fn product_library_tags_list( state: State<'_, AppState>, project_id: String, ) -> Result<Vec<crate::application::ports::AssetTag>, ProductError>"};
const existing = ['src-tauri/src/application/product/library_facade/list.rs', 'src-tauri/src/application/product/library_facade/types.rs', 'src-tauri/src/commands/product.rs', 'src-tauri/src/lib.rs', 'src-tauri/tests/product_library_contract.rs', 'src/features/library/LibraryController.ts', 'src/features/library/LibraryPage.tsx', 'src/features/library/LibraryPage.test.tsx', 'src/product/client.test.ts', 'src/product/client.ts', 'src/product/libraryTypes.ts', 'src/product/transport.ts', 'scripts/m2-create-library-reuse-boundary-guard.mjs', 'src/app/M2CreateLibraryReuseBoundary.test.ts', 'src/app/BackendBoundary.test.ts', 'src-tauri/examples/phase12_query_profile.rs'].sort();
const added = ['scripts/m3-library-findability-boundary-guard.mjs', 'src/app/M3LibraryFindabilityBoundary.test.ts', 'src-tauri/tests/support/library_scale_baseline.rs', 'src-tauri/tests/support/library_findability_contract.rs', 'src/features/library/LibraryFindability.test.tsx', 'src/features/library/LibraryScale.observed.test.tsx'].sort();
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
    input: paths.map(p => `${M3_LIBRARY_PARENT}:${p}\n`).join(''), maxBuffer: 64 * 1024 * 1024 });
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

export function m3LibraryParentReader(root, override) {
  // Memoize reads only within this synchronous validation; no live cache survives
  // a call or replaces a fresh fail-closed read. Parent blobs alone persist.
  const successor = m3VisualParentReader(root);
  const projectedFiles = (root,folder,pattern) => files(root,folder,pattern).filter(p=>!successor.addedPaths.includes(p));
  const live = new Map();
  const disk = (_root,p) => { if (!live.has(p)) live.set(p,successor.read(p)); return live.get(p); };
  const review = override ?? JSON.parse(disk(root, 'docs/architecture/m3-1-library-findability.json'));
  const violations = [...successor.violations], fail = s => violations.push(`m3-1-${s}`);
  if (review.checkpoint !== 'M3_1_LIBRARY_FINDABILITY' || review.parentHead !== M3_LIBRARY_PARENT || review.schemaVersion !== 1) fail('invalid-header');
  if (JSON.stringify(Object.keys(review.paths ?? {}).sort()) !== JSON.stringify([...existing, ...added].sort())) fail('path-set');
  const listed = execFileSync('git', ['ls-tree', '-r', '--name-only', M3_LIBRARY_PARENT, 'src', 'src-tauri/src', 'scripts', 'src-tauri/tests/product_library_contract.rs', 'src-tauri/examples/phase12_query_profile.rs'],
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
  for (const flag of ['queueAuthorityChanged','taskStateMachineChanged','workflowEngineChanged','bindingOccChanged','draftAuthorityChanged','newReuseStore','libraryAuthorityChanged','assetBrowseAuthorityChanged','promptLibraryAuthorityChanged','newSearchAuthority','newCacheAuthority','newStore','newRouter','newPollingOwner','schemaChanged','backupFormatChanged','remoteTelemetry']) if (review.invariants?.[flag] !== false) fail(`invariant:${flag}`);
  const commandSource=disk(root,'src-tauri/src/commands/product.rs');
  const signature=[...commandSource.matchAll(/#\[tauri::command[^\]]*\]\s*pub\s+async\s+fn\s+(\w+)[\s\S]*?(?=\{)/g)].find(m=>m[1]===ADDED_COMMAND.name);
  if (!signature || signature[0].replace(/\s+/g,' ').trim()!==ADDED_COMMAND.signature || !commandSource.includes('.list_tags(&project_id)')) fail('tag-readonly-command');
  const list = disk(root,'src-tauri/src/application/product/library_facade/list.rs');
  if (!list.includes('cursor.favorite_only == favorite_only') || !list.includes('cursor.tag_id == tag_id') || !list.includes('return Err(invalid_query())')) fail('cursor-scope');
  const controller = disk(root,'src/features/library/LibraryController.ts');
  if (!controller.includes('productClient.library.tagsList') || !controller.includes('favoriteOnly,tagId:tagId||null,cursor,limit:30') || controller.includes('items.filter(')) fail('query-owner');
  for (const p of ['src/features/library/LibraryController.ts','src/features/library/LibraryPage.tsx']) if (/services\/(?:tauriClient|ipc)|@tauri-apps|\binvoke\s*\(|SELECT\s|createStore|setInterval/.test(disk(root,p).replace('setInterval(()=>void refresh(),5000)',''))) fail(`transport-owner:${p}`);
  const baselinePath='docs/architecture/m3-1-library-scale-baseline.json';
  const frozenBaseline=text(execFileSync('git',['show','d47216d67ecb2cb948cebd2a0ae113e52d07f135:'+baselinePath],{cwd:root,encoding:'utf8'}));
  if (disk(root,baselinePath)!==frozenBaseline) fail('baseline-budget-drift');
  return { addedCommandSignatures: violations.length ? [] : [ADDED_COMMAND,...successor.addedCommandSignatures], violations, addedPaths: [...added,...successor.addedPaths], afterHashes: {...Object.fromEntries(Object.entries(review.paths ?? {}).map(([p, proof]) => [p, proof.afterHash])),...successor.afterHashes},
    backendAggregateSha256: violations.length ? undefined : successor.backendAggregateSha256 ?? review.backend.afterAggregateHash,
    read: p => violations.length || !existing.includes(p) ? successor.read(p) : before(p) };
}
