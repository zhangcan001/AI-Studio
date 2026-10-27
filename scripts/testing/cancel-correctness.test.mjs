// @vitest-environment node
import { expect, it } from 'vitest';
import { readFileSync, writeFileSync, copyFileSync, mkdirSync, mkdtempSync, rmSync } from 'node:fs';
import { join, dirname, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { execFileSync } from 'node:child_process';
import { promptSectionLoadParentReader } from '../prompt-section-load-successor-guard.mjs';
import { DELETE_INTEGRITY_BASE } from '../delete-integrity-successor-guard.mjs';
import { CANCEL_CORRECTNESS_BASE, CANCEL_CORRECTNESS_PROOF } from '../cancel-correctness-successor-guard.mjs';
import { REVIEW_HARDENING_PROOF as PARENT_PROOF } from '../review-hardening-successor-guard.mjs';
import { uiPhase7BR1MetadataParentReader } from '../ui-phase7b-r1-metadata-successor-guard.mjs';

const orch = 'src-tauri/src/application/production_orchestrator_service.rs';
const queue = 'src-tauri/src/application/production_queue_service.rs';
const repo = 'src-tauri/src/infrastructure/database/repositories/production_queue.rs';

it('run cancel cancels unfinished child batches and dispatching cancel waits for bind', () => {
  const orchSrc = readFileSync(orch, 'utf8');
  expect(orchSrc).toContain('.cancel_batch(project_id, &batch.id)');
  expect(orchSrc).not.toMatch(/if batch\.status == "READY"[\s\S]{0,120}cancel_pending/);
  expect(readFileSync(queue, 'utf8')).toContain('while item.status == ProductionBatchItemStatus::Dispatching && item.task_id.is_none()');
  expect(readFileSync(repo, 'utf8')).toContain("AND (? <> 'CANCELLED' OR task_id IS NULL)");
  expect(promptSectionLoadParentReader('.').violations).toEqual([]);
}, 120000);

it('preserves the frozen review-hardening proof bytes', () => {
  const checked = promptSectionLoadParentReader('.');
  expect(checked.violations).toEqual([]);
  expect(checked.read(orch)).toBe(execFileSync('git', ['show', `${DELETE_INTEGRITY_BASE}:${orch}`], { encoding: 'utf8' }));
  expect(readFileSync(PARENT_PROOF)).toEqual(execFileSync('git', ['show', `${CANCEL_CORRECTNESS_BASE}:${PARENT_PROOF}`]));
}, 120000);

it('rejects forged proof identity/hash, source drift, migrations and undeclared additions', () => {
  const proof = JSON.parse(readFileSync(CANCEL_CORRECTNESS_PROOF, 'utf8'));
  for (const override of [
    { [CANCEL_CORRECTNESS_PROOF]: JSON.stringify({ ...proof, parentHead: 'forged' }) },
    { [CANCEL_CORRECTNESS_PROOF]: JSON.stringify({ ...proof, contracts: { ...proof.contracts, guardsRelaxed: true } }) },
    { [CANCEL_CORRECTNESS_PROOF]: JSON.stringify({ ...proof, files: { ...proof.files, [orch]: { ...proof.files[orch], afterHash: '0'.repeat(64) } } }) },
    { [orch]: readFileSync(orch, 'utf8') + '\n// forged' },
    { 'src-tauri/src/application/shot_video_input_service.rs': 'changed compiler' },
    { 'src/unapproved-default.ts': 'unapproved' },
    { 'src-tauri/migrations/051_unapproved.sql': 'SELECT 1;' },
  ]) expect(promptSectionLoadParentReader('.', override).violations.length).toBeGreaterThan(0);
  expect(promptSectionLoadParentReader('.').violations).toEqual([]);
}, 120000);

it('invalidates warm-cache fixtures on live mutations, deletion and illegal files; restoration passes', () => {
  const root = mkdtempSync(join(tmpdir(), 'cancel-correctness-proof-'));
  try {
    const paths = execFileSync('git', ['ls-files', '--cached', '--others', '--exclude-standard'], { encoding: 'utf8' }).trim().split(/\r?\n/).filter(p => p === 'vite.config.ts' || /^(\.github\/workflows\/|src\/|src-tauri\/|scripts\/|docs\/architecture\/)/.test(p));
    for (const p of paths) { mkdirSync(dirname(join(root, p)), { recursive: true }); copyFileSync(p, join(root, p)); }
    execFileSync('git', ['init', '-q', root]);
    const objects = resolve(execFileSync('git', ['rev-parse', '--git-path', 'objects'], { encoding: 'utf8' }).trim());
    writeFileSync(join(root, '.git/objects/info/alternates'), objects.replaceAll('\\', '/') + '\n');
    const validate = () => promptSectionLoadParentReader(root).violations;
    expect(validate()).toEqual([]);
    for (const p of [CANCEL_CORRECTNESS_PROOF, PARENT_PROOF, orch]) {
      const file = join(root, p), original = readFileSync(file);
      try {
        writeFileSync(file, Buffer.concat([original, Buffer.from('\nmutation')])); expect(validate().length).toBeGreaterThan(0);
        rmSync(file); expect(validate().length).toBeGreaterThan(0);
      } finally { writeFileSync(file, original); }
      expect(validate()).toEqual([]);
    }
  } finally { rmSync(root, { recursive: true, force: true }); }
}, 120000);
