// Authorize Phase 7A source-ready gate fixes before replaying Phase6D or older proofs.
// Historical manifests, Runtime Package bytes, migrations and BACKUP_VERSION stay immutable.
import { cachedBoundary, immutableGit } from './boundary-validation-cache.mjs';
import { createHash } from 'node:crypto';
import { readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';

export const PHASE7A_PARENT = '0d355a166d136e2148088f799f5a94b4c914f41e';
export const PHASE7A_MANIFEST = 'docs/architecture/ui-phase7a-source-ready-successor.json';
export const PHASE7A_EXISTING = [
  'package.json',
  'scripts/ui-phase6d-scale-integrity-successor-guard.mjs',
  'src-tauri/examples/phase12_query_profile.rs',
  'src-tauri/src/application/product/library_facade/types.rs',
  'src-tauri/src/application/source_asset_import_service.rs',
  'src-tauri/src/infrastructure/filesystem/app_data_dirs.rs',
  'src-tauri/tests/dev049_context_resolver.rs',
  'src-tauri/tests/dev050_readiness.rs',
  'src-tauri/tests/support/library_findability_contract.rs',
  'src-tauri/tests/support/library_scale_baseline.rs',
  'src-tauri/tests/support/run_workspace_contract.rs',
  'src/app/AIStudio21CloseoutBoundary.test.ts',
  'src/app/AIStudio21PublicationBoundary.test.ts',
  'src/app/BackendBoundary.test.ts',
  'src/app/FrontendConsolidation.test.tsx',
  'src/app/H3GenerationRepairBoundary.test.ts',
  'src/app/H3ReleaseCloseoutPhase1Boundary.test.ts',
  'src/app/M1OverviewReadinessBoundary.test.ts',
  'src/app/M1ReadinessBoundary.test.ts',
  'src/app/M1SettingsRuntimeBoundary.test.ts',
  'src/app/M2CreateLibraryNavigation.test.tsx',
  'src/app/M2CreateLibraryReuseBoundary.test.ts',
  'src/app/M2RunRecoveryBoundary.test.ts',
  'src/app/M3BoundedVisualLibraryBoundary.test.ts',
  'src/app/M3LibraryFindabilityBoundary.test.ts',
  'src/app/M4ReadonlyMediaIntegrityBoundary.test.ts',
  'src/app/MiniMaxVideoPhase1Boundary.test.ts',
  'src/app/MiniMaxVideoPhase2Boundary.test.ts',
  'src/app/MiniMaxVideoPhase3Boundary.test.ts',
  'src/app/MiniMaxVideoUiPhase3Boundary.test.ts',
  'src/app/MiniMaxVideoUiPhase4Boundary.test.ts',
  'src/app/NativeIpcObservation.test.ts',
  'src/app/PerformanceSuccessor.test.ts',
  'src/app/Phase7Retirement.test.tsx',
  'src/app/QueueLifecycleRepairBoundary.test.ts',
  'src/app/RCBackupAssetVersionRepairBoundary.test.ts',
  'src/app/ReadinessLifecycleBoundary.test.ts',
  'src/app/RuntimePerformanceHarness.test.ts',
  'src/app/StyleBoundary.test.ts',
  'src/app/StyleInventoryCache.test.ts',
  'src/app/UiPhase4NativeImportRepairBoundary.test.ts',
  'src/app/UiPhase4NativeUiBugfixBoundary.test.ts',
  'src/app/UiPhase4SerialQueueBoundary.test.ts',
  'src/app/UiPhase5AStoryboardBoundary.test.ts',
  'src/app/UiPhase5BMixedVideoBoundary.test.ts',
  'src/app/UiPhase5CReviewBoundary.test.ts',
  'src/app/UiPhase5CiRepairBoundary.test.ts',
  'src/app/UiPhase5DCiTimeoutBoundary.test.ts',
  'src/app/UiPhase5ECiShardBoundary.test.ts',
  'src/app/UiPhase5FCiMemoryBoundary.test.ts',
  'src/app/UiPhase5GCiTimeoutBoundary.test.ts',
  'src/app/UiPhase5HCiTimeoutBudgetBoundary.test.ts',
  'src/app/UiPhase5ICiWorkerTeardownBoundary.test.ts',
  'src/app/UiPhase5JCiPerFileWorkerBoundary.test.ts',
  'src/app/UiPhase6ALibrarySourceBoundary.test.ts',
  'src/app/UiPhase6BLibraryShotBindBoundary.test.ts',
  'src/app/UiPhase6CRunsQueueUnifyBoundary.test.ts',
  'src/app/UiPhase6DScaleIntegrityBoundary.test.ts',
  'src/app/v3/VideoShell.test.tsx',
  'src/features/create/CreateController.ts',
  'src/features/create/CreateLibraryBind.phase6b.test.tsx',
  'src/features/create/CreatePage.test.tsx',
  'src/features/create/CreatePage.tsx',
  'src/features/library/LibraryPage.test.tsx',
  'src/features/library/LibraryPagination.test.tsx',
  'src/features/library/LibraryScale.observed.test.tsx',
  'src/features/runs/RunsPage.test.tsx',
  'src/features/runs/RunsPhase6C.test.tsx',
  'src/features/workflow-lab/WorkflowLabBoundary.test.tsx'
].sort();
export const PHASE7A_ADDED = [
  'scripts/ui-phase7a-source-ready-successor-guard.mjs',
  'src/app/UiPhase7ASourceReadyBoundary.test.ts',
  'src/types/mjs-shims.d.ts',
  'scripts/run-boundary-tests.mjs'
].sort();
export const PHASE7A_FILES = [...PHASE7A_EXISTING, ...PHASE7A_ADDED, PHASE7A_MANIFEST];
export const PHASE7A_INVARIANTS = {
  schemaChanged: false, backupFormatChanged: false, backendChanged: false,
  queueAuthorityChanged: false, storeAuthorityChanged: false, runtimePackagesChanged: false,
  historicalProofsRewritten: false, nestedSuccessStagingOnCommitOnly: true,
  sourceGatesRepaired: true, tscNoEmitGreen: true, cargoFmtCheckGreen: true,
  cargoCheckAllTargetsGreen: true, vitestBoundaryJobRunnable: true,
  migrationUnchanged: true, backupVersionUnchanged: true,
  phase5fThrough6dPreserved: true, rawRuntimeBytesPreserved: true, sourceOnly: true,
  nativeGpuExecuted: false, phase6Started: true, phase6dCompleted: true, phase7aSourceReady: true,
};
export const PHASE7A_DATABASE = { migrationBefore: 44, migrationAfter: 44, newTables: 0, backupBefore: 21, backupAfter: 21 };
export const PHASE7A_GROUPS = [
  ['backend','src-tauri/src',/\.rs$/], ['tests','src-tauri/tests',/\.rs$/],
  ['frontend','src',/\.tsx?$/], ['styles','src',/\.css$/],
  ['scripts','scripts',/\.(mjs|ts)$/], ['migrations','src-tauri/migrations',/\.sql$/],
  ['architecture','docs/architecture',/\.json$/], ['packages','src-tauri/runtime_packages',/\.(yaml|json)$/],
  ['domainDocs','docs/architecture',/\.md$/],
];
const directories = [...new Set([...PHASE7A_GROUPS.map(([, dir]) => dir), 'src-tauri/examples','docs','.github'])];
const configs = ['CONTEXT.md','README.md','package.json','pnpm-lock.yaml','vite.config.ts',
  'src-tauri/Cargo.toml','src-tauri/Cargo.lock','src-tauri/tauri.conf.json','src-tauri/build.rs',
  '.github/workflows/ci.yml','docs/AI_STUDIO_2_1_CLOSEOUT.md','docs/RELEASE_NOTES_v2.1.0-personal.md'];
const normalize = text => text.replaceAll('\r\n','\n');
export const phase7AHash = text => createHash('sha256').update(normalize(text)).digest('hex');
const rawHash = bytes => createHash('sha256').update(bytes).digest('hex');
export const phase7AAggregate = (paths, read) => phase7AHash(paths.map(p => `${p}\n${read(p)}`).join('\n'));
const parents = new Map();
export function phase7AParentFacts(root) {
  if (parents.has(root)) return parents.get(root);
  const paths = immutableGit('git',['ls-tree','-r','--name-only',PHASE7A_PARENT],{cwd:root,encoding:'utf8'})
    .trim().split(/\r?\n/).filter(p => configs.includes(p) || directories.some(dir => p.startsWith(dir+'/'))) .sort();
  const bytes = immutableGit('git',['cat-file','--batch'],{
    cwd:root,input:paths.map(p => `${PHASE7A_PARENT}:${p}\n`).join(''),maxBuffer:64*1024*1024,
  });
  const blobs = new Map(); let offset = 0;
  for (const p of paths) {
    const end = bytes.indexOf(10,offset), match = /^([a-f0-9]{40}) blob (\d+)$/.exec(bytes.subarray(offset,end).toString());
    if (!match) throw Error('Phase7A immutable parent missing');
    const start = end+1, length = Number(match[2]);
    if (bytes[start+length] !== 10) throw Error('Phase7A immutable blob boundary');
    blobs.set(p,bytes.subarray(start,start+length));offset=start+length+1;
  }
  if (offset !== bytes.length) throw Error('Phase7A immutable trailing bytes');
  const facts={paths,read:p=>normalize(blobs.get(p).toString('utf8')),raw:p=>blobs.get(p)};
  parents.set(root,facts);return facts;
}
function files(root,dir) {
  return readdirSync(join(root,dir),{withFileTypes:true}).flatMap(entry=>{
    const p=`${dir}/${entry.name}`;
    if(entry.isSymbolicLink()) throw Error('Phase7A symbolic source');
    return entry.isDirectory()?files(root,p):[p];
  }).sort();
}
export const phase7AFixtureFiles = root => [...new Set([...phase7AParentFacts(root).paths,...PHASE7A_FILES])];
function validate(root,override) {
  const violations=[],fail=name=>violations.push('ui-phase7a-source-ready-'+name);
  const live=new Map(),read=p=>{
    if(!live.has(p))live.set(p,normalize(readFileSync(join(root,p),'utf8')));
    return live.get(p);
  };
  const denied=()=>({violations,addedPaths:[],afterHashes:{},read:p=>normalize(readFileSync(join(root,p),'utf8'))});
  let proof,parent;
  try {proof=override??JSON.parse(read(PHASE7A_MANIFEST));parent=phase7AParentFacts(root);}catch{fail('missing-evidence');return denied();}
  const scope=[...PHASE7A_EXISTING,...PHASE7A_ADDED].sort();
  const proofKeys=['schemaVersion','checkpoint','parentHead','planningParentHead','phaseCommitsBeforeCheckpoint',
    'invariants','database','paths',...PHASE7A_GROUPS.map(([name])=>name)].sort();
  if(JSON.stringify(Object.keys(proof).sort())!==JSON.stringify(proofKeys)
    ||read(PHASE7A_MANIFEST)!==JSON.stringify(JSON.parse(read(PHASE7A_MANIFEST)),null,2)+'\n')fail('proof-bytes');
  if(proof.schemaVersion!==1||proof.checkpoint!=='UI_PHASE7A_SOURCE_READY_SUCCESSOR'
    ||proof.parentHead!==PHASE7A_PARENT||proof.planningParentHead!==PHASE7A_PARENT
    ||JSON.stringify(proof.phaseCommitsBeforeCheckpoint)!=='[]')fail('header');
  if(JSON.stringify(Object.keys(proof.paths??{}).sort())!==JSON.stringify(scope))fail('scope');
  if(JSON.stringify(proof.invariants)!==JSON.stringify(PHASE7A_INVARIANTS)
    ||JSON.stringify(proof.database)!==JSON.stringify(PHASE7A_DATABASE))fail('contract');
  if(violations.length)return denied();
  try {
    for(const p of PHASE7A_EXISTING){
      if(!parent.paths.includes(p)||proof.paths[p]?.beforeHash!==phase7AHash(parent.read(p))
        ||proof.paths[p]?.afterHash!==phase7AHash(read(p)))fail('path:'+p);
    }
    for(const p of PHASE7A_ADDED){
      if(parent.paths.includes(p)||proof.paths[p]?.beforeHash!==null
        ||proof.paths[p]?.afterHash!==phase7AHash(read(p)))fail('addition:'+p);
    }
    for(const dir of directories){
      const expected=[...parent.paths.filter(p=>p.startsWith(dir+'/')),
        ...PHASE7A_ADDED.filter(p=>p.startsWith(dir+'/')),
        ...[PHASE7A_MANIFEST].filter(p=>p.startsWith(dir+'/'))].sort();
      if(JSON.stringify(files(root,dir))!==JSON.stringify(expected))fail('inventory:'+dir);
    }
    for(const p of parent.paths.filter(p=>!PHASE7A_EXISTING.includes(p))){
      if(read(p)!==parent.read(p))fail('untouched:'+p);
      if(p.startsWith('src-tauri/migrations/')
        &&rawHash(readFileSync(join(root,p)))!==rawHash(parent.raw(p)))fail('migration-bytes:'+p);
    }
    for(const [name,dir,pattern] of PHASE7A_GROUPS){
      const base=parent.paths.filter(p=>p.startsWith(dir+'/')&&pattern.test(p));
      const current=[...base,...PHASE7A_ADDED.filter(p=>p.startsWith(dir+'/')&&pattern.test(p))].sort();
      const untouched=base.filter(p=>!PHASE7A_EXISTING.includes(p)),record=proof[name];
      if(record?.beforeFiles!==base.length||record?.afterFiles!==current.length
        ||record?.beforeAggregateHash!==phase7AAggregate(base,parent.read)
        ||record?.untouchedAggregateHash!==phase7AAggregate(untouched,parent.read)
        ||record?.afterAggregateHash!==phase7AAggregate(current,read))fail('aggregate:'+name);
    }
    const baselines=JSON.parse(parent.read('docs/architecture/minimax-video-phase2.json')).runtimeByteBaselines;
    const profile=process.env.AI_STUDIO_PHASE2_RUNTIME_BYTE_PROFILE??'git';
    const packages=parent.paths.filter(p=>p.startsWith('src-tauri/runtime_packages/'));
    const rawAggregate=readBytes=>rawHash(JSON.stringify(packages.map(p=>[p,rawHash(readBytes(p))])));
    if(!Object.hasOwn(baselines,profile)||rawAggregate(parent.raw)!==baselines.git
      ||rawAggregate(p=>readFileSync(join(root,p)))!==baselines[profile])fail('runtime-package-bytes');
    const pkg=read('package.json');
    if(!pkg.includes('test:boundary')||!pkg.includes('run-boundary-tests.mjs'))fail('boundary-script');
    const shim=read('src/types/mjs-shims.d.ts');
    if(!shim.includes('declare module "*.mjs"')||!shim.includes('declare module "node:fs"'))fail('mjs-shims');
    for(const frozen of [
      'docs/architecture/ui-phase6d-scale-integrity-successor.json',
      'docs/architecture/ui-phase6c-runs-queue-unify-successor.json',
      'docs/architecture/ui-phase6b-library-shot-bind-successor.json',
      'docs/architecture/ui-phase6a-library-source-successor.json',
    ]) if(read(frozen)!==parent.read(frozen))fail('frozen-proof:'+frozen);
  }catch{fail('missing-evidence');}
  if(violations.length)return denied();
  return {violations,addedPaths:[...PHASE7A_ADDED,PHASE7A_MANIFEST],
    afterHashes:Object.fromEntries(scope.map(p=>[p,proof.paths[p].afterHash])),
    read:p=>PHASE7A_EXISTING.includes(p)?parent.read(p):read(p)};
}
export function uiPhase7AParentReader(root,override) {
  return cachedBoundary(root,'uiPhase7AParentReader',override,()=>validate(root,override),PHASE7A_MANIFEST);
}
