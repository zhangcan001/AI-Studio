// @vitest-environment node
import { expect, it } from 'vitest';
// @ts-expect-error Node-only frozen-successor proof.
import { readFileSync, writeFileSync, copyFileSync, mkdirSync, mkdtempSync, rmSync } from 'node:fs';
// @ts-expect-error Node-only test fixture.
import { execFileSync } from 'node:child_process';
// @ts-expect-error Node-only paths.
import { dirname, join, resolve } from 'node:path';
// @ts-expect-error Node-only tempdir.
import { tmpdir } from 'node:os';
// @ts-expect-error Node-only successor.
import { uiPhase5DParentReader, phase5DParentFacts, phase5DFixtureFiles, PHASE5D_MANIFEST, PHASE5D_PARENT, PHASE5D_EXISTING } from '../../scripts/ui-phase5d-ci-timeout-successor-guard.mjs';
// @ts-expect-error Node-only historic successor.
import { uiPhase5CParentReader } from '../../scripts/ui-phase5c-review-successor-guard.mjs';

it('accepts only a scoped current-byte CI optimization before replaying the frozen Phase5C and historical source', () => {
  const accepted = uiPhase5DParentReader('.');
  expect(accepted.violations).toEqual([]);
  const proof = JSON.parse(readFileSync(PHASE5D_MANIFEST, 'utf8'));
  expect(proof.parentHead).toBe(PHASE5D_PARENT);
  expect(proof.phaseCommitsBeforeCheckpoint).toEqual([]);
  expect(proof.invariants.strictThirtySecondTestsPreserved).toBe(true);
  expect(proof.invariants.eachNegativeProbePreserved).toBe(true);
  for (const p of PHASE5D_EXISTING) expect(accepted.read(p)).toBe(phase5DParentFacts('.').read(p));
  expect(uiPhase5CParentReader('.').violations).toEqual([]);
});

it('refuses metadata forgeries without projecting any historical authority', () => {
  const proof = JSON.parse(readFileSync(PHASE5D_MANIFEST, 'utf8'));
  for (const mutate of [
    (p: any) => { p.parentHead = '0'.repeat(40); },
    (p: any) => { p.paths['src/app/H3ReleaseCloseoutPhase1Boundary.test.ts'].afterHash = '0'.repeat(64); },
    (p: any) => { p.paths['scripts/ui-phase5c-review-successor-guard.mjs'].beforeHash = '0'.repeat(64); },
    (p: any) => { p.invariants.strictThirtySecondTestsPreserved = false; },
    (p: any) => { p.invariants.eachNegativeProbePreserved = false; },
    (p: any) => { p.paths['src/services/tauriClient.ts'] = {beforeHash:null,afterHash:'0'.repeat(64)}; },
  ]) {
    const invalid = structuredClone(proof); mutate(invalid);
    const denied = uiPhase5DParentReader('.', invalid);
    expect(denied.violations.length).toBeGreaterThan(0);
    expect(denied.addedPaths).toEqual([]);
    expect(denied.afterHashes).toEqual({});
  }
});

it('detects live code drift and raw Runtime Package byte drift in an owned Git-alternate fixture, then restores acceptance', () => {
  const workspace = resolve('.'), root = mkdtempSync(join(tmpdir(),'ai-studio-phase5d-ci-'));
  try {
    const git = execFileSync('git',['rev-parse','--path-format=absolute','--git-common-dir'],{encoding:'utf8'}).trim();
    execFileSync('git',['init','--quiet',root]);
    mkdirSync(join(root,'.git/objects/info'),{recursive:true});
    writeFileSync(join(root,'.git/objects/info/alternates'),join(git,'objects')+'\n');
    for (const p of phase5DFixtureFiles(workspace)) {
      mkdirSync(dirname(join(root,p)),{recursive:true});
      copyFileSync(join(workspace,p),join(root,p));
    }
    expect(uiPhase5DParentReader(root).violations).toEqual([]);
    const p = 'src/app/MiniMaxVideoPhase2Boundary.test.ts';
    const original = readFileSync(join(root,p));
    writeFileSync(join(root,p),original.toString('utf8')+'\n// drift\n');
    expect(uiPhase5DParentReader(root).violations.length).toBeGreaterThan(0);
    writeFileSync(join(root,p),original);
    const pkg = 'src-tauri/runtime_packages/minimax_h3_fl2va_t2v_quality_2_2_0/recipe.yaml';
    const originalPackage = readFileSync(join(root,pkg));
    writeFileSync(join(root,pkg),Uint8Array.from([...originalPackage,10]));
    expect(uiPhase5DParentReader(root).violations).toContain('ui-phase5d-ci-timeout-runtime-package-bytes');
    writeFileSync(join(root,pkg),originalPackage);
    expect(uiPhase5DParentReader(root).violations).toEqual([]);
  } finally {
    if (!root.startsWith(resolve(tmpdir()) + '\\') && !root.startsWith(resolve(tmpdir()) + '/')) throw Error('Unsafe fixture path');
    rmSync(root,{recursive:true,force:true,maxRetries:3,retryDelay:100});
  }
},30000);
