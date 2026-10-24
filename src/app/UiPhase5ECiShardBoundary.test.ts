// @vitest-environment node
import {expect,it} from 'vitest';
import {readFileSync,writeFileSync,copyFileSync,mkdirSync,mkdtempSync,rmSync} from 'node:fs';
import {execFileSync} from 'node:child_process';
import {dirname,join,resolve} from 'node:path';
import {tmpdir} from 'node:os';
import {uiPhase5EParentReader,phase5EParentFacts,phase5EFixtureFiles,PHASE5E_MANIFEST,PHASE5E_PARENT,PHASE5E_EXISTING} from '../../scripts/ui-phase5e-ci-shard-successor-guard.mjs';
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
 const workspace=resolve('.'),root=mkdtempSync(join(tmpdir(),'ai-studio-phase5e-ci-'));
 try{
  const git=execFileSync('git',['rev-parse','--path-format=absolute','--git-common-dir'],{encoding:'utf8'}).trim();
  execFileSync('git',['init','--quiet',root]);
  mkdirSync(join(root,'.git/objects/info'),{recursive:true});
  writeFileSync(join(root,'.git/objects/info/alternates'),join(git,'objects')+'\n');
  for(const p of phase5EFixtureFiles(workspace)){
   mkdirSync(dirname(join(root,p)),{recursive:true});copyFileSync(join(workspace,p),join(root,p));
  }
  expect(uiPhase5EParentReader(root).violations).toEqual([]);
  for(const p of ['.github/workflows/ci.yml','src/features/shots/ShotWorkspace.tsx']){
   const file=join(root,p),before=readFileSync(file);
   writeFileSync(file,Uint8Array.from([...before,10]));
   expect(uiPhase5EParentReader(root).violations.length,p).toBeGreaterThan(0);
   writeFileSync(file,before);
  }
  const pkg=join(root,'src-tauri/runtime_packages/minimax_h3_fl2va_t2v_quality_2_2_0/recipe.yaml');
  const before=readFileSync(pkg);writeFileSync(pkg,Uint8Array.from([...before,10]));
  expect(uiPhase5EParentReader(root).violations).toContain('ui-phase5e-ci-shard-runtime-package-bytes');
  writeFileSync(pkg,before);
  const illegal=join(root,'src/IllegalCiOwner.js');writeFileSync(illegal,'unreviewed');
  expect(uiPhase5EParentReader(root).violations).toContain('ui-phase5e-ci-shard-inventory:src');
  rmSync(illegal);
  expect(uiPhase5EParentReader(root).violations).toEqual([]);
 }finally{
  if(!root.startsWith(resolve(tmpdir())+'\\')&&!root.startsWith(resolve(tmpdir())+'/'))throw Error('Fixture outside owned temp');
  rmSync(root,{recursive:true,force:true,maxRetries:3,retryDelay:100});
 }
}, 90000);
