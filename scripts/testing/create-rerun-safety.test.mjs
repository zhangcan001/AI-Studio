import { AUDIT_REPAIR_CHANGED, AUDIT_REPAIR_ADDED, AUDIT_REPAIR_PROOF } from "../audit-repair-successor-guard.mjs";
import {USABILITY_COMPLETION_CHANGED, USABILITY_COMPLETION_ADDED, USABILITY_COMPLETION_PROOF} from '../usability-completion-successor-guard.mjs';
import {STORYBOARD_USAGE_CHANGED, STORYBOARD_USAGE_ADDED, STORYBOARD_USAGE_PROOF} from '../storyboard-usage-successor-guard.mjs';
import {REFERENCE_AUTHORITY_CHANGED, REFERENCE_AUTHORITY_ADDED, REFERENCE_AUTHORITY_PROOF} from '../reference-authority-successor-guard.mjs';
import {INPUT_REFRESH_SAFETY_CHANGED, INPUT_REFRESH_SAFETY_ADDED, INPUT_REFRESH_SAFETY_PROOF} from '../input-refresh-safety-successor-guard.mjs';
// @vitest-environment node
import { expect, it } from 'vitest';
import { readFileSync, writeFileSync, copyFileSync, mkdirSync, mkdtempSync, rmSync } from 'node:fs';
import { join, dirname, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { execFileSync } from 'node:child_process';
import {
  createRerunSafetyParentReader, CREATE_RERUN_SAFETY_BASE, CREATE_RERUN_SAFETY_PROOF,
  CREATE_RERUN_SAFETY_CHANGED, CREATE_RERUN_SAFETY_ADDED, CREATE_RERUN_SAFETY_CONTRACTS,
} from '../create-rerun-safety-successor-guard.mjs';
import { pikaFlexReferenceParentReader, PIKA_FLEX_REFERENCE_PROOF } from '../pika-flex-reference-successor-guard.mjs';
import { uiPhase7BR1MetadataParentReader } from '../ui-phase7b-r1-metadata-successor-guard.mjs';

const source = 'src/features/create/CreateController.ts';
const runtime = 'src-tauri/runtime_packages/minimax_h3_fl2va_i2v_quality_2_2_0/recipe.yaml';
const parentBytes = p => execFileSync('git', ['show', `${CREATE_RERUN_SAFETY_BASE}:${p}`]);

it('verifies the exact rerun-safety successor and projects the frozen parent bytes', () => {
  const result = createRerunSafetyParentReader('.');
  expect(result.violations).toEqual([]);
  expect(result.changedPaths).toEqual([...new Set([...CREATE_RERUN_SAFETY_CHANGED, ...INPUT_REFRESH_SAFETY_CHANGED, ...REFERENCE_AUTHORITY_CHANGED, ...STORYBOARD_USAGE_CHANGED, ...USABILITY_COMPLETION_CHANGED, ...AUDIT_REPAIR_CHANGED])].sort());
  expect(result.addedPaths).toEqual([...new Set([...CREATE_RERUN_SAFETY_ADDED, CREATE_RERUN_SAFETY_PROOF, ...INPUT_REFRESH_SAFETY_ADDED, INPUT_REFRESH_SAFETY_PROOF, ...REFERENCE_AUTHORITY_ADDED, REFERENCE_AUTHORITY_PROOF, ...STORYBOARD_USAGE_ADDED, STORYBOARD_USAGE_PROOF, ...USABILITY_COMPLETION_ADDED, USABILITY_COMPLETION_PROOF, ...AUDIT_REPAIR_ADDED, AUDIT_REPAIR_PROOF])].sort());
  for (const p of CREATE_RERUN_SAFETY_CHANGED) expect(result.read(p)).toBe(parentBytes(p).toString().replaceAll('\r\n', '\n'));
  expect(readFileSync(PIKA_FLEX_REFERENCE_PROOF)).toEqual(parentBytes(PIKA_FLEX_REFERENCE_PROOF));
  expect(pikaFlexReferenceParentReader('.').violations).toEqual([]);
  expect(uiPhase7BR1MetadataParentReader('.').violations).toEqual([]);
}, 120000);

it('rejects forged identity, scope, contracts, hashes and unapproved Queue or Runtime edits', () => {
  const proof = JSON.parse(readFileSync(CREATE_RERUN_SAFETY_PROOF, 'utf8'));
  for (const override of [
    ...['checkpoint', 'parentHead', 'planningParentHead'].map(key => ({ [CREATE_RERUN_SAFETY_PROOF]: JSON.stringify({ ...proof, [key]: 'forged' }) })),
    ...Object.entries(CREATE_RERUN_SAFETY_CONTRACTS).map(([key, value]) => ({ [CREATE_RERUN_SAFETY_PROOF]: JSON.stringify({ ...proof, contracts: { ...proof.contracts, [key]: !value } }) })),
    { [CREATE_RERUN_SAFETY_PROOF]: JSON.stringify({ ...proof, files: {} }) },
    ...['beforeHash', 'afterHash'].map(key => ({ [CREATE_RERUN_SAFETY_PROOF]: JSON.stringify({ ...proof, files: { ...proof.files, [source]: { ...proof.files[source], [key]: '0'.repeat(64) } } }) })),
    ...[...CREATE_RERUN_SAFETY_CHANGED, ...CREATE_RERUN_SAFETY_ADDED, PIKA_FLEX_REFERENCE_PROOF, runtime].map(p => ({ [p]: readFileSync(p, 'utf8') + '\n// unapproved' })),
    { 'src-tauri/src/application/production_queue_service.rs': 'unapproved queue edit' },
    { 'src-tauri/migrations/999_unapproved.sql': 'SELECT 1;' },
  ]) expect(createRerunSafetyParentReader('.', override).violations.length).toBeGreaterThan(0);
  expect(createRerunSafetyParentReader('.').violations).toEqual([]);
}, 120000);

it('keeps live freshness through the full historical chain and rejects deletion, illegal files and raw Runtime drift', () => {
  const root = mkdtempSync(join(tmpdir(), 'ai-studio-rerun-safety-'));
  try {
    const paths = execFileSync('git', ['ls-files', '--cached', '--others', '--exclude-standard'], { encoding: 'utf8' }).trim().split(/\r?\n/).filter(p => p === 'vite.config.ts' || /^(\.github\/workflows\/|src\/|src-tauri\/|scripts\/|docs\/architecture\/)/.test(p));
    for (const p of paths) { mkdirSync(dirname(join(root, p)), { recursive: true }); copyFileSync(p, join(root, p)); }
    execFileSync('git', ['init', '-q', root]);
    const objects = resolve(execFileSync('git', ['rev-parse', '--git-path', 'objects'], { encoding: 'utf8' }).trim());
    writeFileSync(join(root, '.git/objects/info/alternates'), objects.replaceAll('\\', '/') + '\n');
    const validate = () => uiPhase7BR1MetadataParentReader(root).violations;
    expect(validate()).toEqual([]);
    for (const p of [source, CREATE_RERUN_SAFETY_PROOF, PIKA_FLEX_REFERENCE_PROOF]) {
      const file = join(root, p), original = readFileSync(file);
      try {
        writeFileSync(file, Buffer.concat([original, Buffer.from('\nunapproved')]));
        expect(validate().length).toBeGreaterThan(0);
        rmSync(file);
        expect(validate().length).toBeGreaterThan(0);
      } finally { writeFileSync(file, original); }
      expect(validate()).toEqual([]);
    }
    const file = join(root, runtime), original = readFileSync(file), text = original.toString();
    try {
      writeFileSync(file, text.includes('\r\n') ? text.replaceAll('\r\n', '\n') : text.replaceAll('\n', '\r\n'));
      expect(validate().length).toBeGreaterThan(0);
    } finally { writeFileSync(file, original); }
    expect(validate()).toEqual([]);
    const illegal = join(root, 'src/rerun-safety-unapproved.ts');
    try {
      writeFileSync(join(root, '.git/info/exclude'), 'src/rerun-safety-unapproved.ts\n');
      writeFileSync(illegal, 'unapproved');
      expect(validate().length).toBeGreaterThan(0);
    } finally { rmSync(illegal, { force: true }); }
    expect(validate()).toEqual([]);
  } finally {
    if (!resolve(root).startsWith(join(resolve(tmpdir()), 'ai-studio-rerun-safety-'))) throw Error('Unsafe fixture cleanup');
    rmSync(root, { recursive: true, force: true });
  }
}, 120000);
