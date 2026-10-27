// @vitest-environment node
import { expect, it } from 'vitest';
import { readFileSync, writeFileSync, copyFileSync, mkdirSync, mkdtempSync, rmSync } from 'node:fs';
import { join, dirname, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { execFileSync } from 'node:child_process';
import { ciClosedTipFixParentReader, CI_CLOSED_TIP_FIX_BASE, CI_CLOSED_TIP_FIX_PROOF, CI_CLOSED_TIP_FIX_CHANGED, CI_CLOSED_TIP_FIX_ADDED, CI_CLOSED_TIP_FIX_CONTRACTS } from '../ci-closed-tip-fix-successor-guard.mjs';
const PARENT_PROOF = 'docs/architecture/ci-assert-fix-successor.json';
it('validates the standalone ci-closed-tip-fix tip and its declared scope', () => {
  const checked = ciClosedTipFixParentReader('.');
  expect(checked.violations).toEqual([]);
  for (const p of CI_CLOSED_TIP_FIX_CHANGED) expect(checked.changedPaths).toContain(p);
  for (const p of [...CI_CLOSED_TIP_FIX_ADDED, CI_CLOSED_TIP_FIX_PROOF]) expect(checked.addedPaths).toContain(p);
}, 120000);
it('projects changed files to the closed parent tip and preserves its proof bytes', () => {
  const checked = ciClosedTipFixParentReader('.');
  expect(checked.violations).toEqual([]);
  for (const p of CI_CLOSED_TIP_FIX_CHANGED) {
    expect(checked.read(p)).toBe(execFileSync('git', ['show', `${CI_CLOSED_TIP_FIX_BASE}:${p}`], { encoding: 'utf8' }).replaceAll('\r\n', '\n'));
  }
  expect(readFileSync(PARENT_PROOF)).toEqual(execFileSync('git', ['show', `${CI_CLOSED_TIP_FIX_BASE}:${PARENT_PROOF}`]));
}, 120000);
it('keeps successor-aware closed-tip scope asserts and runtime drift fail-closed without raising timeouts', () => {
  expect(readFileSync('scripts/testing/ci-green-fix.test.mjs', 'utf8')).toContain('toContain(p)');
  expect(readFileSync('scripts/testing/folder-batch-import.test.mjs', 'utf8')).toContain('Buffer.concat([original, Buffer.from');
  expect(CI_CLOSED_TIP_FIX_CONTRACTS.timeoutsRaised).toBe(false);
  expect(CI_CLOSED_TIP_FIX_CONTRACTS.guardsRelaxed).toBe(false);
  expect(ciClosedTipFixParentReader('.').violations).toEqual([]);
}, 120000);
it('rejects forged proof identity, contracts, scope, hashes and undeclared changes', () => {
  const proof = JSON.parse(readFileSync(CI_CLOSED_TIP_FIX_PROOF, 'utf8'));
  const sample = CI_CLOSED_TIP_FIX_CHANGED[0];
  for (const override of [
    ...['checkpoint', 'parentHead', 'planningParentHead'].map(key => ({ [CI_CLOSED_TIP_FIX_PROOF]: JSON.stringify({ ...proof, [key]: 'forged' }) })),
    ...Object.entries(CI_CLOSED_TIP_FIX_CONTRACTS).map(([key, value]) => ({ [CI_CLOSED_TIP_FIX_PROOF]: JSON.stringify({ ...proof, contracts: { ...proof.contracts, [key]: !value } }) })),
    { [CI_CLOSED_TIP_FIX_PROOF]: JSON.stringify({ ...proof, files: {} }) },
    ...['beforeHash', 'afterHash'].map(key => ({ [CI_CLOSED_TIP_FIX_PROOF]: JSON.stringify({ ...proof, files: { ...proof.files, [sample]: { ...proof.files[sample], [key]: '0'.repeat(64) } } }) })),
    ...[...CI_CLOSED_TIP_FIX_CHANGED, ...CI_CLOSED_TIP_FIX_ADDED, PARENT_PROOF].map(p => ({ [p]: readFileSync(p, 'utf8') + '\n// forged' })),
    { 'src-tauri/migrations/051_unapproved.sql': 'SELECT 1;' },
  ]) expect(ciClosedTipFixParentReader('.', override).violations.length).toBeGreaterThan(0);
  expect(ciClosedTipFixParentReader('.').violations).toEqual([]);
}, 120000);
it('invalidates warm-cache fixtures on live mutations, deletion and illegal files; restoration passes', () => {
  const root = mkdtempSync(join(tmpdir(), 'ci-closed-tip-fix-proof-'));
  try {
    const paths = execFileSync('git', ['ls-files', '--cached', '--others', '--exclude-standard'], { encoding: 'utf8' }).trim().split(/\r?\n/).filter(p => p === 'vite.config.ts' || /^(\.github\/workflows\/|src\/|src-tauri\/|scripts\/|docs\/architecture\/)/.test(p));
    for (const p of paths) { mkdirSync(dirname(join(root, p)), { recursive: true }); copyFileSync(p, join(root, p)); }
    execFileSync('git', ['init', '-q', root]);
    const objects = resolve(execFileSync('git', ['rev-parse', '--git-path', 'objects'], { encoding: 'utf8' }).trim());
    writeFileSync(join(root, '.git/objects/info/alternates'), objects.replaceAll('\\', '/') + '\n');
    const validate = () => ciClosedTipFixParentReader(root).violations;
    expect(validate()).toEqual([]);
    for (const p of [CI_CLOSED_TIP_FIX_PROOF, PARENT_PROOF, 'scripts/testing/folder-batch-import.test.mjs']) {
      const file = join(root, p), original = readFileSync(file);
      try {
        writeFileSync(file, Buffer.concat([original, Buffer.from('\nmutation')])); expect(validate().length).toBeGreaterThan(0);
        rmSync(file); expect(validate().length).toBeGreaterThan(0);
      } finally { writeFileSync(file, original); }
      expect(validate()).toEqual([]);
    }
  } finally { rmSync(root, { recursive: true, force: true }); }
}, 120000);
