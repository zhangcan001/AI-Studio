// @vitest-environment node
import { expect, it } from 'vitest';
import { readFileSync, writeFileSync, mkdirSync, mkdtempSync, copyFileSync, rmSync } from 'node:fs';
import { join, dirname, resolve } from 'node:path';
import { execFileSync } from 'node:child_process';
import { uiPhase5CiParentReader, PHASE5_CI_MANIFEST, PHASE5_CI_PARENT, PHASE5_CI_EXISTING, phase5CiParentFacts, phase5CiFixtureFiles } from '../../scripts/ui-phase5-ci-repair-successor-guard.mjs';
import { uiPhase5AParentReader } from '../../scripts/ui-phase5a-storyboard-successor-guard.mjs';

it('validates CI-only live changes before projecting the exact frozen Phase5A checkpoint', () => {
  const reader = uiPhase5CiParentReader('.'); expect(reader.violations).toEqual([]);
  const proof = JSON.parse(readFileSync(PHASE5_CI_MANIFEST, 'utf8'));
  expect(proof.parentHead).toBe(PHASE5_CI_PARENT);
  expect(proof.planningParentHead).toBe('ad8db6e20bf6f84b81ce9bc0c788f32669dc552a');
  expect(proof.invariants.freshLiveIdentityPreserved).toBe(true);
  expect(proof.invariants.phase5BProductStarted).toBe(false);
  for (const p of PHASE5_CI_EXISTING) expect(reader.read(p)).toBe(phase5CiParentFacts('.').read(p));
  expect(uiPhase5AParentReader('.').violations).toEqual([]);
}, 90000);

it('rejects forged parent, scope, blob verification and freshness declarations without projection', () => {
  const proof = JSON.parse(readFileSync(PHASE5_CI_MANIFEST, 'utf8'));
  for (const mutate of [
    (p: any) => { p.parentHead = '0'.repeat(40); },
    (p: any) => { p.paths['scripts/boundary-validation-cache.mjs'].afterHash = '0'.repeat(64); },
    (p: any) => { p.paths['scripts/boundary-validation-cache.mjs'].beforeHash = '0'.repeat(64); },
    (p: any) => { p.invariants.freshLiveIdentityPreserved = false; },
    (p: any) => { p.invariants.gitBlobIdentityVerified = false; },
    (p: any) => { p.paths['src/services/tauriClient.ts'] = { beforeHash: null, afterHash: '0'.repeat(64) }; },
  ]) {
    const invalid = structuredClone(proof); mutate(invalid);
    const denied = uiPhase5CiParentReader('.', invalid);
    expect(denied.violations.length).toBeGreaterThan(0); expect(denied.addedPaths).toEqual([]); expect(denied.afterHashes).toEqual({});
  }
}, 90000);

function withPhase5CiFixture(run: (root: string, paths: string[]) => void) {
  const workspace = resolve('.'), root = mkdtempSync(join(workspace, '.codex-phase5ci-probe-'));
  try {
    const git = execFileSync('git', ['rev-parse', '--path-format=absolute', '--git-common-dir'], { encoding: 'utf8' }).trim();
    execFileSync('git', ['init', '--quiet', root]); mkdirSync(join(root, '.git/objects/info'), { recursive: true });
    writeFileSync(join(root, '.git/objects/info/alternates'), join(git, 'objects') + '\n');
    const paths: string[] = phase5CiFixtureFiles(workspace);
    for (const p of paths) { mkdirSync(dirname(join(root, p)), { recursive: true }); copyFileSync(join(workspace, p), join(root, p)); }
    expect(uiPhase5CiParentReader(root).violations).toEqual([]);
    run(root, paths);
  } finally {
    if ((!root.startsWith(workspace + '/') && !root.startsWith(workspace + '\\')) || !root.includes('.codex-phase5ci-probe-')) throw Error('Unsafe fixture cleanup');
    rmSync(root, { recursive: true, force: true });
  }
}

it('keeps live freshness for CI repair source and shell bytes with parent fail-closed checked once', () => withPhase5CiFixture((root) => {
  let parentChecked = false;
  for (const p of ['scripts/boundary-validation-cache.mjs', 'src/app/ShellHost.tsx']) {
    const path = join(root, p), bytes = readFileSync(path);
    writeFileSync(path, bytes.toString() + '\n');
    const denied = uiPhase5CiParentReader(root);
    expect(denied.violations.length, p).toBeGreaterThan(0); expect(denied.addedPaths).toEqual([]);
    if (!parentChecked) { expect(uiPhase5AParentReader(root).violations.length, p).toBeGreaterThan(0); parentChecked = true; }
    writeFileSync(path, bytes); expect(uiPhase5CiParentReader(root).violations, p).toEqual([]);
  }
}), 90000);

it('keeps live freshness for frozen proofs, Runtime raw bytes and manifest deletion', () => withPhase5CiFixture((root, paths) => {
  const runtime = paths.find(p => p.startsWith('src-tauri/runtime_packages/') && p.endsWith('.json'))!;
  let parentChecked = false;
  for (const p of ['docs/architecture/ui-phase5a-storyboard-successor.json', PHASE5_CI_MANIFEST, runtime]) {
    const path = join(root, p), bytes = readFileSync(path);
    writeFileSync(path, bytes.toString() + '\n');
    const denied = uiPhase5CiParentReader(root);
    expect(denied.violations.length, p).toBeGreaterThan(0); expect(denied.addedPaths).toEqual([]);
    if (!parentChecked) { expect(uiPhase5AParentReader(root).violations.length, p).toBeGreaterThan(0); parentChecked = true; }
    writeFileSync(path, bytes); expect(uiPhase5CiParentReader(root).violations, p).toEqual([]);
  }
  const path = join(root, PHASE5_CI_MANIFEST), bytes = readFileSync(path);
  rmSync(path); expect(uiPhase5CiParentReader(root).violations.length).toBeGreaterThan(0);
  writeFileSync(path, bytes); expect(uiPhase5CiParentReader(root).violations).toEqual([]);
}), 300000);

it('keeps live freshness for illegal additions across owned inventories', () => withPhase5CiFixture((root) => {
  for (const [dir, name] of [['src', 'IllegalOwner.js'], ['.github', 'illegal.yml'], ['docs', 'IllegalOwner.js'], ['src-tauri/examples', 'IllegalOwner.rs']]) {
    const extra = join(root, dir, name); writeFileSync(extra, 'unreviewed bytes');
    expect(uiPhase5CiParentReader(root).violations).toContain(`ui-phase5-ci-repair-inventory:${dir}`);
    rmSync(extra); expect(uiPhase5CiParentReader(root).violations).toEqual([]);
  }
}), 90000);
