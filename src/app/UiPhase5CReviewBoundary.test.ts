// @vitest-environment node
import { expect, it } from 'vitest';
// @ts-expect-error Node-only owned proof fixture.
import { readFileSync, writeFileSync, mkdirSync, mkdtempSync, copyFileSync, rmSync } from 'node:fs';
// @ts-expect-error Node-only fixture bounds.
import { join, dirname, resolve } from 'node:path';
// @ts-expect-error Node-only immutable object store.
import { execFileSync } from 'node:child_process';
// @ts-expect-error Node-only successor.
import { uiPhase5CParentReader, PHASE5C_MANIFEST, PHASE5C_PARENT, PHASE5C_EXISTING, phase5CParentFacts, phase5CFixtureFiles } from '../../scripts/ui-phase5c-review-successor-guard.mjs';
// @ts-expect-error Node-only frozen predecessor.
import { uiPhase5BParentReader } from '../../scripts/ui-phase5b-mixed-video-successor-guard.mjs';

it('validates Phase5C live changes before projecting the exact frozen Phase5B checkpoint and historical chain', () => {
  const reader = uiPhase5CParentReader('.'); expect(reader.violations).toEqual([]);
  const proof = JSON.parse(readFileSync(PHASE5C_MANIFEST, 'utf8'));
  expect(proof.parentHead).toBe(PHASE5C_PARENT);
  expect(proof.planningParentHead).toBe('14c9ebbf09b402faf2c6b1fca6864401a946bc7a');
  expect(proof.invariants.asyncOwnerFenced).toBe(true);
  expect(proof.invariants.phase5CStarted).toBe(true);
  for (const p of PHASE5C_EXISTING) expect(reader.read(p)).toBe(phase5CParentFacts('.').read(p));
  expect(uiPhase5BParentReader('.').violations).toEqual([]);
}, 90000);

it('rejects forged parent, scope, blob verification and freshness declarations without projection', () => {
  const proof = JSON.parse(readFileSync(PHASE5C_MANIFEST, 'utf8'));
  for (const mutate of [
    (p: any) => { p.parentHead = '0'.repeat(40); },
    (p: any) => { p.paths['src/features/production/ProductionReviewInbox.tsx'].afterHash = '0'.repeat(64); },
    (p: any) => { p.paths['src/features/production/ProductionReviewInbox.tsx'].beforeHash = '0'.repeat(64); },
    (p: any) => { p.invariants.asyncOwnerFenced = false; },
    (p: any) => { p.invariants.candidateSelectionDistinct = false; },
    (p: any) => { p.invariants.realGpuExecuted = true; },
    (p: any) => { p.paths['src/services/tauriClient.ts'] = { beforeHash: null, afterHash: '0'.repeat(64) }; },
  ]) {
    const invalid = structuredClone(proof); mutate(invalid);
    const denied = uiPhase5CParentReader('.', invalid);
    expect(denied.violations.length).toBeGreaterThan(0); expect(denied.addedPaths).toEqual([]); expect(denied.afterHashes).toEqual({});
  }
}, 90000);

function withPhase5CFixture(run: (root: string, paths: string[]) => void) {
  const workspace = resolve('.'), root = mkdtempSync(join(workspace, '.codex-phase5c-probe-'));
  try {
    const git = execFileSync('git', ['rev-parse', '--path-format=absolute', '--git-common-dir'], { encoding: 'utf8' }).trim();
    execFileSync('git', ['init', '--quiet', root]); mkdirSync(join(root, '.git/objects/info'), { recursive: true });
    writeFileSync(join(root, '.git/objects/info/alternates'), join(git, 'objects') + '\n');
    const paths: string[] = phase5CFixtureFiles(workspace);
    for (const p of paths) { mkdirSync(dirname(join(root, p)), { recursive: true }); copyFileSync(join(workspace, p), join(root, p)); }
    expect(uiPhase5CParentReader(root).violations).toEqual([]);
    run(root, paths);
  } finally {
    if ((!root.startsWith(workspace + '/') && !root.startsWith(workspace + '\\')) || !root.includes('.codex-phase5c-probe-')) throw Error('Unsafe fixture cleanup');
    rmSync(root, { recursive: true, force: true });
  }
}

it('keeps live freshness for review source and shell bytes with parent fail-closed checked once', () => withPhase5CFixture((root) => {
  let parentChecked = false;
  for (const p of ['src/features/production/ProductionReviewInbox.tsx', 'src/app/ShellHost.tsx']) {
    const path = join(root, p), bytes = readFileSync(path);
    writeFileSync(path, bytes.toString() + '\n');
    const denied = uiPhase5CParentReader(root);
    expect(denied.violations.length, p).toBeGreaterThan(0); expect(denied.addedPaths).toEqual([]);
    if (!parentChecked) { expect(uiPhase5BParentReader(root).violations.length, p).toBeGreaterThan(0); parentChecked = true; }
    writeFileSync(path, bytes); expect(uiPhase5CParentReader(root).violations, p).toEqual([]);
  }
}), 90000);

it('keeps live freshness for frozen proofs, Runtime raw bytes and manifest deletion', () => withPhase5CFixture((root, paths) => {
  const runtime = paths.find(p => p.startsWith('src-tauri/runtime_packages/') && p.endsWith('.json'))!;
  let parentChecked = false;
  for (const p of ['docs/architecture/ui-phase5b-mixed-video-successor.json', 'docs/architecture/ui-phase5a-storyboard-successor.json', 'docs/architecture/ui-phase5-ci-repair-successor.json', PHASE5C_MANIFEST, runtime]) {
    const path = join(root, p), bytes = readFileSync(path);
    writeFileSync(path, bytes.toString() + '\n');
    const denied = uiPhase5CParentReader(root);
    expect(denied.violations.length, p).toBeGreaterThan(0); expect(denied.addedPaths).toEqual([]);
    if (!parentChecked) { expect(uiPhase5BParentReader(root).violations.length, p).toBeGreaterThan(0); parentChecked = true; }
    writeFileSync(path, bytes); expect(uiPhase5CParentReader(root).violations, p).toEqual([]);
  }
  const path = join(root, PHASE5C_MANIFEST), bytes = readFileSync(path);
  rmSync(path); expect(uiPhase5CParentReader(root).violations.length).toBeGreaterThan(0);
  writeFileSync(path, bytes); expect(uiPhase5CParentReader(root).violations).toEqual([]);
}), 90000);

it('keeps live freshness for illegal additions across owned inventories', () => withPhase5CFixture((root) => {
  for (const [dir, name] of [['src', 'IllegalOwner.js'], ['.github', 'illegal.yml'], ['docs', 'IllegalOwner.js'], ['src-tauri/examples', 'IllegalOwner.rs']]) {
    const extra = join(root, dir, name); writeFileSync(extra, 'unreviewed bytes');
    expect(uiPhase5CParentReader(root).violations).toContain(`ui-phase5c-review-inventory:${dir}`);
    rmSync(extra); expect(uiPhase5CParentReader(root).violations).toEqual([]);
  }
}), 90000);
