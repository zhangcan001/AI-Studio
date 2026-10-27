// @vitest-environment node
import {expect,it} from 'vitest';
import {readFileSync,writeFileSync,rmSync} from 'node:fs';
import {execFileSync} from 'node:child_process';
import {queueLifecycleParentReader,QUEUE_LIFECYCLE_PARENT} from '../../scripts/queue-lifecycle-repair-successor-guard.mjs';
const manifest='docs/architecture/queue-lifecycle-repair.json';

it('accepts only the reviewed lifecycle repair before replaying immutable source',()=>{
 const proof=JSON.parse(readFileSync(manifest,'utf8'));
 const accepted=queueLifecycleParentReader('.');expect(accepted.violations).toEqual([]);
 const path='src-tauri/src/application/production_queue_service.rs';
 expect(accepted.read(path)).toBe(execFileSync('git',['show',`${QUEUE_LIFECYCLE_PARENT}:${path}`],{encoding:'utf8'}).replaceAll('\r\n','\n'));
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
  const invalid=structuredClone(proof);mutate(invalid);const denied=queueLifecycleParentReader('.',invalid);
  expect(denied.violations.length).toBeGreaterThan(0);expect(denied.addedPaths).toEqual([]);
  expect(denied.read(path)).toBe(readFileSync(path,'utf8').replaceAll('\r\n','\n'));
 }
}, 90000);

it('denies queue drift in repaired sources, untouched backend and historical proofs',()=>{
  // Live working-tree mutate keeps fail-closed drift asserts without copying the tip-inflated Phase2 fixture set.
  for(const p of ['src-tauri/src/application/production_queue_service.rs','src-tauri/src/domain/asset.rs',
   'docs/architecture/h3-release-closeout-phase1.json']){
   const original=readFileSync(p);
   try{
    writeFileSync(p, original.toString() + '\n# drift\n');
    const denied=queueLifecycleParentReader('.');expect(denied.violations.length).toBeGreaterThan(0);expect(denied.addedPaths).toEqual([]);
   }finally{writeFileSync(p,original);}
   expect(queueLifecycleParentReader('.').violations).toEqual([]);
  }
},90000);

it('denies queue drift in packages, frozen configs and illegal backend additions',()=>{
  for(const p of ['src-tauri/runtime_packages/minimax_h3_fl2va_i2v_quality_2_2_0/recipe.yaml','src-tauri/tauri.conf.json']){
   const original=readFileSync(p);
   try{
    writeFileSync(p, original.toString() + '\n# drift\n');
    const denied=queueLifecycleParentReader('.');expect(denied.violations.length).toBeGreaterThan(0);expect(denied.addedPaths).toEqual([]);
   }finally{writeFileSync(p,original);}
   expect(queueLifecycleParentReader('.').violations).toEqual([]);
  }
  const extra='src-tauri/src/unreviewed.rs';
  try{
   writeFileSync(extra,'// unreviewed');
   expect(queueLifecycleParentReader('.').violations).toContain('minimax-video-phase2-backend-files');
  }finally{rmSync(extra);}
  expect(queueLifecycleParentReader('.').violations).toEqual([]);
},90000);
