import {MINIMAX_VIDEO_PHASE1_FILES} from '../../scripts/readiness-lifecycle-successor-guard.mjs';
import {minimaxVideoPhase2FixtureFiles,} from '../../scripts/minimax-video-phase2-successor-guard.mjs';
// @vitest-environment node
import {expect,it} from 'vitest';
import {readFileSync,writeFileSync,copyFileSync,mkdirSync,mkdtempSync,rmSync} from 'node:fs';
import {execFileSync} from 'node:child_process';
import {tmpdir} from 'node:os';
import {join,dirname} from 'node:path';
import {readinessLifecycleParentReader,readinessLifecycleParentFacts,READINESS_LIFECYCLE_PARENT,postRunReadinessParentReader,POST_RUN_READINESS_PARENT} from '../../scripts/readiness-lifecycle-successor-guard.mjs';
const manifest='docs/architecture/readiness-lifecycle-fix.json';
const postRunManifest='docs/architecture/readiness-post-run-fix.json';

it('accepts only the reviewed readiness lifecycle repair before replaying immutable source',()=>{
 const proof=JSON.parse(readFileSync(manifest,'utf8'));
 const accepted=readinessLifecycleParentReader('.');expect(accepted.violations).toEqual([]);
 const path='src/features/create/CreateController.ts';
 expect(accepted.read(path)).toBe(execFileSync('git',['show',`${READINESS_LIFECYCLE_PARENT}:${path}`],{encoding:'utf8'}).replaceAll('\r\n','\n'));
 for(const mutate of [
  (p:typeof proof)=>{p.parentHead='0'.repeat(40);},
  (p:typeof proof)=>{p.paths[path].beforeHash='0'.repeat(64);},
  (p:typeof proof)=>{p.paths[path].afterHash='0'.repeat(64);},
  (p:typeof proof)=>{p.paths['src/services/ipc.ts']={beforeHash:null,afterHash:'0'.repeat(64)};},
  ...Object.keys(proof.invariants).map(key=>(p:typeof proof)=>{p.invariants[key]=!p.invariants[key];}),
  ...['backend','tests','frontend','scripts','migrations','packages','architecture'].flatMap(group=>[
   (p:typeof proof)=>{p[group].untouchedAggregateHash='0'.repeat(64);},
   (p:typeof proof)=>{p[group].afterAggregateHash='0'.repeat(64);},
   (p:typeof proof)=>{p[group].afterFiles++;}]),
 ]){
  const invalid=structuredClone(proof);mutate(invalid);const denied=readinessLifecycleParentReader('.',invalid);
  expect(denied.violations.length).toBeGreaterThan(0);expect(denied.addedPaths).toEqual([]);
  expect(denied.read(path)).toBe(readFileSync(path,'utf8').replaceAll('\r\n','\n'));
 }
}, 90000);

it('accepts only the exact-parent post-run readiness repair without rewriting old proofs',()=>{
 const proof=JSON.parse(readFileSync(postRunManifest,'utf8'));
 const accepted=postRunReadinessParentReader('.');expect(accepted.violations).toEqual([]);
 const path='src/features/create/CreateController.ts';
 expect(accepted.read(path)).toBe(execFileSync('git',['show',`${POST_RUN_READINESS_PARENT}:${path}`],{encoding:'utf8'}).replaceAll('\r\n','\n'));
 for(const mutate of [
  (p:typeof proof)=>{p.parentHead='0'.repeat(40);},
  (p:typeof proof)=>{p.paths[path].beforeHash='0'.repeat(64);},
  (p:typeof proof)=>{p.paths[path].afterHash='0'.repeat(64);},
  (p:typeof proof)=>{p.paths['src/services/ipc.ts']={beforeHash:null,afterHash:'0'.repeat(64)};},
  ...Object.keys(proof.invariants).map(key=>(p:typeof proof)=>{p.invariants[key]=!p.invariants[key];}),
  ...['backend','tests','frontend','styles','scripts','migrations','packages','architecture'].flatMap(group=>[
   (p:typeof proof)=>{p[group].beforeAggregateHash='0'.repeat(64);},
   (p:typeof proof)=>{p[group].untouchedAggregateHash='0'.repeat(64);},
   (p:typeof proof)=>{p[group].afterAggregateHash='0'.repeat(64);},
   (p:typeof proof)=>{p[group].afterFiles++;}]),
 ]){
  const invalid=structuredClone(proof);mutate(invalid);const denied=postRunReadinessParentReader('.',invalid);
  expect(denied.violations.length).toBeGreaterThan(0);expect(denied.addedPaths).toEqual([]);
  expect(denied.read(path)).toBe(readFileSync(path,'utf8').replaceAll('\r\n','\n'));
 }
}, 90000);

