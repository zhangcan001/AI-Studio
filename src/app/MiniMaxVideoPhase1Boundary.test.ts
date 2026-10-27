// @vitest-environment node
import {expect,it} from 'vitest';
import {readFileSync,writeFileSync,rmSync} from 'node:fs';
import {execFileSync} from 'node:child_process';
import {minimaxVideoPhase1ParentReader,readinessLifecycleParentReader,MINIMAX_VIDEO_PHASE1_PARENT,MINIMAX_VIDEO_PHASE1_MANIFEST} from '../../scripts/readiness-lifecycle-successor-guard.mjs';
import {MINIMAX_VIDEO_PHASE2_MANIFEST} from '../../scripts/minimax-video-phase2-successor-guard.mjs';
it('validates current live aggregates before projecting historical source, with no schema/package/UI changes',()=>{
 const proof=JSON.parse(readFileSync(MINIMAX_VIDEO_PHASE1_MANIFEST,'utf8'));
 const accepted=minimaxVideoPhase1ParentReader('.');expect(accepted.violations).toEqual([]);
 const chain=readinessLifecycleParentReader('.');expect(chain.violations).toEqual([]);
 expect(chain.backendAggregateSha256).toBe(JSON.parse(readFileSync(MINIMAX_VIDEO_PHASE2_MANIFEST,'utf8')).backend.afterAggregateHash);
 expect(proof.invariants.schemaChanged).toBe(false);expect(proof.invariants.backupFormatChanged).toBe(false);
 expect(proof.backend.afterAggregateHash).not.toBe(proof.backend.beforeAggregateHash);
 for(const group of ['migrations','packages','styles'])expect(proof[group].afterAggregateHash).toBe(proof[group].beforeAggregateHash);
 const path='src-tauri/src/application/generation_service.rs';
 expect(accepted.read(path)).toBe(execFileSync('git',['show',`${MINIMAX_VIDEO_PHASE1_PARENT}:${path}`],{encoding:'utf8'}).replaceAll('\r\n','\n'));
 for(const mutate of [
  (p:typeof proof)=>{p.parentHead='0'.repeat(40);},
  (p:typeof proof)=>{p.paths[path].beforeHash='0'.repeat(64);},
  (p:typeof proof)=>{p.paths[path].afterHash='0'.repeat(64);},
  (p:typeof proof)=>{p.paths['src/services/ipc.ts']={beforeHash:null,afterHash:'0'.repeat(64)};},
  ...Object.keys(proof.invariants).map(key=>(p:typeof proof)=>{p.invariants[key]=!p.invariants[key];}),
  ...['backend','tests','frontend','styles','scripts','migrations','packages','architecture'].flatMap(group=>[
   (p:typeof proof)=>{p[group].beforeAggregateHash='0'.repeat(64);},
   (p:typeof proof)=>{p[group].untouchedAggregateHash='0'.repeat(64);},
   (p:typeof proof)=>{p[group].afterAggregateHash='0'.repeat(64);},
   (p:typeof proof)=>{p[group].afterFiles++;}]),
 ]){
  const invalid=structuredClone(proof);mutate(invalid);const denied=minimaxVideoPhase1ParentReader('.',invalid);
  expect(denied.violations.length).toBeGreaterThan(0);expect(denied.addedPaths).toEqual([]);
  expect(denied.backendAggregateSha256).toBeUndefined();
  expect(denied.read(path)).toBe(readFileSync(path,'utf8').replaceAll('\r\n','\n'));
 }
}, 90000);

