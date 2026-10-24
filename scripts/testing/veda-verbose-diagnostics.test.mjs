// @vitest-environment node
import { expect, it } from 'vitest';
import { readFileSync, writeFileSync, copyFileSync, mkdirSync, mkdtempSync, rmSync } from 'node:fs';
import { join, dirname, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { execFileSync } from 'node:child_process';
import { vedaVerboseDiagnosticsParentReader, VEDA_VERBOSE_DIAGNOSTICS_BASE, VEDA_VERBOSE_DIAGNOSTICS_PROOF } from '../veda-verbose-diagnostics-successor-guard.mjs';
import { frontendCiFourShardsParentReader as parentReader, FRONTEND_CI_FOUR_SHARDS_PROOF as PARENT_PROOF } from '../frontend-ci-four-shards-successor-guard.mjs';
import { uiPhase7BR1MetadataParentReader } from '../ui-phase7b-r1-metadata-successor-guard.mjs';

const builtin = 'src-tauri/src/application/builtin_runtime_packages.rs';
const policy = 'src-tauri/src/application/minimax_video_product_policy.rs';
const graph = (source, name) => JSON.parse(source.split(`const ${name}: &str = r####"`)[1].split('"####;')[0]);

it('ships Veda reference 1.2.0 that differs from 1.1.0 only by Veda verbose=true', () => {
  const source = readFileSync(builtin, 'utf8').replaceAll('\r\n', '\n');
  const v11 = graph(source, 'VEDA_REFERENCE_V1_1_WORKFLOW'), v12 = graph(source, 'VEDA_REFERENCE_V1_2_WORKFLOW');
  expect(v11['147'].inputs.verbose).toBe(false);
  expect(v12['147'].inputs.verbose).toBe(true);
  expect({ ...v12, 147: { ...v12['147'], inputs: { ...v12['147'].inputs, verbose: false } } }).toEqual(v11);
  expect(v12['147'].inputs.generated_sparsity).toBe('32');
  expect(v12['147'].inputs.reference_sparsity).toBe('32');
  expect(source).toContain('directory: "minimax_h3_reference_veda_turbo_1_2_0"');
  expect(source).toContain('"minimax_h3_reference_veda_turbo_1_2_0" => "minimax_h3_reference_veda_turbo_1_1_0"');
  expect(source).toContain('"minimax_h3_reference_veda_turbo_1_1_0" => "minimax_h3_reference_veda_turbo_1_0_0"');
  const old = execFileSync('git', ['show', `${VEDA_VERBOSE_DIAGNOSTICS_BASE}:${builtin}`], { encoding: 'utf8' }).replaceAll('\r\n', '\n');
  expect(graph(old, 'VEDA_REFERENCE_V1_1_WORKFLOW')).toEqual(v11);
  expect(graph(old, 'VEDA_REFERENCE_WORKFLOW')).toEqual(graph(source, 'VEDA_REFERENCE_WORKFLOW'));
  const rules = readFileSync(policy, 'utf8');
  expect(rules).toContain('"minimax_h3_reference_veda_turbo_1_2_0" => "1.2.0"');
});

it('preserves the frozen frontend-ci-four-shards checkpoint through parent projection', () => {
  const checked = vedaVerboseDiagnosticsParentReader('.');
  expect(checked.violations).toEqual([]);
  expect(checked.read(builtin)).toBe(execFileSync('git', ['show', `${VEDA_VERBOSE_DIAGNOSTICS_BASE}:${builtin}`], {encoding:'utf8'}).replaceAll('\r\n','\n'));
  expect(readFileSync(PARENT_PROOF)).toEqual(execFileSync('git', ['show', `${VEDA_VERBOSE_DIAGNOSTICS_BASE}:${PARENT_PROOF}`]));
  expect(parentReader('.').violations).toEqual([]);
}, 120000);

it('rejects forged proof identity/hash, source drift, migrations and undeclared additions', () => {
  const proof = JSON.parse(readFileSync(VEDA_VERBOSE_DIAGNOSTICS_PROOF, 'utf8'));
  for (const override of [
    {[VEDA_VERBOSE_DIAGNOSTICS_PROOF]: JSON.stringify({...proof, parentHead: 'forged'})},
    {[VEDA_VERBOSE_DIAGNOSTICS_PROOF]: JSON.stringify({...proof, contracts: {...proof.contracts, guardsRelaxed: true}})},
    {[VEDA_VERBOSE_DIAGNOSTICS_PROOF]: JSON.stringify({...proof, files: {...proof.files, [builtin]: {...proof.files[builtin], afterHash: '0'.repeat(64)}}})},
    {[builtin]: readFileSync(builtin, 'utf8').replace('"generated_sparsity": "32"', '"generated_sparsity": "16"')},
    {'src-tauri/src/application/shot_video_input_service.rs': 'changed compiler'},
    {'src/unapproved-default.ts': 'unapproved'},
    {'src-tauri/migrations/051_unapproved.sql': 'SELECT 1;'},
  ]) expect(vedaVerboseDiagnosticsParentReader('.', override).violations.length).toBeGreaterThan(0);
  expect(vedaVerboseDiagnosticsParentReader('.').violations).toEqual([]);
}, 120000);

it('invalidates warm-cache fixtures on live mutations, deletion and illegal files; restoration passes', () => {
  const root = mkdtempSync(join(tmpdir(), 'veda-verbose-diagnostics-proof-'));
  try {
    const paths = execFileSync('git', ['ls-files','--cached','--others','--exclude-standard'], {encoding:'utf8'}).trim().split(/\r?\n/).filter(p => p === 'vite.config.ts' || /^(\.github\/workflows\/|src\/|src-tauri\/|scripts\/|docs\/architecture\/)/.test(p));
    for (const p of paths) { mkdirSync(dirname(join(root,p)), {recursive:true}); copyFileSync(p, join(root,p)); }
    execFileSync('git', ['init','-q',root]);
    const objects = resolve(execFileSync('git', ['rev-parse','--git-path','objects'], {encoding:'utf8'}).trim());
    writeFileSync(join(root,'.git/objects/info/alternates'), objects.replaceAll('\\','/') + '\n');
    const validate = () => uiPhase7BR1MetadataParentReader(root).violations;
    expect(validate()).toEqual([]);
    for (const p of [VEDA_VERBOSE_DIAGNOSTICS_PROOF, PARENT_PROOF, builtin]) {
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
