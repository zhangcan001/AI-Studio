// Authorize the Phase5E CI performance fix before replaying Phase5C or older proofs.
// Nothing in the historical manifests or Runtime Package bytes may be rewritten.
import { cachedBoundary, immutableGit } from './boundary-validation-cache.mjs';
import { uiPhase5FParentReader, phase5FFixtureFiles } from './ui-phase5f-ci-memory-successor-guard.mjs';
import { createHash } from 'node:crypto';
import { readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';

export const PHASE5E_PARENT = '1434008310d450921266b85e1f86f3abeb106a16';
export const PHASE5E_MANIFEST = 'docs/architecture/ui-phase5e-ci-shard-successor.json';
export const PHASE5E_EXISTING = ['.github/workflows/ci.yml','scripts/ui-phase5d-ci-timeout-successor-guard.mjs','src/app/AIStudio21PublicationBoundary.test.ts','src/app/MiniMaxVideoPhase1Boundary.test.ts','src/app/MiniMaxVideoPhase2Boundary.test.ts','src/app/MiniMaxVideoUiPhase4Boundary.test.ts','src/app/UiPhase5AStoryboardBoundary.test.ts','scripts/testing/boundary-validation-cache.test.ts'].sort();
export const PHASE5E_ADDED = ['scripts/ui-phase5e-ci-shard-successor-guard.mjs','src/app/UiPhase5ECiShardBoundary.test.ts'].sort();
export const PHASE5E_FILES = [...PHASE5E_EXISTING, ...PHASE5E_ADDED, PHASE5E_MANIFEST];
export const PHASE5E_INVARIANTS = {schemaChanged:false,backupFormatChanged:false,backendChanged:false,queueAuthorityChanged:false,storeAuthorityChanged:false,runtimePackagesChanged:false,historicalProofsRewritten:false,allFrontendTestsSharded:true,shardCount:3,requiredFrontendSummaryPreserved:true,architectureTscBuildRequired:true,individualNegativeProbesPreserved:true,thirtySecondBoundaryLimitPreserved:true,legacyCacheGuardDeadlineExplicit:true,rawRuntimeBytesPreserved:true,sourceOnly:true,nativeGpuExecuted:false,phase6Started:false};
export const PHASE5E_DATABASE = { migrationBefore: 44, migrationAfter: 44, newTables: 0, backupBefore: 21, backupAfter: 21 };
export const PHASE5E_GROUPS = [
  ['backend','src-tauri/src',/\.rs$/], ['tests','src-tauri/tests',/\.rs$/],
  ['frontend','src',/\.tsx?$/], ['styles','src',/\.css$/],
  ['scripts','scripts',/\.(mjs|ts)$/], ['migrations','src-tauri/migrations',/\.sql$/],
  ['architecture','docs/architecture',/\.json$/], ['packages','src-tauri/runtime_packages',/\.(yaml|json)$/],
  ['domainDocs','docs/architecture',/\.md$/],
];
const directories = [...new Set([...PHASE5E_GROUPS.map(([, dir]) => dir), 'src-tauri/examples','docs','.github'])];
const configs = ['CONTEXT.md','README.md','package.json','pnpm-lock.yaml','vite.config.ts',
  'src-tauri/Cargo.toml','src-tauri/Cargo.lock','src-tauri/tauri.conf.json','src-tauri/build.rs',
  '.github/workflows/ci.yml','docs/AI_STUDIO_2_1_CLOSEOUT.md','docs/RELEASE_NOTES_v2.1.0-personal.md'];
const normalize = text => text.replaceAll('\r\n','\n');
export const phase5EHash = text => createHash('sha256').update(normalize(text)).digest('hex');
const rawHash = bytes => createHash('sha256').update(bytes).digest('hex');
export const phase5EAggregate = (paths, read) => phase5EHash(paths.map(p => `${p}\n${read(p)}`).join('\n'));
const parents = new Map();
export function phase5EParentFacts(root) {
  if (parents.has(root)) return parents.get(root);
  const paths = immutableGit('git',['ls-tree','-r','--name-only',PHASE5E_PARENT],{cwd:root,encoding:'utf8'})
    .trim().split(/\r?\n/).filter(p => configs.includes(p) || directories.some(dir => p.startsWith(dir+'/'))).sort();
  const bytes = immutableGit('git',['cat-file','--batch'],{
    cwd:root,input:paths.map(p => `${PHASE5E_PARENT}:${p}\n`).join(''),maxBuffer:64*1024*1024,
  });
  const blobs = new Map(); let offset = 0;
  for (const p of paths) {
    const end = bytes.indexOf(10,offset), match = /^([a-f0-9]{40}) blob (\d+)$/.exec(bytes.subarray(offset,end).toString());
    if (!match) throw Error('Phase5E immutable parent missing');
    const start = end+1, length = Number(match[2]);
    if (bytes[start+length] !== 10) throw Error('Phase5E immutable blob boundary');
    blobs.set(p,bytes.subarray(start,start+length));offset=start+length+1;
  }
  if (offset !== bytes.length) throw Error('Phase5E immutable trailing bytes');
  const facts={paths,read:p=>normalize(blobs.get(p).toString('utf8')),raw:p=>blobs.get(p)};
  parents.set(root,facts);return facts;
}
function files(root,dir) {
  return readdirSync(join(root,dir),{withFileTypes:true}).flatMap(entry=>{
    const p=`${dir}/${entry.name}`;
    if(entry.isSymbolicLink()) throw Error('Phase5E symbolic source');
    return entry.isDirectory()?files(root,p):[p];
  }).sort();
}
export const phase5EFixtureFiles = root => [...new Set([...phase5EParentFacts(root).paths,...PHASE5E_FILES,...phase5FFixtureFiles(root)])];
function validate(root,override) {
  const successor=uiPhase5FParentReader(root);
  const violations=[...successor.violations],fail=name=>violations.push('ui-phase5e-ci-shard-'+name);
  const live=new Map(),read=p=>{
    if(!live.has(p))live.set(p,normalize(successor.read(p)));
    return live.get(p);
  };
  const denied=()=>({violations,addedPaths:[],afterHashes:{},read:p=>normalize(readFileSync(join(root,p),'utf8'))});
  let proof,parent;
  try {proof=override??JSON.parse(read(PHASE5E_MANIFEST));parent=phase5EParentFacts(root);}catch{fail('missing-evidence');return denied();}
  const scope=[...PHASE5E_EXISTING,...PHASE5E_ADDED].sort();
  const proofKeys=['schemaVersion','checkpoint','parentHead','planningParentHead','phaseCommitsBeforeCheckpoint',
    'invariants','database','paths',...PHASE5E_GROUPS.map(([name])=>name)].sort();
  if(JSON.stringify(Object.keys(proof).sort())!==JSON.stringify(proofKeys)
    ||read(PHASE5E_MANIFEST)!==JSON.stringify(JSON.parse(read(PHASE5E_MANIFEST)),null,2)+'\n')fail('proof-bytes');
  if(proof.schemaVersion!==1||proof.checkpoint!=='UI_PHASE5E_CI_SHARD_SUCCESSOR'
    ||proof.parentHead!==PHASE5E_PARENT||proof.planningParentHead!==PHASE5E_PARENT
    ||JSON.stringify(proof.phaseCommitsBeforeCheckpoint)!=='[]')fail('header');
  if(JSON.stringify(Object.keys(proof.paths??{}).sort())!==JSON.stringify(scope))fail('scope');
  if(JSON.stringify(proof.invariants)!==JSON.stringify(PHASE5E_INVARIANTS)
    ||JSON.stringify(proof.database)!==JSON.stringify(PHASE5E_DATABASE))fail('contract');
  if(violations.length>successor.violations.length)return denied();
  try {
    for(const p of PHASE5E_EXISTING){
      if(!parent.paths.includes(p)||proof.paths[p]?.beforeHash!==phase5EHash(parent.read(p))
        ||proof.paths[p]?.afterHash!==phase5EHash(read(p)))fail('path:'+p);
    }
    for(const p of PHASE5E_ADDED){
      if(parent.paths.includes(p)||proof.paths[p]?.beforeHash!==null
        ||proof.paths[p]?.afterHash!==phase5EHash(read(p)))fail('addition:'+p);
    }
    for(const dir of directories){
      const expected=[...parent.paths.filter(p=>p.startsWith(dir+'/')),
        ...PHASE5E_ADDED.filter(p=>p.startsWith(dir+'/')),
        ...[PHASE5E_MANIFEST].filter(p=>p.startsWith(dir+'/')),
        ...successor.addedPaths.filter(p=>p.startsWith(dir+'/'))].sort();
      if(JSON.stringify(files(root,dir))!==JSON.stringify(expected))fail('inventory:'+dir);
    }
    for(const p of parent.paths.filter(p=>!PHASE5E_EXISTING.includes(p))){
      if(read(p)!==parent.read(p))fail('untouched:'+p);
      if(p.startsWith('src-tauri/migrations/')
        &&rawHash(readFileSync(join(root,p)))!==rawHash(parent.raw(p)))fail('migration-bytes:'+p);
    }
    for(const [name,dir,pattern] of PHASE5E_GROUPS){
      const base=parent.paths.filter(p=>p.startsWith(dir+'/')&&pattern.test(p));
      const current=[...base,...PHASE5E_ADDED.filter(p=>p.startsWith(dir+'/')&&pattern.test(p))].sort();
      const untouched=base.filter(p=>!PHASE5E_EXISTING.includes(p)),record=proof[name];
      if(record?.beforeFiles!==base.length||record?.afterFiles!==current.length
        ||record?.beforeAggregateHash!==phase5EAggregate(base,parent.read)
        ||record?.untouchedAggregateHash!==phase5EAggregate(untouched,parent.read)
        ||record?.afterAggregateHash!==phase5EAggregate(current,read))fail('aggregate:'+name);
    }
    const baselines=JSON.parse(parent.read('docs/architecture/minimax-video-phase2.json')).runtimeByteBaselines;
    const profile=process.env.AI_STUDIO_PHASE2_RUNTIME_BYTE_PROFILE??'git';
    const packages=parent.paths.filter(p=>p.startsWith('src-tauri/runtime_packages/'));
    const rawAggregate=readBytes=>rawHash(JSON.stringify(packages.map(p=>[p,rawHash(readBytes(p))])));
    if(!Object.hasOwn(baselines,profile)||rawAggregate(parent.raw)!==baselines.git
      ||rawAggregate(p=>readFileSync(join(root,p)))!==baselines[profile])fail('runtime-package-bytes');
  }catch{fail('missing-evidence');}
  if(violations.length)return denied();
  return {violations,addedPaths:[...PHASE5E_ADDED,PHASE5E_MANIFEST,...successor.addedPaths],
    afterHashes:{...Object.fromEntries(scope.map(p=>[p,proof.paths[p].afterHash])),...successor.afterHashes},
    read:p=>PHASE5E_EXISTING.includes(p)?parent.read(p):read(p)};
}
export function uiPhase5EParentReader(root,override) {
  return cachedBoundary(root,'uiPhase5EParentReader',override,()=>validate(root,override),PHASE5E_MANIFEST);
}