it('rejects modified policy service and backend bytes with restoration',()=>{
  // Live working-tree mutate — Phase2 FixtureFiles grew past 5k and blows the 90s Boundary budget.
  for(const p of ['src-tauri/src/application/generation_service.rs','src-tauri/src/domain/asset.rs','src-tauri/src/application/minimax_video_product_policy.rs']){
   const before=readFileSync(p);
   try{
    writeFileSync(p, before.toString()+'\n# drift\n');
    const denied=minimaxVideoPhase1ParentReader('.');
    expect(denied.violations.length).toBeGreaterThan(0);
    expect(denied.addedPaths).toEqual([]);
    expect(denied.backendAggregateSha256).toBeUndefined();
   }finally{writeFileSync(p,before);}
  }
  expect(minimaxVideoPhase1ParentReader('.').violations).toEqual([]);
}, 90000);
it('rejects historical proof, literal Runtime Package and config drift',()=>{
  for(const p of ['docs/architecture/readiness-post-run-fix.json',
   'src-tauri/runtime_packages/kera2_t2i_local_v2_1_1_1_90894e9e/recipe.yaml','src-tauri/tauri.conf.json']){
   const before=readFileSync(p);
   try{
    writeFileSync(p, before.toString()+'\n# drift\n');
    const denied=minimaxVideoPhase1ParentReader('.');
    expect(denied.violations.length).toBeGreaterThan(0);
    expect(denied.addedPaths).toEqual([]);
    expect(denied.backendAggregateSha256).toBeUndefined();
   }finally{writeFileSync(p,before);}
  }
  expect(minimaxVideoPhase1ParentReader('.').violations).toEqual([]);
}, 90000);
it('rejects missing Phase1 proof and missing source evidence',()=>{
  const paths=[MINIMAX_VIDEO_PHASE1_MANIFEST,'src-tauri/src/application/generation_service.rs','src-tauri/src/domain/asset.rs',
   'docs/architecture/readiness-post-run-fix.json','src-tauri/runtime_packages/minimax_h3_fl2va_i2v_quality_2_2_0/recipe.yaml'];
  for(const p of paths){
   const bytes=readFileSync(p);
   try{
    rmSync(p);
    const denied=minimaxVideoPhase1ParentReader('.');
    expect(denied.violations).toContain(p===MINIMAX_VIDEO_PHASE1_MANIFEST?'minimax-video-phase1-missing-evidence':'minimax-video-phase2-missing-evidence');
    expect(denied.addedPaths).toEqual([]);
    expect(denied.backendAggregateSha256).toBeUndefined();
   }finally{writeFileSync(p,bytes);}
  }
  expect(minimaxVideoPhase1ParentReader('.').violations).toEqual([]);
}, 90000);
it('rejects illegal frontend, migration and backend additions',()=>{
  for(const p of ['src/unreviewed.ts','src-tauri/migrations/043_unreviewed.sql','src-tauri/src/unreviewed.rs']){
   try{
    writeFileSync(p,'// unreviewed');
    expect(minimaxVideoPhase1ParentReader('.').violations.length).toBeGreaterThan(0);
   }finally{rmSync(p);}
  }
  expect(minimaxVideoPhase1ParentReader('.').violations).toEqual([]);
}, 90000);

it('keeps one policy on all production composition and write/dispatch seams',()=>{
 const read=(p:string)=>readFileSync(p,'utf8');const composition=read('src-tauri/src/lib.rs');
 for(const module of ['GenerationService','GenerationCatalogService','ProductionQueueService','ShotBatchService']){
  const start=composition.indexOf(`${module}::new(`);expect(start).toBeGreaterThan(0);
  const block=composition.slice(start).split(/\n\s*let /)[0];
  expect(block).toContain('.with_new_generation_admission(workflow_registry_service.clone())');
 }
 const queue=read('src-tauri/src/application/production_queue_service.rs');
 for(const method of ['create_internal','prepare_queue_values','inspect_start_admitted','commit_start_admitted','partial_resume','requeue_item_internal','run_loop']){
  const start=queue.indexOf(`fn ${method}(`);expect(start).toBeGreaterThan(0);
  const block=queue.slice(start).split(/\n    (?:pub )?(?:async )?fn /)[0];
  expect(block).toMatch(/ensure_new_generation_allowed|prepare_queue_values|inspect_start_admitted/);
 }
 const batch=read('src-tauri/src/application/shot_batch_service.rs');
 expect(batch).toMatch(/validate_new_items\(&batch\.project_id, items\)\.await\?[\s\S]*?insert_prepared_batch_with_bindings/);
 expect(batch).toMatch(/validate_new_items\(&batch\.project_id, &items\)\.await\?[\s\S]*?insert_batch_with_bindings/);
 expect(read('src-tauri/src/application/generation_service.rs')).toContain('inspect_new_generation_availability(workflow_version_id, recipe_id)');
 expect(read('src-tauri/src/application/project_workflow_binding_service.rs')).toContain('.inspect_new_generation_availability(workflow_version_id, recipe_id)');
 for(const file of ['workflow_benchmark_service','h3_local_import_service','production_orchestrator_service'])
  expect(read(`src-tauri/src/application/${file}.rs`)).toContain('ensure_new_generation_allowed');
 const policy=read('src-tauri/src/application/minimax_video_product_policy.rs');
 expect(policy).toContain('artifact.source_kind != "PRODUCT"');expect(policy).toContain('live.manifest_yaml == expected.manifest_yaml');
 expect(read('src-tauri/src/application/product/h3_resolution.rs')).toContain('input: "first_frame".into()');
}, 90000);
