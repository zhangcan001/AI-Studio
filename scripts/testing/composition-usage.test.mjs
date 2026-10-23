// @vitest-environment node
import { expect, it } from 'vitest';
import { readFileSync, writeFileSync, copyFileSync, mkdirSync, mkdtempSync, rmSync } from 'node:fs';
import { join, dirname, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { execFileSync } from 'node:child_process';
import { compositionUsageParentReader, COMPOSITION_USAGE_BASE, COMPOSITION_USAGE_PROOF } from '../composition-usage-successor-guard.mjs';
import { vedaReferenceMediaParentReader as parentReader, VEDA_REFERENCE_MEDIA_PROOF as PARENT_PROOF } from '../veda-reference-media-successor-guard.mjs';
import { libraryConfirmCiRaceParentReader } from '../library-confirm-ci-race-successor-guard.mjs';
import { uiPhase7BR1MetadataParentReader } from '../ui-phase7b-r1-metadata-successor-guard.mjs';

it('preserves the frozen veda-reference-media checkpoint and validates exact current-byte overrides through parent projection', () => {
  const checked = compositionUsageParentReader('.');
  expect(checked.violations).toEqual([]);
  const row = 'src-tauri/src/application/shot_video_input_service.rs';
  expect(checked.read(row)).toBe(execFileSync('git', ['show', `${COMPOSITION_USAGE_BASE}:${row}`], {encoding:'utf8'}));
  expect(readFileSync(PARENT_PROOF)).toEqual(execFileSync('git', ['show', `${COMPOSITION_USAGE_BASE}:${PARENT_PROOF}`]));
  expect(parentReader('.').violations).toEqual([]);
  expect(libraryConfirmCiRaceParentReader('.', {[row]:readFileSync(row,'utf8')}).violations).toEqual([]);
  expect(libraryConfirmCiRaceParentReader('.', {[row]:readFileSync(row,'utf8')+'\nunapproved edit'}).violations.length).toBeGreaterThan(0);
},120000);

it('rejects forged proof identity/hash, queue changes, Runtime bytes and undeclared additions', () => {
  const proof=JSON.parse(readFileSync(COMPOSITION_USAGE_PROOF,'utf8'));
  const source='src-tauri/src/application/shot_video_input_service.rs';
  for(const override of [
    {[COMPOSITION_USAGE_PROOF]:JSON.stringify({...proof,parentHead:'forged'})},
    {[COMPOSITION_USAGE_PROOF]:JSON.stringify({...proof,contracts:{...proof.contracts,pinnedContractStringsPreserved:false}})},
    {[COMPOSITION_USAGE_PROOF]:JSON.stringify({...proof,files:{...proof.files,[source]:{...proof.files[source],afterHash:'0'.repeat(64)}}})},
    {[source]:readFileSync(source,'utf8')+'\nforged'},
    {'src-tauri/src/application/production_queue_service.rs':'changed queue'},
    {'src-tauri/runtime_packages/minimax_h3_fl2va_i2v_quality_2_2_0/recipe.yaml':'changed runtime'},
    {'src/unapproved-default.ts':'unapproved'},
    {'src-tauri/migrations/048_shot_story_entity_composition_usage.sql':'ALTER TABLE shots ADD COLUMN forged TEXT;'},
    {'src-tauri/migrations/049_unapproved.sql':'SELECT 1;'},
  ])expect(compositionUsageParentReader('.',override).violations.length).toBeGreaterThan(0);
  expect(compositionUsageParentReader('.').violations).toEqual([]);
},120000);

it('invalidates warm-cache fixtures on live mutations, deletion and illegal files; restoration passes', () => {
  const root=mkdtempSync(join(tmpdir(),'composition-usage-proof-'));
  try {
    const paths=execFileSync('git',['ls-files','--cached','--others','--exclude-standard'],{encoding:'utf8'}).trim().split(/\r?\n/).filter(p=>p==='vite.config.ts'||/^(\.github\/workflows\/|src\/|src-tauri\/|scripts\/|docs\/architecture\/)/.test(p));
    for(const p of paths){mkdirSync(dirname(join(root,p)),{recursive:true});copyFileSync(p,join(root,p));}
    execFileSync('git',['init','-q',root]);
    const objects=resolve(execFileSync('git',['rev-parse','--git-path','objects'],{encoding:'utf8'}).trim());
    writeFileSync(join(root,'.git/objects/info/alternates'),objects.replaceAll('\\','/')+'\n');
    const validate=()=>uiPhase7BR1MetadataParentReader(root).violations;
    expect(validate()).toEqual([]);
    for(const p of [COMPOSITION_USAGE_PROOF,PARENT_PROOF,'src/features/create/CreateController.ts']) {
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
