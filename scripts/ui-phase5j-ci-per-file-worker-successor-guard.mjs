// Authorize the Phase5J per-file forks worker + dedicated Boundary job before replaying Phase5I or older proofs.
// Nothing in the historical manifests or Runtime Package bytes may be rewritten.
import { cachedBoundary, immutableGit } from './boundary-validation-cache.mjs';
import { createHash } from 'node:crypto';
import { readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';

export const PHASE5J_PARENT = '07d134bd437dfd49783e081f922063593bf3f110';
export const PHASE5J_MANIFEST = 'docs/architecture/ui-phase5j-ci-per-file-worker-successor.json';
export const PHASE5J_EXISTING = [
  '.github/workflows/ci.yml',
  'scripts/ui-phase5i-ci-worker-teardown-successor-guard.mjs',
  'src/app/UiPhase5ECiShardBoundary.test.ts',
  'src/app/UiPhase5ICiWorkerTeardownBoundary.test.ts',
  'vite.config.ts',
].sort();
export const PHASE5J_ADDED = [
  'scripts/ui-phase5j-ci-per-file-worker-successor-guard.mjs',
  'src/app/UiPhase5JCiPerFileWorkerBoundary.test.ts',
].sort();
export const PHASE5J_FILES = [...PHASE5J_EXISTING, ...PHASE5J_ADDED, PHASE5J_MANIFEST];
export const PHASE5J_INVARIANTS = {
  schemaChanged: false, backupFormatChanged: false, backendChanged: false,
  queueAuthorityChanged: false, storeAuthorityChanged: false, runtimePackagesChanged: false,
  historicalProofsRewritten: false, nestedSuccessStagingOnCommitOnly: true,
  readinessDriftProbesSplit: true, liveFreshnessProbesSplit: true,
  parentFailClosedCheckedOncePerFixture: true, windowsBoundedNinetySecondDriftProbes: true,
  allBoundarySuccessorProbesBudgetedAtNinetySeconds: true,
  vitestForksSingleForkTeardown: false, vitestForksPerFileWorker: true,
  fileParallelismDisabled: true, isolateWorkers: true, teardownTimeoutMs: 120000,
  frontendShardJobTimeoutMinutes: 35, dedicatedBoundaryJob: true,
  boundaryExcludedFromShards: true, frontendShardJobTimeoutMinutesBoundary: 90,
  thirtySecondBoundaryLimitPreserved: false, individualNegativeProbesPreserved: true,
  allFrontendTestsSharded: true, shardCount: 3, requiredFrontendSummaryPreserved: true,
  architectureTscBuildRequired: true, rawRuntimeBytesPreserved: true, sourceOnly: true,
  nativeGpuExecuted: false, phase6Started: false,
};
export const PHASE5J_DATABASE = { migrationBefore: 44, migrationAfter: 44, newTables: 0, backupBefore: 21, backupAfter: 21 };
export const PHASE5J_GROUPS = [
  ['backend','src-tauri/src',/\.rs$/], ['tests','src-tauri/tests',/\.rs$/],
  ['frontend','src',/\.tsx?$/], ['styles','src',/\.css$/],
  ['scripts','scripts',/\.(mjs|ts)$/], ['migrations','src-tauri/migrations',/\.sql$/],
  ['architecture','docs/architecture',/\.json$/], ['packages','src-tauri/runtime_packages',/\.(yaml|json)$/],
  ['domainDocs','docs/architecture',/\.md$/],
];
const directories = [...new Set([...PHASE5J_GROUPS.map(([, dir]) => dir), 'src-tauri/examples','docs','.github'])];
const configs = ['CONTEXT.md','README.md','package.json','pnpm-lock.yaml','vite.config.ts',
  'src-tauri/Cargo.toml','src-tauri/Cargo.lock','src-tauri/tauri.conf.json','src-tauri/build.rs',
  '.github/workflows/ci.yml','docs/AI_STUDIO_2_1_CLOSEOUT.md','docs/RELEASE_NOTES_v2.1.0-personal.md'];
const normalize = text => text.replaceAll('\r\n','\n');
export const phase5JHash = text => createHash('sha256').update(normalize(text)).digest('hex');
const rawHash = bytes => createHash('sha256').update(bytes).digest('hex');
export const phase5JAggregate = (paths, read) => phase5JHash(paths.map(p => `${p}\n${read(p)}`).join('\n'));
const parents = new Map();
export function phase5JParentFacts(root) {
  if (parents.has(root)) return parents.get(root);
  const paths = immutableGit('git',['ls-tree','-r','--name-only',PHASE5J_PARENT],{cwd:root,encoding:'utf8'})
    .trim().split(/\r?\n/).filter(p => configs.includes(p) || directories.some(dir => p.startsWith(dir+'/'))).sort();
  const bytes = immutableGit('git',['cat-file','--batch'],{
    cwd:root,input:paths.map(p => `${PHASE5J_PARENT}:${p}\n`).join(''),maxBuffer:64*1024*1024,
  });
  const blobs = new Map(); let offset = 0;
  for (const p of paths) {
    const end = bytes.indexOf(10,offset), match = /^([a-f0-9]{40}) blob (\d+)$/.exec(bytes.subarray(offset,end).toString());
    if (!match) throw Error('Phase5J immutable parent missing');
    const start = end+1, length = Number(match[2]);
    if (bytes[start+length] !== 10) throw Error('Phase5J immutable blob boundary');
    blobs.set(p,bytes.subarray(start,start+length));offset=start+length+1;
  }
  if (offset !== bytes.length) throw Error('Phase5J immutable trailing bytes');
  const facts={paths,read:p=>normalize(blobs.get(p).toString('utf8')),raw:p=>blobs.get(p)};
  parents.set(root,facts);return facts;
}
function files(root,dir) {
  return readdirSync(join(root,dir),{withFileTypes:true}).flatMap(entry=>{
    const p=`${dir}/${entry.name}`;
    if(entry.isSymbolicLink()) throw Error('Phase5J symbolic source');
    return entry.isDirectory()?files(root,p):[p];
  }).sort();
}
export const phase5JFixtureFiles = root => [...new Set([...phase5JParentFacts(root).paths,...PHASE5J_FILES])];
function validate(root,override) {
  const violations=[],fail=name=>violations.push('ui-phase5j-ci-per-file-worker-'+name);
  const live=new Map(),read=p=>{
    if(!live.has(p))live.set(p,normalize(readFileSync(join(root,p),'utf8')));
    return live.get(p);
  };
  const denied=()=>({violations,addedPaths:[],afterHashes:{},read:p=>normalize(readFileSync(join(root,p),'utf8'))});
  let proof,parent;
  try {proof=override??JSON.parse(read(PHASE5J_MANIFEST));parent=phase5JParentFacts(root);}catch{fail('missing-evidence');return denied();}
  const scope=[...PHASE5J_EXISTING,...PHASE5J_ADDED].sort();
  const proofKeys=['schemaVersion','checkpoint','parentHead','planningParentHead','phaseCommitsBeforeCheckpoint',
    'invariants','database','paths',...PHASE5J_GROUPS.map(([name])=>name)].sort();
  if(JSON.stringify(Object.keys(proof).sort())!==JSON.stringify(proofKeys)
    ||read(PHASE5J_MANIFEST)!==JSON.stringify(JSON.parse(read(PHASE5J_MANIFEST)),null,2)+'\n')fail('proof-bytes');
  if(proof.schemaVersion!==1||proof.checkpoint!=='UI_PHASE5J_CI_PER_FILE_WORKER_SUCCESSOR'
    ||proof.parentHead!==PHASE5J_PARENT||proof.planningParentHead!==PHASE5J_PARENT
    ||JSON.stringify(proof.phaseCommitsBeforeCheckpoint)!=='[]')fail('header');
  if(JSON.stringify(Object.keys(proof.paths??{}).sort())!==JSON.stringify(scope))fail('scope');
  if(JSON.stringify(proof.invariants)!==JSON.stringify(PHASE5J_INVARIANTS)
    ||JSON.stringify(proof.database)!==JSON.stringify(PHASE5J_DATABASE))fail('contract');
  if(violations.length)return denied();
  try {
    for(const p of PHASE5J_EXISTING){
      if(!parent.paths.includes(p)||proof.paths[p]?.beforeHash!==phase5JHash(parent.read(p))
        ||proof.paths[p]?.afterHash!==phase5JHash(read(p)))fail('path:'+p);
    }
    for(const p of PHASE5J_ADDED){
      if(parent.paths.includes(p)||proof.paths[p]?.beforeHash!==null
        ||proof.paths[p]?.afterHash!==phase5JHash(read(p)))fail('addition:'+p);
    }
    for(const dir of directories){
      const expected=[...parent.paths.filter(p=>p.startsWith(dir+'/')),
        ...PHASE5J_ADDED.filter(p=>p.startsWith(dir+'/')),
        ...[PHASE5J_MANIFEST].filter(p=>p.startsWith(dir+'/'))].sort();
      if(JSON.stringify(files(root,dir))!==JSON.stringify(expected))fail('inventory:'+dir);
    }
    for(const p of parent.paths.filter(p=>!PHASE5J_EXISTING.includes(p))){
      if(read(p)!==parent.read(p))fail('untouched:'+p);
      if(p.startsWith('src-tauri/migrations/')
        &&rawHash(readFileSync(join(root,p)))!==rawHash(parent.raw(p)))fail('migration-bytes:'+p);
    }
    for(const [name,dir,pattern] of PHASE5J_GROUPS){
      const base=parent.paths.filter(p=>p.startsWith(dir+'/')&&pattern.test(p));
      const current=[...base,...PHASE5J_ADDED.filter(p=>p.startsWith(dir+'/')&&pattern.test(p))].sort();
      const untouched=base.filter(p=>!PHASE5J_EXISTING.includes(p)),record=proof[name];
      if(record?.beforeFiles!==base.length||record?.afterFiles!==current.length
        ||record?.beforeAggregateHash!==phase5JAggregate(base,parent.read)
        ||record?.untouchedAggregateHash!==phase5JAggregate(untouched,parent.read)
        ||record?.afterAggregateHash!==phase5JAggregate(current,read))fail('aggregate:'+name);
    }
    const baselines=JSON.parse(parent.read('docs/architecture/minimax-video-phase2.json')).runtimeByteBaselines;
    const profile=process.env.AI_STUDIO_PHASE2_RUNTIME_BYTE_PROFILE??'git';
    const packages=parent.paths.filter(p=>p.startsWith('src-tauri/runtime_packages/'));
    const rawAggregate=readBytes=>rawHash(JSON.stringify(packages.map(p=>[p,rawHash(readBytes(p))])));
    if(!Object.hasOwn(baselines,profile)||rawAggregate(parent.raw)!==baselines.git
      ||rawAggregate(p=>readFileSync(join(root,p)))!==baselines[profile])fail('runtime-package-bytes');
  }catch{fail('missing-evidence');}
  if(violations.length)return denied();
  return {violations,addedPaths:[...PHASE5J_ADDED,PHASE5J_MANIFEST],
    afterHashes:Object.fromEntries(scope.map(p=>[p,proof.paths[p].afterHash])),
    read:p=>PHASE5J_EXISTING.includes(p)?parent.read(p):read(p)};
}
export function uiPhase5JParentReader(root,override) {
  return cachedBoundary(root,'uiPhase5JParentReader',override,()=>validate(root,override),PHASE5J_MANIFEST);
}
