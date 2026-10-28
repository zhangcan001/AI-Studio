// Authorize Phase 6A ordinary Library SOURCE/GENERATED filters before replaying Phase5J or older proofs.
// Historical manifests, Runtime Package bytes, migrations and BACKUP_VERSION stay immutable.
import { cachedBoundary, immutableGit, immutableGitStoreIdentity, immutableBlobBytes, immutableBlobText} from './boundary-validation-cache.mjs';
import { uiPhase6BParentReader, phase6BFixtureFiles } from './ui-phase6b-library-shot-bind-successor-guard.mjs';
import { createHash } from 'node:crypto';
import { readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';

export const PHASE6A_PARENT = '1f821da62e7137ab166703cf8c26aa7a4c8d80cd';
export const PHASE6A_MANIFEST = 'docs/architecture/ui-phase6a-library-source-successor.json';
export const PHASE6A_EXISTING = [
  'scripts/ui-phase5j-ci-per-file-worker-successor-guard.mjs',
  'src-tauri/src/application/product/library_facade/list.rs',
  'src-tauri/src/application/product/library_facade/types.rs',
  'src-tauri/tests/product_library_contract.rs',
  'src-tauri/tests/support/library_findability_contract.rs',
  'src/features/library/LibraryController.ts',
  'src/features/library/LibraryDetail.tsx',
  'src/features/library/LibraryFindability.test.tsx',
  'src/features/library/LibraryPage.test.tsx',
  'src/features/library/LibraryPage.tsx',
  'src/features/library/LibraryPagination.test.tsx',
  'src/features/library/libraryModel.ts',
  'src/product/client.test.ts',
  'src/product/libraryTypes.ts',
].sort();
export const PHASE6A_ADDED = [
  'scripts/ui-phase6a-library-source-successor-guard.mjs',
  'src/app/UiPhase6ALibrarySourceBoundary.test.ts',
].sort();
export const PHASE6A_FILES = [...PHASE6A_EXISTING, ...PHASE6A_ADDED, PHASE6A_MANIFEST];
export const PHASE6A_INVARIANTS = {
  schemaChanged: false, backupFormatChanged: false, backendChanged: false,
  queueAuthorityChanged: false, storeAuthorityChanged: false, runtimePackagesChanged: false,
  historicalProofsRewritten: false, nestedSuccessStagingOnCommitOnly: true,
  productLibrarySourceFilterWired: true, libraryCursorIncludesSourceKind: true,
  ordinaryLibraryBlocksNewImageGeneration: true, allCoverageRemainsRecentSummary: true,
  assetLibraryNotRewritten: true, migrationUnchanged: true, backupVersionUnchanged: true,
  phase5fThrough5jPreserved: true, vitestForksPerFileWorker: true, dedicatedBoundaryJob: true,
  rawRuntimeBytesPreserved: true, sourceOnly: true, nativeGpuExecuted: false, phase6Started: true,
};
export const PHASE6A_DATABASE = { migrationBefore: 44, migrationAfter: 44, newTables: 0, backupBefore: 21, backupAfter: 21 };
export const PHASE6A_GROUPS = [
  ['backend','src-tauri/src',/\.rs$/], ['tests','src-tauri/tests',/\.rs$/],
  ['frontend','src',/\.tsx?$/], ['styles','src',/\.css$/],
  ['scripts','scripts',/\.(mjs|ts)$/], ['migrations','src-tauri/migrations',/\.sql$/],
  ['architecture','docs/architecture',/\.json$/], ['packages','src-tauri/runtime_packages',/\.(yaml|json)$/],
  ['domainDocs','docs/architecture',/\.md$/],
];
const directories = [...new Set([...PHASE6A_GROUPS.map(([, dir]) => dir), 'src-tauri/examples','docs','.github'])];
const configs = ['CONTEXT.md','README.md','package.json','pnpm-lock.yaml','vite.config.ts',
  'src-tauri/Cargo.toml','src-tauri/Cargo.lock','src-tauri/tauri.conf.json','src-tauri/build.rs',
  '.github/workflows/ci.yml','docs/AI_STUDIO_2_1_CLOSEOUT.md','docs/RELEASE_NOTES_v2.1.0-personal.md'];
const normalize = text => text.replaceAll('\r\n','\n');
export const phase6AHash = text => createHash('sha256').update(normalize(text)).digest('hex');
const rawHash = bytes => createHash('sha256').update(bytes).digest('hex');
export const phase6AAggregate = (paths, read) => phase6AHash(paths.map(p => `${p}\n${read(p)}`).join('\n'));
// Immutable parent aggregates are pure functions of verified parent blobs.
const parentAggregates=new WeakMap();
const parentAggregate=(paths,parent)=>{let memo=parentAggregates.get(parent);if(!memo)parentAggregates.set(parent,memo=new Map());const key=paths.join('\n');if(!memo.has(key))memo.set(key,phase6AAggregate(paths,parent.read));return memo.get(key);};
const parents = new Map();
export function phase6AParentFacts(root) {
  // Immutable parent facts depend only on the verified object store, not the
  // fixture root; owned fixtures sharing one store share one parsed snapshot.
  const store = immutableGitStoreIdentity(root);
  if (parents.has(store)) return parents.get(store);
  const paths = immutableGit('git',['ls-tree','-r','--name-only',PHASE6A_PARENT],{cwd:root,encoding:'utf8'})
    .trim().split(/\r?\n/).filter(p => configs.includes(p) || directories.some(dir => p.startsWith(dir+'/'))).sort();
  const bytes = immutableGit('git',['cat-file','--batch'],{
    cwd:root,input:paths.map(p => `${PHASE6A_PARENT}:${p}\n`).join(''),maxBuffer:64*1024*1024,
  });
  const blobs = new Map(), blobIds = new Map(); let offset = 0;
  for (const p of paths) {
    const end = bytes.indexOf(10,offset), match = /^([a-f0-9]{40}) blob (\d+)$/.exec(bytes.subarray(offset,end).toString());
    if (!match) throw Error('Phase6A immutable parent missing');
    const start = end+1, length = Number(match[2]);
    if (bytes[start+length] !== 10) throw Error('Phase6A immutable blob boundary');
    blobIds.set(p,match[1]);blobs.set(p,immutableBlobBytes(match[1],bytes.subarray(start,start+length)));offset=start+length+1;
  }
  if (offset !== bytes.length) throw Error('Phase6A immutable trailing bytes');
  const facts={paths,read:(texts=>p=>{if(!texts.has(p))texts.set(p,immutableBlobText(blobIds.get(p),()=>blobs.get(p)));return texts.get(p);})(new Map()),raw:p=>blobs.get(p)};
  parents.set(store,facts);return facts;
}
function files(root,dir) {
  return readdirSync(join(root,dir),{withFileTypes:true}).flatMap(entry=>{
    const p=`${dir}/${entry.name}`;
    if(entry.isSymbolicLink()) throw Error('Phase6A symbolic source');
    return entry.isDirectory()?files(root,p):[p];
  }).sort();
}
export const phase6AFixtureFiles = root => [...new Set([...phase6AParentFacts(root).paths,...PHASE6A_FILES,...phase6BFixtureFiles(root)])];
function validate(root,override) {
  const successor=uiPhase6BParentReader(root);
  const violations=[...successor.violations],fail=name=>violations.push('ui-phase6a-library-source-'+name);
  const live=new Map(),read=p=>{
    if(!live.has(p))live.set(p,normalize(successor.read(p)));
    return live.get(p);
  };
  const denied=()=>({violations,addedPaths:[],afterHashes:{},read:p=>normalize(readFileSync(join(root,p),'utf8'))});
  let proof,parent;
  try {proof=override??JSON.parse(read(PHASE6A_MANIFEST));parent=phase6AParentFacts(root);}catch{fail('missing-evidence');return denied();}
  const scope=[...PHASE6A_EXISTING,...PHASE6A_ADDED].sort();
  const proofKeys=['schemaVersion','checkpoint','parentHead','planningParentHead','phaseCommitsBeforeCheckpoint',
    'invariants','database','paths',...PHASE6A_GROUPS.map(([name])=>name)].sort();
  if(JSON.stringify(Object.keys(proof).sort())!==JSON.stringify(proofKeys)
    ||read(PHASE6A_MANIFEST)!==JSON.stringify(JSON.parse(read(PHASE6A_MANIFEST)),null,2)+'\n')fail('proof-bytes');
  if(proof.schemaVersion!==1||proof.checkpoint!=='UI_PHASE6A_LIBRARY_SOURCE_SUCCESSOR'
    ||proof.parentHead!==PHASE6A_PARENT||proof.planningParentHead!==PHASE6A_PARENT
    ||JSON.stringify(proof.phaseCommitsBeforeCheckpoint)!=='[]')fail('header');
  if(JSON.stringify(Object.keys(proof.paths??{}).sort())!==JSON.stringify(scope))fail('scope');
  if(JSON.stringify(proof.invariants)!==JSON.stringify(PHASE6A_INVARIANTS)
    ||JSON.stringify(proof.database)!==JSON.stringify(PHASE6A_DATABASE))fail('contract');
  if(violations.length>successor.violations.length)return denied();
  try {
    for(const p of PHASE6A_EXISTING){
      if(!parent.paths.includes(p)||proof.paths[p]?.beforeHash!==phase6AHash(parent.read(p))
        ||proof.paths[p]?.afterHash!==phase6AHash(read(p)))fail('path:'+p);
    }
    for(const p of PHASE6A_ADDED){
      if(parent.paths.includes(p)||proof.paths[p]?.beforeHash!==null
        ||proof.paths[p]?.afterHash!==phase6AHash(read(p)))fail('addition:'+p);
    }
    for(const dir of directories){
      const expected=[...parent.paths.filter(p=>p.startsWith(dir+'/')),
        ...PHASE6A_ADDED.filter(p=>p.startsWith(dir+'/')),
        ...[PHASE6A_MANIFEST].filter(p=>p.startsWith(dir+'/')),
        ...successor.addedPaths.filter(p=>p.startsWith(dir+'/'))].sort();
      if(JSON.stringify(files(root,dir))!==JSON.stringify(expected))fail('inventory:'+dir);
    }
    for(const p of parent.paths.filter(p=>!PHASE6A_EXISTING.includes(p))){
      if(read(p)!==parent.read(p))fail('untouched:'+p);
      if(p.startsWith('src-tauri/migrations/')
        &&rawHash(readFileSync(join(root,p)))!==rawHash(parent.raw(p)))fail('migration-bytes:'+p);
    }
    for(const [name,dir,pattern] of PHASE6A_GROUPS){
      const base=parent.paths.filter(p=>p.startsWith(dir+'/')&&pattern.test(p));
      const current=[...base,...PHASE6A_ADDED.filter(p=>p.startsWith(dir+'/')&&pattern.test(p))].sort();
      const untouched=base.filter(p=>!PHASE6A_EXISTING.includes(p)),record=proof[name];
      if(record?.beforeFiles!==base.length||record?.afterFiles!==current.length
        ||record?.beforeAggregateHash!==parentAggregate(base,parent)
        ||record?.untouchedAggregateHash!==parentAggregate(untouched,parent)
        ||record?.afterAggregateHash!==phase6AAggregate(current,read))fail('aggregate:'+name);
    }
    const baselines=JSON.parse(parent.read('docs/architecture/minimax-video-phase2.json')).runtimeByteBaselines;
    const profile=process.env.AI_STUDIO_PHASE2_RUNTIME_BYTE_PROFILE??'git';
    const packages=parent.paths.filter(p=>p.startsWith('src-tauri/runtime_packages/'));
    const rawAggregate=readBytes=>rawHash(JSON.stringify(packages.map(p=>[p,rawHash(readBytes(p))])));
    if(!Object.hasOwn(baselines,profile)||rawAggregate(parent.raw)!==baselines.git
      ||rawAggregate(p=>readFileSync(join(root,p)))!==baselines[profile])fail('runtime-package-bytes');
    const list=read('src-tauri/src/application/product/library_facade/list.rs');
    if(!list.includes('cursor.source_kind == source_kind')||!list.includes('LibrarySourceKind::Source')
      ||!list.includes('AssetSourceFilter::Source')
      ||!list.includes('match source_kind {'))fail('list-source-wiring');
    const types=read('src-tauri/src/application/product/library_facade/types.rs');
    if(!types.includes('enum LibrarySourceKind')||!types.includes('pub source_kind: LibrarySourceKind'))fail('types-source-kind');
    const controller=read('src/features/library/LibraryController.ts');
    if(!controller.includes('sourceKind,cursor,limit:30')||controller.includes('items.filter('))fail('controller-source');
    if(!controller.includes('!returnRoute && stage==="image"'))fail('controller-a08');
    const detail=read('src/features/library/LibraryDetail.tsx');
    if(detail.includes('用于图片创作')||!detail.includes('用于视频创作')||!detail.includes('历史图片可预览与审计'))fail('detail-a08');
    const page=read('src/features/library/LibraryPage.tsx');
    if(!page.includes('素材来源')||!page.includes('sourceKind'))fail('page-source-ui');
    const advanced=read('src/features/assets/AssetLibrary.tsx');
    if(advanced!==parent.read('src/features/assets/AssetLibrary.tsx'))fail('asset-library-rewritten');
    for(const frozen of [
      'docs/architecture/ui-phase5f-ci-memory-successor.json',
      'docs/architecture/ui-phase5g-ci-timeout-successor.json',
      'docs/architecture/ui-phase5h-ci-timeout-budget-successor.json',
      'docs/architecture/ui-phase5i-ci-worker-teardown-successor.json',
      'docs/architecture/ui-phase5j-ci-per-file-worker-successor.json',
      'docs/architecture/m3-1-library-findability.json',
      'docs/architecture/m4-1-readonly-media-integrity.json',
    ]) if(read(frozen)!==parent.read(frozen))fail('frozen-proof:'+frozen);
  }catch{fail('missing-evidence');}
  if(violations.length)return denied();
  return {violations,addedPaths:[...PHASE6A_ADDED,PHASE6A_MANIFEST,...successor.addedPaths],
    afterHashes:{...Object.fromEntries(scope.map(p=>[p,proof.paths[p].afterHash])),...successor.afterHashes},
    read:p=>PHASE6A_EXISTING.includes(p)?parent.read(p):read(p)};
}
export function uiPhase6AParentReader(root,override) {
  return cachedBoundary(root,'uiPhase6AParentReader',override,()=>validate(root,override),PHASE6A_MANIFEST);
}
