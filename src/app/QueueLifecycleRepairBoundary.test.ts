// @ts-expect-error Node-only successor fixture.
import {MINIMAX_VIDEO_PHASE1_FILES} from '../../scripts/readiness-lifecycle-successor-guard.mjs';
// @ts-expect-error Node-only successor.
import {minimaxVideoPhase2FixtureFiles,MINIMAX_VIDEO_PHASE2_MANIFEST} from '../../scripts/minimax-video-phase2-successor-guard.mjs';
// @vitest-environment node
import {expect,it} from 'vitest';
// @ts-expect-error Node-only boundary fixture.
import {readFileSync,writeFileSync,copyFileSync,mkdirSync,mkdtempSync,rmSync} from 'node:fs';
// @ts-expect-error Node-only boundary fixture.
import {execFileSync} from 'node:child_process';
// @ts-expect-error Node-only boundary fixture.
import {tmpdir} from 'node:os';
// @ts-expect-error Node-only boundary fixture.
import {join,dirname} from 'node:path';
// @ts-expect-error Node-only successor.
import {queueLifecycleParentReader,queueLifecycleParentFacts,QUEUE_LIFECYCLE_PARENT} from '../../scripts/queue-lifecycle-repair-successor-guard.mjs';
const manifest='docs/architecture/queue-lifecycle-repair.json';
const successorManifest='docs/architecture/readiness-lifecycle-fix.json';

it('accepts only the reviewed lifecycle repair before replaying immutable source',()=>{
 const proof=JSON.parse(readFileSync(manifest,'utf8'));
 const accepted=queueLifecycleParentReader('.');expect(accepted.violations).toEqual([]);
 const path='src-tauri/src/application/production_queue_service.rs';
 expect(accepted.read(path)).toBe(execFileSync('git',['show',`${QUEUE_LIFECYCLE_PARENT}:${path}`],{encoding:'utf8'}).replaceAll('\r\n','\n'));
 for(const mutate of [
  (p:typeof proof)=>{p.parentHead='0'.repeat(40);},
  (p:typeof proof)=>{p.paths[path].beforeHash='0'.repeat(64);},
  (p:typeof proof)=>{p.paths[path].afterHash='0'.repeat(64);},
  (p:typeof proof)=>{p.paths['src/services/ipc.ts']={beforeHash:null,afterHash:'0'.repeat(64)};},
  ...Object.keys(proof.invariants).map(key=>(p:typeof proof)=>{p.invariants[key]=!p.invariants[key];}),
  ...['backend','tests','frontend','scripts','migrations','packages','architecture'].flatMap(group=>[
   (p:typeof proof)=>{p[group].untouchedAggregateHash='0'.repeat(64);},
   (p:typeof proof)=>{p[group].afterAggregateHash='0'.repeat(64);},
   (p:typeof proof)=>{p[group].afterFiles++;}]),
 ]){
  const invalid=structuredClone(proof);mutate(invalid);const denied=queueLifecycleParentReader('.',invalid);
  expect(denied.violations.length).toBeGreaterThan(0);expect(denied.addedPaths).toEqual([]);
  expect(denied.read(path)).toBe(readFileSync(path,'utf8').replaceAll('\r\n','\n'));
 }
},30000);

it('denies drift in repaired sources, untouched files, historical proofs, packages and frozen configs',()=>{
 const root=mkdtempSync(join(tmpdir(),'ai-studio-queue-boundary-'));
 try {
  const git=execFileSync('git',['rev-parse','--absolute-git-dir'],{encoding:'utf8'}).trim();
  execFileSync('git',['init','--quiet',root]);mkdirSync(join(root,'.git/objects/info'),{recursive:true});
  writeFileSync(join(root,'.git/objects/info/alternates'),join(git,'objects')+'\n');
  const proof=JSON.parse(readFileSync(manifest,'utf8'));
  const successorProof=JSON.parse(readFileSync(successorManifest,'utf8'));
  for(const p of new Set([...MINIMAX_VIDEO_PHASE1_FILES,...minimaxVideoPhase2FixtureFiles('.'),...queueLifecycleParentFacts('.').paths,...Object.keys(proof.paths),manifest,...Object.keys(successorProof.paths),successorManifest,'docs/architecture/readiness-post-run-fix.json']) as Set<string>){
   mkdirSync(dirname(join(root,p)),{recursive:true});copyFileSync(p,join(root,p));
  }
  expect(queueLifecycleParentReader(root).violations).toEqual([]);
  for(const p of ['src-tauri/src/application/production_queue_service.rs','src-tauri/src/domain/asset.rs',
   'docs/architecture/h3-release-closeout-phase1.json','src-tauri/runtime_packages/minimax_h3_fl2va_i2v_quality_2_2_0/recipe.yaml','src-tauri/tauri.conf.json']){
   const path=join(root,p),original=readFileSync(path,'utf8');writeFileSync(path,original+'\n# drift\n');
   const denied=queueLifecycleParentReader(root);expect(denied.violations.length).toBeGreaterThan(0);expect(denied.addedPaths).toEqual([]);
   writeFileSync(path,original);expect(queueLifecycleParentReader(root).violations).toEqual([]);
  }
  writeFileSync(join(root,'src-tauri/src/unreviewed.rs'),'// unreviewed');
  expect(queueLifecycleParentReader(root).violations).toContain('minimax-video-phase2-backend-files');
 }finally{rmSync(root,{recursive:true,force:true});}
},30000);
