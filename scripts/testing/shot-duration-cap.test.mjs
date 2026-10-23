// @vitest-environment node
import { expect, it } from 'vitest';
import { readFileSync, writeFileSync, copyFileSync, mkdirSync, mkdtempSync, rmSync } from 'node:fs';
import { join, dirname, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { execFileSync } from 'node:child_process';
import { shotDurationCapParentReader, SHOT_DURATION_CAP_BASE, SHOT_DURATION_CAP_PROOF } from '../shot-duration-cap-successor-guard.mjs';
import { batchCreateReasonParentReader as parentReader, BATCH_CREATE_REASON_PROOF as PARENT_PROOF } from '../batch-create-reason-successor-guard.mjs';
import { libraryConfirmCiRaceParentReader } from '../library-confirm-ci-race-successor-guard.mjs';
import { uiPhase7BR1MetadataParentReader } from '../ui-phase7b-r1-metadata-successor-guard.mjs';

it('preserves the frozen batch-create-reason checkpoint and validates exact current-byte overrides through parent projection', () => {
  const checked = shotDurationCapParentReader('.');
  expect(checked.violations).toEqual([]);
  const row = 'src-tauri/src/application/product/h3_resolution.rs';
  expect(checked.read(row)).toBe(execFileSync('git', ['show', `${SHOT_DURATION_CAP_BASE}:${row}`], {encoding:'utf8'}));
  expect(readFileSync(PARENT_PROOF)).toEqual(execFileSync('git', ['show', `${SHOT_DURATION_CAP_BASE}:${PARENT_PROOF}`]));
  expect(parentReader('.').violations).toEqual([]);
  expect(libraryConfirmCiRaceParentReader('.', {[row]:readFileSync(row,'utf8')}).violations).toEqual([]);
  expect(libraryConfirmCiRaceParentReader('.', {[row]:readFileSync(row,'utf8')+'\nunapproved edit'}).violations.length).toBeGreaterThan(0);
},120000);

it('rejects forged proof identity/hash, queue changes, Runtime bytes and undeclared additions', () => {
  const proof=JSON.parse(readFileSync(SHOT_DURATION_CAP_PROOF,'utf8'));
  const source='src-tauri/src/application/product/h3_resolution.rs';
  for(const override of [
    {[SHOT_DURATION_CAP_PROOF]:JSON.stringify({...proof,parentHead:'forged'})},
    {[SHOT_DURATION_CAP_PROOF]:JSON.stringify({...proof,contracts:{...proof.contracts,pinnedContractStringsPreserved:false}})},
    {[SHOT_DURATION_CAP_PROOF]:JSON.stringify({...proof,files:{...proof.files,[source]:{...proof.files[source],afterHash:'0'.repeat(64)}}})},
    {[source]:readFileSync(source,'utf8')+'\nforged'},
    {'src-tauri/src/application/production_queue_service.rs':'changed queue'},
    {'src-tauri/runtime_packages/minimax_h3_fl2va_i2v_quality_2_2_0/recipe.yaml':'changed runtime'},
    {'src/unapproved-default.ts':'unapproved'},
    {'src/product/shotDuration.ts':'export const MAX_SHOT_DURATION_SECONDS = 15;'},
    {'src-tauri/migrations/049_unapproved.sql':'SELECT 1;'},
  ])expect(shotDurationCapParentReader('.',override).violations.length).toBeGreaterThan(0);
  expect(shotDurationCapParentReader('.').violations).toEqual([]);
},120000);

it('invalidates warm-cache fixtures on live mutations, deletion and illegal files; restoration passes', () => {
  const root=mkdtempSync(join(tmpdir(),'shot-duration-cap-proof-'));
  try {
    const paths=execFileSync('git',['ls-files','--cached','--others','--exclude-standard'],{encoding:'utf8'}).trim().split(/\r?\n/).filter(p=>p==='vite.config.ts'||/^(\.github\/workflows\/|src\/|src-tauri\/|scripts\/|docs\/architecture\/)/.test(p));
    for(const p of paths){mkdirSync(dirname(join(root,p)),{recursive:true});copyFileSync(p,join(root,p));}
    execFileSync('git',['init','-q',root]);
    const objects=resolve(execFileSync('git',['rev-parse','--git-path','objects'],{encoding:'utf8'}).trim());
    writeFileSync(join(root,'.git/objects/info/alternates'),objects.replaceAll('\\','/')+'\n');
    const validate=()=>uiPhase7BR1MetadataParentReader(root).violations;
    expect(validate()).toEqual([]);
    for(const p of [SHOT_DURATION_CAP_PROOF,PARENT_PROOF,'src/features/create/CreateController.ts']) {
      const file=join(root,p),original=readFileSync(file);
      try {
        writeFileSync(file,Buffer.concat([original,Buffer.from('\nmutation')]));expect(validate().length).toBeGreaterThan(0);
        rmSync(file);expect(validate().length).toBeGreaterThan(0);
      } finally {writeFileSync(file,original);}
      expect(validate()).toEqual([]);
    }
    const runtime=join(root,'src-tauri/runtime_packages/minimax_h3_fl2va_i2v_quality_2_2_0/recipe.yaml');
    const original=readFileSync(runtime);
    try {const text=original.toString('utf8');writeFileSync(runtime,text.includes('\r\n')?text.replaceAll('\r\n','\n'):text.replaceAll('\n','\r\n'));expect(validate().length).toBeGreaterThan(0);}
    finally {writeFileSync(runtime,original);}
    expect(validate()).toEqual([]);
    const illegal=join(root,'src/unapproved-default.ts');
    try {writeFileSync(illegal,'unapproved');expect(validate().length).toBeGreaterThan(0);}
    finally {rmSync(illegal);}
    expect(validate()).toEqual([]);
  } finally {rmSync(root,{recursive:true,force:true});}
},120000);