async function withPostRunFixture(run:(root:string)=>void|Promise<void>){
 const root=mkdtempSync(join(tmpdir(),'ai-studio-post-run-boundary-'));
 try {
  const git=execFileSync('git',['rev-parse','--absolute-git-dir'],{encoding:'utf8'}).trim();
  execFileSync('git',['init','--quiet',root]);mkdirSync(join(root,'.git/objects/info'),{recursive:true});
  writeFileSync(join(root,'.git/objects/info/alternates'),join(git,'objects')+'\n');
  for(const p of new Set<string>([...MINIMAX_VIDEO_PHASE1_FILES,...minimaxVideoPhase2FixtureFiles('.'),...readinessLifecycleParentFacts('.',POST_RUN_READINESS_PARENT).paths,postRunManifest])){
   mkdirSync(dirname(join(root,p)),{recursive:true});copyFileSync(p,join(root,p));
  }
  expect(postRunReadinessParentReader(root).violations).toEqual([]);
  await run(root);
 }finally{rmSync(root,{recursive:true,force:true}); await new Promise<void>(r=>setTimeout(r,0));}
}

async function withReadinessFixture(run:(root:string)=>void|Promise<void>){
 const root=mkdtempSync(join(tmpdir(),'ai-studio-readiness-boundary-'));
 try {
  const git=execFileSync('git',['rev-parse','--absolute-git-dir'],{encoding:'utf8'}).trim();
  execFileSync('git',['init','--quiet',root]);mkdirSync(join(root,'.git/objects/info'),{recursive:true});
  writeFileSync(join(root,'.git/objects/info/alternates'),join(git,'objects')+'\n');
  const proof=JSON.parse(readFileSync(manifest,'utf8'));
  for(const p of new Set([...MINIMAX_VIDEO_PHASE1_FILES,...minimaxVideoPhase2FixtureFiles('.'),...readinessLifecycleParentFacts('.').paths,...Object.keys(proof.paths),manifest,postRunManifest]) as Set<string>){
   mkdirSync(dirname(join(root,p)),{recursive:true});copyFileSync(p,join(root,p));
  }
  expect(readinessLifecycleParentReader(root).violations).toEqual([]);
  await run(root);
 }finally{rmSync(root,{recursive:true,force:true}); await new Promise<void>(r=>setTimeout(r,0));}
}

async function expectDriftDenied(root:string,reader:(root:string)=> {violations:string[];addedPaths:string[]},relative:string){
 const path=join(root,relative),original=readFileSync(path,'utf8');
 writeFileSync(path,original+'\n# drift\n');
 const denied=reader(root);
 expect(denied.violations.length).toBeGreaterThan(0);
 expect(denied.addedPaths).toEqual([]);
 writeFileSync(path,original);
 expect(reader(root).violations).toEqual([]);
 await new Promise<void>(r=>setTimeout(r,0));
}

it('denies post-run drift in controller, readiness test and previous readiness proof',()=>withPostRunFixture(async root=>{
 // A rejected old proof must not leak the accepted successor's parent projection.
 const invalidOld=JSON.parse(readFileSync(join(root,manifest),'utf8'));
 const controller='src/features/create/CreateController.ts';invalidOld.paths[controller].afterHash='0'.repeat(64);
 const deniedOld=readinessLifecycleParentReader(root,invalidOld);
 expect(deniedOld.violations.length).toBeGreaterThan(0);expect(deniedOld.addedPaths).toEqual([]);
 expect(deniedOld.read(controller)).toBe(readFileSync(join(root,controller),'utf8').replaceAll('\r\n','\n'));
 for(const p of ['src/features/create/CreateController.ts','src/features/create/CreateReadinessLifecycle.test.tsx',manifest]){
  await expectDriftDenied(root,postRunReadinessParentReader,p);
 }
}),360000);

it('denies post-run drift in runtime packages, frozen configs, missing evidence and illegal additions',()=>withPostRunFixture(async root=>{
 for(const p of ['src-tauri/runtime_packages/minimax_h3_fl2va_i2v_quality_2_2_0/recipe.yaml','src-tauri/tauri.conf.json']){
  await expectDriftDenied(root,postRunReadinessParentReader,p);
 }
 const path=join(root,postRunManifest),original=readFileSync(path,'utf8');rmSync(path);
 expect(postRunReadinessParentReader(root).violations).toContain('post-run-readiness-missing-evidence');
 writeFileSync(path,original);expect(postRunReadinessParentReader(root).violations).toEqual([]);
 writeFileSync(join(root,'src/unreviewed.ts'),'// unreviewed');
 expect(postRunReadinessParentReader(root).violations).toContain('minimax-video-phase2-frontend-files');
}),360000);

it('denies readiness drift in repaired sources, untouched backend and historical proofs',()=>withReadinessFixture(async root=>{
 for(const p of ['src/features/create/CreateController.ts','src-tauri/src/domain/asset.rs',
  'docs/architecture/h3-release-closeout-phase1.json']){
  await expectDriftDenied(root,readinessLifecycleParentReader,p);
 }
}),360000);

it('denies readiness drift in packages, frozen configs and illegal backend additions',()=>withReadinessFixture(async root=>{
 for(const p of ['src-tauri/runtime_packages/minimax_h3_fl2va_i2v_quality_2_2_0/recipe.yaml','src-tauri/tauri.conf.json']){
  await expectDriftDenied(root,readinessLifecycleParentReader,p);
 }
 writeFileSync(join(root,'src-tauri/src/unreviewed.rs'),'// unreviewed');
 expect(readinessLifecycleParentReader(root).violations).toContain('minimax-video-phase2-backend-files');
}),360000);
