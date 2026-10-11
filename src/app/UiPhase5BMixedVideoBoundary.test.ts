// @vitest-environment node
import { expect, it } from 'vitest';
// @ts-expect-error Node-only owned proof fixture.
import { readFileSync, writeFileSync, mkdirSync, mkdtempSync, copyFileSync, rmSync } from 'node:fs';
// @ts-expect-error Node-only fixture bounds.
import { join, dirname, resolve } from 'node:path';
// @ts-expect-error Node-only immutable object store.
import { execFileSync } from 'node:child_process';
// @ts-expect-error Node-only successor.
import { uiPhase5BParentReader, PHASE5B_MANIFEST, PHASE5B_PARENT, PHASE5B_EXISTING, phase5BParentFacts, phase5BFixtureFiles } from '../../scripts/ui-phase5b-mixed-video-successor-guard.mjs';
// @ts-expect-error Node-only frozen predecessor.
import { uiPhase5AParentReader } from '../../scripts/ui-phase5a-storyboard-successor-guard.mjs';

it('validates Phase5B live changes before projecting the exact frozen CI checkpoint and Phase5A history', () => {
  const reader = uiPhase5BParentReader('.'); expect(reader.violations).toEqual([]);
  const proof = JSON.parse(readFileSync(PHASE5B_MANIFEST, 'utf8'));
  expect(proof.parentHead).toBe(PHASE5B_PARENT);
  expect(proof.planningParentHead).toBe('0f288edf207873bcdbada0a67bc5dbdf4a24b2ce');
  expect(proof.invariants.hierarchyScopeRevalidated).toBe(true);
  expect(proof.invariants.phase5BStarted).toBe(true);
  for (const p of PHASE5B_EXISTING) expect(reader.read(p)).toBe(phase5BParentFacts('.').read(p));
  expect(uiPhase5AParentReader('.').violations).toEqual([]);
});

it('rejects forged parent, scope, blob verification and freshness declarations without projection', () => {
  const proof = JSON.parse(readFileSync(PHASE5B_MANIFEST, 'utf8'));
  for (const mutate of [
    (p: any) => { p.parentHead = '0'.repeat(40); },
    (p: any) => { p.paths['src-tauri/src/application/production_preparation_service.rs'].afterHash = '0'.repeat(64); },
    (p: any) => { p.paths['src-tauri/src/application/production_preparation_service.rs'].beforeHash = '0'.repeat(64); },
    (p: any) => { p.invariants.hierarchyScopeRevalidated = false; },
    (p: any) => { p.invariants.explicitQueueStart = false; },
    (p: any) => { p.paths['src/services/tauriClient.ts'] = { beforeHash: null, afterHash: '0'.repeat(64) }; },
  ]) {
    const invalid = structuredClone(proof); mutate(invalid);
    const denied = uiPhase5BParentReader('.', invalid);
    expect(denied.violations.length).toBeGreaterThan(0); expect(denied.addedPaths).toEqual([]); expect(denied.afterHashes).toEqual({});
  }
});

it('keeps live freshness for source, frozen proof, Runtime raw bytes, deletion and illegal addition; restoration passes', () => {
  const workspace = resolve('.'), root = mkdtempSync(join(workspace, '.codex-phase5b-probe-'));
  try {
    const git = execFileSync('git', ['rev-parse', '--path-format=absolute', '--git-common-dir'], { encoding: 'utf8' }).trim();
    execFileSync('git', ['init', '--quiet', root]); mkdirSync(join(root, '.git/objects/info'), { recursive: true });
    writeFileSync(join(root, '.git/objects/info/alternates'), join(git, 'objects') + '\n');
    const paths: string[] = phase5BFixtureFiles(workspace);
    for (const p of paths) { mkdirSync(dirname(join(root, p)), { recursive: true }); copyFileSync(join(workspace, p), join(root, p)); }
    expect(uiPhase5BParentReader(root).violations).toEqual([]);
    const runtime = paths.find(p => p.startsWith('src-tauri/runtime_packages/') && p.endsWith('.json'))!;
    for (const p of ['src-tauri/src/application/production_preparation_service.rs', 'src/app/ShellHost.tsx',
      'docs/architecture/ui-phase5a-storyboard-successor.json', 'docs/architecture/ui-phase5-ci-repair-successor.json', PHASE5B_MANIFEST, runtime]) {
      const path = join(root, p), bytes = readFileSync(path);
      writeFileSync(path, bytes.toString() + '\n');
      const denied = uiPhase5BParentReader(root);
      expect(denied.violations.length, p).toBeGreaterThan(0); expect(denied.addedPaths).toEqual([]);
      expect(uiPhase5AParentReader(root).violations.length, p).toBeGreaterThan(0);
      writeFileSync(path, bytes); expect(uiPhase5BParentReader(root).violations, p).toEqual([]);
    }
    const path = join(root, PHASE5B_MANIFEST), bytes = readFileSync(path);
    rmSync(path); expect(uiPhase5BParentReader(root).violations.length).toBeGreaterThan(0);
    writeFileSync(path, bytes); expect(uiPhase5BParentReader(root).violations).toEqual([]);
    for (const [dir, name] of [['src', 'IllegalOwner.js'], ['.github', 'illegal.yml'], ['docs', 'IllegalOwner.js'], ['src-tauri/examples', 'IllegalOwner.rs']]) {
      const extra = join(root, dir, name); writeFileSync(extra, 'unreviewed bytes');
      expect(uiPhase5BParentReader(root).violations).toContain(`ui-phase5b-mixed-video-inventory:${dir}`);
      rmSync(extra); expect(uiPhase5BParentReader(root).violations).toEqual([]);
    }
  } finally {
    if ((!root.startsWith(workspace + '/') && !root.startsWith(workspace + '\\')) || !root.includes('.codex-phase5b-probe-')) throw Error('Unsafe fixture cleanup');
    rmSync(root, { recursive: true, force: true });
  }
}, 30000);
