// @vitest-environment node
import {expect,it} from 'vitest';
// @ts-expect-error Node-only boundary evidence.
import {readFileSync,writeFileSync,copyFileSync,mkdirSync,mkdtempSync,rmSync} from 'node:fs';
// @ts-expect-error Node-only boundary evidence.
import {execFileSync} from 'node:child_process';
// @ts-expect-error Node-only boundary evidence.
import {tmpdir} from 'node:os';
// @ts-expect-error Node-only boundary evidence.
import {join,dirname} from 'node:path';
// @ts-expect-error Build-time helper, not browser runtime.
import {h3RepairParentReader,H3_REPAIR_PARENT,h3ParentFacts} from '../../scripts/h3-generation-repair-successor-guard.mjs';
const manifest='docs/architecture/h3-generation-repair-successor.json';
it('validates the exact runtime repair before projecting immutable publication history',()=>{
 const accepted=h3RepairParentReader('.');expect(accepted.violations).toEqual([]);
 const p='src/features/create/CreateInputs.tsx';
 expect(readFileSync(p,'utf8')).toContain('视频分辨率');
 expect(accepted.read(p)).toBe(execFileSync('git',['show',`${H3_REPAIR_PARENT}:${p}`],{encoding:'utf8'}).replaceAll('\r\n','\n'));
 const proof=JSON.parse(readFileSync(manifest,'utf8'));
 const mutations=[(x:typeof proof)=>{x.parentHead='0'.repeat(40);},(x:typeof proof)=>{x.paths[p].afterHash='0'.repeat(64);},
  (x:typeof proof)=>{x.backend.untouchedAggregateHash='0'.repeat(64);},(x:typeof proof)=>{x.frontend.afterFiles++;},
  ...Object.keys(proof.invariants).map(k=>(x:typeof proof)=>{x.invariants[k]=!x.invariants[k];})];
 for(const mutate of mutations){const changed=structuredClone(proof);mutate(changed);const rejected=h3RepairParentReader('.',changed);
  expect(rejected.violations.length).toBeGreaterThan(0);expect(rejected.addedPaths).toEqual([]);
  expect(rejected.read(p)).toBe(readFileSync(p,'utf8').replaceAll('\r\n','\n'));}
},30000);
it('rejects fresh live drift and extra files without hiding it behind cached evidence',()=>{
 const root=mkdtempSync(join(tmpdir(),'ai-studio-h3-repair-'));
 try{
  const gitDir=execFileSync('git',['rev-parse','--absolute-git-dir'],{encoding:'utf8'}).trim();
  execFileSync('git',['init','--quiet',root]);mkdirSync(join(root,'.git/objects/info'),{recursive:true});
  writeFileSync(join(root,'.git/objects/info/alternates'),join(gitDir,'objects')+'\n');
  const proof=JSON.parse(readFileSync(manifest,'utf8'));
  const paths=[...h3ParentFacts('.').paths,...Object.keys(proof.paths),manifest];
  for(const p of new Set(paths) as Set<string>){mkdirSync(dirname(join(root,p)),{recursive:true});copyFileSync(p,join(root,p));}
  expect(h3RepairParentReader(root).violations).toEqual([]);
  for(const p of ['src/features/create/CreateInputs.tsx','src-tauri/src/domain/asset.rs','src-tauri/tauri.conf.json']){
   const path=join(root,p),original=readFileSync(path,'utf8');writeFileSync(path,original+'\n// unauthorized drift\n');
   const rejected=h3RepairParentReader(root);expect(rejected.violations.length).toBeGreaterThan(0);expect(rejected.addedPaths).toEqual([]);
   writeFileSync(path,original);expect(h3RepairParentReader(root).violations).toEqual([]);
  }
  writeFileSync(join(root,'src-tauri/src/unreviewed.rs'),'// extra source');
  expect(h3RepairParentReader(root).violations).toContain('h3-repair-backend-files');
 }finally{rmSync(root,{recursive:true,force:true});}
},30000);
