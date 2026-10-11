// @vitest-environment node
import {expect,it} from 'vitest';
// @ts-expect-error Node-only proof test.
import {readFileSync,writeFileSync,copyFileSync,mkdirSync,mkdtempSync,rmSync} from 'node:fs';
// @ts-expect-error Node-only fixture.
import {execFileSync} from 'node:child_process';
// @ts-expect-error Node-only paths.
import {dirname,join,resolve} from 'node:path';
// @ts-expect-error Node-only tmpdir.
import {tmpdir} from 'node:os';
// @ts-expect-error Node-only Phase5F successor.
import {uiPhase5FParentReader,phase5FParentFacts,phase5FFixtureFiles,PHASE5F_MANIFEST,PHASE5F_PARENT,PHASE5F_EXISTING} from '../../scripts/ui-phase5f-ci-memory-successor-guard.mjs';
// @ts-expect-error Immutable Phase5E parent.
import {uiPhase5EParentReader} from '../../scripts/ui-phase5e-ci-shard-successor-guard.mjs';

it('keeps three shards, raises the frontend heap ceiling, and leaves individual bounded probes in force',()=>{
 const ci=readFileSync('.github/workflows/ci.yml','utf8');
 expect(ci).toContain('NODE_OPTIONS: --max-old-space-size=8192');
 expect(ci).toContain('matrix:\n        shard: [1, 2, 3]');
 expect(ci).toContain('--maxWorkers=1 --shard=${{ matrix.shard }}/3');
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
 const workspace=resolve('.'),root=mkdtempSync(join(tmpdir(),'ai-studio-phase5f-ci-'));
 try{
  const git=execFileSync('git',['rev-parse','--path-format=absolute','--git-common-dir'],{encoding:'utf8'}).trim();
  execFileSync('git',['init','--quiet',root]);
  mkdirSync(join(root,'.git/objects/info'),{recursive:true});
  writeFileSync(join(root,'.git/objects/info/alternates'),join(git,'objects')+'\n');
  for(const p of phase5FFixtureFiles(workspace)){
   mkdirSync(dirname(join(root,p)),{recursive:true});copyFileSync(join(workspace,p),join(root,p));
  }
  expect(uiPhase5FParentReader(root).violations).toEqual([]);
  for(const p of ['.github/workflows/ci.yml','scripts/boundary-validation-cache.mjs','src/app/ReadinessLifecycleBoundary.test.ts']){
   const file=join(root,p),before=readFileSync(file);
   writeFileSync(file,Uint8Array.from([...before,10]));
   expect(uiPhase5FParentReader(root).violations.length,p).toBeGreaterThan(0);
   writeFileSync(file,before);
  }
  const pkg=join(root,'src-tauri/runtime_packages/minimax_h3_fl2va_t2v_quality_2_2_0/recipe.yaml');
  const before=readFileSync(pkg);writeFileSync(pkg,Uint8Array.from([...before,10]));
  expect(uiPhase5FParentReader(root).violations).toContain('ui-phase5f-ci-memory-runtime-package-bytes');
  writeFileSync(pkg,before);
  const illegal=join(root,'src/IllegalMemoryOwner.js');writeFileSync(illegal,'unreviewed');
  expect(uiPhase5FParentReader(root).violations).toContain('ui-phase5f-ci-memory-inventory:src');
  rmSync(illegal);
  expect(uiPhase5FParentReader(root).violations).toEqual([]);
 }finally{
  if(!root.startsWith(resolve(tmpdir())+'\\')&&!root.startsWith(resolve(tmpdir())+'/'))throw Error('Fixture outside owned temp');
  rmSync(root,{recursive:true,force:true,maxRetries:3,retryDelay:100});
 }
}, 90000);
