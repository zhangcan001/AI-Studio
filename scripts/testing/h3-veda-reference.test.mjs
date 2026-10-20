// @vitest-environment node
import { expect, it } from 'vitest';
import { readFileSync, writeFileSync, copyFileSync, mkdirSync, mkdtempSync, rmSync } from 'node:fs';
import { join, dirname, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { execFileSync } from 'node:child_process';
import { h3VedaReferenceParentReader, H3_VEDA_REFERENCE_BASE, H3_VEDA_REFERENCE_PROOF } from '../h3-veda-reference-successor-guard.mjs';
import { entityCategoryLibraryParentReader, ENTITY_CATEGORY_LIBRARY_PROOF } from '../entity-category-library-successor-guard.mjs';
import { uiPhase7BR1MetadataParentReader } from '../ui-phase7b-r1-metadata-successor-guard.mjs';

it('validates h3-veda-reference changes and projects original bytes through historical readers', () => {
  const checked = h3VedaReferenceParentReader('.');
  expect(checked.violations).toEqual([]);
  expect(checked.read('src/features/create/StoryboardShotRows.tsx')).toBe(execFileSync('git', ['show', `${H3_VEDA_REFERENCE_BASE}:src/features/create/StoryboardShotRows.tsx`], {encoding:'utf8'}));
  expect(readFileSync(ENTITY_CATEGORY_LIBRARY_PROOF)).toEqual(execFileSync('git', ['show', `${H3_VEDA_REFERENCE_BASE}:${ENTITY_CATEGORY_LIBRARY_PROOF}`]));
  expect(entityCategoryLibraryParentReader('.').violations).toEqual([]);
}, 120000);

it('rejects forged proof, altered source, runtime bytes and undeclared product changes', () => {
  const proof = JSON.parse(readFileSync(H3_VEDA_REFERENCE_PROOF,'utf8'));
  for (const override of [
    {[H3_VEDA_REFERENCE_PROOF]: JSON.stringify({...proof, parentHead:'forged'})},
    {[H3_VEDA_REFERENCE_PROOF]: JSON.stringify({...proof, contracts:{...proof.contracts,presentationOnly:false}})},
    {'src/features/create/StoryboardShotRows.tsx':':root { --studio-bg: red; }'},
    {'src-tauri/runtime_packages/minimax_h3_fl2va_i2v_quality_2_2_0/recipe.yaml':'tampered runtime'},
    {'src-tauri/src/application/production_queue_service.rs':'tampered queue'},
    {'src/illegal-ui-file.ts':'unapproved'},
  ]) expect(h3VedaReferenceParentReader('.',override).violations.length).toBeGreaterThan(0);
  expect(h3VedaReferenceParentReader('.').violations).toEqual([]);
},120000);

it('invalidates cached evidence on deletion, live source mutation, raw runtime drift and illegal additions', () => {
  const root=mkdtempSync(join(tmpdir(),'workbench-visual-proof-'));
  try {
    const paths=execFileSync('git',['ls-files','--cached','--others','--exclude-standard'],{encoding:'utf8'}).trim().split(/\r?\n/);
    for(const p of paths){mkdirSync(dirname(join(root,p)),{recursive:true});copyFileSync(p,join(root,p));}
    execFileSync('git',['init','-q',root]);
    const objects=resolve(execFileSync('git',['rev-parse','--git-path','objects'],{encoding:'utf8'}).trim());
    writeFileSync(join(root,'.git/objects/info/alternates'),objects.replaceAll('\\','/')+'\n');
    const validate=()=>uiPhase7BR1MetadataParentReader(root).violations;
    expect(validate()).toEqual([]);
    for(const p of [H3_VEDA_REFERENCE_PROOF,'src/features/create/StoryboardShotRows.test.tsx']) {
      const file=join(root,p), original=readFileSync(file);
      try {
        writeFileSync(file,Buffer.concat([original,Buffer.from('\ninvalid mutation')]));
        expect(validate().length).toBeGreaterThan(0);
        rmSync(file);expect(validate().length).toBeGreaterThan(0);
      } finally {writeFileSync(file,original);}
      expect(validate()).toEqual([]);
    }
    const runtime=join(root,'src-tauri/runtime_packages/minimax_h3_fl2va_i2v_quality_2_2_0/recipe.yaml');
    const original=readFileSync(runtime);
    try {
      const text=original.toString('utf8');
      const changed=text.includes('\r\n')?text.replaceAll('\r\n','\n'):text.replaceAll('\n','\r\n');
      writeFileSync(runtime,changed);expect(validate().length).toBeGreaterThan(0);
    } finally {writeFileSync(runtime,original);}
    expect(validate()).toEqual([]);
    const illegal=join(root,'src/styles/unapproved-theme.css');
    try {writeFileSync(illegal,'body { color: red; }');expect(validate().length).toBeGreaterThan(0);}
    finally {rmSync(illegal);}
    expect(validate()).toEqual([]);
  } finally {rmSync(root,{recursive:true,force:true});}
},120000);
