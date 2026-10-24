// @vitest-environment node
import { expect, it } from 'vitest';
import { readFileSync, writeFileSync, copyFileSync, mkdirSync, mkdtempSync, rmSync } from 'node:fs';
import { join, dirname, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { execFileSync } from 'node:child_process';
import { staleBindingMigrationParentReader, STALE_BINDING_MIGRATION_BASE, STALE_BINDING_MIGRATION_PROOF } from '../stale-binding-migration-successor-guard.mjs';
import { vedaVerboseDiagnosticsParentReader as parentReader, VEDA_VERBOSE_DIAGNOSTICS_PROOF as PARENT_PROOF } from '../veda-verbose-diagnostics-successor-guard.mjs';
import { uiPhase7BR1MetadataParentReader } from '../ui-phase7b-r1-metadata-successor-guard.mjs';

const repo = 'src-tauri/src/infrastructure/database/repositories/workflow_library.rs';

it('moves only same-workflow PRODUCT selections, in the registering transaction, without migrations', () => {
  const source = readFileSync(repo, 'utf8').replaceAll('\r\n', '\n');
  expect(source).toContain('migrate_stale_bindings(transaction, &package.workflow_id, package.created_at)');
  expect(source).toContain("w.source_kind = 'PRODUCT'");
  expect(source).toContain('WHERE workflow_id = ? AND id <> ?');
  expect(source).toContain('revision = revision + 1');
  expect(source).not.toMatch(/UPDATE (production_batch_items|production_stages|tasks)/);
  expect(source).not.toMatch(/DELETE FROM (workflow_versions|recipes|shot_video_input)/);
  expect(execFileSync('git', ['ls-files', 'src-tauri/migrations'], { encoding: 'utf8' }))
    .toBe(execFileSync('git', ['ls-tree', '-r', '--name-only', STALE_BINDING_MIGRATION_BASE, 'src-tauri/migrations'], { encoding: 'utf8' }));
});

it('preserves the frozen veda-verbose-diagnostics checkpoint through parent projection', () => {
  const checked = staleBindingMigrationParentReader('.');
  expect(checked.violations).toEqual([]);
  expect(checked.read(repo)).toBe(execFileSync('git', ['show', `${STALE_BINDING_MIGRATION_BASE}:${repo}`], {encoding:'utf8'}).replaceAll('\r\n','\n'));
  expect(readFileSync(PARENT_PROOF)).toEqual(execFileSync('git', ['show', `${STALE_BINDING_MIGRATION_BASE}:${PARENT_PROOF}`]));
  expect(parentReader('.').violations).toEqual([]);
}, 120000);

it('rejects forged proof identity/hash, source drift, migrations and undeclared additions', () => {
  const proof = JSON.parse(readFileSync(STALE_BINDING_MIGRATION_PROOF, 'utf8'));
  for (const override of [
    {[STALE_BINDING_MIGRATION_PROOF]: JSON.stringify({...proof, parentHead: 'forged'})},
    {[STALE_BINDING_MIGRATION_PROOF]: JSON.stringify({...proof, contracts: {...proof.contracts, guardsRelaxed: true}})},
    {[STALE_BINDING_MIGRATION_PROOF]: JSON.stringify({...proof, files: {...proof.files, [repo]: {...proof.files[repo], afterHash: '0'.repeat(64)}}})},
    {[repo]: readFileSync(repo, 'utf8').replace('revision = revision + 1', 'revision = revision')},
    {'src-tauri/src/application/shot_video_input_service.rs': 'changed compiler'},
    {'src/unapproved-default.ts': 'unapproved'},
    {'src-tauri/migrations/051_unapproved.sql': 'SELECT 1;'},
  ]) expect(staleBindingMigrationParentReader('.', override).violations.length).toBeGreaterThan(0);
  expect(staleBindingMigrationParentReader('.').violations).toEqual([]);
}, 120000);

it('invalidates warm-cache fixtures on live mutations, deletion and illegal files; restoration passes', () => {
  const root = mkdtempSync(join(tmpdir(), 'stale-binding-migration-proof-'));
  try {
    const paths = execFileSync('git', ['ls-files','--cached','--others','--exclude-standard'], {encoding:'utf8'}).trim().split(/\r?\n/).filter(p => p === 'vite.config.ts' || /^(\.github\/workflows\/|src\/|src-tauri\/|scripts\/|docs\/architecture\/)/.test(p));
    for (const p of paths) { mkdirSync(dirname(join(root,p)), {recursive:true}); copyFileSync(p, join(root,p)); }
    execFileSync('git', ['init','-q',root]);
    const objects = resolve(execFileSync('git', ['rev-parse','--git-path','objects'], {encoding:'utf8'}).trim());
    writeFileSync(join(root,'.git/objects/info/alternates'), objects.replaceAll('\\','/') + '\n');
    const validate = () => uiPhase7BR1MetadataParentReader(root).violations;
    expect(validate()).toEqual([]);
    for (const p of [STALE_BINDING_MIGRATION_PROOF, PARENT_PROOF, repo]) {
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
