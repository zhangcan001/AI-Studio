import { cachedBoundary, immutableGit as execFileSync } from './boundary-validation-cache.mjs';
// Validate only the reviewed readiness repair before replaying immutable queue/source proofs.
import {createHash} from 'node:crypto';
import {readFileSync,readdirSync} from 'node:fs';
import {join} from 'node:path';
export const READINESS_LIFECYCLE_PARENT='7c82c8bf07dda09d47480d95300b11c401fd9be5';
export const POST_RUN_READINESS_PARENT='f869335c61d6aa1b5751b5f8b778694960b87970';
const postRunManifest='docs/architecture/readiness-post-run-fix.json';
const postRunPaths=['src/features/create/CreateController.ts','src/features/create/CreateReadinessLifecycle.test.tsx',
 'scripts/readiness-lifecycle-successor-guard.mjs','src/app/ReadinessLifecycleBoundary.test.ts',
 'src/app/QueueLifecycleRepairBoundary.test.ts','src/app/H3ReleaseCloseoutPhase1Boundary.test.ts',
 'src/app/H3GenerationRepairBoundary.test.ts','src/app/AIStudio21PublicationBoundary.test.ts'];
const manifest='docs/architecture/readiness-lifecycle-fix.json';
const existing=["src-tauri/src/application/comfy_preflight_service.rs","src-tauri/src/application/comfy_service.rs","src-tauri/src/application/diagnostics_service.rs","src/app/App.tsx","src/app/NormalProductPages.tsx","src/features/create/CreateController.ts","src/features/create/CreatePage.test.tsx","src/features/create/CreateResults.tsx","src/types/comfy.ts","scripts/queue-lifecycle-repair-successor-guard.mjs","src/app/QueueLifecycleRepairBoundary.test.ts","src/app/H3ReleaseCloseoutPhase1Boundary.test.ts","src/app/H3GenerationRepairBoundary.test.ts","src/app/AIStudio21PublicationBoundary.test.ts","src/features/runs/RunsPage.test.tsx","src/features/library/LibraryPage.test.tsx"];
const added=["src/app/useRuntimeStatusMonitor.ts","src/app/useRuntimeStatusMonitor.test.tsx","src/features/create/CreateReadinessLifecycle.test.tsx","scripts/readiness-lifecycle-successor-guard.mjs","src/app/ReadinessLifecycleBoundary.test.ts"];
export const READINESS_LIFECYCLE_GROUPS=[['backend','src-tauri/src',/\.rs$/],['tests','src-tauri/tests',/\.rs$/],
 ['frontend','src',/\.tsx?$/],['styles','src',/\.css$/],['scripts','scripts',/\.(mjs|ts)$/],
 ['migrations','src-tauri/migrations',/\.sql$/],['architecture','docs/architecture',/\.json$/],['packages','src-tauri/runtime_packages',/\.(yaml|json)$/]];
const configs=['package.json','pnpm-lock.yaml','src-tauri/Cargo.toml','src-tauri/Cargo.lock',
 'src-tauri/tauri.conf.json','src-tauri/build.rs','.github/workflows/ci.yml','README.md',
 'docs/AI_STUDIO_2_1_CLOSEOUT.md','docs/RELEASE_NOTES_v2.1.0-personal.md'];
