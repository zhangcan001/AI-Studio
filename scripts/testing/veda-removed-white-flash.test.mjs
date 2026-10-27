// @vitest-environment node
import { expect, it } from 'vitest';
import { readFileSync, writeFileSync, copyFileSync, mkdirSync, mkdtempSync, rmSync } from 'node:fs';
import { join, dirname, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { execFileSync } from 'node:child_process';
import { vedaRemovedWhiteFlashParentReader, VEDA_REMOVED_WHITE_FLASH_BASE, VEDA_REMOVED_WHITE_FLASH_PROOF } from '../veda-removed-white-flash-successor-guard.mjs';
import { supersededImportPinParentReader as parentReader, SUPERSEDED_IMPORT_PIN_PROOF as PARENT_PROOF } from '../superseded-import-pin-successor-guard.mjs';
import { singleShotBatchControlParentReader } from '../single-shot-batch-control-successor-guard.mjs';
import { deleteIntegrityParentReader } from '../delete-integrity-successor-guard.mjs';
import { CANCEL_CORRECTNESS_BASE } from '../cancel-correctness-successor-guard.mjs';
import { uiPhase7BR1MetadataParentReader } from '../ui-phase7b-r1-metadata-successor-guard.mjs';

const builtin = 'src-tauri/src/application/builtin_runtime_packages.rs';
const policy = 'src-tauri/src/application/minimax_video_product_policy.rs';
const migration = 'src-tauri/src/infrastructure/database/repositories/workflow_library.rs';
const graph = (source, name) => JSON.parse(source.split(`const ${name}: &str = r####"`)[1].split('"####;')[0]);

it('ships Veda reference 1.3.0 that is 1.2.0 with the Veda node bypassed', () => {
  const source = readFileSync(builtin, 'utf8').replaceAll('\r\n', '\n');
  const v12 = graph(source, 'VEDA_REFERENCE_V1_2_WORKFLOW'), v13 = graph(source, 'VEDA_REFERENCE_V1_3_WORKFLOW');
  expect(v12['147'].class_type).toBe('VedaSparseAttention');
  expect(v13['147']).toBeUndefined();
  expect(Object.values(v13).some(n => n.class_type === 'VedaSparseAttention')).toBe(false);
  expect(v13['126'].inputs.model).toEqual(['145', 0]);
  const { 147: _veda, ...rest } = v12;
  expect({ ...rest, 126: { ...v12['126'], inputs: { ...v12['126'].inputs, model: ['145', 0] } } }).toEqual(v13);
  expect(source).toContain('directory: "minimax_h3_reference_veda_turbo_1_3_0"');
  expect(source).toContain('"minimax_h3_reference_veda_turbo_1_3_0" => "minimax_h3_reference_veda_turbo_1_2_0"');
  expect(source).toContain('"minimax_h3_reference_veda_turbo_1_2_0" => "minimax_h3_reference_veda_turbo_1_1_0"');
  const old = execFileSync('git', ['show', `${VEDA_REMOVED_WHITE_FLASH_BASE}:${builtin}`], { encoding: 'utf8' }).replaceAll('\r\n', '\n');
  for (const name of ['VEDA_REFERENCE_WORKFLOW', 'VEDA_REFERENCE_V1_1_WORKFLOW', 'VEDA_REFERENCE_V1_2_WORKFLOW'])
    expect(graph(old, name)).toEqual(graph(source, name));
  expect(readFileSync(policy, 'utf8')).toContain('"minimax_h3_reference_veda_turbo_1_3_0" => "1.3.0"');
  // Existing bindings and package pins follow the current version via the
  // unchanged stale-binding migration and import remap.
  // Later successor checkpoints own these bytes; compare this checkpoint's validated projection.
  expect(deleteIntegrityParentReader('.').violations).toEqual([]);
  // workflow_library later owned by review-hardening; pin to cancel tip base instead of Veda-era bytes.
  expect(execFileSync('git', ['show', `${CANCEL_CORRECTNESS_BASE}:${migration}`], { encoding: 'utf8' }).replaceAll('\r\n','\n')).toContain('migrate_stale_bindings');
  expect(execFileSync('git', ['ls-files', 'src-tauri/migrations'], { encoding: 'utf8' }))
    .toBe(execFileSync('git', ['ls-tree', '-r', '--name-only', VEDA_REMOVED_WHITE_FLASH_BASE, 'src-tauri/migrations'], { encoding: 'utf8' }));
});

it('preserves the frozen superseded-import-pin checkpoint through parent projection', () => {
  const tip = deleteIntegrityParentReader('.');
  expect(tip.violations).toEqual([]);
  expect(readFileSync(builtin,'utf8').replaceAll('\r\n','\n')).toBe(execFileSync('git', ['show', `${CANCEL_CORRECTNESS_BASE}:${builtin}`], {encoding:'utf8'}).replaceAll('\r\n','\n'));
  expect(readFileSync(PARENT_PROOF)).toEqual(execFileSync('git', ['show', `${CANCEL_CORRECTNESS_BASE}:${PARENT_PROOF}`]));
}, 120000);

it('rejects forged proof identity/hash, source drift, migrations and undeclared additions', () => {
  const proof = JSON.parse(readFileSync(VEDA_REMOVED_WHITE_FLASH_PROOF, 'utf8'));
  for (const override of [
    {[VEDA_REMOVED_WHITE_FLASH_PROOF]: JSON.stringify({...proof, parentHead: 'forged'})},
    {[VEDA_REMOVED_WHITE_FLASH_PROOF]: JSON.stringify({...proof, contracts: {...proof.contracts, guardsRelaxed: true}})},
    {[VEDA_REMOVED_WHITE_FLASH_PROOF]: JSON.stringify({...proof, files: {...proof.files, [builtin]: {...proof.files[builtin], afterHash: '0'.repeat(64)}}})},
    {[builtin]: readFileSync(builtin, 'utf8').replace('"generated_sparsity": "32"', '"generated_sparsity": "16"')},
    {[builtin]: readFileSync(builtin, 'utf8').replace('"minimax_h3_reference_veda_turbo_1_3_0" => "minimax_h3_reference_veda_turbo_1_2_0",', '')},
    {'src-tauri/src/application/shot_video_input_service.rs': 'changed compiler'},
    {'src/unapproved-default.ts': 'unapproved'},
    {'src-tauri/migrations/051_unapproved.sql': 'SELECT 1;'},
  ]) expect(deleteIntegrityParentReader('.', override).violations.length).toBeGreaterThan(0);
  expect(deleteIntegrityParentReader('.').violations).toEqual([]);
}, 120000);

it('invalidates warm-cache fixtures on live mutations, deletion and illegal files; restoration passes', () => {
  const root = mkdtempSync(join(tmpdir(), 'veda-removed-white-flash-proof-'));
  try {
    const paths = execFileSync('git', ['ls-files','--cached','--others','--exclude-standard'], {encoding:'utf8'}).trim().split(/\r?\n/).filter(p => p === 'vite.config.ts' || /^(\.github\/workflows\/|src\/|src-tauri\/|scripts\/|docs\/architecture\/)/.test(p));
    for (const p of paths) { mkdirSync(dirname(join(root,p)), {recursive:true}); copyFileSync(p, join(root,p)); }
    execFileSync('git', ['init','-q',root]);
    const objects = resolve(execFileSync('git', ['rev-parse','--git-path','objects'], {encoding:'utf8'}).trim());
    writeFileSync(join(root,'.git/objects/info/alternates'), objects.replaceAll('\\','/') + '\n');
    const validate = () => deleteIntegrityParentReader(root).violations;
    expect(validate()).toEqual([]);
    for (const p of [VEDA_REMOVED_WHITE_FLASH_PROOF, PARENT_PROOF, builtin]) {
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
