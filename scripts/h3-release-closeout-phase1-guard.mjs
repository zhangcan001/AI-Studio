import { cachedBoundary, immutableGit as execFileSync } from './boundary-validation-cache.mjs';
// Post-release runtime repair: validate current scope before replaying immutable history.
import {createHash} from 'node:crypto';
import {readFileSync,readdirSync} from 'node:fs';
import {join} from 'node:path';
export const H3_CLOSEOUT_PARENT='5dcc9c63e3718818e976b039f77ca4f11a83527f';
const manifest='docs/architecture/h3-release-closeout-phase1.json';
const existing=["src-tauri/tests/dev082_unified_workflow_library.rs", "src-tauri/tests/dev059_production_package.rs", "scripts/2-1-publication-successor-guard.mjs", "scripts/2-1-rc-backup-asset-version-repair-guard.mjs", "scripts/ai-studio-2-1-closeout-boundary-guard.mjs", "scripts/h3-generation-repair-successor-guard.mjs", "scripts/m1-overview-readiness-boundary-guard.mjs", "scripts/m1-readiness-boundary-guard.mjs", "scripts/m1-settings-runtime-boundary-guard.mjs", "scripts/m2-create-library-reuse-boundary-guard.mjs", "scripts/m2-run-recovery-boundary-guard.mjs", "scripts/m3-bounded-visual-library-boundary-guard.mjs", "scripts/m3-library-findability-boundary-guard.mjs", "scripts/m4-readonly-media-integrity-boundary-guard.mjs", "scripts/phase14-release-guard.mjs", "src-tauri/src/application/builtin_runtime_packages.rs", "src-tauri/src/application/generation_input_preparer.rs", "src-tauri/src/application/generation_service.rs", "src-tauri/src/application/product/creation_facade/submission.rs", "src-tauri/src/application/product/h3_resolution.rs", "src-tauri/src/infrastructure/database/repositories/workflow_library.rs", "src/app/AIStudio21PublicationBoundary.test.ts", "src/app/H3GenerationRepairBoundary.test.ts", "src/features/create/CreateController.ts", "src/features/create/CreatePage.test.tsx"];
const added=["scripts/boundary-validation-cache.mjs", "scripts/h3-release-closeout-phase1-guard.mjs", "src-tauri/runtime_packages/minimax_h3_fl2va_first_last_quality_2_2_0/manifest.yaml", "src-tauri/runtime_packages/minimax_h3_fl2va_first_last_quality_2_2_0/recipe.yaml", "src-tauri/runtime_packages/minimax_h3_fl2va_first_last_quality_2_2_0/workflow_api.json", "src-tauri/runtime_packages/minimax_h3_fl2va_i2v_quality_2_2_0/manifest.yaml", "src-tauri/runtime_packages/minimax_h3_fl2va_i2v_quality_2_2_0/recipe.yaml", "src-tauri/runtime_packages/minimax_h3_fl2va_i2v_quality_2_2_0/workflow_api.json", "src-tauri/runtime_packages/minimax_h3_fl2va_t2v_quality_2_2_0/manifest.yaml", "src-tauri/runtime_packages/minimax_h3_fl2va_t2v_quality_2_2_0/recipe.yaml", "src-tauri/runtime_packages/minimax_h3_fl2va_t2v_quality_2_2_0/workflow_api.json", "src-tauri/runtime_packages/minimax_h3_reference_video_quality_2_2_0/manifest.yaml", "src-tauri/runtime_packages/minimax_h3_reference_video_quality_2_2_0/recipe.yaml", "src-tauri/runtime_packages/minimax_h3_reference_video_quality_2_2_0/workflow_api.json", "src/app/H3ReleaseCloseoutPhase1Boundary.test.ts"];
export const H3_CLOSEOUT_GROUPS=[['backend','src-tauri/src',/\.rs$/],['tests','src-tauri/tests',/\.rs$/],
 ['frontend','src',/\.tsx?$/],['styles','src',/\.css$/],['scripts','scripts',/\.(mjs|ts)$/],
 ['migrations','src-tauri/migrations',/\.sql$/],['architecture','docs/architecture',/\.json$/],['packages','src-tauri/runtime_packages',/\.(yaml|json)$/]];
const configs=['package.json','pnpm-lock.yaml','src-tauri/Cargo.toml','src-tauri/Cargo.lock',
 'src-tauri/tauri.conf.json','src-tauri/build.rs','.github/workflows/ci.yml','README.md',
 'docs/AI_STUDIO_2_1_CLOSEOUT.md','docs/RELEASE_NOTES_v2.1.0-personal.md'];
