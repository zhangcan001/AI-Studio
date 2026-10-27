// @vitest-environment node
import { expect, it } from 'vitest';
import { readFileSync, writeFileSync, copyFileSync, mkdirSync, mkdtempSync, rmSync } from 'node:fs';
import { join, dirname, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { execFileSync } from 'node:child_process';
import { successorChainParentReader } from '../successor-chain-successor-guard.mjs';
import { PROMPT_SECTION_LOAD_BASE, PROMPT_SECTION_LOAD_PROOF } from '../prompt-section-load-successor-guard.mjs';
import { DELETE_INTEGRITY_PROOF as PARENT_PROOF } from '../delete-integrity-successor-guard.mjs';

const shot = 'src/features/create/StoryboardShotRows.tsx';

it('keeps prompt sections until read succeeds and blocks save on read failure', () => {
  const src = readFileSync(shot, 'utf8');
  expect(src).toContain('sectionsReady');
  expect(src).toContain('提示词分段读取失败');
  expect(src).not.toMatch(/\.catch\(\(\) => undefined\)/);
  expect(successorChainParentReader('.').violations).toEqual([]);
}, 120000);

it('preserves the frozen delete-integrity proof bytes', () => {
  const checked = successorChainParentReader('.');
  expect(checked.violations).toEqual([]);
  expect(checked.read(shot)).toBe(execFileSync('git', ['show', `${PROMPT_SECTION_LOAD_BASE}:${shot}`], { encoding: 'utf8' }).replaceAll('\r\n', '\n'));
  expect(readFileSync(PARENT_PROOF)).toEqual(execFileSync('git', ['show', `${PROMPT_SECTION_LOAD_BASE}:${PARENT_PROOF}`]));
}, 120000);

it('rejects forged proof identity/hash, source drift, migrations and undeclared additions', () => {
  const proof = JSON.parse(readFileSync(PROMPT_SECTION_LOAD_PROOF, 'utf8'));
  for (const override of [
    { [PROMPT_SECTION_LOAD_PROOF]: JSON.stringify({ ...proof, parentHead: 'forged' }) },
    { [PROMPT_SECTION_LOAD_PROOF]: JSON.stringify({ ...proof, contracts: { ...proof.contracts, guardsRelaxed: true } }) },
    { [PROMPT_SECTION_LOAD_PROOF]: JSON.stringify({ ...proof, files: { ...proof.files, [shot]: { ...proof.files[shot], afterHash: '0'.repeat(64) } } }) },
    { [shot]: readFileSync(shot, 'utf8') + '\n// forged' },
    { 'src-tauri/src/application/shot_video_input_service.rs': 'changed compiler' },
    { 'src/unapproved-section.ts': 'unapproved' },
    { 'src-tauri/migrations/051_unapproved.sql': 'SELECT 1;' },
  ]) expect(successorChainParentReader('.', override).violations.length).toBeGreaterThan(0);
  expect(successorChainParentReader('.').violations).toEqual([]);
}, 120000);

it('invalidates warm-cache fixtures on live mutations, deletion and illegal files; restoration passes', () => {
  const root = mkdtempSync(join(tmpdir(), 'prompt-section-load-proof-'));
  try {
    const paths = execFileSync('git', ['ls-files', '--cached', '--others', '--exclude-standard'], { encoding: 'utf8' }).trim().split(/\r?\n/).filter(p => p === 'vite.config.ts' || /^(\.github\/workflows\/|src\/|src-tauri\/|scripts\/|docs\/architecture\/)/.test(p));
    for (const p of paths) { mkdirSync(dirname(join(root, p)), { recursive: true }); copyFileSync(p, join(root, p)); }
    execFileSync('git', ['init', '-q', root]);
    const objects = resolve(execFileSync('git', ['rev-parse', '--git-path', 'objects'], { encoding: 'utf8' }).trim());
    writeFileSync(join(root, '.git/objects/info/alternates'), objects.replaceAll('\\', '/') + '\n');
    const validate = () => successorChainParentReader(root).violations;
    expect(validate()).toEqual([]);
    for (const p of [PROMPT_SECTION_LOAD_PROOF, PARENT_PROOF, shot]) {
      const file = join(root, p), original = readFileSync(file);
      try {
        writeFileSync(file, Buffer.concat([original, Buffer.from('\nmutation')])); expect(validate().length).toBeGreaterThan(0);
        rmSync(file); expect(validate().length).toBeGreaterThan(0);
      } finally { writeFileSync(file, original); }
      expect(validate()).toEqual([]);
    }
  } finally { rmSync(root, { recursive: true, force: true }); }
}, 120000);
