// @vitest-environment node
import {expect,it} from 'vitest';
// @ts-expect-error Node-only fixture helper.
import {readFileSync,writeFileSync,copyFileSync,mkdirSync,mkdtempSync,rmSync} from 'node:fs';
// @ts-expect-error Node-only fixture helper.
import {execFileSync} from 'node:child_process';
// @ts-expect-error Node-only fixture helper.
import {tmpdir} from 'node:os';
// @ts-expect-error Node-only fixture helper.
import {join,dirname} from 'node:path';
// @ts-expect-error Node-only successor.
import {h3CloseoutParentReader,h3CloseoutParentFacts,H3_CLOSEOUT_PARENT} from '../../scripts/h3-release-closeout-phase1-guard.mjs';
// @ts-expect-error Node-only cache safety checks.
import {cachedBoundary} from '../../scripts/boundary-validation-cache.mjs';
const manifest='docs/architecture/h3-release-closeout-phase1.json';
it('validates exact scope, immutable packages, aggregates and invariants before old history',()=>{
 const proof=JSON.parse(readFileSync(manifest,'utf8'));
 const accepted=h3CloseoutParentReader('.');expect(accepted.violations).toEqual([]);
 const path='src/features/create/CreateController.ts';
 expect(accepted.read(path)).toBe(execFileSync('git',['show',`${H3_CLOSEOUT_PARENT}:${path}`],{encoding:'utf8'}).replaceAll('\r\n','\n'));
 for(const mutate of [
  (p:typeof proof)=>{p.parentHead='0'.repeat(40);},
  (p:typeof proof)=>{p.paths[path].beforeHash='0'.repeat(64);},
  (p:typeof proof)=>{p.paths[path].afterHash='0'.repeat(64);},
  (p:typeof proof)=>{p.paths['src/services/ipc.ts']={beforeHash:null,afterHash:'0'.repeat(64)};},
  ...['backend','frontend','scripts','migrations','packages','architecture'].flatMap(group=>[
   (p:typeof proof)=>{p[group].untouchedAggregateHash='0'.repeat(64);},
   (p:typeof proof)=>{p[group].afterAggregateHash='0'.repeat(64);},
   (p:typeof proof)=>{p[group].afterFiles++;}]),
  ...Object.keys(proof.invariants).map(key=>(p:typeof proof)=>{p.invariants[key]=!p.invariants[key];})]){
   const invalid=structuredClone(proof);mutate(invalid);const denied=h3CloseoutParentReader('.',invalid);
   expect(denied.violations.length).toBeGreaterThan(0);expect(denied.addedPaths).toEqual([]);
   expect(denied.read(path)).toBe(readFileSync(path,'utf8').replaceAll('\r\n','\n'));
 }
},30000);
it('fresh bytes and path sets invalidate cached acceptance, including published package drift',()=>{
 const root=mkdtempSync(join(tmpdir(),'ai-studio-h3-closeout-'));
 try {
  const git=execFileSync('git',['rev-parse','--absolute-git-dir'],{encoding:'utf8'}).trim();
  execFileSync('git',['init','--quiet',root]);mkdirSync(join(root,'.git/objects/info'),{recursive:true});
  writeFileSync(join(root,'.git/objects/info/alternates'),join(git,'objects')+'\n');
  const proof=JSON.parse(readFileSync(manifest,'utf8'));
  const queue=JSON.parse(readFileSync('docs/architecture/queue-lifecycle-repair.json','utf8'));
  for(const p of new Set([...h3CloseoutParentFacts('.').paths,...Object.keys(proof.paths),manifest,...Object.keys(queue.paths),'docs/architecture/queue-lifecycle-repair.json']) as Set<string>){
   mkdirSync(dirname(join(root,p)),{recursive:true});copyFileSync(p,join(root,p));
  }
  expect(h3CloseoutParentReader(root).violations).toEqual([]);
  for(const p of ['src/features/create/CreateController.ts','src-tauri/src/domain/asset.rs','src-tauri/runtime_packages/minimax_h3_fl2va_i2v_quality_2_1_0/recipe.yaml','src-tauri/tauri.conf.json']){
   const path=join(root,p),original=readFileSync(path,'utf8');writeFileSync(path,original+'\n# drift\n');
   const denied=h3CloseoutParentReader(root);expect(denied.violations.length).toBeGreaterThan(0);expect(denied.addedPaths).toEqual([]);
   writeFileSync(path,original);expect(h3CloseoutParentReader(root).violations).toEqual([]);
  }
  writeFileSync(join(root,'src-tauri/src/unreviewed.rs'),'// unreviewed');
  expect(h3CloseoutParentReader(root).violations).toContain('queue-lifecycle-backend-files');
 }finally{rmSync(root,{recursive:true,force:true});}
},30000);
it('shares nested replay but never trusts mtime, unchanged length or stale mutable tag identity',()=>{
 const root=mkdtempSync(join(tmpdir(),'ai-studio-boundary-cache-'));
 try{
  mkdirSync(join(root,'src'));writeFileSync(join(root,'src/a.ts'),'one');
  mkdirSync(join(root,'.git/refs/tags'),{recursive:true});writeFileSync(join(root,'.git/refs/tags/v2.1.0-personal'),'tag-one');
  let calls=0;
  const validate=()=>{calls++;return {violations:[],read:()=>readFileSync(join(root,'src/a.ts'),'utf8')};};
  const run=()=>cachedBoundary(root,'owned-cache-check',undefined,validate);
  run();run();expect(calls).toBe(1);
  writeFileSync(join(root,'README.md'),'frozen docs');run();expect(calls).toBe(2);
  writeFileSync(join(root,'src/a.ts'),'two');run();expect(calls).toBe(3);
  writeFileSync(join(root,'src/b.ts'),'extra');run();expect(calls).toBe(4);
  writeFileSync(join(root,'.git/refs/tags/v2.1.0-personal'),'tag-two');run();expect(calls).toBe(5);
 }finally{rmSync(root,{recursive:true,force:true});}
});

it('denies metadata-only amendments without replay and revokes command signature grants',()=>{
 const root=mkdtempSync(join(tmpdir(),'ai-studio-boundary-metadata-'));
 try{
  mkdirSync(join(root,'src'));mkdirSync(join(root,'docs/architecture'),{recursive:true});
  const path='docs/architecture/owned-proof.json',proof={schemaVersion:1,invariants:{readonly:true}};
  writeFileSync(join(root,path),JSON.stringify(proof));writeFileSync(join(root,'src/a.ts'),'one');
  let calls=0;
  const validate=()=>{calls++;return {violations:[],addedCommandSignatures:['readonly-command'],read:()=> 'historical bytes'};};
  cachedBoundary(root,'owned-metadata',undefined,validate,path);
  writeFileSync(join(root,'src/a.ts'),'two');
  const denied=cachedBoundary(root,'owned-metadata',{...proof,invariants:{readonly:false}},validate,path);
  expect(calls).toBe(1);expect(denied.violations.length).toBeGreaterThan(0);
  expect(denied.addedCommandSignatures).toEqual([]);expect(denied.addedPaths).toEqual([]);
  expect(denied.read('src/a.ts')).toBe('two');
  cachedBoundary(root,'owned-metadata',undefined,validate,path);expect(calls).toBe(2);
 }finally{rmSync(root,{recursive:true,force:true});}
});
