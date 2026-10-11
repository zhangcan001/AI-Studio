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
// @ts-expect-error Node-only Phase5G successor.
import {uiPhase5GParentReader,phase5GParentFacts,phase5GFixtureFiles,PHASE5G_MANIFEST,PHASE5G_PARENT,PHASE5G_EXISTING} from '../../scripts/ui-phase5g-ci-timeout-successor-guard.mjs';
// @ts-expect-error Immutable Phase5F parent.
import {uiPhase5FParentReader} from '../../scripts/ui-phase5f-ci-memory-successor-guard.mjs';

it('splits live freshness probes, checks parent fail-closed once, and bounds Windows drift probes at 90s',()=>{
 const phase5b=readFileSync('src/app/UiPhase5BMixedVideoBoundary.test.ts','utf8');
 expect(phase5b).toContain("it('keeps live freshness for source and shell bytes with parent fail-closed checked once'");
 expect(phase5b).toContain("it('keeps live freshness for frozen proofs, Runtime raw bytes and manifest deletion'");
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
});

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
});

it('refuses edited timeout probes, Phase5F wiring and Runtime package drift in isolated fixture',()=>{
 const workspace=resolve('.'),root=mkdtempSync(join(tmpdir(),'ai-studio-phase5g-ci-'));
 try{
  const git=execFileSync('git',['rev-parse','--path-format=absolute','--git-common-dir'],{encoding:'utf8'}).trim();
  execFileSync('git',['init','--quiet',root]);
  mkdirSync(join(root,'.git/objects/info'),{recursive:true});
  writeFileSync(join(root,'.git/objects/info/alternates'),join(git,'objects')+'\n');
  for(const p of phase5GFixtureFiles(workspace)){
   mkdirSync(dirname(join(root,p)),{recursive:true});copyFileSync(join(workspace,p),join(root,p));
  }
  expect(uiPhase5GParentReader(root).violations).toEqual([]);
  for(const p of ['scripts/ui-phase5f-ci-memory-successor-guard.mjs','src/app/UiPhase5BMixedVideoBoundary.test.ts','src/app/ReadinessLifecycleBoundary.test.ts']){
   const file=join(root,p),before=readFileSync(file);
   writeFileSync(file,Uint8Array.from([...before,10]));
   expect(uiPhase5GParentReader(root).violations.length,p).toBeGreaterThan(0);
   writeFileSync(file,before);
  }
  const pkg=join(root,'src-tauri/runtime_packages/minimax_h3_fl2va_t2v_quality_2_2_0/recipe.yaml');
  const before=readFileSync(pkg);writeFileSync(pkg,Uint8Array.from([...before,10]));
  expect(uiPhase5GParentReader(root).violations).toContain('ui-phase5g-ci-timeout-runtime-package-bytes');
  writeFileSync(pkg,before);
  const illegal=join(root,'src/IllegalTimeoutOwner.js');writeFileSync(illegal,'unreviewed');
  expect(uiPhase5GParentReader(root).violations).toContain('ui-phase5g-ci-timeout-inventory:src');
  rmSync(illegal);
  expect(uiPhase5GParentReader(root).violations).toEqual([]);
 }finally{
  if(!root.startsWith(resolve(tmpdir())+'\\')&&!root.startsWith(resolve(tmpdir())+'/'))throw Error('Fixture outside owned temp');
  rmSync(root,{recursive:true,force:true,maxRetries:3,retryDelay:100});
 }
},90000);
