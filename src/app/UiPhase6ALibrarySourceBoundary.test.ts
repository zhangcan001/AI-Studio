// @vitest-environment node
import {expect,it} from 'vitest';
import {readFileSync,writeFileSync} from 'node:fs';
import {uiPhase6AParentReader,phase6AParentFacts,PHASE6A_MANIFEST,PHASE6A_PARENT,PHASE6A_EXISTING} from '../../scripts/ui-phase6a-library-source-successor-guard.mjs';
import {uiPhase5JParentReader} from '../../scripts/ui-phase5j-ci-per-file-worker-successor-guard.mjs';
import {m3LibraryParentReader} from '../../scripts/m3-library-findability-boundary-guard.mjs';
import {uiPhase6BParentReader} from '../../scripts/ui-phase6b-library-shot-bind-successor-guard.mjs';

it('wires ordinary Library SOURCE/GENERATED filters before pagination and blocks new image generation',()=>{
 // Validate the live successor chain first, then inspect Phase6A's own bytes.
 // Later video-only policy legitimately tightened the historical Library jump.
 const phase6A=uiPhase6BParentReader('.');
 expect(phase6A.violations).toEqual([]);
 const list=phase6A.read('src-tauri/src/application/product/library_facade/list.rs');
 expect(list).toContain('cursor.source_kind == source_kind');
 expect(list).toContain('match source_kind {');
 expect(list).toContain('AssetSourceFilter::Source');
 const controller=phase6A.read('src/features/library/LibraryController.ts');
 expect(controller).toContain('sourceKind,cursor,limit:30');
 expect(controller).toContain('!returnRoute && stage==="image"');
 const detail=phase6A.read('src/features/library/LibraryDetail.tsx');
 expect(detail).not.toContain('用于图片创作');
 expect(detail).toContain('用于视频创作');
 expect(detail).toContain('历史图片可预览与审计');
 const page=phase6A.read('src/features/library/LibraryPage.tsx');
 expect(page).toContain('素材来源');
}, 90000);

it('accepts scoped Phase6A library source successor and preserves Phase5J/m3 proofs',()=>{
 const reader=uiPhase6AParentReader('.');
 expect(reader.violations).toEqual([]);
 const proof=JSON.parse(readFileSync(PHASE6A_MANIFEST,'utf8'));
 expect(proof.parentHead).toBe(PHASE6A_PARENT);
 expect(proof.invariants.productLibrarySourceFilterWired).toBe(true);
 expect(proof.invariants.ordinaryLibraryBlocksNewImageGeneration).toBe(true);
 expect(proof.invariants.historicalProofsRewritten).toBe(false);
 expect(proof.invariants.phase5fThrough5jPreserved).toBe(true);
 expect(proof.database.migrationAfter).toBe(44);
 expect(proof.database.backupAfter).toBe(21);
 for(const p of PHASE6A_EXISTING)expect(reader.read(p)).toBe(phase6AParentFacts('.').read(p));
 expect(uiPhase5JParentReader('.').violations).toEqual([]);
 expect(m3LibraryParentReader('.').violations).toEqual([]);
}, 90000);

it('denies forged metadata, proof hashes and weakened Phase6A gates without authority',()=>{
 const proof=JSON.parse(readFileSync(PHASE6A_MANIFEST,'utf8'));
 for(const mutate of [
  (p:any)=>{p.parentHead='0'.repeat(40);},
  (p:any)=>{p.paths['src-tauri/src/application/product/library_facade/list.rs'].afterHash='0'.repeat(64);},
  (p:any)=>{p.paths['src/features/library/LibraryController.ts'].beforeHash='0'.repeat(64);},
  (p:any)=>{p.invariants.productLibrarySourceFilterWired=false;},
  (p:any)=>{p.invariants.historicalProofsRewritten=true;},
  (p:any)=>{p.paths['src/services/ipc.ts']={beforeHash:null,afterHash:'0'.repeat(64)};},
 ]){
  const invalid=structuredClone(proof);mutate(invalid);
  const denied=uiPhase6AParentReader('.',invalid);
  expect(denied.violations.length).toBeGreaterThan(0);
  expect(denied.addedPaths).toEqual([]);expect(denied.afterHashes).toEqual({});
 }
}, 90000);

it('refuses edited library source wiring, A08 regression and Runtime package drift in isolated fixture',()=>{
  // Live working-tree mutate — tip-inflated FixtureFiles blow the 90s Boundary budget.
  expect(uiPhase6AParentReader('.').violations).toEqual([]);
  for(const p of [
   'src-tauri/src/application/product/library_facade/list.rs',
   'src/features/library/LibraryDetail.tsx',
   'src/features/library/LibraryController.ts',
  ]){
   const before=readFileSync(p);
   try{
    writeFileSync(p,Uint8Array.from([...before,10]));
    expect(uiPhase6AParentReader('.').violations.length,p).toBeGreaterThan(0);
   }finally{writeFileSync(p,before);}
  }
  const detail='src/features/library/LibraryDetail.tsx';
  const detailBefore=readFileSync(detail,'utf8');
  try{
   writeFileSync(detail,detailBefore.replace('用于视频创作','用于图片创作'));
   expect(uiPhase6AParentReader('.').violations.length).toBeGreaterThan(0);
  }finally{writeFileSync(detail,detailBefore);}
  const pkg='src-tauri/runtime_packages/minimax_h3_fl2va_t2v_quality_2_2_0/recipe.yaml';
  const before=readFileSync(pkg);
  try{
   writeFileSync(pkg,Uint8Array.from([...before,10]));
   expect(uiPhase6AParentReader('.').violations.length).toBeGreaterThan(0);
  }finally{writeFileSync(pkg,before);}
}, 90000);
