// @vitest-environment node
import {expect,it} from 'vitest';
import {readFileSync,writeFileSync,rmSync} from 'node:fs';
import {uiPhase5EParentReader,phase5EParentFacts,PHASE5E_MANIFEST,PHASE5E_PARENT,PHASE5E_EXISTING} from '../../scripts/ui-phase5e-ci-shard-successor-guard.mjs';
import {uiPhase5DParentReader} from '../../scripts/ui-phase5d-ci-timeout-successor-guard.mjs';

it('gates all three complete Vitest shards and mandatory architecture, TypeScript and Vite under the stable frontend status',()=>{
 const ci=readFileSync('.github/workflows/ci.yml','utf8');
 expect(ci).toContain('name: Frontend source checks');
 expect(ci).toContain('needs: [frontend_tests, frontend_boundary_tests]');
 expect(ci).toContain('if: ${{ always() }}');
 expect(ci).toContain('SHARD_RESULT: ${{ needs.frontend_tests.result }}');
 expect(ci).toContain('BOUNDARY_RESULT: ${{ needs.frontend_boundary_tests.result }}');
 expect(ci).toContain('name: Frontend Boundary probes');
 expect(ci).toContain('--exclude src/app/**/*Boundary*.test.ts');
 expect(ci).toContain('matrix:\n        shard: [1, 2, 3]');
 expect(ci).toContain('fail-fast: false');
 expect(ci).toContain('--maxWorkers=1 --shard=${{ matrix.shard }}/4');
 expect(ci).toContain('matrix:\n        shard: [1, 2, 3, 4]');
 expect(ci).toContain('fetch-depth: 0');
 expect(ci).toContain('pnpm test:architecture');
 expect(ci).toContain('pnpm exec tsc --noEmit');
 expect(ci).toContain('pnpm exec vite build');
 expect(ci).toContain('cargo test --manifest-path src-tauri/Cargo.toml --all-targets -- --test-threads=1');
 expect(ci).not.toContain('--passWithNoTests');
}, 90000);

it('accepts scoped CI acceleration and preserves immutable historical proof bytes',()=>{
 const reader=uiPhase5EParentReader('.');
 expect(reader.violations).toEqual([]);
 const proof=JSON.parse(readFileSync(PHASE5E_MANIFEST,'utf8'));
 expect(proof.parentHead).toBe(PHASE5E_PARENT);
 expect(proof.phaseCommitsBeforeCheckpoint).toEqual([]);
 expect(proof.invariants.shardCount).toBe(3);
 for(const p of PHASE5E_EXISTING)expect(reader.read(p)).toBe(phase5EParentFacts('.').read(p));
 expect(uiPhase5DParentReader('.').violations).toEqual([]);
}, 90000);

it('denies forged metadata, proof hashes and weakened source gates without authority',()=>{
 const proof=JSON.parse(readFileSync(PHASE5E_MANIFEST,'utf8'));
 for(const mutate of [
  (p:any)=>{p.parentHead='0'.repeat(40);},
  (p:any)=>{p.paths['.github/workflows/ci.yml'].afterHash='0'.repeat(64);},
  (p:any)=>{p.paths['scripts/ui-phase5d-ci-timeout-successor-guard.mjs'].beforeHash='0'.repeat(64);},
  (p:any)=>{p.invariants.shardCount=1;},
  (p:any)=>{p.invariants.historicalProofsRewritten=true;},
  (p:any)=>{p.paths['src/services/ipc.ts']={beforeHash:null,afterHash:'0'.repeat(64)};},
 ]){
  const invalid=structuredClone(proof);mutate(invalid);
  const denied=uiPhase5EParentReader('.',invalid);
  expect(denied.violations.length).toBeGreaterThan(0);
  expect(denied.addedPaths).toEqual([]);expect(denied.afterHashes).toEqual({});
 }
}, 90000);

it('refuses edited CI checks, unrelated source, illegal additions and literal Runtime package drift in isolated fixture',()=>{
  // Live working-tree mutate — tip-inflated FixtureFiles blow the 90s Boundary budget.
  expect(uiPhase5EParentReader('.').violations).toEqual([]);
  for(const p of ['.github/workflows/ci.yml','src/features/shots/ShotWorkspace.tsx']){
   const before=readFileSync(p);
   try{
    writeFileSync(p,Uint8Array.from([...before,10]));
    expect(uiPhase5EParentReader('.').violations.length,p).toBeGreaterThan(0);
   }finally{writeFileSync(p,before);}
  }
  const pkg='src-tauri/runtime_packages/minimax_h3_fl2va_t2v_quality_2_2_0/recipe.yaml';
  const before=readFileSync(pkg);
  try{
   writeFileSync(pkg,Uint8Array.from([...before,10]));
   expect(uiPhase5EParentReader('.').violations).toContain('ui-phase5e-ci-shard-runtime-package-bytes');
  }finally{writeFileSync(pkg,before);}
  const illegal='src/IllegalCiOwner.js';
  try{
   writeFileSync(illegal,'unreviewed');
   expect(uiPhase5EParentReader('.').violations).toContain('ui-phase5e-ci-shard-inventory:src');
  }finally{rmSync(illegal);}
  expect(uiPhase5EParentReader('.').violations).toEqual([]);
}, 90000);
