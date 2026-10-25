// @vitest-environment node
import {expect,it} from 'vitest';
import {readFileSync,writeFileSync,copyFileSync,mkdirSync,mkdtempSync,rmSync} from 'node:fs';
import {join,dirname,resolve} from 'node:path';
import {tmpdir} from 'node:os';
import {execFileSync} from 'node:child_process';
import {singleShotBatchControlParentReader,SINGLE_SHOT_BATCH_CONTROL_BASE,SINGLE_SHOT_BATCH_CONTROL_PROOF} from '../single-shot-batch-control-successor-guard.mjs';
import {vedaRemovedWhiteFlashParentReader,VEDA_REMOVED_WHITE_FLASH_PROOF} from '../veda-removed-white-flash-successor-guard.mjs';
import {uiPhase7BR1MetadataParentReader} from '../ui-phase7b-r1-metadata-successor-guard.mjs';
const source='src-tauri/src/application/product/run_facade/detail.rs';
it('validates single-Shot input projection while preserving the frozen parent proof',()=>{
 const checked=singleShotBatchControlParentReader('.');expect(checked.violations).toEqual([]);
 expect(checked.read(source)).toBe(execFileSync('git',['show',`${SINGLE_SHOT_BATCH_CONTROL_BASE}:${source}`],{encoding:'utf8'}).replaceAll('\r\n','\n'));
 expect(readFileSync(VEDA_REMOVED_WHITE_FLASH_PROOF)).toEqual(execFileSync('git',['show',`${SINGLE_SHOT_BATCH_CONTROL_BASE}:${VEDA_REMOVED_WHITE_FLASH_PROOF}`]));
 expect(vedaRemovedWhiteFlashParentReader('.').violations).toEqual([]);
},120000);
it('rejects forged manifests, unregistered changes and execution/Runtime drift',()=>{
 const proof=JSON.parse(readFileSync(SINGLE_SHOT_BATCH_CONTROL_PROOF,'utf8'));
 for(const override of [
 {[SINGLE_SHOT_BATCH_CONTROL_PROOF]:JSON.stringify({...proof,parentHead:'forged'})},
 {[SINGLE_SHOT_BATCH_CONTROL_PROOF]:JSON.stringify({...proof,contracts:{...proof.contracts,productionQueueOnlyExecution:false}})},
 {[source]:readFileSync(source,'utf8')+'\n// unapproved'},
 {'src-tauri/src/application/production_queue_service.rs':'unapproved queue'},
 {'src-tauri/runtime_packages/minimax_h3_fl2va_i2v_quality_2_2_0/recipe.yaml':'unapproved runtime'},
 {'src/unapproved-shot.ts':'unapproved'},
 ])expect(singleShotBatchControlParentReader('.',override).violations.length).toBeGreaterThan(0);
 expect(singleShotBatchControlParentReader('.').violations).toEqual([]);
},120000);
it('keeps warm-cache freshness for source, proof deletion, illegal files and Runtime raw bytes; restoration passes',()=>{
 const root=mkdtempSync(join(tmpdir(),'single-shot-batch-proof-'));
 try{
  const paths=execFileSync('git',['ls-files','--cached','--others','--exclude-standard'],{encoding:'utf8'}).trim().split(/\r?\n/).filter(p=>p==='vite.config.ts'||/^(\.github\/workflows\/|src\/|src-tauri\/|scripts\/|docs\/architecture\/)/.test(p));
  for(const p of paths){mkdirSync(dirname(join(root,p)),{recursive:true});copyFileSync(p,join(root,p));}
  execFileSync('git',['init','-q',root]);
  writeFileSync(join(root,'.git/objects/info/alternates'),resolve(execFileSync('git',['rev-parse','--git-path','objects'],{encoding:'utf8'}).trim()).replaceAll('\\','/')+'\n');
  const validate=()=>uiPhase7BR1MetadataParentReader(root).violations;expect(validate()).toEqual([]);
  for(const p of [source,SINGLE_SHOT_BATCH_CONTROL_PROOF,VEDA_REMOVED_WHITE_FLASH_PROOF]){
   const file=join(root,p),original=readFileSync(file);
   try{writeFileSync(file,Buffer.concat([original,Buffer.from('\nmutation')]));expect(validate().length).toBeGreaterThan(0);rmSync(file);expect(validate().length).toBeGreaterThan(0);}finally{writeFileSync(file,original);}
   expect(validate()).toEqual([]);
  }
  const file=join(root,'src-tauri/runtime_packages/minimax_h3_fl2va_i2v_quality_2_2_0/recipe.yaml'),original=readFileSync(file),text=original.toString();
  try{writeFileSync(file,text.includes('\r\n')?text.replaceAll('\r\n','\n'):text.replaceAll('\n','\r\n'));expect(validate().length).toBeGreaterThan(0);}finally{writeFileSync(file,original);}
  expect(validate()).toEqual([]);
  const illegal=join(root,'src/unapproved-shot.ts');try{writeFileSync(illegal,'unapproved');expect(validate().length).toBeGreaterThan(0);}finally{rmSync(illegal);}expect(validate()).toEqual([]);
 }finally{if(!resolve(root).startsWith(resolve(tmpdir())+'\\single-shot-batch-proof-')&&!resolve(root).startsWith(resolve(tmpdir())+'/single-shot-batch-proof-'))throw Error('Unsafe fixture path');rmSync(root,{recursive:true,force:true});}
},120000);
