// @vitest-environment node
import {expect,it} from 'vitest';
// @ts-expect-error Node-only boundary fixture.
import {readFileSync,writeFileSync,copyFileSync,mkdirSync,mkdtempSync,rmSync} from 'node:fs';
// @ts-expect-error Node-only boundary fixture.
import {execFileSync} from 'node:child_process';
// @ts-expect-error Node-only boundary fixture.
import {tmpdir} from 'node:os';
// @ts-expect-error Node-only boundary fixture.
import {join,dirname} from 'node:path';
// @ts-expect-error Node-only successor.
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
},30000);

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
},30000);

it('denies post-run drift including the previous readiness proof and runtime packages',()=>{
 const root=mkdtempSync(join(tmpdir(),'ai-studio-post-run-boundary-'));
 try {
  const git=execFileSync('git',['rev-parse','--absolute-git-dir'],{encoding:'utf8'}).trim();
  execFileSync('git',['init','--quiet',root]);mkdirSync(join(root,'.git/objects/info'),{recursive:true});
  writeFileSync(join(root,'.git/objects/info/alternates'),join(git,'objects')+'\n');
  for(const p of [...readinessLifecycleParentFacts('.',POST_RUN_READINESS_PARENT).paths,postRunManifest]){
   mkdirSync(dirname(join(root,p)),{recursive:true});copyFileSync(p,join(root,p));
  }
  expect(postRunReadinessParentReader(root).violations).toEqual([]);
  // A rejected old proof must not leak the accepted successor's parent projection.
  const invalidOld=JSON.parse(readFileSync(join(root,manifest),'utf8'));
  const controller='src/features/create/CreateController.ts';invalidOld.paths[controller].afterHash='0'.repeat(64);
  const deniedOld=readinessLifecycleParentReader(root,invalidOld);
  expect(deniedOld.violations.length).toBeGreaterThan(0);expect(deniedOld.addedPaths).toEqual([]);
  expect(deniedOld.read(controller)).toBe(readFileSync(join(root,controller),'utf8').replaceAll('\r\n','\n'));
  for(const p of ['src/features/create/CreateController.ts','src/features/create/CreateReadinessLifecycle.test.tsx',
   manifest,'src-tauri/runtime_packages/minimax_h3_fl2va_i2v_quality_2_2_0/recipe.yaml','src-tauri/tauri.conf.json']){
   const path=join(root,p),original=readFileSync(path,'utf8');writeFileSync(path,original+'\n# drift\n');
   const denied=postRunReadinessParentReader(root);expect(denied.violations.length).toBeGreaterThan(0);expect(denied.addedPaths).toEqual([]);
   writeFileSync(path,original);expect(postRunReadinessParentReader(root).violations).toEqual([]);
  }
  const path=join(root,postRunManifest),original=readFileSync(path,'utf8');rmSync(path);
  expect(postRunReadinessParentReader(root).violations).toContain('post-run-readiness-missing-evidence');
  writeFileSync(path,original);expect(postRunReadinessParentReader(root).violations).toEqual([]);
  writeFileSync(join(root,'src/unreviewed.ts'),'// unreviewed');
  expect(postRunReadinessParentReader(root).violations).toContain('post-run-readiness-frontend-files');
 }finally{rmSync(root,{recursive:true,force:true});}
},30000);

it('denies drift in repaired sources, untouched files, historical proofs, packages and frozen configs',()=>{
 const root=mkdtempSync(join(tmpdir(),'ai-studio-readiness-boundary-'));
 try {
  const git=execFileSync('git',['rev-parse','--absolute-git-dir'],{encoding:'utf8'}).trim();
  execFileSync('git',['init','--quiet',root]);mkdirSync(join(root,'.git/objects/info'),{recursive:true});
  writeFileSync(join(root,'.git/objects/info/alternates'),join(git,'objects')+'\n');
  const proof=JSON.parse(readFileSync(manifest,'utf8'));
  for(const p of new Set([...readinessLifecycleParentFacts('.').paths,...Object.keys(proof.paths),manifest,postRunManifest]) as Set<string>){
   mkdirSync(dirname(join(root,p)),{recursive:true});copyFileSync(p,join(root,p));
  }
  expect(readinessLifecycleParentReader(root).violations).toEqual([]);
  for(const p of ['src/features/create/CreateController.ts','src-tauri/src/domain/asset.rs',
   'docs/architecture/h3-release-closeout-phase1.json','src-tauri/runtime_packages/minimax_h3_fl2va_i2v_quality_2_2_0/recipe.yaml','src-tauri/tauri.conf.json']){
   const path=join(root,p),original=readFileSync(path,'utf8');writeFileSync(path,original+'\n# drift\n');
   const denied=readinessLifecycleParentReader(root);expect(denied.violations.length).toBeGreaterThan(0);expect(denied.addedPaths).toEqual([]);
   writeFileSync(path,original);expect(readinessLifecycleParentReader(root).violations).toEqual([]);
  }
  writeFileSync(join(root,'src-tauri/src/unreviewed.rs'),'// unreviewed');
  expect(readinessLifecycleParentReader(root).violations).toContain('post-run-readiness-backend-files');
 }finally{rmSync(root,{recursive:true,force:true});}
},30000);
