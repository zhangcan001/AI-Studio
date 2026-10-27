// @vitest-environment node
import {expect,it} from 'vitest';
import {readFileSync,writeFileSync,rmSync} from 'node:fs';
import {uiPhase5FParentReader,phase5FParentFacts,PHASE5F_MANIFEST,PHASE5F_PARENT,PHASE5F_EXISTING} from '../../scripts/ui-phase5f-ci-memory-successor-guard.mjs';
import {uiPhase5EParentReader} from '../../scripts/ui-phase5e-ci-shard-successor-guard.mjs';

it('keeps three shards, raises the frontend heap ceiling, and leaves individual bounded probes in force',()=>{
 const ci=readFileSync('.github/workflows/ci.yml','utf8');
 expect(ci).toContain('NODE_OPTIONS: --max-old-space-size=8192');
 expect(ci).toContain('matrix:\n        shard: [1, 2, 3]');
 expect(ci).toContain('--maxWorkers=1 --shard=${{ matrix.shard }}/4');
 expect(ci).toContain('matrix:\n        shard: [1, 2, 3, 4]');
 expect(ci).toContain('name: Frontend source checks');
 const cache=readFileSync('scripts/boundary-validation-cache.mjs','utf8');
 expect(cache).toContain('staged: new Map()');
 expect(cache).toContain('session.staged.clear()');
 const readiness=readFileSync('src/app/ReadinessLifecycleBoundary.test.ts','utf8');
 expect(readiness).toContain("it('denies post-run drift in controller, readiness test and previous readiness proof'");
 expect(readiness).toContain("it('denies post-run drift in runtime packages, frozen configs, missing evidence and illegal additions'");
 expect(readiness).toContain("it('denies readiness drift in repaired sources, untouched backend and historical proofs'");
 expect(readiness).toContain("it('denies readiness drift in packages, frozen configs and illegal backend additions'");
 expect((readiness.match(/90000\)/g) ?? []).length).toBeGreaterThanOrEqual(6);
}, 90000);

it('accepts scoped CI memory repair and preserves immutable historical proof bytes',()=>{
 const reader=uiPhase5FParentReader('.');
 expect(reader.violations).toEqual([]);
 const proof=JSON.parse(readFileSync(PHASE5F_MANIFEST,'utf8'));
 expect(proof.parentHead).toBe(PHASE5F_PARENT);
 expect(proof.phaseCommitsBeforeCheckpoint).toEqual([]);
 expect(proof.invariants.nestedSuccessStagingOnCommitOnly).toBe(true);
 expect(proof.invariants.readinessDriftProbesSplit).toBe(true);
 expect(proof.invariants.thirtySecondBoundaryLimitPreserved).toBe(true);
 for(const p of PHASE5F_EXISTING)expect(reader.read(p)).toBe(phase5FParentFacts('.').read(p));
 expect(uiPhase5EParentReader('.').violations).toEqual([]);
}, 90000);

it('denies forged metadata, proof hashes and weakened memory gates without authority',()=>{
 const proof=JSON.parse(readFileSync(PHASE5F_MANIFEST,'utf8'));
 for(const mutate of [
  (p:any)=>{p.parentHead='0'.repeat(40);},
  (p:any)=>{p.paths['scripts/boundary-validation-cache.mjs'].afterHash='0'.repeat(64);},
  (p:any)=>{p.paths['src/app/ReadinessLifecycleBoundary.test.ts'].beforeHash='0'.repeat(64);},
  (p:any)=>{p.invariants.nestedSuccessStagingOnCommitOnly=false;},
  (p:any)=>{p.invariants.historicalProofsRewritten=true;},
  (p:any)=>{p.paths['src/services/ipc.ts']={beforeHash:null,afterHash:'0'.repeat(64)};},
 ]){
  const invalid=structuredClone(proof);mutate(invalid);
  const denied=uiPhase5FParentReader('.',invalid);
  expect(denied.violations.length).toBeGreaterThan(0);
  expect(denied.addedPaths).toEqual([]);expect(denied.afterHashes).toEqual({});
 }
}, 90000);

it('refuses edited CI heap, cache staging, readiness probes and Runtime package drift in isolated fixture',()=>{
  // Live working-tree mutate — tip-inflated FixtureFiles blow the 90s Boundary budget.
  expect(uiPhase5FParentReader('.').violations).toEqual([]);
  for(const p of ['.github/workflows/ci.yml','scripts/boundary-validation-cache.mjs','src/app/ReadinessLifecycleBoundary.test.ts']){
   const before=readFileSync(p);
   try{
    writeFileSync(p,Uint8Array.from([...before,10]));
    expect(uiPhase5FParentReader('.').violations.length,p).toBeGreaterThan(0);
   }finally{writeFileSync(p,before);}
  }
  const pkg='src-tauri/runtime_packages/minimax_h3_fl2va_t2v_quality_2_2_0/recipe.yaml';
  const before=readFileSync(pkg);
  try{
   writeFileSync(pkg,Uint8Array.from([...before,10]));
   expect(uiPhase5FParentReader('.').violations).toContain('ui-phase5f-ci-memory-runtime-package-bytes');
  }finally{writeFileSync(pkg,before);}
  const illegal='src/IllegalMemoryOwner.js';
  try{
   writeFileSync(illegal,'unreviewed');
   expect(uiPhase5FParentReader('.').violations).toContain('ui-phase5f-ci-memory-inventory:src');
  }finally{rmSync(illegal);}
  expect(uiPhase5FParentReader('.').violations).toEqual([]);
}, 90000);
