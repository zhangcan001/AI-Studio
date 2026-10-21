// @vitest-environment node
import { expect, it } from 'vitest';
import { readFileSync, writeFileSync, copyFileSync, mkdirSync, mkdtempSync, rmSync } from 'node:fs';
import { join, dirname, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { execFileSync } from 'node:child_process';
import { defaultReferenceGeneratorParentReader, DEFAULT_REFERENCE_GENERATOR_BASE, DEFAULT_REFERENCE_GENERATOR_PROOF } from '../default-reference-generator-successor-guard.mjs';
import { folderBatchImportParentReader, FOLDER_BATCH_IMPORT_PROOF } from '../folder-batch-import-successor-guard.mjs';
import { libraryConfirmCiRaceParentReader } from '../library-confirm-ci-race-successor-guard.mjs';
import { uiPhase7BR1MetadataParentReader } from '../ui-phase7b-r1-metadata-successor-guard.mjs';

it('preserves the frozen folder checkpoint and validates exact current-byte overrides through parent projection', () => {
  const checked = defaultReferenceGeneratorParentReader('.');
  expect(checked.violations).toEqual([]);
  const row = 'src/features/create/StoryboardShotRows.tsx';
  expect(checked.read(row)).toBe(execFileSync('git', ['show', `${DEFAULT_REFERENCE_GENERATOR_BASE}:${row}`], {encoding:'utf8'}));
  expect(readFileSync(FOLDER_BATCH_IMPORT_PROOF)).toEqual(execFileSync('git', ['show', `${DEFAULT_REFERENCE_GENERATOR_BASE}:${FOLDER_BATCH_IMPORT_PROOF}`]));
  expect(folderBatchImportParentReader('.').violations).toEqual([]);
  expect(libraryConfirmCiRaceParentReader('.', {[row]:readFileSync(row,'utf8')}).violations).toEqual([]);
  expect(libraryConfirmCiRaceParentReader('.', {[row]:readFileSync(row,'utf8')+'\nunapproved edit'}).violations.length).toBeGreaterThan(0);
},120000);

it('rejects forged proof identity/hash, queue changes, Runtime bytes and undeclared additions', () => {
  const proof=JSON.parse(readFileSync(DEFAULT_REFERENCE_GENERATOR_PROOF,'utf8'));
  const source='src/features/create/createModel.ts';
  for(const override of [
    {[DEFAULT_REFERENCE_GENERATOR_PROOF]:JSON.stringify({...proof,parentHead:'forged'})},
    {[DEFAULT_REFERENCE_GENERATOR_PROOF]:JSON.stringify({...proof,contracts:{...proof.contracts,existingShotSelectionPreserved:false}})},
    {[DEFAULT_REFERENCE_GENERATOR_PROOF]:JSON.stringify({...proof,files:{...proof.files,[source]:{...proof.files[source],afterHash:'0'.repeat(64)}}})},
    {[source]:readFileSync(source,'utf8')+'\nforged'},
    {'src-tauri/src/application/production_queue_service.rs':'changed queue'},
    {'src-tauri/runtime_packages/minimax_h3_fl2va_i2v_quality_2_2_0/recipe.yaml':'changed runtime'},
    {'src/unapproved-default.ts':'unapproved'},
  ])expect(defaultReferenceGeneratorParentReader('.',override).violations.length).toBeGreaterThan(0);
  expect(defaultReferenceGeneratorParentReader('.').violations).toEqual([]);
},120000);

it('invalidates warm-cache fixtures on live mutations, deletion and illegal files; restoration passes', () => {
  const root=mkdtempSync(join(tmpdir(),'default-reference-proof-'));
  try {
    const paths=execFileSync('git',['ls-files','--cached','--others','--exclude-standard'],{encoding:'utf8'}).trim().split(/\r?\n/).filter(p=>p==='vite.config.ts'||/^(\.github\/workflows\/|src\/|src-tauri\/|scripts\/|docs\/architecture\/)/.test(p));
    for(const p of paths){mkdirSync(dirname(join(root,p)),{recursive:true});copyFileSync(p,join(root,p));}
    execFileSync('git',['init','-q',root]);
    const objects=resolve(execFileSync('git',['rev-parse','--git-path','objects'],{encoding:'utf8'}).trim());
    writeFileSync(join(root,'.git/objects/info/alternates'),objects.replaceAll('\\','/')+'\n');
    const validate=()=>uiPhase7BR1MetadataParentReader(root).violations;
    expect(validate()).toEqual([]);
    for(const p of [DEFAULT_REFERENCE_GENERATOR_PROOF,FOLDER_BATCH_IMPORT_PROOF,'src/features/create/CreateController.ts']) {
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