const normalize=s=>s.replaceAll('\r\n','\n');
export const readinessLifecycleHash=s=>createHash('sha256').update(normalize(s)).digest('hex');
export const readinessLifecycleAggregate=(paths,read)=>readinessLifecycleHash(paths.map(p=>`${p}\n${read(p)}`).join('\n'));
const files=(root,dir,pattern)=>readdirSync(join(root,dir),{withFileTypes:true}).flatMap(e=>{
 const p=`${dir}/${e.name}`;return e.isDirectory()?files(root,p,pattern):pattern.test(p)?[p]:[];
}).sort();
const parents=new Map(),aggregates=new Map();
export function readinessLifecycleParentFacts(root,parent=READINESS_LIFECYCLE_PARENT){
 const key=JSON.stringify([root,parent]);if(parents.has(key))return parents.get(key);
 const paths=execFileSync('git',['ls-tree','-r','--name-only',parent],{cwd:root,encoding:'utf8'})
  .trim().split(/\r?\n/).filter(p=>configs.includes(p)||READINESS_LIFECYCLE_GROUPS.some(([,d,r])=>p.startsWith(d+'/')&&r.test(p))).sort();
 const bytes=execFileSync('git',['cat-file','--batch'],{cwd:root,input:paths.map(p=>`${parent}:${p}\n`).join(''),maxBuffer:64*1024*1024});
 const blobs=new Map();let offset=0;
 for(const p of paths){const end=bytes.indexOf(10,offset),m=/^[a-f0-9]+ blob (\d+)$/.exec(bytes.subarray(offset,end).toString());
  if(!m)throw Error('H3 parent blob missing');const start=end+1,length=Number(m[1]);
  if(bytes[start+length]!==10)throw Error('H3 parent blob boundary');blobs.set(p,normalize(bytes.subarray(start,start+length).toString('utf8')));offset=start+length+1;}
 if(offset!==bytes.length)throw Error('H3 parent trailing bytes');
 const result={paths,read:p=>blobs.get(p)};parents.set(key,result);return result;
}
function postRunReadinessParentReaderUncached(root,override){
 const live=new Map(),disk=p=>{if(!live.has(p))live.set(p,normalize(readFileSync(join(root,p),'utf8')));return live.get(p);};
 const violations=[],fail=s=>violations.push(`post-run-readiness-${s}`),rejected=()=>({violations,addedPaths:[],afterHashes:{},read:disk});
 let proof,facts;
 try{proof=override??JSON.parse(disk(postRunManifest));facts=readinessLifecycleParentFacts(root,POST_RUN_READINESS_PARENT);}catch{fail('missing-evidence');return rejected();}
 if(proof.schemaVersion!==1||proof.checkpoint!=='READINESS_POST_RUN_FIX'||proof.parentHead!==POST_RUN_READINESS_PARENT)fail('header');
 if(JSON.stringify(Object.keys(proof.paths??{}).sort())!==JSON.stringify([...postRunPaths].sort()))fail('scope');
 const invariants={schemaChanged:false,queueAuthorityChanged:false,backendChanged:false,immutablePackagesPreserved:true,
  runLifecycleFreshness:true,submitTimeAuthorityPreserved:true,readinessPollingAdded:false};
 if(Object.keys(proof.invariants??{}).length!==Object.keys(invariants).length||Object.entries(invariants).some(([k,v])=>proof.invariants?.[k]!==v))fail('repair-contract');
 if(violations.length)return rejected();
 for(const p of postRunPaths)if(proof.paths[p]?.beforeHash!==readinessLifecycleHash(facts.read(p))||proof.paths[p]?.afterHash!==readinessLifecycleHash(disk(p)))fail(`path:${p}`);
 for(const [name,dir,pattern] of READINESS_LIFECYCLE_GROUPS){
  const base=facts.paths.filter(p=>p.startsWith(dir+'/')&&pattern.test(p)),untouched=base.filter(p=>!postRunPaths.includes(p));
  const current=files(root,dir,pattern).filter(p=>p!==postRunManifest),r=proof[name];
  if(JSON.stringify(current)!==JSON.stringify(base)||r?.beforeFiles!==base.length||r?.afterFiles!==current.length)fail(`${name}-files`);
  if(untouched.some(p=>disk(p)!==facts.read(p)))fail(`${name}-untouched-bytes`);
  if(violations.length)continue;
  if(r?.beforeAggregateHash!==readinessLifecycleAggregate(base,facts.read)||r?.untouchedAggregateHash!==readinessLifecycleAggregate(untouched,facts.read)||r?.afterAggregateHash!==readinessLifecycleAggregate(current,disk))fail(`${name}-aggregate`);
 }
 for(const p of configs)if(disk(p)!==facts.read(p))fail(`frozen:${p}`);
 if(violations.length)return rejected();
 return {violations,addedPaths:[postRunManifest],backendAggregateSha256:proof.backend.afterAggregateHash,
  afterHashes:Object.fromEntries(postRunPaths.map(p=>[p,proof.paths[p].afterHash])),read:p=>postRunPaths.includes(p)?facts.read(p):disk(p)};
}
export function postRunReadinessParentReader(root,override){
 return cachedBoundary(root,'postRunReadinessParentReader',override,()=>postRunReadinessParentReaderUncached(root,override),postRunManifest);
}
function readinessLifecycleParentReaderUncached(root,override){
 const successor=postRunReadinessParentReader(root);
 const live=new Map(),disk=p=>{if(!live.has(p))live.set(p,normalize(successor.read(p)));return live.get(p);};
 const violations=[...successor.violations],fail=s=>violations.push(`readiness-lifecycle-${s}`),rejected=()=>({violations,addedPaths:[],afterHashes:{},read:p=>normalize(readFileSync(join(root,p),'utf8'))});
 let proof,facts;
 try{proof=override??JSON.parse(disk(manifest));facts=readinessLifecycleParentFacts(root);}catch{fail('missing-evidence');return rejected();}
 if(proof.schemaVersion!==1||proof.checkpoint!=='READINESS_LIFECYCLE_FIX'||proof.parentHead!==READINESS_LIFECYCLE_PARENT)fail('header');
 if(JSON.stringify(Object.keys(proof.paths??{}).sort())!==JSON.stringify([...existing,...added].sort()))fail('scope');
 for(const flag of ['schemaChanged','backupFormatChanged','queueAuthorityChanged','bindingOccChanged',
  'recognitionSemanticsChanged','replayFixturesChanged','releaseAssetsReplaced','remoteTelemetry'])if(proof.invariants?.[flag]!==false)fail(`invariant:${flag}`);
 if(proof.invariants?.runtimeChanged!==true||proof.invariants?.immutablePackagesPreserved!==true||proof.invariants?.sharedProductAdmission!==true||proof.invariants?.readinessLifecycleFreshness!==true||proof.invariants?.reusedBackendRuntimeGeneration!==true||proof.invariants?.submitTimeAuthorityPreserved!==true)fail('repair-contract');
 if(violations.length)return rejected();
 for(const p of existing)if(proof.paths[p]?.beforeHash!==readinessLifecycleHash(facts.read(p))||proof.paths[p]?.afterHash!==readinessLifecycleHash(disk(p)))fail(`path:${p}`);
 for(const p of added)if(facts.paths.includes(p)||proof.paths[p]?.beforeHash!==null||proof.paths[p]?.afterHash!==readinessLifecycleHash(disk(p)))fail(`addition:${p}`);
 for(const [name,dir,pattern] of READINESS_LIFECYCLE_GROUPS){
  const base=facts.paths.filter(p=>p.startsWith(dir+'/')&&pattern.test(p)),untouched=base.filter(p=>!existing.includes(p));
  const current=files(root,dir,pattern).filter(p=>p!==manifest&&!successor.addedPaths.includes(p)),expected=[...base,...added.filter(p=>p.startsWith(dir+'/')&&pattern.test(p))].sort();
  const r=proof[name];
  if(JSON.stringify(current)!==JSON.stringify(expected)||r?.beforeFiles!==base.length||r?.afterFiles!==current.length)fail(`${name}-files`);
  if(untouched.some(p=>disk(p)!==facts.read(p)))fail(`${name}-untouched-bytes`);
  const key=JSON.stringify([root,name,existing.concat(added).filter(p=>current.includes(p)).map(p=>[p,proof.paths[p].afterHash])]);
  // Cache only content-addressed aggregates after fresh byte/path validation.
  if(!violations.length&&!aggregates.has(key)){
   const before=readinessLifecycleAggregate(base,facts.read),unchanged=untouched.length===base.length&&JSON.stringify(current)===JSON.stringify(base);
   aggregates.set(key,{before,untouched:untouched.length===base.length?before:readinessLifecycleAggregate(untouched,facts.read),after:unchanged?before:readinessLifecycleAggregate(current,disk)});
  }
  const a=aggregates.get(key);
  if(!a||r?.beforeAggregateHash!==a.before||r?.untouchedAggregateHash!==a.untouched||r?.afterAggregateHash!==a.after)fail(`${name}-aggregate`);
 }
 for(const p of configs)if(disk(p)!==facts.read(p))fail(`frozen:${p}`);
 if(violations.length)return rejected();
 return {violations,addedPaths:[...added,manifest,...successor.addedPaths],backendAggregateSha256:successor.backendAggregateSha256,afterHashes:{...Object.fromEntries([...existing,...added].map(p=>[p,proof.paths[p].afterHash])),...successor.afterHashes},read:p=>existing.includes(p)?facts.read(p):disk(p)};
}

export function readinessLifecycleParentReader(root, override) {
  return cachedBoundary(root, "readinessLifecycleParentReader", override, () => readinessLifecycleParentReaderUncached(root, override), 'docs/architecture/readiness-lifecycle-fix.json');
}
