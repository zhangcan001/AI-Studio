// @vitest-environment node
import { expect, it } from 'vitest';
import { readFileSync, writeFileSync, copyFileSync, mkdirSync, mkdtempSync, rmSync } from 'node:fs';
import { join, dirname, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { execFileSync } from 'node:child_process';
import { supersededImportPinParentReader, SUPERSEDED_IMPORT_PIN_BASE, SUPERSEDED_IMPORT_PIN_PROOF } from '../superseded-import-pin-successor-guard.mjs';
import { staleBindingMigrationParentReader as parentReader, STALE_BINDING_MIGRATION_PROOF as PARENT_PROOF } from '../stale-binding-migration-successor-guard.mjs';
import { uiPhase7BR1MetadataParentReader } from '../ui-phase7b-r1-metadata-successor-guard.mjs';

const repo = 'src-tauri/src/infrastructure/database/repositories/generation_definition.rs';
const importer = 'src-tauri/src/application/mod.rs';

it('resolves only superseded PRODUCT pins at import, with a warning, without migrations', () => {
  const source = readFileSync(repo, 'utf8').replaceAll('\r\n', '\n');
  expect(source).toContain('async fn resolve_superseded_pair(');
  expect(source).toContain("w.source_kind = 'PRODUCT'");
  expect(source).toContain('rv.workflow_id = w.id');
  expect(source).toContain('let [recipe] = recipes.as_slice() else');
  const importing = readFileSync(importer, 'utf8').replaceAll('\r\n', '\n');
  expect(importing).toContain('.resolve_superseded_pair(&generator.workflow_version_id, &generator.recipe_id)');
  expect(importing).toContain('导入包指定的工作流「{}」版本已更新，已改用最新版本 {}');
  expect(importing).toContain('warnings.extend(generator_warnings);');
  expect(execFileSync('git', ['ls-files', 'src-tauri/migrations'], { encoding: 'utf8' }))
    .toBe(execFileSync('git', ['ls-tree', '-r', '--name-only', SUPERSEDED_IMPORT_PIN_BASE, 'src-tauri/migrations'], { encoding: 'utf8' }));
});

it('preserves the frozen stale-binding-migration checkpoint through parent projection', () => {
  const checked = supersededImportPinParentReader('.');
  expect(checked.violations).toEqual([]);
  expect(checked.read(repo)).toBe(execFileSync('git', ['show', `${SUPERSEDED_IMPORT_PIN_BASE}:${repo}`], {encoding:'utf8'}).replaceAll('\r\n','\n'));
  expect(readFileSync(PARENT_PROOF)).toEqual(execFileSync('git', ['show', `${SUPERSEDED_IMPORT_PIN_BASE}:${PARENT_PROOF}`]));
  expect(parentReader('.').violations).toEqual([]);
}, 120000);

it('rejects forged proof identity/hash, source drift, migrations and undeclared additions', () => {
  const proof = JSON.parse(readFileSync(SUPERSEDED_IMPORT_PIN_PROOF, 'utf8'));
  for (const override of [
    {[SUPERSEDED_IMPORT_PIN_PROOF]: JSON.stringify({...proof, parentHead: 'forged'})},
    {[SUPERSEDED_IMPORT_PIN_PROOF]: JSON.stringify({...proof, contracts: {...proof.contracts, guardsRelaxed: true}})},
    {[SUPERSEDED_IMPORT_PIN_PROOF]: JSON.stringify({...proof, files: {...proof.files, [repo]: {...proof.files[repo], afterHash: '0'.repeat(64)}}})},
    {[repo]: readFileSync(repo, 'utf8').replace('rv.workflow_id = w.id', '1 = 1')},
    {'src-tauri/src/application/shot_video_input_service.rs': 'changed compiler'},
    {'src/unapproved-default.ts': 'unapproved'},
    {'src-tauri/migrations/051_unapproved.sql': 'SELECT 1;'},
  ]) expect(supersededImportPinParentReader('.', override).violations.length).toBeGreaterThan(0);
  expect(supersededImportPinParentReader('.').violations).toEqual([]);
}, 120000);

it('invalidates warm-cache fixtures on live mutations, deletion and illegal files; restoration passes', () => {
  const root = mkdtempSync(join(tmpdir(), 'superseded-import-pin-proof-'));
  try {
    const paths = execFileSync('git', ['ls-files','--cached','--others','--exclude-standard'], {encoding:'utf8'}).trim().split(/\r?\n/).filter(p => p === 'vite.config.ts' || /^(\.github\/workflows\/|src\/|src-tauri\/|scripts\/|docs\/architecture\/)/.test(p));
    for (const p of paths) { mkdirSync(dirname(join(root,p)), {recursive:true}); copyFileSync(p, join(root,p)); }
    execFileSync('git', ['init','-q',root]);
    const objects = resolve(execFileSync('git', ['rev-parse','--git-path','objects'], {encoding:'utf8'}).trim());
    writeFileSync(join(root,'.git/objects/info/alternates'), objects.replaceAll('\\','/') + '\n');
    const validate = () => uiPhase7BR1MetadataParentReader(root).violations;
    expect(validate()).toEqual([]);
    for (const p of [SUPERSEDED_IMPORT_PIN_PROOF, PARENT_PROOF, repo]) {
      const file = join(root,p), original = readFileSync(file);
      try {
        writeFileSync(file, Buffer.concat([original, Buffer.from('\nmutation')])); expect(validate().length).toBeGreaterThan(0);
        rmSync(file); expect(validate().length).toBeGreaterThan(0);
      } finally { writeFileSync(file, original); }
      expect(validate()).toEqual([]);
    }
    const illegal = join(root, 'src/unapproved-default.ts');
    try { writeFileSync(illegal, 'unapproved'); expect(validate().length).toBeGreaterThan(0); }
    finally { rmSync(illegal); }
    expect(validate()).toEqual([]);
  } finally { rmSync(root, {recursive:true, force:true}); }
}, 120000);
