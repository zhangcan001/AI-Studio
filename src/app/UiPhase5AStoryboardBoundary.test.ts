// @vitest-environment node
import { expect, it } from 'vitest';
import { readFileSync, writeFileSync, mkdirSync, mkdtempSync, copyFileSync, rmSync } from 'node:fs';
import { join, dirname, resolve } from 'node:path';
import { execFileSync } from 'node:child_process';
import { uiPhase5AParentReader, phase5AFixtureFiles, PHASE5A_MANIFEST, PHASE5A_PARENT, PHASE5A_EXISTING, phase5AParentFacts } from '../../scripts/ui-phase5a-storyboard-successor-guard.mjs';
import { uiPhase4SerialQueueParentReader } from '../../scripts/ui-phase4-serial-queue-successor-guard.mjs';

it('validates the actual Phase5A parent and replays frozen Phase4 without rewriting historical proofs', () => {
  const reader = uiPhase5AParentReader('.');
  expect(reader.violations).toEqual([]);
  const proof = JSON.parse(readFileSync(PHASE5A_MANIFEST, 'utf8'));
  expect(proof.parentHead).toBe(PHASE5A_PARENT);
  expect(proof.planningParentHead).toBe(PHASE5A_PARENT);
  expect(proof.phaseCommitsBeforeCheckpoint).toEqual([]);
  expect(proof.database).toEqual({ migrationBefore: 44, migrationAfter: 44, newTables: 0, backupBefore: 21, backupAfter: 21 });
  for (const path of PHASE5A_EXISTING) expect(reader.read(path), path).toBe(phase5AParentFacts('.').read(path));
  expect(uiPhase4SerialQueueParentReader('.').violations).toEqual([]);
}, 90000);

it('rejects forged planning parent, scope, hashes, schema and execution invariants', () => {
  const proof = JSON.parse(readFileSync(PHASE5A_MANIFEST, 'utf8'));
  for (const mutate of [
    (p: any) => { p.planningParentHead = '0'.repeat(40); },
    (p: any) => { p.paths['src/services/tauriClient.ts'] = { afterHash: '0'.repeat(64) }; },
    (p: any) => { p.paths['src/features/shots/ShotWorkspace.tsx'].beforeHash = '0'.repeat(64); },
    (p: any) => { p.paths['src/features/shots/ShotWorkspace.tsx'].afterHash = '0'.repeat(64); },
    (p: any) => { p.database.migrationAfter = 45; },
    (p: any) => { p.invariants.preparationStartsGeneration = true; },
  ]) {
    const forged = structuredClone(proof); mutate(forged);
    const rejected = uiPhase5AParentReader('.', forged);
    expect(rejected.violations.length).toBeGreaterThan(0);
    expect(rejected.addedPaths).toEqual([]);
    expect(rejected.afterHashes).toEqual({});
  }
}, 90000);

async function withPhase5AFixture(run:(root:string,runtimePath:string)=>void){
 const workspace=resolve('.'),root=mkdtempSync(join(workspace,'.codex-phase5a-probe-'));
 try{
  const git=execFileSync('git',['rev-parse','--path-format=absolute','--git-common-dir'],{encoding:'utf8'}).trim();
  execFileSync('git',['init','--quiet',root]);
  mkdirSync(join(root,'.git/objects/info'),{recursive:true});
  writeFileSync(join(root,'.git/objects/info/alternates'),join(git,'objects')+'\n');
  for(const p of phase5AFixtureFiles(workspace)){
   mkdirSync(dirname(join(root,p)),{recursive:true});copyFileSync(join(workspace,p),join(root,p));
  }
  expect(uiPhase5AParentReader(root).violations).toEqual([]);
  const runtimePath=phase5AFixtureFiles(workspace).find((p:string)=>p.startsWith('src-tauri/runtime_packages/')&&p.endsWith('.json'))!;
  run(root,runtimePath);
 }finally{
  if(!root.startsWith(workspace+'/')&&!root.startsWith(workspace+'\\'))throw Error('Fixture outside workspace');
  if(!root.includes('.codex-phase5a-probe-'))throw Error('Unexpected fixture path');
  rmSync(root,{recursive:true,force:true});
  await new Promise<void>(resolve=>setTimeout(resolve,0));
 }
}
it.each(['src/features/shots/ShotWorkspace.tsx','src/app/ShellHost.tsx','vite.config.ts'])(
 'rejects live edited source/config: %s',p=>withPhase5AFixture(root=>{
  const path=join(root,p),original=readFileSync(path);
  try{
   writeFileSync(path,readFileSync(path,'utf8')+'\n');
   const rejected=uiPhase5AParentReader(root);
   expect(rejected.violations.length,p).toBeGreaterThan(0);
   expect(rejected.addedPaths,p).toEqual([]);
   expect(uiPhase4SerialQueueParentReader(root).violations.length,p).toBeGreaterThan(0);
  }finally{writeFileSync(path,original);}
  expect(uiPhase5AParentReader(root).violations).toEqual([]);
 }), 90000);
it.each(['docs/architecture/ui-phase4-serial-queue-successor.json',PHASE5A_MANIFEST,'runtime'] as const)(
 'rejects historical proof and exact Runtime bytes: %s',p=>withPhase5AFixture((root,runtimePath)=>{
  const path=join(root,p==='runtime'?runtimePath:p),original=readFileSync(path);
  try{
   writeFileSync(path,readFileSync(path,'utf8')+'\n');
   const rejected=uiPhase5AParentReader(root);
   expect(rejected.violations.length,p).toBeGreaterThan(0);
   expect(rejected.addedPaths,p).toEqual([]);
   expect(uiPhase4SerialQueueParentReader(root).violations.length,p).toBeGreaterThan(0);
  }finally{writeFileSync(path,original);}
  expect(uiPhase5AParentReader(root).violations).toEqual([]);
 }), 90000);
it('invalidates missing evidence and illegal queue authority additions',()=>withPhase5AFixture(root=>{
 const proofPath=join(root,PHASE5A_MANIFEST),proofBytes=readFileSync(proofPath);
 rmSync(proofPath);expect(uiPhase5AParentReader(root).violations.length).toBeGreaterThan(0);
 writeFileSync(proofPath,proofBytes);
 const extra=join(root,'src/IllegalQueueOwner.js');
 writeFileSync(extra,'export const illegalOwner = true;');
 expect(uiPhase5AParentReader(root).violations).toContain('ui-phase5a-storyboard-inventory:src');
 rmSync(extra);expect(uiPhase5AParentReader(root).violations).toEqual([]);
}), 90000);
