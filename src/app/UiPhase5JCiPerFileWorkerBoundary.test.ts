// @vitest-environment node
import {expect,it} from 'vitest';
import {readFileSync,writeFileSync,rmSync} from 'node:fs';
import {uiPhase5JParentReader,phase5JParentFacts,PHASE5J_MANIFEST,PHASE5J_PARENT,PHASE5J_EXISTING} from '../../scripts/ui-phase5j-ci-per-file-worker-successor-guard.mjs';
import {uiPhase5IParentReader} from '../../scripts/ui-phase5i-ci-worker-teardown-successor-guard.mjs';

it('uses per-file forks workers and a dedicated Boundary job instead of singleFork',()=>{
 const vite=readFileSync('vite.config.ts','utf8');
 expect(vite).toContain('pool: "forks"');
 expect(vite).toContain('singleFork: false');
 expect(vite).toContain('fileParallelism: false');
 expect(vite).toContain('isolate: true');
 expect(vite).toContain('teardownTimeout: 120_000');
 const ci=readFileSync('.github/workflows/ci.yml','utf8');
 expect(ci).toContain('--maxWorkers=1 --shard=${{ matrix.shard }}/4');
 expect(ci).toContain('matrix:\n        shard: [1, 2, 3, 4]');
 expect(ci).toContain('--exclude src/app/**/*Boundary*.test.ts');
 expect(ci).toContain('name: Frontend Boundary probes');
 expect(ci).toContain('needs: [frontend_tests, frontend_boundary_tests]');
 expect(ci).toContain('timeout-minutes: 90');
}, 90000);

it('accepts scoped per-file worker repair and preserves immutable historical proof bytes',()=>{
 const reader=uiPhase5JParentReader('.');
 expect(reader.violations).toEqual([]);
 const proof=JSON.parse(readFileSync(PHASE5J_MANIFEST,'utf8'));
 expect(proof.parentHead).toBe(PHASE5J_PARENT);
 expect(proof.invariants.vitestForksPerFileWorker).toBe(true);
 expect(proof.invariants.vitestForksSingleForkTeardown).toBe(false);
 expect(proof.invariants.dedicatedBoundaryJob).toBe(true);
 expect(proof.invariants.boundaryExcludedFromShards).toBe(true);
 expect(proof.invariants.historicalProofsRewritten).toBe(false);
 for(const p of PHASE5J_EXISTING)expect(reader.read(p)).toBe(phase5JParentFacts('.').read(p));
 expect(uiPhase5IParentReader('.').violations).toEqual([]);
}, 90000);

it('denies forged metadata, proof hashes and weakened per-file worker gates without authority',()=>{
 const proof=JSON.parse(readFileSync(PHASE5J_MANIFEST,'utf8'));
 for(const mutate of [
  (p:any)=>{p.parentHead='0'.repeat(40);},
  (p:any)=>{p.paths['vite.config.ts'].afterHash='0'.repeat(64);},
  (p:any)=>{p.paths['.github/workflows/ci.yml'].beforeHash='0'.repeat(64);},
  (p:any)=>{p.invariants.vitestForksPerFileWorker=false;},
  (p:any)=>{p.invariants.historicalProofsRewritten=true;},
  (p:any)=>{p.paths['src/services/ipc.ts']={beforeHash:null,afterHash:'0'.repeat(64)};},
 ]){
  const invalid=structuredClone(proof);mutate(invalid);
  const denied=uiPhase5JParentReader('.',invalid);
  expect(denied.violations.length).toBeGreaterThan(0);
  expect(denied.addedPaths).toEqual([]);expect(denied.afterHashes).toEqual({});
 }
}, 90000);

it('refuses edited per-file worker config, Boundary job wiring and Runtime package drift in isolated fixture',()=>{
  // Live working-tree mutate — tip-inflated FixtureFiles blow the 90s Boundary budget.
  expect(uiPhase5JParentReader('.').violations).toEqual([]);
  for(const p of ['vite.config.ts','.github/workflows/ci.yml','src/app/UiPhase5ICiWorkerTeardownBoundary.test.ts']){
   const before=readFileSync(p);
   try{
    writeFileSync(p,Uint8Array.from([...before,10]));
    expect(uiPhase5JParentReader('.').violations.length,p).toBeGreaterThan(0);
   }finally{writeFileSync(p,before);}
  }
  const pkg='src-tauri/runtime_packages/minimax_h3_fl2va_t2v_quality_2_2_0/recipe.yaml';
  const before=readFileSync(pkg);
  try{
   writeFileSync(pkg,Uint8Array.from([...before,10]));
   expect(uiPhase5JParentReader('.').violations).toContain('ui-phase5j-ci-per-file-worker-runtime-package-bytes');
  }finally{writeFileSync(pkg,before);}
  const illegal='src/IllegalPerFileWorkerOwner.js';
  try{
   writeFileSync(illegal,'unreviewed');
   expect(uiPhase5JParentReader('.').violations).toContain('ui-phase5j-ci-per-file-worker-inventory:src');
  }finally{rmSync(illegal);}
  expect(uiPhase5JParentReader('.').violations).toEqual([]);
}, 90000);
