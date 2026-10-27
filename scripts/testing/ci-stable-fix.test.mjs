// @vitest-environment node
import { expect, it } from 'vitest';
import { readFileSync, writeFileSync, copyFileSync, mkdirSync, mkdtempSync, rmSync } from 'node:fs';
import { join, dirname, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { execFileSync } from 'node:child_process';
import { ciStableFixParentReader, CI_STABLE_FIX_BASE, CI_STABLE_FIX_PROOF, CI_STABLE_FIX_CHANGED, CI_STABLE_FIX_ADDED, CI_STABLE_FIX_CONTRACTS } from '../ci-stable-fix-successor-guard.mjs';
const PARENT_PROOF = 'docs/architecture/ci-closed-tip-fix-successor.json';
it('validates the standalone ci-stable-fix tip and its declared scope', () => {
  const checked = ciStableFixParentReader('.');
  expect(checked.violations).toEqual([]);
  expect(checked.changedPaths).toEqual(CI_STABLE_FIX_CHANGED);
  expect(checked.addedPaths).toEqual([...CI_STABLE_FIX_ADDED, CI_STABLE_FIX_PROOF]);
}, 120000);
it('projects changed files to the closed parent tip and preserves its proof bytes', () => {
  const checked = ciStableFixParentReader('.');
  expect(checked.violations).toEqual([]);
  for (const p of CI_STABLE_FIX_CHANGED) {
    expect(checked.read(p)).toBe(execFileSync('git', ['show', `${CI_STABLE_FIX_BASE}:${p}`], { encoding: 'utf8' }).replaceAll('\r\n', '\n'));
  }
  expect(readFileSync(PARENT_PROOF)).toEqual(execFileSync('git', ['show', `${CI_STABLE_FIX_BASE}:${PARENT_PROOF}`]));
}, 120000);
it('fixes green Phase5C needle escape and FolderBatch page waitFor without raising timeouts', () => {
  const green = readFileSync('scripts/testing/ci-green-fix.test.mjs', 'utf8');
  expect(green).toContain("UiPhase5CReviewBoundary.test.ts");
  expect(green).toContain("bytes.toString()");
  expect(readFileSync('src/app/UiPhase5CReviewBoundary.test.ts', 'utf8')).toContain("writeFileSync(p, bytes.toString() + '\\n')");
  expect(readFileSync('src/features/shots/FolderBatchImportPanel.test.tsx', 'utf8')).toContain('waitFor(() => expect(screen.getByRole("button",{name:"编辑导入镜头 51"}))');
  expect(CI_STABLE_FIX_CONTRACTS.timeoutsRaised).toBe(false);
  expect(CI_STABLE_FIX_CONTRACTS.guardsRelaxed).toBe(false);
  expect(ciStableFixParentReader('.').violations).toEqual([]);
}, 120000);
it('rejects forged proof identity, contracts, scope, hashes and undeclared changes', () => {
  const proof = JSON.parse(readFileSync(CI_STABLE_FIX_PROOF, 'utf8'));
  const sample = CI_STABLE_FIX_CHANGED[0];
  for (const override of [
    ...['checkpoint', 'parentHead', 'planningParentHead'].map(key => ({ [CI_STABLE_FIX_PROOF]: JSON.stringify({ ...proof, [key]: 'forged' }) })),
    ...Object.entries(CI_STABLE_FIX_CONTRACTS).map(([key, value]) => ({ [CI_STABLE_FIX_PROOF]: JSON.stringify({ ...proof, contracts: { ...proof.contracts, [key]: !value } }) })),
    { [CI_STABLE_FIX_PROOF]: JSON.stringify({ ...proof, files: {} }) },
    ...['beforeHash', 'afterHash'].map(key => ({ [CI_STABLE_FIX_PROOF]: JSON.stringify({ ...proof, files: { ...proof.files, [sample]: { ...proof.files[sample], [key]: '0'.repeat(64) } } }) })),
    ...[...CI_STABLE_FIX_CHANGED, ...CI_STABLE_FIX_ADDED, PARENT_PROOF].map(p => ({ [p]: readFileSync(p, 'utf8') + '\n// forged' })),
    { 'src-tauri/migrations/051_unapproved.sql': 'SELECT 1;' },
  ]) expect(ciStableFixParentReader('.', override).violations.length).toBeGreaterThan(0);
  expect(ciStableFixParentReader('.').violations).toEqual([]);
}, 120000);
it('invalidates warm-cache fixtures on live mutations, deletion and illegal files; restoration passes', () => {
  const root = mkdtempSync(join(tmpdir(), 'ci-stable-fix-proof-'));
  try {
    const paths = execFileSync('git', ['ls-files', '--cached', '--others', '--exclude-standard'], { encoding: 'utf8' }).trim().split(/\r?\n/).filter(p => p === 'vite.config.ts' || /^(\.github\/workflows\/|src\/|src-tauri\/|scripts\/|docs\/architecture\/)/.test(p));
    for (const p of paths) { mkdirSync(dirname(join(root, p)), { recursive: true }); copyFileSync(p, join(root, p)); }
    execFileSync('git', ['init', '-q', root]);
    const objects = resolve(execFileSync('git', ['rev-parse', '--git-path', 'objects'], { encoding: 'utf8' }).trim());
    writeFileSync(join(root, '.git/objects/info/alternates'), objects.replaceAll('\\', '/') + '\n');
    const validate = () => ciStableFixParentReader(root).violations;
    expect(validate()).toEqual([]);
    for (const p of [CI_STABLE_FIX_PROOF, PARENT_PROOF, 'scripts/testing/ci-green-fix.test.mjs']) {
      const file = join(root, p), original = readFileSync(file);
      try {
        writeFileSync(file, Buffer.concat([original, Buffer.from('\nmutation')])); expect(validate().length).toBeGreaterThan(0);
        rmSync(file); expect(validate().length).toBeGreaterThan(0);
      } finally { writeFileSync(file, original); }
      expect(validate()).toEqual([]);
    }
  } finally { rmSync(root, { recursive: true, force: true }); }
}, 120000);
