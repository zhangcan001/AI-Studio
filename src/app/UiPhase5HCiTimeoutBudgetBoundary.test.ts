// @vitest-environment node
import {expect,it} from 'vitest';
import {readFileSync,writeFileSync,copyFileSync,mkdirSync,mkdtempSync,rmSync} from 'node:fs';
import {execFileSync} from 'node:child_process';
import {dirname,join,resolve} from 'node:path';
import {tmpdir} from 'node:os';
import {uiPhase5HParentReader,phase5HParentFacts,phase5HFixtureFiles,PHASE5H_MANIFEST,PHASE5H_PARENT,PHASE5H_EXISTING} from '../../scripts/ui-phase5h-ci-timeout-budget-successor-guard.mjs';
import {uiPhase5GParentReader} from '../../scripts/ui-phase5g-ci-timeout-successor-guard.mjs';

it('budgets every Boundary successor/drift it() at a fail-closed 90s Windows ceiling',()=>{
 const files=[
  'src/app/H3GenerationRepairBoundary.test.ts',
  'src/app/MiniMaxVideoPhase1Boundary.test.ts',
  'src/app/MiniMaxVideoUiPhase4Boundary.test.ts',
  'src/app/UiPhase4SerialQueueBoundary.test.ts',
  'src/app/UiPhase5AStoryboardBoundary.test.ts',
  'src/app/UiPhase5BMixedVideoBoundary.test.ts',
  'src/app/ReadinessLifecycleBoundary.test.ts',
 ];
 for(const file of files){
  const text=readFileSync(file,'utf8');
  const its=[...text.matchAll(/^[ \t]*it\s*\(/gm)];
  expect(its.length,file).toBeGreaterThan(0);
  const budgets=[...text.matchAll(/\}\)?\s*,\s*90000\s*\)/g)];
  expect(budgets.length,file).toBeGreaterThanOrEqual(its.length);
  expect(text).not.toMatch(/\}\)?\s*,\s*30000\s*\)/);
  expect(text).not.toMatch(/\}\)?\s*,\s*5000\s*\)/);
 }
}, 90000);

it('accepts scoped CI timeout-budget repair and preserves immutable historical proof bytes',()=>{
 const reader=uiPhase5HParentReader('.');
 expect(reader.violations).toEqual([]);
 const proof=JSON.parse(readFileSync(PHASE5H_MANIFEST,'utf8'));
 expect(proof.parentHead).toBe(PHASE5H_PARENT);
 expect(proof.phaseCommitsBeforeCheckpoint).toEqual([]);
 expect(proof.invariants.allBoundarySuccessorProbesBudgetedAtNinetySeconds).toBe(true);
 expect(proof.invariants.windowsBoundedNinetySecondDriftProbes).toBe(true);
 expect(proof.invariants.historicalProofsRewritten).toBe(false);
 for(const p of PHASE5H_EXISTING)expect(reader.read(p)).toBe(phase5HParentFacts('.').read(p));
 expect(uiPhase5GParentReader('.').violations).toEqual([]);
}, 90000);

it('denies forged metadata, proof hashes and weakened timeout-budget gates without authority',()=>{
 const proof=JSON.parse(readFileSync(PHASE5H_MANIFEST,'utf8'));
 for(const mutate of [
  (p:any)=>{p.parentHead='0'.repeat(40);},
  (p:any)=>{p.paths['scripts/ui-phase5g-ci-timeout-successor-guard.mjs'].afterHash='0'.repeat(64);},
  (p:any)=>{p.paths['src/app/H3GenerationRepairBoundary.test.ts'].beforeHash='0'.repeat(64);},
  (p:any)=>{p.invariants.allBoundarySuccessorProbesBudgetedAtNinetySeconds=false;},
  (p:any)=>{p.invariants.historicalProofsRewritten=true;},
  (p:any)=>{p.paths['src/services/ipc.ts']={beforeHash:null,afterHash:'0'.repeat(64)};},
 ]){
  const invalid=structuredClone(proof);mutate(invalid);
  const denied=uiPhase5HParentReader('.',invalid);
  expect(denied.violations.length).toBeGreaterThan(0);
  expect(denied.addedPaths).toEqual([]);expect(denied.afterHashes).toEqual({});
 }
}, 90000);

it('refuses edited timeout budgets, Phase5G wiring and Runtime package drift in isolated fixture',()=>{
 const workspace=resolve('.'),root=mkdtempSync(join(tmpdir(),'ai-studio-phase5h-ci-'));
 try{
  const git=execFileSync('git',['rev-parse','--path-format=absolute','--git-common-dir'],{encoding:'utf8'}).trim();
  execFileSync('git',['init','--quiet',root]);
  mkdirSync(join(root,'.git/objects/info'),{recursive:true});
  writeFileSync(join(root,'.git/objects/info/alternates'),join(git,'objects')+'\n');
  for(const p of phase5HFixtureFiles(workspace)){
   mkdirSync(dirname(join(root,p)),{recursive:true});copyFileSync(join(workspace,p),join(root,p));
  }
  expect(uiPhase5HParentReader(root).violations).toEqual([]);
  for(const p of ['scripts/ui-phase5g-ci-timeout-successor-guard.mjs','src/app/H3GenerationRepairBoundary.test.ts','src/app/UiPhase4SerialQueueBoundary.test.ts']){
   const file=join(root,p),before=readFileSync(file);
   writeFileSync(file,Uint8Array.from([...before,10]));
   expect(uiPhase5HParentReader(root).violations.length,p).toBeGreaterThan(0);
   writeFileSync(file,before);
  }
  const pkg=join(root,'src-tauri/runtime_packages/minimax_h3_fl2va_t2v_quality_2_2_0/recipe.yaml');
  const before=readFileSync(pkg);writeFileSync(pkg,Uint8Array.from([...before,10]));
  expect(uiPhase5HParentReader(root).violations).toContain('ui-phase5h-ci-timeout-budget-runtime-package-bytes');
  writeFileSync(pkg,before);
  const illegal=join(root,'src/IllegalTimeoutBudgetOwner.js');writeFileSync(illegal,'unreviewed');
  expect(uiPhase5HParentReader(root).violations).toContain('ui-phase5h-ci-timeout-budget-inventory:src');
  rmSync(illegal);
  expect(uiPhase5HParentReader(root).violations).toEqual([]);
 }finally{
  if(!root.startsWith(resolve(tmpdir())+'\\')&&!root.startsWith(resolve(tmpdir())+'/'))throw Error('Fixture outside owned temp');
  rmSync(root,{recursive:true,force:true,maxRetries:3,retryDelay:100});
 }
}, 90000);
