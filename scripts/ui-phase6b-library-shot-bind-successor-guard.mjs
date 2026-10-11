// Authorize Phase 6B Library→exact Shot video input binding before replaying Phase6A or older proofs.
// Historical manifests, Runtime Package bytes, migrations and BACKUP_VERSION stay immutable.
import { cachedBoundary, immutableGit } from './boundary-validation-cache.mjs';
import { createHash } from 'node:crypto';
import { readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';

export const PHASE6B_PARENT = '32048fd3213597572c807fdc0a3e9cfa19c52752';
export const PHASE6B_MANIFEST = 'docs/architecture/ui-phase6b-library-shot-bind-successor.json';
export const PHASE6B_EXISTING = [
  'scripts/ui-phase6a-library-source-successor-guard.mjs',
  'src-tauri/src/application/product/library_facade/relations.rs',
  'src-tauri/tests/product_library_contract.rs',
  'src/features/create/CreateController.ts',
  'src/features/create/CreatePage.tsx',
  'src/features/create/usePersistentVideoInputs.test.tsx',
  'src/features/create/usePersistentVideoInputs.ts',
  'src/features/library/LibraryDetail.tsx',
  'src/features/library/libraryModel.ts',
].sort();
export const PHASE6B_ADDED = [
  'scripts/ui-phase6b-library-shot-bind-successor-guard.mjs',
  'src/app/UiPhase6BLibraryShotBindBoundary.test.ts',
  'src/features/create/CreateLibraryBind.phase6b.test.tsx',
].sort();
export const PHASE6B_FILES = [...PHASE6B_EXISTING, ...PHASE6B_ADDED, PHASE6B_MANIFEST];
export const PHASE6B_INVARIANTS = {
  schemaChanged: false, backupFormatChanged: false, backendChanged: false,
  queueAuthorityChanged: false, storeAuthorityChanged: false, runtimePackagesChanged: false,
  historicalProofsRewritten: false, nestedSuccessStagingOnCommitOnly: true,
  libraryExactShotInputBind: true, persistentVideoInputOccReuse: true,
  generatedVideoReferenceConfirmOnly: true, generatedImageBlockedFromImageSlots: true,
  noTaskOrRunOnBind: true, noAutoFrameExtract: true, frozenBatchSnapshotsUntouched: true,
  deletePrecheckProtectsVideoInputBindings: true, libraryRelationsDeepLinkShotVideoInput: true,
  migrationUnchanged: true, backupVersionUnchanged: true,
  phase5fThrough6aPreserved: true, rawRuntimeBytesPreserved: true, sourceOnly: true,
  nativeGpuExecuted: false, phase6Started: true,
};
export const PHASE6B_DATABASE = { migrationBefore: 44, migrationAfter: 44, newTables: 0, backupBefore: 21, backupAfter: 21 };
export const PHASE6B_GROUPS = [
  ['backend','src-tauri/src',/\.rs$/], ['tests','src-tauri/tests',/\.rs$/],
  ['frontend','src',/\.tsx?$/], ['styles','src',/\.css$/],
  ['scripts','scripts',/\.(mjs|ts)$/], ['migrations','src-tauri/migrations',/\.sql$/],
  ['architecture','docs/architecture',/\.json$/], ['packages','src-tauri/runtime_packages',/\.(yaml|json)$/],
  ['domainDocs','docs/architecture',/\.md$/],
];
const directories = [...new Set([...PHASE6B_GROUPS.map(([, dir]) => dir), 'src-tauri/examples','docs','.github'])];
const configs = ['CONTEXT.md','README.md','package.json','pnpm-lock.yaml','vite.config.ts',
  'src-tauri/Cargo.toml','src-tauri/Cargo.lock','src-tauri/tauri.conf.json','src-tauri/build.rs',
  '.github/workflows/ci.yml','docs/AI_STUDIO_2_1_CLOSEOUT.md','docs/RELEASE_NOTES_v2.1.0-personal.md'];
const normalize = text => text.replaceAll('\r\n','\n');
export const phase6BHash = text => createHash('sha256').update(normalize(text)).digest('hex');
const rawHash = bytes => createHash('sha256').update(bytes).digest('hex');
export const phase6BAggregate = (paths, read) => phase6BHash(paths.map(p => `${p}\n${read(p)}`).join('\n'));
const parents = new Map();
export function phase6BParentFacts(root) {
  if (parents.has(root)) return parents.get(root);
  const paths = immutableGit('git',['ls-tree','-r','--name-only',PHASE6B_PARENT],{cwd:root,encoding:'utf8'})
    .trim().split(/\r?\n/).filter(p => configs.includes(p) || directories.some(dir => p.startsWith(dir+'/'))).sort();
  const bytes = immutableGit('git',['cat-file','--batch'],{
    cwd:root,input:paths.map(p => `${PHASE6B_PARENT}:${p}\n`).join(''),maxBuffer:64*1024*1024,
  });
  const blobs = new Map(); let offset = 0;
  for (const p of paths) {
    const end = bytes.indexOf(10,offset), match = /^([a-f0-9]{40}) blob (\d+)$/.exec(bytes.subarray(offset,end).toString());
    if (!match) throw Error('Phase6B immutable parent missing');
    const start = end+1, length = Number(match[2]);
    if (bytes[start+length] !== 10) throw Error('Phase6B immutable blob boundary');
    blobs.set(p,bytes.subarray(start,start+length));offset=start+length+1;
  }
  if (offset !== bytes.length) throw Error('Phase6B immutable trailing bytes');
  const facts={paths,read:p=>normalize(blobs.get(p).toString('utf8')),raw:p=>blobs.get(p)};
  parents.set(root,facts);return facts;
}
function files(root,dir) {
  return readdirSync(join(root,dir),{withFileTypes:true}).flatMap(entry=>{
    const p=`${dir}/${entry.name}`;
    if(entry.isSymbolicLink()) throw Error('Phase6B symbolic source');
    return entry.isDirectory()?files(root,p):[p];
  }).sort();
}
export const phase6BFixtureFiles = root => [...new Set([...phase6BParentFacts(root).paths,...PHASE6B_FILES])];
function validate(root,override) {
  const violations=[],fail=name=>violations.push('ui-phase6b-library-shot-bind-'+name);
  const live=new Map(),read=p=>{
    if(!live.has(p))live.set(p,normalize(readFileSync(join(root,p),'utf8')));
    return live.get(p);
  };
  const denied=()=>({violations,addedPaths:[],afterHashes:{},read:p=>normalize(readFileSync(join(root,p),'utf8'))});
  let proof,parent;
  try {proof=override??JSON.parse(read(PHASE6B_MANIFEST));parent=phase6BParentFacts(root);}catch{fail('missing-evidence');return denied();}
  const scope=[...PHASE6B_EXISTING,...PHASE6B_ADDED].sort();
  const proofKeys=['schemaVersion','checkpoint','parentHead','planningParentHead','phaseCommitsBeforeCheckpoint',
    'invariants','database','paths',...PHASE6B_GROUPS.map(([name])=>name)].sort();
  if(JSON.stringify(Object.keys(proof).sort())!==JSON.stringify(proofKeys)
    ||read(PHASE6B_MANIFEST)!==JSON.stringify(JSON.parse(read(PHASE6B_MANIFEST)),null,2)+'\n')fail('proof-bytes');
  if(proof.schemaVersion!==1||proof.checkpoint!=='UI_PHASE6B_LIBRARY_SHOT_BIND_SUCCESSOR'
    ||proof.parentHead!==PHASE6B_PARENT||proof.planningParentHead!==PHASE6B_PARENT
    ||JSON.stringify(proof.phaseCommitsBeforeCheckpoint)!=='[]')fail('header');
  if(JSON.stringify(Object.keys(proof.paths??{}).sort())!==JSON.stringify(scope))fail('scope');
  if(JSON.stringify(proof.invariants)!==JSON.stringify(PHASE6B_INVARIANTS)
    ||JSON.stringify(proof.database)!==JSON.stringify(PHASE6B_DATABASE))fail('contract');
  if(violations.length)return denied();
  try {
    for(const p of PHASE6B_EXISTING){
      if(!parent.paths.includes(p)||proof.paths[p]?.beforeHash!==phase6BHash(parent.read(p))
        ||proof.paths[p]?.afterHash!==phase6BHash(read(p)))fail('path:'+p);
    }
    for(const p of PHASE6B_ADDED){
      if(parent.paths.includes(p)||proof.paths[p]?.beforeHash!==null
        ||proof.paths[p]?.afterHash!==phase6BHash(read(p)))fail('addition:'+p);
    }
    for(const dir of directories){
      const expected=[...parent.paths.filter(p=>p.startsWith(dir+'/')),
        ...PHASE6B_ADDED.filter(p=>p.startsWith(dir+'/')),
        ...[PHASE6B_MANIFEST].filter(p=>p.startsWith(dir+'/'))].sort();
      if(JSON.stringify(files(root,dir))!==JSON.stringify(expected))fail('inventory:'+dir);
    }
    for(const p of parent.paths.filter(p=>!PHASE6B_EXISTING.includes(p))){
      if(read(p)!==parent.read(p))fail('untouched:'+p);
      if(p.startsWith('src-tauri/migrations/')
        &&rawHash(readFileSync(join(root,p)))!==rawHash(parent.raw(p)))fail('migration-bytes:'+p);
    }
    for(const [name,dir,pattern] of PHASE6B_GROUPS){
      const base=parent.paths.filter(p=>p.startsWith(dir+'/')&&pattern.test(p));
      const current=[...base,...PHASE6B_ADDED.filter(p=>p.startsWith(dir+'/')&&pattern.test(p))].sort();
      const untouched=base.filter(p=>!PHASE6B_EXISTING.includes(p)),record=proof[name];
      if(record?.beforeFiles!==base.length||record?.afterFiles!==current.length
        ||record?.beforeAggregateHash!==phase6BAggregate(base,parent.read)
        ||record?.untouchedAggregateHash!==phase6BAggregate(untouched,parent.read)
        ||record?.afterAggregateHash!==phase6BAggregate(current,read))fail('aggregate:'+name);
    }
    const baselines=JSON.parse(parent.read('docs/architecture/minimax-video-phase2.json')).runtimeByteBaselines;
    const profile=process.env.AI_STUDIO_PHASE2_RUNTIME_BYTE_PROFILE??'git';
    const packages=parent.paths.filter(p=>p.startsWith('src-tauri/runtime_packages/'));
    const rawAggregate=readBytes=>rawHash(JSON.stringify(packages.map(p=>[p,rawHash(readBytes(p))])));
    if(!Object.hasOwn(baselines,profile)||rawAggregate(parent.raw)!==baselines.git
      ||rawAggregate(p=>readFileSync(join(root,p)))!==baselines[profile])fail('runtime-package-bytes');
    const hook=read('src/features/create/usePersistentVideoInputs.ts');
    if(!hook.includes('async function bindAsset')||!hook.includes('inputsSave')
      ||hook.includes('generate('))fail('bind-asset-hook');
    const controller=read('src/features/create/CreateController.ts');
    if(!controller.includes('videoInputs.bindAsset')||!controller.includes('libraryNeedsGeneratedVideoConfirm')
      ||!controller.includes('setValue(field.key,mediaValue(field,[libraryIntent.assetId]))')
      ||/generate\(|referencesSet|setReferences/.test(controller.slice(controller.indexOf('async function applyLibraryAsset'),controller.indexOf('function setValue'))))fail('apply-bind-only');
    const page=read('src/features/create/CreatePage.tsx');
    if(!page.includes('确认将生成视频仅作为视频参考')||!page.includes('不会创建任务或启动生成'))fail('create-bind-ui');
    const relations=read('src-tauri/src/application/product/library_facade/relations.rs');
    if(!relations.includes('shot_video_input_ids')||!relations.includes('正式视频输入'))fail('relations-video-input');
    const detail=read('src/features/library/LibraryDetail.tsx');
    if(!detail.includes('不会自动抽帧作为首帧')||!detail.includes('用于视频创作'))fail('detail-reuse-copy');
    for(const frozen of [
      'docs/architecture/ui-phase6a-library-source-successor.json',
      'docs/architecture/ui-phase5j-ci-per-file-worker-successor.json',
      'docs/architecture/ui-phase5i-ci-worker-teardown-successor.json',
      'docs/architecture/m3-1-library-findability.json',
      'docs/architecture/m2-2-create-library-reuse.json',
    ]) if(read(frozen)!==parent.read(frozen))fail('frozen-proof:'+frozen);
  }catch{fail('missing-evidence');}
  if(violations.length)return denied();
  return {violations,addedPaths:[...PHASE6B_ADDED,PHASE6B_MANIFEST],
    afterHashes:Object.fromEntries(scope.map(p=>[p,proof.paths[p].afterHash])),
    read:p=>PHASE6B_EXISTING.includes(p)?parent.read(p):read(p)};
}
export function uiPhase6BParentReader(root,override) {
  return cachedBoundary(root,'uiPhase6BParentReader',override,()=>validate(root,override),PHASE6B_MANIFEST);
}