const normalize=s=>s.replaceAll('\r\n','\n');
export const h3CloseoutHash=s=>createHash('sha256').update(normalize(s)).digest('hex');
export const h3CloseoutAggregate=(paths,read)=>h3CloseoutHash(paths.map(p=>`${p}\n${read(p)}`).join('\n'));
const files=(root,dir,pattern)=>readdirSync(join(root,dir),{withFileTypes:true}).flatMap(e=>{
 const p=`${dir}/${e.name}`;return e.isDirectory()?files(root,p,pattern):pattern.test(p)?[p]:[];
}).sort();
const parents=new Map(),aggregates=new Map();
export function h3CloseoutParentFacts(root){
 if(parents.has(root))return parents.get(root);
 const paths=execFileSync('git',['ls-tree','-r','--name-only',H3_CLOSEOUT_PARENT],{cwd:root,encoding:'utf8'})
  .trim().split(/\r?\n/).filter(p=>configs.includes(p)||H3_CLOSEOUT_GROUPS.some(([,d,r])=>p.startsWith(d+'/')&&r.test(p))).sort();
 const bytes=execFileSync('git',['cat-file','--batch'],{cwd:root,input:paths.map(p=>`${H3_CLOSEOUT_PARENT}:${p}\n`).join(''),maxBuffer:64*1024*1024});
 const blobs=new Map();let offset=0;
 for(const p of paths){const end=bytes.indexOf(10,offset),m=/^[a-f0-9]+ blob (\d+)$/.exec(bytes.subarray(offset,end).toString());
  if(!m)throw Error('H3 parent blob missing');const start=end+1,length=Number(m[1]);
  if(bytes[start+length]!==10)throw Error('H3 parent blob boundary');blobs.set(p,normalize(bytes.subarray(start,start+length).toString('utf8')));offset=start+length+1;}
 if(offset!==bytes.length)throw Error('H3 parent trailing bytes');
 const result={paths,read:p=>blobs.get(p)};parents.set(root,result);return result;
}
function h3CloseoutParentReaderUncached(root,override){
 const live=new Map(),disk=p=>{if(!live.has(p))live.set(p,normalize(readFileSync(join(root,p),'utf8')));return live.get(p);};
 const violations=[],fail=s=>violations.push(`h3-closeout-${s}`),rejected=()=>({violations,addedPaths:[],afterHashes:{},read:disk});
 let proof,facts;
 try{proof=override??JSON.parse(disk(manifest));facts=h3CloseoutParentFacts(root);}catch{fail('missing-evidence');return rejected();}
 if(proof.schemaVersion!==1||proof.checkpoint!=='H3_RELEASE_CLOSEOUT_PHASE1'||proof.parentHead!==H3_CLOSEOUT_PARENT)fail('header');
 if(JSON.stringify(Object.keys(proof.paths??{}).sort())!==JSON.stringify([...existing,...added].sort()))fail('scope');
 for(const flag of ['schemaChanged','backupFormatChanged','queueAuthorityChanged','bindingOccChanged',
  'recognitionSemanticsChanged','replayFixturesChanged','releaseAssetsReplaced','remoteTelemetry'])if(proof.invariants?.[flag]!==false)fail(`invariant:${flag}`);
 if(proof.invariants?.runtimeChanged!==true||proof.invariants?.immutablePackagesPreserved!==true||proof.invariants?.sharedProductAdmission!==true)fail('repair-contract');
 if(violations.length)return rejected();
 for(const p of existing)if(proof.paths[p]?.beforeHash!==h3CloseoutHash(facts.read(p))||proof.paths[p]?.afterHash!==h3CloseoutHash(disk(p)))fail(`path:${p}`);
 for(const p of added)if(facts.paths.includes(p)||proof.paths[p]?.beforeHash!==null||proof.paths[p]?.afterHash!==h3CloseoutHash(disk(p)))fail(`addition:${p}`);
 for(const [name,dir,pattern] of H3_CLOSEOUT_GROUPS){
  const base=facts.paths.filter(p=>p.startsWith(dir+'/')&&pattern.test(p)),untouched=base.filter(p=>!existing.includes(p));
  const current=files(root,dir,pattern).filter(p=>p!==manifest),expected=[...base,...added.filter(p=>p.startsWith(dir+'/')&&pattern.test(p))].sort();
  const r=proof[name];
  if(JSON.stringify(current)!==JSON.stringify(expected)||r?.beforeFiles!==base.length||r?.afterFiles!==current.length)fail(`${name}-files`);
  if(untouched.some(p=>disk(p)!==facts.read(p)))fail(`${name}-untouched-bytes`);
  const key=JSON.stringify([root,name,existing.concat(added).filter(p=>current.includes(p)).map(p=>[p,proof.paths[p].afterHash])]);
  // Cache only content-addressed aggregates after fresh byte/path validation.
  if(!violations.length&&!aggregates.has(key)){
   const before=h3CloseoutAggregate(base,facts.read),unchanged=untouched.length===base.length&&JSON.stringify(current)===JSON.stringify(base);
   aggregates.set(key,{before,untouched:untouched.length===base.length?before:h3CloseoutAggregate(untouched,facts.read),after:unchanged?before:h3CloseoutAggregate(current,disk)});
  }
  const a=aggregates.get(key);
  if(!a||r?.beforeAggregateHash!==a.before||r?.untouchedAggregateHash!==a.untouched||r?.afterAggregateHash!==a.after)fail(`${name}-aggregate`);
 }
 for(const p of configs)if(disk(p)!==facts.read(p))fail(`frozen:${p}`);
 if(violations.length)return rejected();
 return {violations,addedPaths:[...added,manifest],backendAggregateSha256:proof.backend.afterAggregateHash,afterHashes:Object.fromEntries([...existing,...added].map(p=>[p,proof.paths[p].afterHash])),read:p=>existing.includes(p)?facts.read(p):disk(p)};
}

export function h3CloseoutParentReader(root, override) {
  return cachedBoundary(root, "h3CloseoutParentReader", override, () => h3CloseoutParentReaderUncached(root, override), 'docs/architecture/h3-release-closeout-phase1.json');
}
