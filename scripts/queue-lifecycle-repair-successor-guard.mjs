import { cachedBoundary, immutableGit as execFileSync } from './boundary-validation-cache.mjs';
import { readinessLifecycleParentReader } from './readiness-lifecycle-successor-guard.mjs';
// Validate the exact queue repair before projecting the immutable source checkpoint.
import {createHash} from 'node:crypto';
import {readFileSync,readdirSync} from 'node:fs';
import {join} from 'node:path';
export const QUEUE_LIFECYCLE_PARENT='21904bc8c7d833cb6d287ad11cfbc4237893acf5';
const manifest='docs/architecture/queue-lifecycle-repair.json';
const existing=["src-tauri/src/application/dev027_e2e.rs", "src-tauri/src/application/shot_batch_service.rs", "src-tauri/src/application/generation_service.rs", "src-tauri/src/application/task_execution_registry.rs", "src-tauri/src/application/task_recovery_service.rs", "src-tauri/src/application/production_queue_service.rs", "src-tauri/src/infrastructure/database/repositories/production_queue.rs", "src-tauri/src/lib.rs", "src-tauri/tests/dev061b_queue_recovery.rs", "scripts/h3-release-closeout-phase1-guard.mjs", "src/app/H3ReleaseCloseoutPhase1Boundary.test.ts", "src/app/H3GenerationRepairBoundary.test.ts", "src/app/AIStudio21PublicationBoundary.test.ts"];
const added=["scripts/queue-lifecycle-repair-successor-guard.mjs", "src/app/QueueLifecycleRepairBoundary.test.ts", "src-tauri/tests/support/queue_lifecycle_regressions.rs"];
export const QUEUE_LIFECYCLE_GROUPS=[['backend','src-tauri/src',/\.rs$/],['tests','src-tauri/tests',/\.rs$/],
 ['frontend','src',/\.tsx?$/],['styles','src',/\.css$/],['scripts','scripts',/\.(mjs|ts)$/],
 ['migrations','src-tauri/migrations',/\.sql$/],['architecture','docs/architecture',/\.json$/],['packages','src-tauri/runtime_packages',/\.(yaml|json)$/]];
const configs=['package.json','pnpm-lock.yaml','src-tauri/Cargo.toml','src-tauri/Cargo.lock',
 'src-tauri/tauri.conf.json','src-tauri/build.rs','.github/workflows/ci.yml','README.md',
 'docs/AI_STUDIO_2_1_CLOSEOUT.md','docs/RELEASE_NOTES_v2.1.0-personal.md'];
