// @vitest-environment node
import { expect, it } from 'vitest';
import { readFileSync, writeFileSync, copyFileSync, mkdirSync, mkdtempSync, rmSync } from 'node:fs';
import { join, dirname, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { execFileSync } from 'node:child_process';
import { promptSectionsWithoutAtParentReader, PROMPT_SECTIONS_WITHOUT_AT_BASE, PROMPT_SECTIONS_WITHOUT_AT_PROOF, PROMPT_SECTIONS_WITHOUT_AT_CHANGED, PROMPT_SECTIONS_WITHOUT_AT_ADDED, PROMPT_SECTIONS_WITHOUT_AT_CONTRACTS } from '../prompt-sections-without-at-successor-guard.mjs';
const PARENT_PROOF = 'docs/architecture/prompt-section-load-successor.json';

const source = 'src-tauri/src/application/shot_video_input_service.rs';

it('validates the standalone prompt-sections-without-at tip and its declared scope', () => {
  const checked = promptSectionsWithoutAtParentReader('.');
  expect(checked.violations).toEqual([]);
  expect(checked.changedPaths).toEqual(PROMPT_SECTIONS_WITHOUT_AT_CHANGED);
  expect(checked.addedPaths).toEqual([...PROMPT_SECTIONS_WITHOUT_AT_ADDED, PROMPT_SECTIONS_WITHOUT_AT_PROOF]);
}, 120000);

it('projects changed files to the closed parent tip and preserves its proof bytes', () => {
  const checked = promptSectionsWithoutAtParentReader('.');
  expect(checked.violations).toEqual([]);
  for (const p of PROMPT_SECTIONS_WITHOUT_AT_CHANGED) {
    expect(checked.read(p)).toBe(execFileSync('git', ['show', `${PROMPT_SECTIONS_WITHOUT_AT_BASE}:${p}`], { encoding: 'utf8' }).replaceAll('\r\n', '\n'));
  }
  expect(readFileSync(PARENT_PROOF)).toEqual(execFileSync('git', ['show', `${PROMPT_SECTIONS_WITHOUT_AT_BASE}:${PARENT_PROOF}`]));
}, 120000);

it('rejects forged proof identity, contracts, scope, hashes and undeclared changes', () => {
  const proof = JSON.parse(readFileSync(PROMPT_SECTIONS_WITHOUT_AT_PROOF, 'utf8'));
  for (const override of [
    ...['checkpoint', 'parentHead', 'planningParentHead'].map(key => ({ [PROMPT_SECTIONS_WITHOUT_AT_PROOF]: JSON.stringify({ ...proof, [key]: 'forged' }) })),
    ...Object.entries(PROMPT_SECTIONS_WITHOUT_AT_CONTRACTS).map(([key, value]) => ({ [PROMPT_SECTIONS_WITHOUT_AT_PROOF]: JSON.stringify({ ...proof, contracts: { ...proof.contracts, [key]: !value } }) })),
    { [PROMPT_SECTIONS_WITHOUT_AT_PROOF]: JSON.stringify({ ...proof, files: {} }) },
    ...['beforeHash', 'afterHash'].map(key => ({ [PROMPT_SECTIONS_WITHOUT_AT_PROOF]: JSON.stringify({ ...proof, files: { ...proof.files, [source]: { ...proof.files[source], [key]: '0'.repeat(64) } } }) })),
    ...[...PROMPT_SECTIONS_WITHOUT_AT_CHANGED, ...PROMPT_SECTIONS_WITHOUT_AT_ADDED, PARENT_PROOF].map(p => ({ [p]: readFileSync(p, 'utf8') + '\n// forged' })),
    { 'src/features/create/StoryboardShotRows.tsx': 'changed frontend' },
    { 'src-tauri/runtime_packages/minimax_h3_fl2va_i2v_quality_2_2_0/recipe.yaml': 'changed runtime' },
    { 'src/unapproved-sections-without-at.ts': 'unapproved' },
    { 'src-tauri/src/unapproved_sections_without_at.rs': 'unapproved' },
    { 'src-tauri/migrations/051_unapproved.sql': 'SELECT 1;' },
  ]) expect(promptSectionsWithoutAtParentReader('.', override).violations.length).toBeGreaterThan(0);
  expect(promptSectionsWithoutAtParentReader('.').violations).toEqual([]);
}, 120000);

it('invalidates warm-cache fixtures on mutation, deletion and ignored additions; restoration passes', () => {
  const root = mkdtempSync(join(tmpdir(), 'prompt-sections-without-at-proof-'));
  try {
    const paths = execFileSync('git', ['ls-files', '--cached', '--others', '--exclude-standard'], { encoding: 'utf8' }).trim().split(/\r?\n/).filter(p => p === 'vite.config.ts' || /^(\.github\/workflows\/|src\/|src-tauri\/|scripts\/|docs\/architecture\/)/.test(p));
    for (const p of paths) { mkdirSync(dirname(join(root, p)), { recursive: true }); copyFileSync(p, join(root, p)); }
    execFileSync('git', ['init', '-q', root]);
    const objects = resolve(execFileSync('git', ['rev-parse', '--git-path', 'objects'], { encoding: 'utf8' }).trim());
    writeFileSync(join(root, '.git/objects/info/alternates'), objects.replaceAll('\\', '/') + '\n');
    const validate = () => promptSectionsWithoutAtParentReader(root).violations;
    expect(validate()).toEqual([]);
    for (const p of [PROMPT_SECTIONS_WITHOUT_AT_PROOF, PARENT_PROOF, source, ...PROMPT_SECTIONS_WITHOUT_AT_ADDED]) {
      const file = join(root, p), original = readFileSync(file);
      try {
        writeFileSync(file, Buffer.concat([original, Buffer.from('\nmutation')])); expect(validate().length).toBeGreaterThan(0);
        rmSync(file); expect(validate().length).toBeGreaterThan(0);
      } finally { writeFileSync(file, original); }
      expect(validate()).toEqual([]);
    }
    writeFileSync(join(root, '.git/info/exclude'), 'src/unapproved-sections-without-at.ts\n');
    const illegal = join(root, 'src/unapproved-sections-without-at.ts');
    try { writeFileSync(illegal, 'unapproved'); expect(validate()).toContain('prompt-sections-without-at-illegal-file'); }
    finally { rmSync(illegal); }
    expect(validate()).toEqual([]);
  } finally { rmSync(root, { recursive: true, force: true }); }
}, 120000);
