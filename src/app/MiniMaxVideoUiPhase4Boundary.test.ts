// @vitest-environment node
import { expect, it } from 'vitest';
// @ts-expect-error Node-only owned fixture.
import { readFileSync, writeFileSync, mkdirSync, mkdtempSync, copyFileSync, rmSync } from 'node:fs';
// @ts-expect-error Node-only owned fixture.
import { join, dirname, resolve } from 'node:path';
// @ts-expect-error Node-only Git object-store fixture.
import { execFileSync } from 'node:child_process';
// @ts-expect-error Node-only legal successor.
import { uiPhase4ParentReader, UI_PHASE4_PARENT, UI_PHASE4_MANIFEST } from '../../scripts/minimax-video-v2-ui-phase4-successor-guard.mjs';
// @ts-expect-error Historical proof fixture.
import { uiPhase3ParentReader, uiPhase3FixtureFiles } from '../../scripts/minimax-video-v2-ui-phase3-successor-guard.mjs';
// @ts-expect-error Historical proof reader.
import { uiPhase3CiRepairParentReader } from '../../scripts/ui-phase3-ci-repair-successor-guard.mjs';
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
},30000);
it('fresh live cache rejects edits, proofs/deletion, illegal additions and literal packages; restoration passes',()=>{
  const root=resolve('.'),fixture=mkdtempSync(join(root,'.codex-ui-phase4-probe-'));
  try {
    execFileSync('git',['init','--quiet',fixture]);
    const git=execFileSync('git',['rev-parse','--absolute-git-dir'],{cwd:root,encoding:'utf8'}).trim();
    writeFileSync(join(fixture,'.git/objects/info/alternates'),join(git,'objects')+'\n');
    const paths=[...new Set<string>(uiPhase3FixtureFiles(root))];
    for(const p of paths){mkdirSync(dirname(join(fixture,p)),{recursive:true});copyFileSync(join(root,p),join(fixture,p));}
    const pass=()=>{expect(uiPhase4ParentReader(fixture).violations).toEqual([]);};pass();
    const deny=()=>{const denied=uiPhase4ParentReader(fixture);expect(denied.violations.length).toBeGreaterThan(0);expect(denied.addedPaths).toEqual([]);expect(uiPhase3ParentReader(fixture).violations.length).toBeGreaterThan(0);};
    for(const p of ['src/features/create/CreatePage.tsx','src/app/ShellHost.tsx','docs/architecture/ui-phase3-ci-repair.json',UI_PHASE4_MANIFEST,
      paths.find((p:string)=>p.startsWith('src-tauri/runtime_packages/')&&p.endsWith('.json'))!]){
      const original=readFileSync(join(fixture,p));
      if(p===UI_PHASE4_MANIFEST){const forged=JSON.parse(original.toString());forged.parentHead='0'.repeat(40);writeFileSync(join(fixture,p),JSON.stringify(forged));}
      else writeFileSync(join(fixture,p),new Uint8Array([...original,10]));
      deny();writeFileSync(join(fixture,p),original);pass();
    }
    const original=readFileSync(join(fixture,UI_PHASE4_MANIFEST));rmSync(join(fixture,UI_PHASE4_MANIFEST));deny();writeFileSync(join(fixture,UI_PHASE4_MANIFEST),original);pass();
    const illegal=join(fixture,'src/features/create/IllegalAuthority.ts');writeFileSync(illegal,'export const authority = true;');deny();rmSync(illegal);pass();
  }finally{if(!fixture.startsWith(root+'/')&&!fixture.startsWith(root+'\\'))throw Error('outside owned workspace');if(!fixture.includes('.codex-ui-phase4-probe-'))throw Error('unexpected fixture');rmSync(fixture,{recursive:true,force:true});}
},30000);
