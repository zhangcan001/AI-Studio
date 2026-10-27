// @vitest-environment node
import { expect, it } from 'vitest';
import { readFileSync, writeFileSync, copyFileSync, mkdirSync, mkdtempSync, rmSync } from 'node:fs';
import { join, dirname, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { execFileSync } from 'node:child_process';
import { ciProbeFixParentReader } from '../ci-probe-fix-successor-guard.mjs';
import { PROMPT_SECTION_LOAD_BASE } from '../prompt-section-load-successor-guard.mjs';
import { DELETE_INTEGRITY_BASE, DELETE_INTEGRITY_PROOF } from '../delete-integrity-successor-guard.mjs';
import { CANCEL_CORRECTNESS_PROOF as PARENT_PROOF } from '../cancel-correctness-successor-guard.mjs';

const shot = 'src-tauri/src/infrastructure/database/repositories/shot.rs';
const asset = 'src-tauri/src/application/asset_deletion_service.rs';

it('blocks story-entity asset refs, refuses retry-parent delete, and deletes shots atomically', () => {
  expect(readFileSync(asset, 'utf8')).toContain('story_entity_cover_names');
  expect(readFileSync('src-tauri/src/infrastructure/database/repositories/production_queue.rs', 'utf8')).toContain('delete_terminal_item_refuses_retry_parents_in_every_retry_state');
  expect(readFileSync(shot, 'utf8')).toContain('DELETE FROM shots WHERE project_id = ? AND id = ?');
  expect(ciProbeFixParentReader('.').violations).toEqual([]);
}, 120000);

it('preserves the frozen cancel-correctness proof bytes', () => {
  const checked = ciProbeFixParentReader('.');
  expect(checked.violations).toEqual([]);
  expect(readFileSync(PARENT_PROOF)).toEqual(execFileSync('git', ['show', `${DELETE_INTEGRITY_BASE}:${PARENT_PROOF}`]));
}, 120000);

it('rejects forged proof identity/hash, source drift, migrations and undeclared additions', () => {
  const proof = JSON.parse(readFileSync(DELETE_INTEGRITY_PROOF, 'utf8'));
  for (const override of [
    { [DELETE_INTEGRITY_PROOF]: JSON.stringify({ ...proof, parentHead: 'forged' }) },
    { [DELETE_INTEGRITY_PROOF]: JSON.stringify({ ...proof, contracts: { ...proof.contracts, guardsRelaxed: true } }) },
    { [DELETE_INTEGRITY_PROOF]: JSON.stringify({ ...proof, files: { ...proof.files, [shot]: { ...proof.files[shot], afterHash: '0'.repeat(64) } } }) },
    { [shot]: readFileSync(shot, 'utf8') + '\n// forged' },
    { 'src-tauri/src/application/shot_video_input_service.rs': 'changed compiler' },
    { 'src/unapproved-delete.ts': 'unapproved' },
    { 'src-tauri/migrations/051_unapproved.sql': 'SELECT 1;' },
  ]) expect(ciProbeFixParentReader('.', override).violations.length).toBeGreaterThan(0);
  expect(ciProbeFixParentReader('.').violations).toEqual([]);
}, 120000);

it('invalidates warm-cache fixtures on live mutations, deletion and illegal files; restoration passes', () => {
  const root = mkdtempSync(join(tmpdir(), 'delete-integrity-proof-'));
  try {
    const paths = execFileSync('git', ['ls-files', '--cached', '--others', '--exclude-standard'], { encoding: 'utf8' }).trim().split(/\r?\n/).filter(p => p === 'vite.config.ts' || /^(\.github\/workflows\/|src\/|src-tauri\/|scripts\/|docs\/architecture\/)/.test(p));
    for (const p of paths) { mkdirSync(dirname(join(root, p)), { recursive: true }); copyFileSync(p, join(root, p)); }
    execFileSync('git', ['init', '-q', root]);
    const objects = resolve(execFileSync('git', ['rev-parse', '--git-path', 'objects'], { encoding: 'utf8' }).trim());
    writeFileSync(join(root, '.git/objects/info/alternates'), objects.replaceAll('\\', '/') + '\n');
    const validate = () => ciProbeFixParentReader(root).violations;
    expect(validate()).toEqual([]);
    for (const p of [DELETE_INTEGRITY_PROOF, PARENT_PROOF, shot]) {
      const file = join(root, p), original = readFileSync(file);
      try {
        writeFileSync(file, Buffer.concat([original, Buffer.from('\nmutation')])); expect(validate().length).toBeGreaterThan(0);
        rmSync(file); expect(validate().length).toBeGreaterThan(0);
      } finally { writeFileSync(file, original); }
      expect(validate()).toEqual([]);
    }
  } finally { rmSync(root, { recursive: true, force: true }); }
}, 120000);
