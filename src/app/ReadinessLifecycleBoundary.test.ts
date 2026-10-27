// @vitest-environment node
import {expect,it} from 'vitest';
import {readFileSync,writeFileSync,rmSync} from 'node:fs';
import {execFileSync} from 'node:child_process';
import {readinessLifecycleParentReader,READINESS_LIFECYCLE_PARENT,postRunReadinessParentReader,POST_RUN_READINESS_PARENT} from '../../scripts/readiness-lifecycle-successor-guard.mjs';
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

function expectDriftDenied(reader:(root:string)=> {violations:string[];addedPaths:string[]},relative:string){
 const original=readFileSync(relative);
 try{
  writeFileSync(relative, original.toString()+'\n# drift\n');
  const denied=reader('.');
  expect(denied.violations.length).toBeGreaterThan(0);
  expect(denied.addedPaths).toEqual([]);
 }finally{writeFileSync(relative,original);}
 expect(reader('.').violations).toEqual([]);
}

it('denies post-run drift in controller, readiness test and previous readiness proof',()=>{
  // Live working-tree mutate — Phase2 FixtureFiles grew past 5k and blows the 90s Boundary budget.
  const invalidOld=JSON.parse(readFileSync(manifest,'utf8'));
  const controller='src/features/create/CreateController.ts';invalidOld.paths[controller].afterHash='0'.repeat(64);
  const deniedOld=readinessLifecycleParentReader('.',invalidOld);
  expect(deniedOld.violations.length).toBeGreaterThan(0);expect(deniedOld.addedPaths).toEqual([]);
  expect(deniedOld.read(controller)).toBe(readFileSync(controller,'utf8').replaceAll('\r\n','\n'));
  for(const p of ['src/features/create/CreateController.ts','src/features/create/CreateReadinessLifecycle.test.tsx',manifest]){
   expectDriftDenied(postRunReadinessParentReader,p);
  }
},90000);

it('denies post-run drift in runtime packages, frozen configs, missing evidence and illegal additions',()=>{
 for(const p of ['src-tauri/runtime_packages/minimax_h3_fl2va_i2v_quality_2_2_0/recipe.yaml','src-tauri/tauri.conf.json']){
  expectDriftDenied(postRunReadinessParentReader,p);
 }
 const original=readFileSync(postRunManifest);
 try{
  rmSync(postRunManifest);
  expect(postRunReadinessParentReader('.').violations).toContain('post-run-readiness-missing-evidence');
 }finally{writeFileSync(postRunManifest,original);}
 expect(postRunReadinessParentReader('.').violations).toEqual([]);
 const extra='src/unreviewed.ts';
 try{
  writeFileSync(extra,'// unreviewed');
  expect(postRunReadinessParentReader('.').violations).toContain('minimax-video-phase2-frontend-files');
 }finally{rmSync(extra);}
 expect(postRunReadinessParentReader('.').violations).toEqual([]);
},90000);

it('denies readiness drift in repaired sources, untouched backend and historical proofs',()=>{
 for(const p of ['src/features/create/CreateController.ts','src-tauri/src/domain/asset.rs',
  'docs/architecture/h3-release-closeout-phase1.json']){
  expectDriftDenied(readinessLifecycleParentReader,p);
 }
},90000);

it('denies readiness drift in packages, frozen configs and illegal backend additions',()=>{
 for(const p of ['src-tauri/runtime_packages/minimax_h3_fl2va_i2v_quality_2_2_0/recipe.yaml','src-tauri/tauri.conf.json']){
  expectDriftDenied(readinessLifecycleParentReader,p);
 }
 const extra='src-tauri/src/unreviewed.rs';
 try{
  writeFileSync(extra,'// unreviewed');
  expect(readinessLifecycleParentReader('.').violations).toContain('minimax-video-phase2-backend-files');
 }finally{rmSync(extra);}
 expect(readinessLifecycleParentReader('.').violations).toEqual([]);
},90000);