const normalize=s=>s.replaceAll('\r\n','\n');
export const queueLifecycleHash=s=>createHash('sha256').update(normalize(s)).digest('hex');
export const queueLifecycleAggregate=(paths,read)=>queueLifecycleHash(paths.map(p=>`${p}\n${read(p)}`).join('\n'));
const files=(root,dir,pattern)=>readdirSync(join(root,dir),{withFileTypes:true}).flatMap(e=>{
 const p=`${dir}/${e.name}`;return e.isDirectory()?files(root,p,pattern):pattern.test(p)?[p]:[];
}).sort();
const parents=new Map(),aggregates=new Map();
export function queueLifecycleParentFacts(root){
 if(parents.has(root))return parents.get(root);
 const paths=execFileSync('git',['ls-tree','-r','--name-only',QUEUE_LIFECYCLE_PARENT],{cwd:root,encoding:'utf8'})
  .trim().split(/\r?\n/).filter(p=>configs.includes(p)||QUEUE_LIFECYCLE_GROUPS.some(([,d,r])=>p.startsWith(d+'/')&&r.test(p))).sort();
 const bytes=execFileSync('git',['cat-file','--batch'],{cwd:root,input:paths.map(p=>`${QUEUE_LIFECYCLE_PARENT}:${p}\n`).join(''),maxBuffer:64*1024*1024});
 const blobs=new Map();let offset=0;
 for(const p of paths){const end=bytes.indexOf(10,offset),m=/^[a-f0-9]+ blob (\d+)$/.exec(bytes.subarray(offset,end).toString());
  if(!m)throw Error('H3 parent blob missing');const start=end+1,length=Number(m[1]);
  if(bytes[start+length]!==10)throw Error('H3 parent blob boundary');blobs.set(p,normalize(bytes.subarray(start,start+length).toString('utf8')));offset=start+length+1;}
 if(offset!==bytes.length)throw Error('H3 parent trailing bytes');
 const result={paths,read:p=>blobs.get(p)};parents.set(root,result);return result;
}
function queueLifecycleParentReaderUncached(root,override){
 const successor=readinessLifecycleParentReader(root);
 const live=new Map(),disk=p=>{if(!live.has(p))live.set(p,normalize(successor.read(p)));return live.get(p);};
 const violations=[...successor.violations],fail=s=>violations.push(`queue-lifecycle-${s}`),rejected=()=>({violations,addedPaths:[],afterHashes:{},read:p=>normalize(readFileSync(join(root,p),'utf8'))});
 let proof,facts;
 try{proof=override??JSON.parse(disk(manifest));facts=queueLifecycleParentFacts(root);}catch{fail('missing-evidence');return rejected();}
 if(proof.schemaVersion!==1||proof.checkpoint!=='QUEUE_LIFECYCLE_REPAIR'||proof.parentHead!==QUEUE_LIFECYCLE_PARENT)fail('header');
 if(JSON.stringify(Object.keys(proof.paths??{}).sort())!==JSON.stringify([...existing,...added].sort()))fail('scope');
 for(const flag of ['schemaChanged','backupFormatChanged','queueAuthorityChanged','bindingOccChanged',
  'recognitionSemanticsChanged','replayFixturesChanged','releaseAssetsReplaced','remoteTelemetry'])if(proof.invariants?.[flag]!==false)fail(`invariant:${flag}`);
 if(proof.invariants?.runtimeChanged!==true||proof.invariants?.immutablePackagesPreserved!==true||proof.invariants?.sharedProductAdmission!==true||proof.invariants?.liveExecutionRecoveryIsolation!==true||proof.invariants?.atomicPauseDispatch!==true||proof.invariants?.losslessWorkerResume!==true)fail('repair-contract');
 if(violations.length)return rejected();
 for(const p of existing)if(proof.paths[p]?.beforeHash!==queueLifecycleHash(facts.read(p))||proof.paths[p]?.afterHash!==queueLifecycleHash(disk(p)))fail(`path:${p}`);
 for(const p of added)if(facts.paths.includes(p)||proof.paths[p]?.beforeHash!==null||proof.paths[p]?.afterHash!==queueLifecycleHash(disk(p)))fail(`addition:${p}`);
 for(const [name,dir,pattern] of QUEUE_LIFECYCLE_GROUPS){
  const base=facts.paths.filter(p=>p.startsWith(dir+'/')&&pattern.test(p)),untouched=base.filter(p=>!existing.includes(p));
  const current=files(root,dir,pattern).filter(p=>p!==manifest&&!successor.addedPaths.includes(p)),expected=[...base,...added.filter(p=>p.startsWith(dir+'/')&&pattern.test(p))].sort();
  const r=proof[name];
  if(JSON.stringify(current)!==JSON.stringify(expected)||r?.beforeFiles!==base.length||r?.afterFiles!==current.length)fail(`${name}-files`);
  if(untouched.some(p=>disk(p)!==facts.read(p)))fail(`${name}-untouched-bytes`);
  const key=JSON.stringify([root,name,existing.concat(added).filter(p=>current.includes(p)).map(p=>[p,proof.paths[p].afterHash])]);
  // Cache only content-addressed aggregates after fresh byte/path validation.
  if(!violations.length&&!aggregates.has(key)){
   const before=queueLifecycleAggregate(base,facts.read),unchanged=untouched.length===base.length&&JSON.stringify(current)===JSON.stringify(base);
   aggregates.set(key,{before,untouched:untouched.length===base.length?before:queueLifecycleAggregate(untouched,facts.read),after:unchanged?before:queueLifecycleAggregate(current,disk)});
  }
  const a=aggregates.get(key);
  if(!a||r?.beforeAggregateHash!==a.before||r?.untouchedAggregateHash!==a.untouched||r?.afterAggregateHash!==a.after)fail(`${name}-aggregate`);
 }
 for(const p of configs)if(disk(p)!==facts.read(p))fail(`frozen:${p}`);
 if(violations.length)return rejected();
 return {violations,addedPaths:[...added,manifest,...successor.addedPaths],backendAggregateSha256:successor.backendAggregateSha256,afterHashes:{...Object.fromEntries([...existing,...added].map(p=>[p,proof.paths[p].afterHash])),...successor.afterHashes},read:p=>existing.includes(p)?facts.read(p):disk(p)};
}

export function queueLifecycleParentReader(root, override) {
  return cachedBoundary(root, "queueLifecycleParentReader", override, () => queueLifecycleParentReaderUncached(root, override), 'docs/architecture/queue-lifecycle-repair.json');
}
