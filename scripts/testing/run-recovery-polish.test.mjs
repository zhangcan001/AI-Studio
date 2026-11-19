import { AUDIT70_CI_FIXTURES_CHANGED, AUDIT70_CI_FIXTURES_ADDED, AUDIT70_CI_FIXTURES_PROOF } from "../audit70-ci-fixtures-successor-guard.mjs";
import { AUDIT70_CI_VALIDATION_CHANGED, AUDIT70_CI_VALIDATION_ADDED, AUDIT70_CI_VALIDATION_PROOF } from "../audit70-ci-validation-successor-guard.mjs";
import { AUDIT70_CI_REPAIR_CHANGED, AUDIT70_CI_REPAIR_ADDED, AUDIT70_CI_REPAIR_PROOF } from "../audit70-ci-repair-successor-guard.mjs";
import { AUDIT70_RECOVERY_CHANGED, AUDIT70_RECOVERY_ADDED, AUDIT70_RECOVERY_PROOF } from "../audit70-recovery-successor-guard.mjs";
import { AUDIT70_COMPLETION_CHANGED, AUDIT70_COMPLETION_ADDED, AUDIT70_COMPLETION_PROOF } from "../audit70-completion-successor-guard.mjs";
import { QUEUE_SYNC_IMPORT_PAGING_CHANGED, QUEUE_SYNC_IMPORT_PAGING_ADDED, QUEUE_SYNC_IMPORT_PAGING_PROOF } from "../queue-sync-import-paging-successor-guard.mjs";
// @vitest-environment node
import { expect, it } from 'vitest';
import { runOverrideProbeSession } from '../boundary-validation-cache.mjs';
import { readFileSync, writeFileSync, copyFileSync, mkdirSync, mkdtempSync, rmSync } from 'node:fs';
import { join, dirname, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { execFileSync } from 'node:child_process';
import { LIBRARY_HIDDEN_PROMPTS_CHANGED, LIBRARY_HIDDEN_PROMPTS_ADDED, LIBRARY_HIDDEN_PROMPTS_PROOF } from '../library-hidden-prompts-successor-guard.mjs';
import {
  runRecoveryPolishParentReader, RUN_RECOVERY_POLISH_BASE, RUN_RECOVERY_POLISH_PROOF,
  RUN_RECOVERY_POLISH_CHANGED, RUN_RECOVERY_POLISH_ADDED, RUN_RECOVERY_POLISH_CONTRACTS,
} from '../run-recovery-polish-successor-guard.mjs';
import { storyboardIntegrityParentReader, STORYBOARD_INTEGRITY_PROOF } from '../storyboard-integrity-successor-guard.mjs';
import { uiPhase7BR1MetadataParentReader } from '../ui-phase7b-r1-metadata-successor-guard.mjs';

const source = 'src/features/runs/runRecoveryPresentation.ts';
const runtime = 'src-tauri/runtime_packages/minimax_h3_fl2va_i2v_quality_2_2_0/recipe.yaml';
const parentBytes = p => execFileSync('git', ['show', `${RUN_RECOVERY_POLISH_BASE}:${p}`]);

it('verifies the exact run-recovery-polish successor and projects the frozen parent bytes', () => {
  const result = runRecoveryPolishParentReader('.');
  expect(result.violations).toEqual([]);
  expect(result.changedPaths).toEqual([...new Set([...RUN_RECOVERY_POLISH_CHANGED, ...LIBRARY_HIDDEN_PROMPTS_CHANGED, ...QUEUE_SYNC_IMPORT_PAGING_CHANGED, ...AUDIT70_RECOVERY_CHANGED, ...AUDIT70_COMPLETION_CHANGED, ...AUDIT70_CI_REPAIR_CHANGED, ...AUDIT70_CI_VALIDATION_CHANGED, ...AUDIT70_CI_FIXTURES_CHANGED])].sort());
  expect(result.addedPaths).toEqual([...new Set([...RUN_RECOVERY_POLISH_ADDED, RUN_RECOVERY_POLISH_PROOF, ...LIBRARY_HIDDEN_PROMPTS_ADDED, LIBRARY_HIDDEN_PROMPTS_PROOF, ...QUEUE_SYNC_IMPORT_PAGING_ADDED, QUEUE_SYNC_IMPORT_PAGING_PROOF, ...AUDIT70_RECOVERY_ADDED, AUDIT70_RECOVERY_PROOF, ...AUDIT70_COMPLETION_ADDED, AUDIT70_COMPLETION_PROOF, ...AUDIT70_CI_REPAIR_ADDED, AUDIT70_CI_REPAIR_PROOF, ...AUDIT70_CI_VALIDATION_ADDED, AUDIT70_CI_VALIDATION_PROOF, ...AUDIT70_CI_FIXTURES_ADDED, AUDIT70_CI_FIXTURES_PROOF])].sort());
  for (const p of RUN_RECOVERY_POLISH_CHANGED) expect(result.read(p)).toBe(parentBytes(p).toString().replaceAll('\r\n', '\n'));
  expect(readFileSync(STORYBOARD_INTEGRITY_PROOF)).toEqual(parentBytes(STORYBOARD_INTEGRITY_PROOF));
  expect(storyboardIntegrityParentReader('.').violations).toEqual([]);
  expect(uiPhase7BR1MetadataParentReader('.').violations).toEqual([]);
}, 120000);

it('rejects forged identity, scope, contracts, hashes and unapproved Queue or Runtime edits', () => {
  const proof = JSON.parse(readFileSync(RUN_RECOVERY_POLISH_PROOF, 'utf8'));
  runOverrideProbeSession('.', () => {
    for (const override of [
      ...['checkpoint', 'parentHead', 'planningParentHead'].map(key => ({ [RUN_RECOVERY_POLISH_PROOF]: JSON.stringify({ ...proof, [key]: 'forged' }) })),
      ...Object.entries(RUN_RECOVERY_POLISH_CONTRACTS).map(([key, value]) => ({ [RUN_RECOVERY_POLISH_PROOF]: JSON.stringify({ ...proof, contracts: { ...proof.contracts, [key]: !value } }) })),
      { [RUN_RECOVERY_POLISH_PROOF]: JSON.stringify({ ...proof, files: {} }) },
      ...['beforeHash', 'afterHash'].map(key => ({ [RUN_RECOVERY_POLISH_PROOF]: JSON.stringify({ ...proof, files: { ...proof.files, [source]: { ...proof.files[source], [key]: '0'.repeat(64) } } }) })),
      ...[...RUN_RECOVERY_POLISH_CHANGED, ...RUN_RECOVERY_POLISH_ADDED, STORYBOARD_INTEGRITY_PROOF, runtime].map(p => ({ [p]: readFileSync(p, 'utf8') + '\n// unapproved' })),
      { 'src-tauri/src/application/production_queue_service.rs': 'unapproved queue edit' },
      { 'src-tauri/migrations/999_unapproved.sql': 'SELECT 1;' },
    ]) expect(runRecoveryPolishParentReader('.', override).violations.length).toBeGreaterThan(0);
  });
  expect(runRecoveryPolishParentReader('.').violations).toEqual([]);
}, 120000);

it('keeps live freshness through the full historical chain and rejects deletion, illegal files and raw Runtime drift', () => {
  const root = mkdtempSync(join(tmpdir(), 'ai-studio-reference-authority-'));
  try {
    const paths = execFileSync('git', ['ls-files', '--cached', '--others', '--exclude-standard'], { encoding: 'utf8' }).trim().split(/\r?\n/).filter(p => p === 'vite.config.ts' || /^(\.github\/workflows\/|src\/|src-tauri\/|scripts\/|docs\/architecture\/)/.test(p));
    for (const p of paths) { mkdirSync(dirname(join(root, p)), { recursive: true }); copyFileSync(p, join(root, p)); }
    execFileSync('git', ['init', '-q', root]);
    const objects = resolve(execFileSync('git', ['rev-parse', '--git-path', 'objects'], { encoding: 'utf8' }).trim());
    writeFileSync(join(root, '.git/objects/info/alternates'), objects.replaceAll('\\', '/') + '\n');
    const validate = () => uiPhase7BR1MetadataParentReader(root).violations;
    expect(validate()).toEqual([]);
    for (const p of [source, RUN_RECOVERY_POLISH_PROOF, STORYBOARD_INTEGRITY_PROOF]) {
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
    if (!resolve(root).startsWith(join(resolve(tmpdir()), 'ai-studio-reference-authority-'))) throw Error('Unsafe fixture cleanup');
    rmSync(root, { recursive: true, force: true });
  }
}, 120000);
