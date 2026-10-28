// @vitest-environment node
import {expect,it} from 'vitest';
import {readFileSync,writeFileSync,rmSync} from 'node:fs';
import {uiPhase5GParentReader,phase5GParentFacts,PHASE5G_MANIFEST,PHASE5G_PARENT,PHASE5G_EXISTING} from '../../scripts/ui-phase5g-ci-timeout-successor-guard.mjs';
import {uiPhase5FParentReader} from '../../scripts/ui-phase5f-ci-memory-successor-guard.mjs';

it('splits live freshness probes, checks parent fail-closed once, and bounds Windows drift probes at 90s',()=>{
 const phase5b=readFileSync('src/app/UiPhase5BMixedVideoBoundary.test.ts','utf8');
 expect(phase5b).toContain("it('keeps live freshness for source and shell bytes with parent fail-closed checked once'");
 expect(phase5b).toContain("'keeps live freshness for frozen proofs, Runtime raw bytes: %s'");
 expect(phase5b).toContain("it('keeps live freshness for manifest deletion'");
 expect(phase5b).toContain("it('keeps live freshness for illegal additions across owned inventories'");
 expect(phase5b).toContain('parentChecked');
 expect((phase5b.match(/90000\)/g) ?? []).length).toBeGreaterThanOrEqual(3);
 const readiness=readFileSync('src/app/ReadinessLifecycleBoundary.test.ts','utf8');
 expect((readiness.match(/90000\)/g) ?? []).length).toBeGreaterThanOrEqual(4);
 const queue=readFileSync('src/app/QueueLifecycleRepairBoundary.test.ts','utf8');
 expect(queue).toContain("it('denies queue drift in repaired sources, untouched backend and historical proofs'");
 expect(queue).toContain("it('denies queue drift in packages, frozen configs and illegal backend additions'");
 expect((queue.match(/90000\)/g) ?? []).length).toBeGreaterThanOrEqual(2);
 const phase2=readFileSync('src/app/MiniMaxVideoPhase2Boundary.test.ts','utf8');
 expect(phase2).toContain("it('rejects illegal scripts, migrations and Runtime Package additions'");
 expect(phase2).toContain('90000');
}, 90000);

it('accepts scoped CI timeout repair and preserves immutable historical proof bytes',()=>{
 const reader=uiPhase5GParentReader('.');
 expect(reader.violations).toEqual([]);
 const proof=JSON.parse(readFileSync(PHASE5G_MANIFEST,'utf8'));
 expect(proof.parentHead).toBe(PHASE5G_PARENT);
 expect(proof.phaseCommitsBeforeCheckpoint).toEqual([]);
 expect(proof.invariants.liveFreshnessProbesSplit).toBe(true);
 expect(proof.invariants.parentFailClosedCheckedOncePerFixture).toBe(true);
 expect(proof.invariants.windowsBoundedNinetySecondDriftProbes).toBe(true);
 expect(proof.invariants.thirtySecondBoundaryLimitPreserved).toBe(false);
 expect(proof.invariants.historicalProofsRewritten).toBe(false);
 for(const p of PHASE5G_EXISTING)expect(reader.read(p)).toBe(phase5GParentFacts('.').read(p));
 expect(uiPhase5FParentReader('.').violations).toEqual([]);
},90000);

it('denies forged metadata, proof hashes and weakened timeout gates without authority',()=>{
 const proof=JSON.parse(readFileSync(PHASE5G_MANIFEST,'utf8'));
 for(const mutate of [
  (p:any)=>{p.parentHead='0'.repeat(40);},
  (p:any)=>{p.paths['scripts/ui-phase5f-ci-memory-successor-guard.mjs'].afterHash='0'.repeat(64);},
  (p:any)=>{p.paths['src/app/UiPhase5BMixedVideoBoundary.test.ts'].beforeHash='0'.repeat(64);},
  (p:any)=>{p.invariants.windowsBoundedNinetySecondDriftProbes=false;},
  (p:any)=>{p.invariants.historicalProofsRewritten=true;},
  (p:any)=>{p.paths['src/services/ipc.ts']={beforeHash:null,afterHash:'0'.repeat(64)};},
 ]){
  const invalid=structuredClone(proof);mutate(invalid);
  const denied=uiPhase5GParentReader('.',invalid);
  expect(denied.violations.length).toBeGreaterThan(0);
  expect(denied.addedPaths).toEqual([]);expect(denied.afterHashes).toEqual({});
 }
}, 90000);

it('refuses edited timeout probes, Phase5F wiring and Runtime package drift in isolated fixture',()=>{
  // Live working-tree mutate — tip-inflated FixtureFiles blow the 90s Boundary budget.
  expect(uiPhase5GParentReader('.').violations).toEqual([]);
  for(const p of ['scripts/ui-phase5f-ci-memory-successor-guard.mjs','src/app/UiPhase5BMixedVideoBoundary.test.ts','src/app/ReadinessLifecycleBoundary.test.ts']){
   const before=readFileSync(p);
   try{
    writeFileSync(p,Uint8Array.from([...before,10]));
    expect(uiPhase5GParentReader('.').violations.length,p).toBeGreaterThan(0);
   }finally{writeFileSync(p,before);}
  }
  const pkg='src-tauri/runtime_packages/minimax_h3_fl2va_t2v_quality_2_2_0/recipe.yaml';
  const before=readFileSync(pkg);
  try{
   writeFileSync(pkg,Uint8Array.from([...before,10]));
   expect(uiPhase5GParentReader('.').violations).toContain('ui-phase5g-ci-timeout-runtime-package-bytes');
  }finally{writeFileSync(pkg,before);}
  const illegal='src/IllegalTimeoutOwner.js';
  try{
   writeFileSync(illegal,'unreviewed');
   expect(uiPhase5GParentReader('.').violations).toContain('ui-phase5g-ci-timeout-inventory:src');
  }finally{rmSync(illegal);}
  expect(uiPhase5GParentReader('.').violations).toEqual([]);
},90000);
