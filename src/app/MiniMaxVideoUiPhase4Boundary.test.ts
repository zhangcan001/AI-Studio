// @vitest-environment node
import { expect, it } from 'vitest';
import { readFileSync, writeFileSync, mkdirSync, mkdtempSync, copyFileSync, rmSync } from 'node:fs';
import { join, dirname, resolve } from 'node:path';
import { execFileSync } from 'node:child_process';
import { uiPhase4ParentReader, UI_PHASE4_PARENT, UI_PHASE4_MANIFEST } from '../../scripts/minimax-video-v2-ui-phase4-successor-guard.mjs';
import { uiPhase3ParentReader, uiPhase3FixtureFiles } from '../../scripts/minimax-video-v2-ui-phase3-successor-guard.mjs';
import { uiPhase3CiRepairParentReader } from '../../scripts/ui-phase3-ci-repair-successor-guard.mjs';
import { uiPhase4CiRepairParentReader, UI_PHASE4_CI_REPAIR_MANIFEST } from '../../scripts/ui-phase4-ci-repair-successor-guard.mjs';
it('validates style CI repair before the immutable Phase4 checkpoint and rejects forged repair evidence',()=>{
  const proof=JSON.parse(readFileSync(UI_PHASE4_CI_REPAIR_MANIFEST,'utf8'));
  expect(proof.parentHead).toBe('70dff1874d9314ce800c58c3bd49a99b04321e16');
  const accepted=uiPhase4CiRepairParentReader('.');expect(accepted.violations).toEqual([]);
  expect(accepted.read('scripts/style-boundary-guard.mjs')).not.toContain('const classCallers=');
  expect(readFileSync('scripts/style-boundary-guard.mjs','utf8')).toContain('const classCallers=');
  for(const modify of [
    (p:typeof proof)=>{p.parentHead='0'.repeat(40);},
    (p:typeof proof)=>{p.invariants.validationOnly=false;},
    (p:typeof proof)=>{p.paths['scripts/style-boundary-guard.mjs'].afterHash='0'.repeat(64);},
    (p:typeof proof)=>{p.paths['src/services/ipc.ts']={beforeHash:null,afterHash:'0'.repeat(64)};},
  ]){const bad=structuredClone(proof);modify(bad);const denied=uiPhase4CiRepairParentReader('.',bad);expect(denied.violations.length).toBeGreaterThan(0);expect(denied.addedPaths).toEqual([]);}
}, 90000);
it('validates Phase4 before projecting immutable repair and UI proofs; rejects forged evidence',()=>{
  const proof=JSON.parse(readFileSync(UI_PHASE4_MANIFEST,'utf8'));
  expect(proof.parentHead).toBe(UI_PHASE4_PARENT);expect(proof.phaseCommitsBeforeCheckpoint).toEqual([]);
  const accepted=uiPhase4ParentReader('.');expect(accepted.violations).toEqual([]);
  expect(uiPhase3CiRepairParentReader('.').violations).toEqual([]);expect(uiPhase3ParentReader('.').violations).toEqual([]);
  expect(accepted.read('src/features/create/CreatePage.tsx')).not.toContain('CreateVideoWorkspace');
  expect(readFileSync('src/features/create/CreatePage.tsx','utf8')).toContain('CreateVideoWorkspace');
  expect(proof.database).toEqual({migrationBefore:43,migrationAfter:43,newTables:0,backupBefore:21,backupAfter:21});
  for(const modify of [
    (p:typeof proof)=>{p.parentHead='0'.repeat(40);},
    (p:typeof proof)=>{p.invariants.schemaChanged=true;},
    (p:typeof proof)=>{p.paths['src/features/create/CreatePage.tsx'].afterHash='0'.repeat(64);},
    (p:typeof proof)=>{p.frontend.untouchedAggregateHash='0'.repeat(64);},
    (p:typeof proof)=>{p.paths['src/services/ipc.ts']={beforeHash:null,afterHash:'0'.repeat(64)};},
  ]) {const invalid=structuredClone(proof);modify(invalid);const denied=uiPhase4ParentReader('.',invalid);expect(denied.violations.length).toBeGreaterThan(0);expect(denied.addedPaths).toEqual([]);}
}, 90000);
// Rebuild the serial probes at one owned path. Historical immutable caches are
// root-keyed; unique paths per assertion retain duplicate trees until worker exit.
let phase4Fixture: string | undefined;
async function withPhase4Probe(run:(fixture:string,paths:string[])=>void){
 const root=resolve('.'),fixture=phase4Fixture ??= mkdtempSync(join(root,'.codex-ui-phase4-probe-'));
 mkdirSync(fixture,{recursive:true});
 try{
  execFileSync('git',['init','--quiet',fixture]);
  const git=execFileSync('git',['rev-parse','--absolute-git-dir'],{cwd:root,encoding:'utf8'}).trim();
  writeFileSync(join(fixture,'.git/objects/info/alternates'),join(git,'objects')+'\n');
  const paths=[...new Set<string>(uiPhase3FixtureFiles(root))];
  for(const p of paths){mkdirSync(dirname(join(fixture,p)),{recursive:true});copyFileSync(join(root,p),join(fixture,p));}
  expect(uiPhase4ParentReader(fixture).violations).toEqual([]);
  run(fixture,paths);
 }finally{
  if(!fixture.startsWith(root+'/')&&!fixture.startsWith(root+'\\'))throw Error('outside owned workspace');
  if(!fixture.includes('.codex-ui-phase4-probe-'))throw Error('unexpected fixture');
  rmSync(fixture,{recursive:true,force:true});
  // Allow Vitest worker RPC updates between expensive synchronous proof replays.
  await new Promise<void>(resolve=>setTimeout(resolve,0));
 }
}
const denyPhase4=(fixture:string)=>{
 const denied=uiPhase4ParentReader(fixture);
 expect(denied.violations.length).toBeGreaterThan(0);
 expect(denied.addedPaths).toEqual([]);
 expect(uiPhase3ParentReader(fixture).violations.length).toBeGreaterThan(0);
};
it.each(['src/features/create/CreatePage.tsx','scripts/style-boundary-guard.mjs','src/app/ShellHost.tsx'])('rejects edited source and style bytes: %s',p=>withPhase4Probe(fixture=>{
  const path=join(fixture,p),original=readFileSync(path);
  writeFileSync(path,new Uint8Array([...original,10]));
  denyPhase4(fixture);writeFileSync(path,original);
 expect(uiPhase4ParentReader(fixture).violations).toEqual([]);
}), 90000);
it.each(['docs/architecture/ui-phase3-ci-repair.json',UI_PHASE4_MANIFEST,UI_PHASE4_CI_REPAIR_MANIFEST])('rejects altered Phase3 and Phase4 proof: %s',p=>withPhase4Probe(fixture=>{

  const path=join(fixture,p),original=readFileSync(path);
  if(p===UI_PHASE4_MANIFEST||p===UI_PHASE4_CI_REPAIR_MANIFEST){
   const forged=JSON.parse(original.toString());forged.parentHead='0'.repeat(40);
   writeFileSync(path,JSON.stringify(forged));
  }else writeFileSync(path,new Uint8Array([...original,10]));
  denyPhase4(fixture);writeFileSync(path,original);
 expect(uiPhase4ParentReader(fixture).violations).toEqual([]);
}), 90000);
it.each(['runtime','missing-proof','illegal-source'] as const)(
 'rejects literal Runtime, missing proof or illegal source: %s',kind=>withPhase4Probe((fixture,paths)=>{
  if(kind==='runtime'){
   const p=paths.find(p=>p.startsWith('src-tauri/runtime_packages/')&&p.endsWith('.json'))!;
   const path=join(fixture,p),original=readFileSync(path);
   try{writeFileSync(path,new Uint8Array([...original,10]));denyPhase4(fixture);}
   finally{writeFileSync(path,original);}
  }else if(kind==='missing-proof'){
   const path=join(fixture,UI_PHASE4_MANIFEST),original=readFileSync(path);
   try{rmSync(path);denyPhase4(fixture);}
   finally{writeFileSync(path,original);}
  }else{
   const path=join(fixture,'src/features/create/IllegalAuthority.ts');
   try{writeFileSync(path,'export const authority = true;');denyPhase4(fixture);}
   finally{rmSync(path,{force:true});}
  }
  expect(uiPhase4ParentReader(fixture).violations).toEqual([]);
 }), 90000);
