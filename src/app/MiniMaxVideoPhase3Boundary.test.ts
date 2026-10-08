// @vitest-environment node
import { expect, it } from 'vitest';
// @ts-expect-error Node-only boundary fixture.
import { readFileSync } from 'node:fs';
// @ts-expect-error Node-only boundary fixture.
import { execFileSync } from 'node:child_process';
// @ts-expect-error Node-only successor.
import { minimaxVideoPhase3ParentReader, MINIMAX_VIDEO_PHASE3_PARENT, MINIMAX_VIDEO_PHASE3_MANIFEST } from '../../scripts/minimax-video-phase3-successor-guard.mjs';
// @ts-expect-error Node-only successor.
import { minimaxVideoPhase2ParentReader } from '../../scripts/minimax-video-phase2-successor-guard.mjs';
// @ts-expect-error Node-only successor.
import { MINIMAX_VIDEO_PHASE1_MANIFEST } from '../../scripts/readiness-lifecycle-successor-guard.mjs';

it('keeps the Phase 2 proof byte-stable and projects that checkpoint to older readers', () => {
  const proof = JSON.parse(readFileSync(MINIMAX_VIDEO_PHASE3_MANIFEST, 'utf8'));
  const phase1 = JSON.parse(readFileSync(MINIMAX_VIDEO_PHASE1_MANIFEST, 'utf8'));
  const phase2 = JSON.parse(readFileSync('docs/architecture/minimax-video-phase2.json', 'utf8'));
  expect(proof.parentHead).toBe(MINIMAX_VIDEO_PHASE3_PARENT);
  expect(proof.invariants.schemaChanged).toBe(false);
  expect(proof.invariants.backupFormatChanged).toBe(false);
  expect(proof.database).toEqual({ migrationBefore: 43, migrationAfter: 43, newTables: 0, backupBefore: 21, backupAfter: 21 });
  expect(phase1.invariants.schemaChanged).toBe(false);
  expect(phase1.invariants.backupFormatChanged).toBe(false);
  expect(phase2.invariants.schemaChanged).toBe(true);
  expect(phase2.invariants.backupFormatChanged).toBe(true);
  expect(phase2.database.migrationAfter).toBe(43);
  const accepted = minimaxVideoPhase3ParentReader('.');
  expect(accepted.violations).toEqual([]);
  expect(minimaxVideoPhase2ParentReader('.').violations).toEqual([]);
  const path = 'src-tauri/src/application/shot_batch_service.rs';
  expect(accepted.read(path)).toBe(execFileSync('git', ['show', `${MINIMAX_VIDEO_PHASE3_PARENT}:${path}`], { encoding: 'utf8' }).replaceAll('\r\n', '\n'));
  const forged = structuredClone(proof);
  forged.parentHead = '0'.repeat(40);
  const rejected = minimaxVideoPhase3ParentReader('.', forged);
  expect(rejected.violations.length).toBeGreaterThan(0);
  expect(rejected.addedPaths).toEqual([]);
}, 30000);
