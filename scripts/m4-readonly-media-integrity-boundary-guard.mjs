import { cachedBoundary, immutableGit as execFileSync } from './boundary-validation-cache.mjs';
// M3-2 successor validates live bytes before historical guards read their parent.
// Historical manifests remain immutable; no authority or transport exemption.
import { createHash } from 'node:crypto';
import { readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';
import { closeoutParentReader } from './ai-studio-2-1-closeout-boundary-guard.mjs';

export const M4_MEDIA_PARENT = '7926f74ea3a77771d489fc834c93ed876d64163c';
const ADDED_COMMAND = {"name": "product_library_media_verify", "signature": "#[tauri::command(rename_all = \"camelCase\")] pub async fn product_library_media_verify( state: State<'_, AppState>, project_id: String, resource: ResourceRef, ) -> Result<crate::application::asset_query_service::MediaIntegrityReport, ProductError>"};
const existing = ["src-tauri/src/application/asset_query_service.rs", "src-tauri/src/application/media_probe.rs", "src-tauri/src/application/ports/asset_store.rs", "src-tauri/src/application/ports/mod.rs", "src-tauri/src/application/product/library_facade/detail.rs", "src-tauri/src/commands/product.rs", "src-tauri/src/infrastructure/filesystem/asset_store.rs", "src-tauri/src/lib.rs", "src-tauri/tests/product_library_contract.rs", "src/features/library/LibraryController.ts", "src/features/library/LibraryDetail.tsx", "src/features/library/LibraryPage.tsx", "src/product/client.ts", "src/product/client.test.ts", "src/product/errors.ts", "src/product/libraryTypes.ts", "src/product/transport.ts", "scripts/m3-bounded-visual-library-boundary-guard.mjs", "src/app/M3BoundedVisualLibraryBoundary.test.ts", "scripts/phase14-release-guard.mjs", "scripts/phase13-observability-guard.mjs", "src/app/BackendBoundary.test.ts"].sort();
const added = ["src-tauri/src/application/asset_query_service/media_integrity.rs", "src-tauri/tests/support/media_integrity_contract.rs", "src/features/library/MediaIntegrity.test.tsx", "src/features/library/MediaIntegrityPanel.tsx", "src/features/library/useLibraryMediaInspection.ts", "scripts/m4-readonly-media-integrity-boundary-guard.mjs", "src/app/M4ReadonlyMediaIntegrityBoundary.test.ts"].sort();
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
const frozenArtifacts = ["docs/architecture/m1-1-readiness.json", "docs/architecture/m1-2-overview-readiness.json", "docs/architecture/m1-3-settings-runtime.json", "docs/architecture/m2-1-run-recovery.json", "docs/architecture/m2-2-create-library-reuse.json", "docs/architecture/m3-1-library-findability.json", "docs/architecture/m3-2-bounded-visual-library.json", "docs/architecture/m3-1-library-scale-baseline.json", "docs/architecture/m3-1-library-scale-candidate.json", "docs/architecture/m3-2-library-scale-candidate.json", "docs/architecture/m3-2-library-targets.json"];
function parentBlobs(root, paths) {
  const cacheKey = JSON.stringify([root, paths]);
  if (immutable.has(cacheKey)) return immutable.get(cacheKey);
  const bytes = execFileSync('git', ['cat-file', '--batch'], { cwd: root,
    input: paths.map(p => `${M4_MEDIA_PARENT}:${p}\n`).join(''), maxBuffer: 64 * 1024 * 1024 });
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

function m4MediaParentReaderUncached(root, override) {
  // Memoize reads only within this synchronous validation; no live cache survives
  // a call or replaces a fresh fail-closed read. Parent blobs alone persist.
  const closeout = closeoutParentReader(root);
  const live = new Map();
  const disk = (_root,p) => { if (!live.has(p)) live.set(p,text(closeout.read(p))); return live.get(p); };
  const review = override ?? JSON.parse(disk(root, 'docs/architecture/m4-1-readonly-media-integrity.json'));
  const violations = [...closeout.violations], fail = s => violations.push(`m4-1-${s}`);
  if (review.checkpoint !== 'M4_1_READONLY_MEDIA_INTEGRITY' || review.parentHead !== M4_MEDIA_PARENT || review.schemaVersion !== 1) fail('invalid-header');
  if (JSON.stringify(Object.keys(review.paths ?? {}).sort()) !== JSON.stringify([...existing, ...added].sort())) fail('path-set');
  for (const flag of ["assetRepositoryAuthorityChanged", "assetStoreAuthorityChanged", "mediaRepairAuthorityAdded", "mediaRelinkAdded", "maintenancePersistenceAdded", "newMaintenanceStore", "newPollingOwner", "newBackgroundScanner", "queueAuthorityChanged", "taskStateMachineChanged", "workflowEngineChanged", "bindingOccChanged", "schemaChanged", "backupFormatChanged", "remoteTelemetry"]) if (review.invariants?.[flag] !== false) fail(`invariant:${flag}`);
  // Invalid proof shape/invariants fail closed without any historical projection.
  if (violations.length) return { violations, addedPaths: [...added,...closeout.addedPaths], addedCommandSignatures: [], afterHashes: {}, backendAggregateSha256: undefined, read: p => disk(root,p) };
  // Only immutable Git objects/file sets are reused. Every live byte and full
  // untouched aggregate is still read and validated anew on every invocation.
  if (!parentFileSets.has(root)) parentFileSets.set(root, execFileSync('git', ['ls-tree', '-r', '--name-only', M4_MEDIA_PARENT, 'src', 'src-tauri/src', 'scripts', 'src-tauri/tests/product_library_contract.rs', 'src-tauri/examples/phase12_query_profile.rs'],
    { cwd: root, encoding: 'utf8' }).trim().split(/\r?\n/).filter(p => /\.(tsx?|rs|css|mjs)$/.test(p)).sort());
  const listed = parentFileSets.get(root);
  const blobs = parentBlobs(root, [...listed,...frozenArtifacts]), before = p => blobs.get(p);
  for (const p of existing) if (review.paths?.[p]?.beforeHash !== hash(before(p)) || review.paths?.[p]?.afterHash !== hash(disk(root, p))) fail(`path-drift:${p}`);
  for (const p of added) if (blobs.has(p) || review.paths?.[p]?.beforeHash !== null || review.paths?.[p]?.afterHash !== hash(disk(root, p))) fail(`addition-drift:${p}`);
  for (const [name, folder, pattern] of [['backend', 'src-tauri/src', /\.rs$/], ['frontend', 'src', /\.tsx?$/], ['styles', 'src', /\.css$/], ['scripts', 'scripts', /\.mjs$/]]) {
    const base = listed.filter(p => p.startsWith(`${folder}/`) && pattern.test(p)), current = files(root, folder, pattern).filter(p=>!closeout.addedPaths.includes(p));
    const expected = [...base, ...added.filter(p => p.startsWith(`${folder}/`) && pattern.test(p))].sort();
    const unchanged = base.filter(p => !existing.includes(p));
    const proof = review[name];
    if (JSON.stringify(current) !== JSON.stringify(expected) || proof?.beforeFiles !== base.length || proof?.afterFiles !== current.length) fail(`${name}-file-set`);
    if (proof?.beforeAggregateHash !== aggregate(base, before) || proof?.afterAggregateHash !== aggregate(current, p => disk(root, p)) ||
      proof?.untouchedAggregateHash !== aggregate(unchanged, before) || proof?.untouchedAggregateHash !== aggregate(unchanged, p => disk(root, p))) fail(`${name}-aggregate`);
  }

  const commandSource=disk(root,'src-tauri/src/commands/product.rs');
  const signature=[...commandSource.matchAll(/#\[tauri::command[^\]]*\]\s*pub\s+async\s+fn\s+(\w+)[\s\S]*?(?=\{)/g)].find(m=>m[1]===ADDED_COMMAND.name);
  if(!signature||signature[0].replace(/\s+/g,' ').trim()!==ADDED_COMMAND.signature||!commandSource.includes('.media_verify(&project_id, &resource)'))fail('readonly-command');
  const verifier=disk(root,'src-tauri/src/application/asset_query_service/media_integrity.rs');
  if(!verifier.includes('.open_read_stream(')||!verifier.includes('.next_chunk()')||!verifier.includes('32 * 1024 * 1024')||!verifier.includes('1024 * 1024')||!verifier.includes('asset.sha256')||!verifier.includes('inspect_preview('))fail('bounded-read-contract');
  if(/\.(?:insert|delete|save|write_image|write_thumbnail|write_video|stage_for_delete|commit_staged_delete)\s*\(|asset_repository\s*\.\s*update\s*\(|sqlx|storage_path\s*=|sha256\s*=/.test(verifier))fail('verifier-write-dependency');
  const dto=verifier.slice(verifier.indexOf('pub struct MediaIntegrityReport'),verifier.indexOf('impl AssetQueryService'));
  if(/path|root|sha256|hash_value/i.test(dto)||!dto.includes('pub checksum: MediaChecksum')||!dto.includes('pub preview: MediaPreview'))fail('private-independent-facts');
  const port=disk(root,'src-tauri/src/application/ports/asset_store.rs'),fs=disk(root,'src-tauri/src/infrastructure/filesystem/asset_store.rs');
  if(!port.includes('enum AssetReadInspection')||!port.includes('fn resolve_asset_read_path')||!port.includes('std::io::ErrorKind::NotFound')||!fs.includes('inspect_asset_read_path(project_root, path)')||/\.contains\("(?:not found|missing|permission)/i.test(port))fail('shared-typed-boundary');
  const scan=disk(root,'src/features/library/useLibraryMediaInspection.ts');
  if(!scan.includes('SCAN_PAGE_SIZE = 20')||!scan.includes('SCAN_RESULT_LIMIT = 50')||!scan.includes('await productClient.library.mediaVerify')||!scan.includes('if (!owns() || stopped.current) break')||!scan.includes('singleEpoch.current === requestEpoch'))fail('explicit-bounded-scan');
  if(/setInterval|setTimeout|Promise\.all|zustand|@tauri-apps|\binvoke\s*\(|node:fs|library\.(?:delete|resourceEdit|useInCreation)|localStorage/.test(scan))fail('scan-authority-or-write');
  for(const p of ['src/features/library/MediaIntegrityPanel.tsx','src/features/library/useLibraryMediaInspection.ts'])if(/@tauri-apps|\binvoke\s*\(|node:fs|storagePath|canonicalPath|thumbnailPath|absolutePath/.test(disk(root,p)))fail('frontend-path-or-raw-transport:'+p);
  for(const p of frozenArtifacts)if(disk(root,p)!==before(p))fail('immutable-artifact:'+p);
  const migrations=files(root,'src-tauri/migrations',/\.sql$/);
  const prior=execFileSync('git',['ls-tree','-r','--name-only',M4_MEDIA_PARENT,'src-tauri/migrations'],{cwd:root,encoding:'utf8'}).trim().split(/\r?\n/).filter(p=>p.endsWith('.sql')).sort();
  if(JSON.stringify(migrations)!==JSON.stringify(prior)||migrations.some(p=>/\/043/.test(p)))fail('migration-drift');
  const oldMigrations=parentBlobs(root,prior);
  if(aggregate(migrations,p=>disk(root,p))!==aggregate(prior,p=>oldMigrations.get(p)))fail('migration-bytes');
  return { violations, addedCommandSignatures: violations.length?[]:[ADDED_COMMAND], addedPaths: [...added,...closeout.addedPaths],
    afterHashes: {...Object.fromEntries(Object.entries(review.paths??{}).map(([p,proof])=>[p,proof.afterHash])),...closeout.afterHashes},
    backendAggregateSha256: violations.length?undefined:closeout.backendAggregateSha256,
    read:p=>violations.length||!existing.includes(p)?disk(root,p):before(p) };
}

export function m4MediaParentReader(root, override) {
  return cachedBoundary(root, "m4MediaParentReader", override, () => m4MediaParentReaderUncached(root, override), 'docs/architecture/m4-1-readonly-media-integrity.json');
}
