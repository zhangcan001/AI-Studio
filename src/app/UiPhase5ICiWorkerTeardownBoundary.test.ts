// @vitest-environment node
import {expect,it} from 'vitest';
import {readFileSync,writeFileSync,copyFileSync,mkdirSync,mkdtempSync,rmSync} from 'node:fs';
import {execFileSync} from 'node:child_process';
import {dirname,join,resolve} from 'node:path';
import {tmpdir} from 'node:os';
import {uiPhase5IParentReader,phase5IParentFacts,phase5IFixtureFiles,PHASE5I_MANIFEST,PHASE5I_PARENT,PHASE5I_EXISTING} from '../../scripts/ui-phase5i-ci-worker-teardown-successor-guard.mjs';
import {uiPhase5HParentReader} from '../../scripts/ui-phase5h-ci-timeout-budget-successor-guard.mjs';

it('keeps forks without singleFork and a dedicated Boundary CI job after long probes',()=>{
 const vite=readFileSync('vite.config.ts','utf8');
 expect(vite).toContain('pool: "forks"');
 expect(vite).toContain('singleFork: false');
 expect(vite).toContain('fileParallelism: false');
 expect(vite).toContain('isolate: true');
 expect(vite).toContain('teardownTimeout: 120_000');
 const ci=readFileSync('.github/workflows/ci.yml','utf8');
 expect(ci).toContain('timeout-minutes: 35');
 expect(ci).toContain('matrix:\n        shard: [1, 2, 3]');
 expect(ci).toContain('--maxWorkers=1 --shard=${{ matrix.shard }}/3');
 expect(ci).toContain('--exclude src/app/**/*Boundary*.test.ts');
 expect(ci).toContain('name: Frontend Boundary probes');
 expect(ci).toContain('NODE_OPTIONS: --max-old-space-size=8192');
}, 90000);

it('accepts scoped worker-teardown repair and preserves immutable historical proof bytes',()=>{
 const reader=uiPhase5IParentReader('.');
 expect(reader.violations).toEqual([]);
 const proof=JSON.parse(readFileSync(PHASE5I_MANIFEST,'utf8'));
 expect(proof.parentHead).toBe(PHASE5I_PARENT);
 expect(proof.phaseCommitsBeforeCheckpoint).toEqual([]);
 expect(proof.invariants.vitestForksSingleForkTeardown).toBe(true);
 expect(proof.invariants.fileParallelismDisabled).toBe(true);
 expect(proof.invariants.teardownTimeoutMs).toBe(120000);
 expect(proof.invariants.frontendShardJobTimeoutMinutes).toBe(35);
 expect(proof.invariants.historicalProofsRewritten).toBe(false);
 for(const p of PHASE5I_EXISTING)expect(reader.read(p)).toBe(phase5IParentFacts('.').read(p));
 expect(uiPhase5HParentReader('.').violations).toEqual([]);
}, 90000);

it('denies forged metadata, proof hashes and weakened teardown gates without authority',()=>{
 const proof=JSON.parse(readFileSync(PHASE5I_MANIFEST,'utf8'));
 for(const mutate of [
  (p:any)=>{p.parentHead='0'.repeat(40);},
  (p:any)=>{p.paths['vite.config.ts'].afterHash='0'.repeat(64);},
  (p:any)=>{p.paths['.github/workflows/ci.yml'].beforeHash='0'.repeat(64);},
  (p:any)=>{p.invariants.vitestForksSingleForkTeardown=false;},
  (p:any)=>{p.invariants.historicalProofsRewritten=true;},
  (p:any)=>{p.paths['src/services/ipc.ts']={beforeHash:null,afterHash:'0'.repeat(64)};},
 ]){
  const invalid=structuredClone(proof);mutate(invalid);
  const denied=uiPhase5IParentReader('.',invalid);
  expect(denied.violations.length).toBeGreaterThan(0);
  expect(denied.addedPaths).toEqual([]);expect(denied.afterHashes).toEqual({});
 }
}, 90000);

it('refuses edited vitest teardown config, CI budget and Runtime package drift in isolated fixture',()=>{
 const workspace=resolve('.'),root=mkdtempSync(join(tmpdir(),'ai-studio-phase5i-ci-'));
 try{
  const git=execFileSync('git',['rev-parse','--path-format=absolute','--git-common-dir'],{encoding:'utf8'}).trim();
  execFileSync('git',['init','--quiet',root]);
  mkdirSync(join(root,'.git/objects/info'),{recursive:true});
  writeFileSync(join(root,'.git/objects/info/alternates'),join(git,'objects')+'\n');
  for(const p of phase5IFixtureFiles(workspace)){
   mkdirSync(dirname(join(root,p)),{recursive:true});copyFileSync(join(workspace,p),join(root,p));
  }
  expect(uiPhase5IParentReader(root).violations).toEqual([]);
  for(const p of ['vite.config.ts','.github/workflows/ci.yml','scripts/ui-phase5h-ci-timeout-budget-successor-guard.mjs']){
   const file=join(root,p),before=readFileSync(file);
   writeFileSync(file,Uint8Array.from([...before,10]));
   expect(uiPhase5IParentReader(root).violations.length,p).toBeGreaterThan(0);
   writeFileSync(file,before);
  }
  const pkg=join(root,'src-tauri/runtime_packages/minimax_h3_fl2va_t2v_quality_2_2_0/recipe.yaml');
  const before=readFileSync(pkg);writeFileSync(pkg,Uint8Array.from([...before,10]));
  expect(uiPhase5IParentReader(root).violations).toContain('ui-phase5i-ci-worker-teardown-runtime-package-bytes');
  writeFileSync(pkg,before);
  const illegal=join(root,'src/IllegalWorkerTeardownOwner.js');writeFileSync(illegal,'unreviewed');
  expect(uiPhase5IParentReader(root).violations).toContain('ui-phase5i-ci-worker-teardown-inventory:src');
  rmSync(illegal);
  expect(uiPhase5IParentReader(root).violations).toEqual([]);
 }finally{
  if(!root.startsWith(resolve(tmpdir())+'\\')&&!root.startsWith(resolve(tmpdir())+'/'))throw Error('Fixture outside owned temp');
  rmSync(root,{recursive:true,force:true,maxRetries:3,retryDelay:100});
 }
}, 90000);
