// Validation-only successor: approve live repair bytes before replaying UI Phase3.
import { cachedBoundary } from './boundary-validation-cache.mjs';
import { uiPhase4ParentReader } from './minimax-video-v2-ui-phase4-successor-guard.mjs';
import { readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';
import { createHash } from 'node:crypto';
import { readinessLifecycleParentFacts, READINESS_LIFECYCLE_GROUPS, readinessLifecycleHash, readinessLifecycleAggregate } from './readiness-lifecycle-successor-guard.mjs';
export const CI_REPAIR_PARENT = '1702f3b4b1434fc3d02c96a63346a272740b21d4';
export const CI_REPAIR_MANIFEST = 'docs/architecture/ui-phase3-ci-repair.json';
export const CI_REPAIR_EXISTING = ['scripts/boundary-validation-cache.mjs','scripts/minimax-video-v2-ui-phase3-successor-guard.mjs',
  'scripts/testing/release-boundary.test.ts','src/app/ReadinessLifecycleBoundary.test.ts'].sort();
export const CI_REPAIR_ADDED = ['scripts/ui-phase3-ci-repair-successor-guard.mjs','scripts/testing/boundary-validation-cache.test.ts',
  'docs/architecture/ui-phase3-ci-repair-plan.md'].sort();
export const CI_REPAIR_FILES = [...CI_REPAIR_EXISTING,...CI_REPAIR_ADDED,CI_REPAIR_MANIFEST];
export const ciRepairGroups = () => [...READINESS_LIFECYCLE_GROUPS,['domainDocs','docs/architecture',/\.md$/]];
export const CI_REPAIR_INVARIANTS = {businessRuntimeChanged:false,schemaChanged:false,backupFormatChanged:false,
  queueAuthorityChanged:false,storeAuthorityChanged:false,uiV2Changed:false,historicalProofsPreserved:true,
  immutablePackagesPreserved:true,liveAcceptanceFresh:true,validationScopeChanged:false};
const normalize = s => s.replaceAll('\r\n','\n');
const files = (root,dir,pattern) => readdirSync(join(root,dir),{withFileTypes:true}).flatMap(e => {
  const p = `${dir}/${e.name}`; return e.isDirectory() ? files(root,p,pattern) : pattern.test(p) ? [p] : [];
}).sort();
const frozen = ['CONTEXT.md','README.md','package.json','pnpm-lock.yaml','src-tauri/Cargo.toml','src-tauri/Cargo.lock',
  'src-tauri/tauri.conf.json','src-tauri/build.rs','.github/workflows/ci.yml','docs/AI_STUDIO_2_1_CLOSEOUT.md','docs/RELEASE_NOTES_v2.1.0-personal.md'];
const sha = bytes => createHash('sha256').update(bytes).digest('hex');
function validate(root,override) {
  const successor = uiPhase4ParentReader(root);
  const violations = [...successor.violations], fail = s => violations.push(`ui-phase3-ci-repair-${s}`);
  const live = new Map(), disk = p => {if(!live.has(p))live.set(p,normalize(successor.read(p)));return live.get(p);};
  const rejected = () => ({violations,addedPaths:[],afterHashes:{},read:p=>normalize(readFileSync(join(root,p),'utf8'))});
  let proof,facts;
  try {proof=override??JSON.parse(disk(CI_REPAIR_MANIFEST));facts=readinessLifecycleParentFacts(root,CI_REPAIR_PARENT);}
  catch {fail('missing-evidence');return rejected();}
  const scope=[...CI_REPAIR_EXISTING,...CI_REPAIR_ADDED].sort();
  if(proof.schemaVersion!==1||proof.checkpoint!=='UI_PHASE3_MERGE_CI_REPAIR'||proof.parentHead!==CI_REPAIR_PARENT
    ||proof.planningParentHead!==CI_REPAIR_PARENT||JSON.stringify(proof.phaseCommitsBeforeCheckpoint)!=='[]')fail('header');
  if(JSON.stringify(Object.keys(proof.paths??{}).sort())!==JSON.stringify(scope))fail('scope');
  if(JSON.stringify(proof.invariants)!==JSON.stringify(CI_REPAIR_INVARIANTS))fail('contract');
  if(JSON.stringify(proof.database)!==JSON.stringify({migrationBefore:43,migrationAfter:43,newTables:0,backupBefore:21,backupAfter:21}))fail('database');
  if(violations.length)return rejected();
  try {
    for(const p of CI_REPAIR_EXISTING)if(proof.paths[p]?.beforeHash!==readinessLifecycleHash(facts.read(p))||proof.paths[p]?.afterHash!==readinessLifecycleHash(disk(p)))fail(`path:${p}`);
    for(const p of CI_REPAIR_ADDED)if(facts.paths.includes(p)||proof.paths[p]?.beforeHash!==null||proof.paths[p]?.afterHash!==readinessLifecycleHash(disk(p)))fail(`addition:${p}`);
    for(const [name,dir,pattern] of ciRepairGroups()) {
      const base=facts.paths.filter(p=>p.startsWith(dir+'/')&&pattern.test(p)),untouched=base.filter(p=>!CI_REPAIR_EXISTING.includes(p));
      const current=files(root,dir,pattern).filter(p=>p!==CI_REPAIR_MANIFEST&&!successor.addedPaths.includes(p)),expected=[...base,...CI_REPAIR_ADDED.filter(p=>p.startsWith(dir+'/')&&pattern.test(p))].sort(),r=proof[name];
      if(JSON.stringify(current)!==JSON.stringify(expected)||r?.beforeFiles!==base.length||r?.afterFiles!==current.length)fail(`${name}-files`);
      if(untouched.some(p=>disk(p)!==facts.read(p)))fail(`${name}-untouched-bytes`);
      if(r?.beforeAggregateHash!==readinessLifecycleAggregate(base,facts.read)||r?.untouchedAggregateHash!==readinessLifecycleAggregate(untouched,facts.read)||r?.afterAggregateHash!==readinessLifecycleAggregate(current,disk))fail(`${name}-aggregate`);
      if(name==='packages') {
        const profiles=JSON.parse(facts.read('docs/architecture/minimax-video-phase2.json')).runtimeByteBaselines;
        const profile=process.env.AI_STUDIO_PHASE2_RUNTIME_BYTE_PROFILE??'git';
        if(!Object.hasOwn(profiles,profile)||sha(JSON.stringify(base.map(p=>[p,sha(readFileSync(join(root,p)))])))!==profiles[profile]
          ||JSON.stringify(files(root,dir,/./))!==JSON.stringify(base))fail('runtime-package-bytes');
      }
    }
    for(const p of frozen)if(disk(p)!==facts.read(p))fail(`frozen:${p}`);
  }catch{fail('missing-evidence');}
  if(violations.length)return rejected();
  return {violations,addedPaths:[...CI_REPAIR_ADDED,CI_REPAIR_MANIFEST,...successor.addedPaths],afterHashes:{...Object.fromEntries(scope.map(p=>[p,proof.paths[p].afterHash])),...successor.afterHashes},
    read:p=>CI_REPAIR_EXISTING.includes(p)?facts.read(p):disk(p)};
}
export function uiPhase3CiRepairParentReader(root,override) {
  return cachedBoundary(root,'uiPhase3CiRepairParentReader',override,()=>validate(root,override),CI_REPAIR_MANIFEST);
}
