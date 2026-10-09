// Phase4: validate the real workbench before projecting the immutable CI-repair parent.
import { cachedBoundary } from './boundary-validation-cache.mjs';
import { readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';
import { createHash } from 'node:crypto';
import { readinessLifecycleParentFacts, READINESS_LIFECYCLE_GROUPS, readinessLifecycleHash, readinessLifecycleAggregate } from './readiness-lifecycle-successor-guard.mjs';
export const UI_PHASE4_PARENT = '75a7bcdc0f47b8becafa4eab5e433c87b17c1ff8';
export const UI_PHASE4_MANIFEST = 'docs/architecture/minimax-video-v2-ui-phase4.json';
export const UI_PHASE4_EXISTING = [
  'scripts/ui-phase3-ci-repair-successor-guard.mjs', 'scripts/minimax-video-v2-ui-phase3-successor-guard.mjs',
  'src/features/create/CreateController.ts', 'src/features/create/CreateInputs.tsx',
  'src/features/create/CreatePage.tsx', 'src/features/create/CreatePage.css',
  'src/features/create/CreateResults.tsx', 'src/features/create/usePersistentVideoInputs.ts',
  'src/features/create/usePersistentVideoInputs.test.tsx', 'src/product/generatorPresentation.ts',
].sort();
export const UI_PHASE4_ADDED = [
  'scripts/minimax-video-v2-ui-phase4-successor-guard.mjs',
  'docs/architecture/minimax-video-v2-ui-phase4-plan.md',
  'src/features/create/CreateMediaPreview.tsx', 'src/features/create/CreateVideoWorkspace.tsx',
  'src/features/create/CreateVideoWorkspace.test.tsx', 'src/app/MiniMaxVideoUiPhase4Boundary.test.ts',
].sort();
export const UI_PHASE4_FILES = [...UI_PHASE4_EXISTING,...UI_PHASE4_ADDED,UI_PHASE4_MANIFEST];
export const uiPhase4Groups = () => [...READINESS_LIFECYCLE_GROUPS,['domainDocs','docs/architecture',/\.md$/]];
export const UI_PHASE4_INVARIANTS = {businessRuntimeChanged:false,schemaChanged:false,backupFormatChanged:false,
  queueAuthorityChanged:false,storeAuthorityChanged:false,uiV2Changed:true,historicalProofsPreserved:true,
  immutablePackagesPreserved:true,liveAcceptanceFresh:true,validationScopeChanged:false};
const normalize = s => s.replaceAll('\r\n','\n');
const files = (root,dir,pattern) => readdirSync(join(root,dir),{withFileTypes:true}).flatMap(e => {
  const p = `${dir}/${e.name}`; return e.isDirectory() ? files(root,p,pattern) : pattern.test(p) ? [p] : [];
}).sort();
const frozen = ['CONTEXT.md','README.md','package.json','pnpm-lock.yaml','src-tauri/Cargo.toml','src-tauri/Cargo.lock',
  'src-tauri/tauri.conf.json','src-tauri/build.rs','.github/workflows/ci.yml','docs/AI_STUDIO_2_1_CLOSEOUT.md','docs/RELEASE_NOTES_v2.1.0-personal.md'];
const sha = bytes => createHash('sha256').update(bytes).digest('hex');
function validate(root,override) {
  const violations = [], fail = s => violations.push(`minimax-video-v2-ui-phase4-${s}`);
  const live = new Map(), disk = p => {if(!live.has(p))live.set(p,normalize(readFileSync(join(root,p),'utf8')));return live.get(p);};
  const rejected = () => ({violations,addedPaths:[],afterHashes:{},read:p=>normalize(readFileSync(join(root,p),'utf8'))});
  let proof,facts;
  try {proof=override??JSON.parse(disk(UI_PHASE4_MANIFEST));facts=readinessLifecycleParentFacts(root,UI_PHASE4_PARENT);}
  catch {fail('missing-evidence');return rejected();}
  const scope=[...UI_PHASE4_EXISTING,...UI_PHASE4_ADDED].sort();
  if(proof.schemaVersion!==1||proof.checkpoint!=='MINIMAX_VIDEO_V2_UI_PHASE_4'||proof.parentHead!==UI_PHASE4_PARENT
    ||proof.planningParentHead!==UI_PHASE4_PARENT||JSON.stringify(proof.phaseCommitsBeforeCheckpoint)!=='[]')fail('header');
  if(JSON.stringify(Object.keys(proof.paths??{}).sort())!==JSON.stringify(scope))fail('scope');
  if(JSON.stringify(proof.invariants)!==JSON.stringify(UI_PHASE4_INVARIANTS))fail('contract');
  if(JSON.stringify(proof.database)!==JSON.stringify({migrationBefore:43,migrationAfter:43,newTables:0,backupBefore:21,backupAfter:21}))fail('database');
  if(violations.length)return rejected();
  try {
    for(const p of UI_PHASE4_EXISTING)if(proof.paths[p]?.beforeHash!==readinessLifecycleHash(facts.read(p))||proof.paths[p]?.afterHash!==readinessLifecycleHash(disk(p)))fail(`path:${p}`);
    for(const p of UI_PHASE4_ADDED)if(facts.paths.includes(p)||proof.paths[p]?.beforeHash!==null||proof.paths[p]?.afterHash!==readinessLifecycleHash(disk(p)))fail(`addition:${p}`);
    for(const [name,dir,pattern] of uiPhase4Groups()) {
      const base=facts.paths.filter(p=>p.startsWith(dir+'/')&&pattern.test(p)),untouched=base.filter(p=>!UI_PHASE4_EXISTING.includes(p));
      const current=files(root,dir,pattern).filter(p=>p!==UI_PHASE4_MANIFEST),expected=[...base,...UI_PHASE4_ADDED.filter(p=>p.startsWith(dir+'/')&&pattern.test(p))].sort(),r=proof[name];
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
  return {violations,addedPaths:[...UI_PHASE4_ADDED,UI_PHASE4_MANIFEST],afterHashes:Object.fromEntries(scope.map(p=>[p,proof.paths[p].afterHash])),
    read:p=>UI_PHASE4_EXISTING.includes(p)?facts.read(p):disk(p)};
}
export function uiPhase4ParentReader(root,override) {
  return cachedBoundary(root,'uiPhase4ParentReader',override,()=>validate(root,override),UI_PHASE4_MANIFEST);
}
