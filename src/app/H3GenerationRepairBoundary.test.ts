// @vitest-environment node
import {expect,it} from 'vitest';
import {readFileSync,writeFileSync,mkdirSync,mkdtempSync,rmSync} from 'node:fs';
import {execFileSync} from 'node:child_process';
import {tmpdir} from 'node:os';
import {join,dirname} from 'node:path';
import {h3RepairParentReader,H3_REPAIR_PARENT} from '../../scripts/h3-generation-repair-successor-guard.mjs';
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
}, 90000);
it('rejects fresh live drift and extra files without hiding it behind cached evidence',()=>{
  // Live working-tree mutate keeps fail-closed drift/illegal asserts without copying the tip-inflated Phase2 fixture set.
  const p='src/features/create/CreateInputs.tsx', original=readFileSync(p);
  try{
   writeFileSync(p, original.toString() + '\n// unauthorized drift\n');
   const rejected=h3RepairParentReader('.');expect(rejected.violations.length).toBeGreaterThan(0);expect(rejected.addedPaths).toEqual([]);
  }finally{writeFileSync(p,original);}
  expect(h3RepairParentReader('.').violations).toEqual([]);
  const extra='src-tauri/src/unreviewed.rs';
  try{
   writeFileSync(extra,'// extra source');
   expect(h3RepairParentReader('.').violations).toContain('minimax-video-phase2-backend-files');
  }finally{rmSync(extra);}
  expect(h3RepairParentReader('.').violations).toEqual([]);
}, 90000);

import {m2ReuseParentReader} from '../../scripts/m2-create-library-reuse-boundary-guard.mjs';
it('rejects impossible review metadata before historical replay and leaves live bytes unprojected',()=>{
 const root=mkdtempSync(join(tmpdir(),'ai-studio-h3-invalid-review-'));
 try {
  const path='src/product/types.ts',value='live H3 resolution metadata';
  mkdirSync(dirname(join(root,path)),{recursive:true});writeFileSync(join(root,path),value);
  const proof=JSON.parse(readFileSync('docs/architecture/m2-2-create-library-reuse.json','utf8'));
  proof.parentHead='0'.repeat(40);
  const rejected=m2ReuseParentReader(root,proof);
  expect(rejected.violations).toContain('m2-2-invalid-header');
  expect(rejected.addedPaths).toEqual([]);expect(rejected.afterHashes).toEqual({});
  expect(rejected.backendAggregateSha256).toBeUndefined();expect(rejected.read(path)).toBe(value);
 } finally {rmSync(root,{recursive:true,force:true});}
}, 90000);
