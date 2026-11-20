import { PROJECT_AUDIT_REPAIR_PROOF } from "../audit70-ci-integration-successor-guard.mjs";
import { AUDIT70_CI_INTEGRATION_CHANGED, AUDIT70_CI_INTEGRATION_ADDED, AUDIT70_CI_INTEGRATION_PROOF } from "../audit70-ci-integration-successor-guard.mjs";
import { AUDIT70_CI_FIXTURES_CHANGED, AUDIT70_CI_FIXTURES_ADDED, AUDIT70_CI_FIXTURES_PROOF } from "../audit70-ci-fixtures-successor-guard.mjs";
import { AUDIT70_CI_VALIDATION_CHANGED, AUDIT70_CI_VALIDATION_ADDED, AUDIT70_CI_VALIDATION_PROOF } from "../audit70-ci-validation-successor-guard.mjs";
import { AUDIT70_CI_REPAIR_CHANGED, AUDIT70_CI_REPAIR_ADDED, AUDIT70_CI_REPAIR_PROOF } from "../audit70-ci-repair-successor-guard.mjs";
import { AUDIT70_RECOVERY_CHANGED, AUDIT70_RECOVERY_ADDED, AUDIT70_RECOVERY_PROOF } from "../audit70-recovery-successor-guard.mjs";
import { AUDIT70_COMPLETION_CHANGED, AUDIT70_COMPLETION_ADDED, AUDIT70_COMPLETION_PROOF } from "../audit70-completion-successor-guard.mjs";
import { QUEUE_SYNC_IMPORT_PAGING_CHANGED, QUEUE_SYNC_IMPORT_PAGING_ADDED, QUEUE_SYNC_IMPORT_PAGING_PROOF } from "../queue-sync-import-paging-successor-guard.mjs";
import { LIBRARY_HIDDEN_PROMPTS_CHANGED, LIBRARY_HIDDEN_PROMPTS_ADDED, LIBRARY_HIDDEN_PROMPTS_PROOF } from "../library-hidden-prompts-successor-guard.mjs";
import { RUN_RECOVERY_POLISH_CHANGED, RUN_RECOVERY_POLISH_ADDED, RUN_RECOVERY_POLISH_PROOF } from "../run-recovery-polish-successor-guard.mjs";
import { STORYBOARD_INTEGRITY_CHANGED, STORYBOARD_INTEGRITY_ADDED, STORYBOARD_INTEGRITY_PROOF } from "../storyboard-integrity-successor-guard.mjs";
import { STORYBOARD_FEEDBACK_CHANGED, STORYBOARD_FEEDBACK_ADDED, STORYBOARD_FEEDBACK_PROOF } from "../storyboard-feedback-successor-guard.mjs";
import { PRODUCTION_SAFETY_CHANGED, PRODUCTION_SAFETY_ADDED, PRODUCTION_SAFETY_PROOF } from "../production-safety-successor-guard.mjs";
import { USAGE_POLISH_CHANGED, USAGE_POLISH_ADDED, USAGE_POLISH_PROOF } from "../usage-polish-successor-guard.mjs";
import { CONVENIENCE_CHANGED, CONVENIENCE_ADDED, CONVENIENCE_PROOF } from "../convenience-successor-guard.mjs";
import { USAGE_FLOW_CHANGED, USAGE_FLOW_ADDED, USAGE_FLOW_PROOF } from "../usage-flow-successor-guard.mjs";
// @vitest-environment node
import { expect, it } from 'vitest';
import { runOverrideProbeSession } from '../boundary-validation-cache.mjs';
import { readFileSync, writeFileSync, copyFileSync, mkdirSync, mkdtempSync, rmSync } from 'node:fs';
import { join, dirname, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { execFileSync } from 'node:child_process';
import { cachedBoundary } from '../boundary-validation-cache.mjs';
import {
  auditRepairParentReader, AUDIT_REPAIR_BASE, AUDIT_REPAIR_PROOF,
  AUDIT_REPAIR_CHANGED, AUDIT_REPAIR_ADDED, AUDIT_REPAIR_CONTRACTS,
} from '../audit-repair-successor-guard.mjs';
import { usabilityCompletionParentReader as pikaFlexReferenceParentReader, USABILITY_COMPLETION_PROOF as PIKA_FLEX_REFERENCE_PROOF } from '../usability-completion-successor-guard.mjs';
import { uiPhase7BR1MetadataParentReader } from '../ui-phase7b-r1-metadata-successor-guard.mjs';

const source = 'src/features/create/StoryboardShotRows.tsx';
const runtime = 'src-tauri/runtime_packages/minimax_h3_fl2va_i2v_quality_2_2_0/recipe.yaml';
const parentBytes = p => execFileSync('git', ['show', `${AUDIT_REPAIR_BASE}:${p}`]);

it('verifies the exact audit-repair successor and projects the frozen parent bytes', () => {
  const result = auditRepairParentReader('.');
  expect(result.violations).toEqual([]);
  expect(result.changedPaths).toEqual([...new Set([...AUDIT_REPAIR_CHANGED, ...USAGE_FLOW_CHANGED, ...CONVENIENCE_CHANGED, ...USAGE_POLISH_CHANGED, ...PRODUCTION_SAFETY_CHANGED, ...STORYBOARD_FEEDBACK_CHANGED, ...STORYBOARD_INTEGRITY_CHANGED, ...RUN_RECOVERY_POLISH_CHANGED, ...LIBRARY_HIDDEN_PROMPTS_CHANGED, ...QUEUE_SYNC_IMPORT_PAGING_CHANGED, ...AUDIT70_RECOVERY_CHANGED, ...AUDIT70_COMPLETION_CHANGED, ...AUDIT70_CI_REPAIR_CHANGED, ...AUDIT70_CI_VALIDATION_CHANGED, ...AUDIT70_CI_FIXTURES_CHANGED, ...AUDIT70_CI_INTEGRATION_CHANGED])].sort());
  expect(result.addedPaths).toEqual([...new Set([...AUDIT_REPAIR_ADDED, AUDIT_REPAIR_PROOF, ...USAGE_FLOW_ADDED, USAGE_FLOW_PROOF, ...CONVENIENCE_ADDED, CONVENIENCE_PROOF, ...USAGE_POLISH_ADDED, USAGE_POLISH_PROOF, ...PRODUCTION_SAFETY_ADDED, PRODUCTION_SAFETY_PROOF, ...STORYBOARD_FEEDBACK_ADDED, STORYBOARD_FEEDBACK_PROOF, ...STORYBOARD_INTEGRITY_ADDED, STORYBOARD_INTEGRITY_PROOF, ...RUN_RECOVERY_POLISH_ADDED, RUN_RECOVERY_POLISH_PROOF, ...LIBRARY_HIDDEN_PROMPTS_ADDED, LIBRARY_HIDDEN_PROMPTS_PROOF, ...QUEUE_SYNC_IMPORT_PAGING_ADDED, QUEUE_SYNC_IMPORT_PAGING_PROOF, ...AUDIT70_RECOVERY_ADDED, AUDIT70_RECOVERY_PROOF, ...AUDIT70_COMPLETION_ADDED, AUDIT70_COMPLETION_PROOF, ...AUDIT70_CI_REPAIR_ADDED, AUDIT70_CI_REPAIR_PROOF, ...AUDIT70_CI_VALIDATION_ADDED, AUDIT70_CI_VALIDATION_PROOF, ...AUDIT70_CI_FIXTURES_ADDED, AUDIT70_CI_FIXTURES_PROOF, ...AUDIT70_CI_INTEGRATION_ADDED, AUDIT70_CI_INTEGRATION_PROOF, PROJECT_AUDIT_REPAIR_PROOF])].sort());
  for (const p of AUDIT_REPAIR_CHANGED) expect(result.read(p)).toBe(parentBytes(p).toString().replaceAll('\r\n', '\n'));
  expect(readFileSync(PIKA_FLEX_REFERENCE_PROOF)).toEqual(parentBytes(PIKA_FLEX_REFERENCE_PROOF));
  expect(pikaFlexReferenceParentReader('.').violations).toEqual([]);
  expect(uiPhase7BR1MetadataParentReader('.').violations).toEqual([]);
}, 120000);

it('rejects forged identity, scope, contracts, hashes and unapproved Queue or Runtime edits', () => {
  const proof = JSON.parse(readFileSync(AUDIT_REPAIR_PROOF, 'utf8'));
  runOverrideProbeSession('.', () => {
    for (const override of [
      ...['checkpoint', 'parentHead', 'planningParentHead'].map(key => ({ [AUDIT_REPAIR_PROOF]: JSON.stringify({ ...proof, [key]: 'forged' }) })),
      ...Object.entries(AUDIT_REPAIR_CONTRACTS).map(([key, value]) => ({ [AUDIT_REPAIR_PROOF]: JSON.stringify({ ...proof, contracts: { ...proof.contracts, [key]: !value } }) })),
      { [AUDIT_REPAIR_PROOF]: JSON.stringify({ ...proof, files: {} }) },
      ...['beforeHash', 'afterHash'].map(key => ({ [AUDIT_REPAIR_PROOF]: JSON.stringify({ ...proof, files: { ...proof.files, [source]: { ...proof.files[source], [key]: '0'.repeat(64) } } }) })),
      ...[...AUDIT_REPAIR_CHANGED, ...AUDIT_REPAIR_ADDED, PIKA_FLEX_REFERENCE_PROOF, runtime].map(p => ({ [p]: readFileSync(p, 'utf8') + '\n// unapproved' })),
      { 'src-tauri/src/application/production_queue_service.rs': 'unapproved queue edit' },
      { 'src-tauri/migrations/999_unapproved.sql': 'SELECT 1;' },
    ]) {
      // Share one live-byte read session for this override across the guard chain.
      const result = cachedBoundary('.', 'audit-repair-override-probe', override,
        () => auditRepairParentReader('.', override));
      expect(result.violations.length).toBeGreaterThan(0);
    }
  });
  expect(auditRepairParentReader('.').violations).toEqual([]);
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
    for (const p of [source, AUDIT_REPAIR_PROOF, PIKA_FLEX_REFERENCE_PROOF]) {
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
