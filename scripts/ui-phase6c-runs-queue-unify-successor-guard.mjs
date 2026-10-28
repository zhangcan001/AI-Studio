// Authorize Phase 6C unified Runs/Queue/RunRef projection before replaying Phase6B or older proofs.
// Historical manifests, Runtime Package bytes, migrations and BACKUP_VERSION stay immutable.
import { cachedBoundary, immutableGit, immutableGitStoreIdentity, immutableBlobBytes, immutableBlobText} from './boundary-validation-cache.mjs';
import { createHash } from 'node:crypto';
import { readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';
import { uiPhase6DParentReader, phase6DFixtureFiles } from './ui-phase6d-scale-integrity-successor-guard.mjs';

export const PHASE6C_PARENT = 'c05314c11b32183f8ec73527645348c2f249a8c2';
export const PHASE6C_MANIFEST = 'docs/architecture/ui-phase6c-runs-queue-unify-successor.json';
export const PHASE6C_EXISTING = [
  'scripts/ui-phase6b-library-shot-bind-successor-guard.mjs',
  'src-tauri/tests/support/run_workspace_contract.rs',
  'src/features/runs/RunDetail.tsx',
  'src/features/runs/RunsController.ts',
  'src/features/runs/RunsPage.tsx',
  'src/features/runs/runRecoveryPresentation.ts',
  'src/features/runs/runsModel.ts',
].sort();
export const PHASE6C_ADDED = [
  'scripts/ui-phase6c-runs-queue-unify-successor-guard.mjs',
  'src/app/UiPhase6CRunsQueueUnifyBoundary.test.ts',
  'src/features/runs/RunsPhase6C.test.tsx',
].sort();
export const PHASE6C_FILES = [...PHASE6C_EXISTING, ...PHASE6C_ADDED, PHASE6C_MANIFEST];
export const PHASE6C_INVARIANTS = {
  schemaChanged: false, backupFormatChanged: false, backendChanged: false,
  queueAuthorityChanged: false, storeAuthorityChanged: false, runtimePackagesChanged: false,
  historicalProofsRewritten: false, nestedSuccessStagingOnCommitOnly: true,
  runsQueueSameAuthority: true, runRefSourceIdIdentity: true,
  honestRecentCoverageNoMixedCursor: true, runCursorUnsupportedPreserved: true,
  explicitStartNonPreemptivePause: true, cancelPendingDoesNotKillInference: true,
  successSelectReviewIndependent: true, noNewRunQueuePersistentModel: true,
  migrationUnchanged: true, backupVersionUnchanged: true,
  phase5fThrough6bPreserved: true, rawRuntimeBytesPreserved: true, sourceOnly: true,
  nativeGpuExecuted: false, phase6Started: true,
};
export const PHASE6C_DATABASE = { migrationBefore: 44, migrationAfter: 44, newTables: 0, backupBefore: 21, backupAfter: 21 };
export const PHASE6C_GROUPS = [
  ['backend','src-tauri/src',/\.rs$/], ['tests','src-tauri/tests',/\.rs$/],
  ['frontend','src',/\.tsx?$/], ['styles','src',/\.css$/],
  ['scripts','scripts',/\.(mjs|ts)$/], ['migrations','src-tauri/migrations',/\.sql$/],
  ['architecture','docs/architecture',/\.json$/], ['packages','src-tauri/runtime_packages',/\.(yaml|json)$/],
  ['domainDocs','docs/architecture',/\.md$/],
];
const directories = [...new Set([...PHASE6C_GROUPS.map(([, dir]) => dir), 'src-tauri/examples','docs','.github'])];
const configs = ['CONTEXT.md','README.md','package.json','pnpm-lock.yaml','vite.config.ts',
  'src-tauri/Cargo.toml','src-tauri/Cargo.lock','src-tauri/tauri.conf.json','src-tauri/build.rs',
  '.github/workflows/ci.yml','docs/AI_STUDIO_2_1_CLOSEOUT.md','docs/RELEASE_NOTES_v2.1.0-personal.md'];
const normalize = text => text.replaceAll('\r\n','\n');
export const phase6CHash = text => createHash('sha256').update(normalize(text)).digest('hex');
const rawHash = bytes => createHash('sha256').update(bytes).digest('hex');
export const phase6CAggregate = (paths, read) => phase6CHash(paths.map(p => `${p}\n${read(p)}`).join('\n'));
// Immutable parent aggregates are pure functions of verified parent blobs.
const parentAggregates=new WeakMap();
const parentAggregate=(paths,parent)=>{let memo=parentAggregates.get(parent);if(!memo)parentAggregates.set(parent,memo=new Map());const key=paths.join('\n');if(!memo.has(key))memo.set(key,phase6CAggregate(paths,parent.read));return memo.get(key);};
const parents = new Map();
export function phase6CParentFacts(root) {
  // Immutable parent facts depend only on the verified object store, not the
  // fixture root; owned fixtures sharing one store share one parsed snapshot.
  const store = immutableGitStoreIdentity(root);
  if (parents.has(store)) return parents.get(store);
  const paths = immutableGit('git',['ls-tree','-r','--name-only',PHASE6C_PARENT],{cwd:root,encoding:'utf8'})
    .trim().split(/\r?\n/).filter(p => configs.includes(p) || directories.some(dir => p.startsWith(dir+'/'))) .sort();
  const bytes = immutableGit('git',['cat-file','--batch'],{
    cwd:root,input:paths.map(p => `${PHASE6C_PARENT}:${p}\n`).join(''),maxBuffer:64*1024*1024,
  });
  const blobs = new Map(), blobIds = new Map(); let offset = 0;
  for (const p of paths) {
    const end = bytes.indexOf(10,offset), match = /^([a-f0-9]{40}) blob (\d+)$/.exec(bytes.subarray(offset,end).toString());
    if (!match) throw Error('Phase6C immutable parent missing');
    const start = end+1, length = Number(match[2]);
    if (bytes[start+length] !== 10) throw Error('Phase6C immutable blob boundary');
    blobIds.set(p,match[1]);blobs.set(p,immutableBlobBytes(match[1],bytes.subarray(start,start+length)));offset=start+length+1;
  }
  if (offset !== bytes.length) throw Error('Phase6C immutable trailing bytes');
  const facts={paths,read:(texts=>p=>{if(!texts.has(p))texts.set(p,immutableBlobText(blobIds.get(p),()=>blobs.get(p)));return texts.get(p);})(new Map()),raw:p=>blobs.get(p)};
  parents.set(store,facts);return facts;
}
function files(root,dir) {
  return readdirSync(join(root,dir),{withFileTypes:true}).flatMap(entry=>{
    const p=`${dir}/${entry.name}`;
    if(entry.isSymbolicLink()) throw Error('Phase6C symbolic source');
    return entry.isDirectory()?files(root,p):[p];
  }).sort();
}
export const phase6CFixtureFiles = root => [...new Set([...phase6CParentFacts(root).paths,...PHASE6C_FILES,...phase6DFixtureFiles(root)])];
function validate(root,override) {
  const successor=uiPhase6DParentReader(root);
  const violations=[...successor.violations],fail=name=>violations.push('ui-phase6c-runs-queue-unify-'+name);
  const live=new Map(),read=p=>{
    if(!live.has(p))live.set(p,normalize(successor.read(p)));
    return live.get(p);
  };
  const denied=()=>({violations,addedPaths:[],afterHashes:{},read:p=>normalize(readFileSync(join(root,p),'utf8'))});
  let proof,parent;
  try {proof=override??JSON.parse(read(PHASE6C_MANIFEST));parent=phase6CParentFacts(root);}catch{fail('missing-evidence');return denied();}
  const scope=[...PHASE6C_EXISTING,...PHASE6C_ADDED].sort();
  const proofKeys=['schemaVersion','checkpoint','parentHead','planningParentHead','phaseCommitsBeforeCheckpoint',
    'invariants','database','paths',...PHASE6C_GROUPS.map(([name])=>name)].sort();
  if(JSON.stringify(Object.keys(proof).sort())!==JSON.stringify(proofKeys)
    ||read(PHASE6C_MANIFEST)!==JSON.stringify(JSON.parse(read(PHASE6C_MANIFEST)),null,2)+'\n')fail('proof-bytes');
  if(proof.schemaVersion!==1||proof.checkpoint!=='UI_PHASE6C_RUNS_QUEUE_UNIFY_SUCCESSOR'
    ||proof.parentHead!==PHASE6C_PARENT||proof.planningParentHead!==PHASE6C_PARENT
    ||JSON.stringify(proof.phaseCommitsBeforeCheckpoint)!=='[]')fail('header');
  if(JSON.stringify(Object.keys(proof.paths??{}).sort())!==JSON.stringify(scope))fail('scope');
  if(JSON.stringify(proof.invariants)!==JSON.stringify(PHASE6C_INVARIANTS)
    ||JSON.stringify(proof.database)!==JSON.stringify(PHASE6C_DATABASE))fail('contract');
  if(violations.length>successor.violations.length)return denied();
  try {
    for(const p of PHASE6C_EXISTING){
      if(!parent.paths.includes(p)||proof.paths[p]?.beforeHash!==phase6CHash(parent.read(p))
        ||proof.paths[p]?.afterHash!==phase6CHash(read(p)))fail('path:'+p);
    }
    for(const p of PHASE6C_ADDED){
      if(parent.paths.includes(p)||proof.paths[p]?.beforeHash!==null
        ||proof.paths[p]?.afterHash!==phase6CHash(read(p)))fail('addition:'+p);
    }
    for(const dir of directories){
      const expected=[...parent.paths.filter(p=>p.startsWith(dir+'/')),
        ...PHASE6C_ADDED.filter(p=>p.startsWith(dir+'/')),
        ...[PHASE6C_MANIFEST].filter(p=>p.startsWith(dir+'/')),
        ...successor.addedPaths.filter(p=>p.startsWith(dir+'/'))].sort();
      if(JSON.stringify(files(root,dir))!==JSON.stringify(expected))fail('inventory:'+dir);
    }
    for(const p of parent.paths.filter(p=>!PHASE6C_EXISTING.includes(p))){
      if(read(p)!==parent.read(p))fail('untouched:'+p);
      if(p.startsWith('src-tauri/migrations/')
        &&rawHash(readFileSync(join(root,p)))!==rawHash(parent.raw(p)))fail('migration-bytes:'+p);
    }
    for(const [name,dir,pattern] of PHASE6C_GROUPS){
      const base=parent.paths.filter(p=>p.startsWith(dir+'/')&&pattern.test(p));
      const current=[...base,...PHASE6C_ADDED.filter(p=>p.startsWith(dir+'/')&&pattern.test(p))].sort();
      const untouched=base.filter(p=>!PHASE6C_EXISTING.includes(p)),record=proof[name];
      if(record?.beforeFiles!==base.length||record?.afterFiles!==current.length
        ||record?.beforeAggregateHash!==parentAggregate(base,parent)
        ||record?.untouchedAggregateHash!==parentAggregate(untouched,parent)
        ||record?.afterAggregateHash!==phase6CAggregate(current,read))fail('aggregate:'+name);
    }
    const baselines=JSON.parse(parent.read('docs/architecture/minimax-video-phase2.json')).runtimeByteBaselines;
    const profile=process.env.AI_STUDIO_PHASE2_RUNTIME_BYTE_PROFILE??'git';
    const packages=parent.paths.filter(p=>p.startsWith('src-tauri/runtime_packages/'));
    const rawAggregate=readBytes=>rawHash(JSON.stringify(packages.map(p=>[p,rawHash(readBytes(p))])));
    if(!Object.hasOwn(baselines,profile)||rawAggregate(parent.raw)!==baselines.git
      ||rawAggregate(p=>readFileSync(join(root,p)))!==baselines[profile])fail('runtime-package-bytes');
    const model=read('src/features/runs/runsModel.ts');
    if(!model.includes('coverageMessage')||!model.includes('sourceLabels')||!model.includes('不提供完整混合运行历史分页'))fail('runs-model-coverage');
    const page=read('src/features/runs/RunsPage.tsx');
    if(!page.includes('coverageMessage')||!page.includes('sourceLabels')||page.includes('nextPage')||page.includes('loadMore'))fail('runs-page-honest');
    const detail=read('src/features/runs/RunDetail.tsx');
    if(!detail.includes('actionLabel')||!detail.includes('不抢占')||!detail.includes('不强制中断'))fail('runs-detail-semantics');
    const controller=read('src/features/runs/RunsController.ts');
    if(!controller.includes('list(route.projectId, filter, null)'))fail('runs-controller-no-cursor');
    const workspace=read('src-tauri/src/application/product/run_facade/workspace.rs');
    if(!workspace.includes('RUN_CURSOR_UNSUPPORTED')||!workspace.includes('RECENT_50_TASKS_AND_PRODUCTION_WITH_UNARCHIVED_QUEUES')
      ||!workspace.includes('next_cursor: None'))fail('workspace-honest-coverage');
    for(const frozen of [
      'docs/architecture/ui-phase6b-library-shot-bind-successor.json',
      'docs/architecture/ui-phase6a-library-source-successor.json',
      'docs/architecture/ui-phase5j-ci-per-file-worker-successor.json',
      'docs/architecture/m2-2-create-library-reuse.json',
    ]) if(read(frozen)!==parent.read(frozen))fail('frozen-proof:'+frozen);
  }catch{fail('missing-evidence');}
  if(violations.length)return denied();
  return {violations,addedPaths:[...PHASE6C_ADDED,PHASE6C_MANIFEST,...successor.addedPaths],
    afterHashes:{...Object.fromEntries(scope.map(p=>[p,proof.paths[p].afterHash])),...successor.afterHashes},
    read:p=>PHASE6C_EXISTING.includes(p)?parent.read(p):read(p)};
}
export function uiPhase6CParentReader(root,override) {
  return cachedBoundary(root,'uiPhase6CParentReader',override,()=>validate(root,override),PHASE6C_MANIFEST);
}
