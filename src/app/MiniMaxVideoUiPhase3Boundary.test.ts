// @vitest-environment node
import { expect, it } from 'vitest';
// @ts-expect-error Node-only fixture.
import { readFileSync, writeFileSync, mkdirSync, mkdtempSync, copyFileSync, rmSync } from 'node:fs';
// @ts-expect-error Node-only fixture.
import { join, dirname, resolve } from 'node:path';
// @ts-expect-error Node-only boundary.
import { uiPhase3ParentReader, uiPhase3FixtureFiles, UI_PHASE3_PARENT, UI_PHASE3_MANIFEST } from '../../scripts/minimax-video-v2-ui-phase3-successor-guard.mjs';
// @ts-expect-error Node-only historical boundary.
import { minimaxVideoPhase3ParentReader } from '../../scripts/minimax-video-phase3-successor-guard.mjs';
it('validates live UI bytes before projecting all three historical immutable proofs', () => {
  const accepted=uiPhase3ParentReader('.');
  expect(accepted.violations).toEqual([]);
  expect(minimaxVideoPhase3ParentReader('.').violations).toEqual([]);
  const proof=JSON.parse(readFileSync(UI_PHASE3_MANIFEST,'utf8'));
  expect(proof.parentHead).toBe(UI_PHASE3_PARENT);
  expect(proof.invariants.uiV2Changed).toBe(true);
  expect(proof.invariants.schemaChanged).toBe(false);
  expect(proof.database).toEqual({migrationBefore:43,migrationAfter:43,newTables:0,backupBefore:21,backupAfter:21});
  const old=accepted.read('src/app/v3/AppShellV3.tsx');
  expect(old).toContain('stage: "image"');
  expect(readFileSync('src/app/v3/AppShellV3.tsx','utf8')).toContain('stage: "video"');
  const forged=structuredClone(proof); forged.invariants.uiV2Changed=false;
  const rejected=uiPhase3ParentReader('.',forged);
  expect(rejected.violations.length).toBeGreaterThan(0);
  expect(rejected.addedPaths).toEqual([]);
},30000);
it.each(['declared-bytes','undeclared-bytes','undeclared-style','illegal-addition','historical-proof','package-bytes'] as const)('rejects %s without authorizing a historical projection', kind => {
  const root=resolve('.'), fixture=mkdtempSync(join(root,'.codex-ui-phase3-probe-'));
  try {
    for(const p of new Set<string>(uiPhase3FixtureFiles(root))) {mkdirSync(dirname(join(fixture,p)),{recursive:true}); copyFileSync(join(root,p),join(fixture,p));}
    const p=kind==='declared-bytes'?'src/app/v3/AppShellV3.tsx'
      :kind==='undeclared-bytes'?'src/app/ShellHost.tsx'
      :kind==='undeclared-style'?'src/styles/appFoundation.css'
      :kind==='illegal-addition'?'src/app/v3/IllegalOwner.ts'
      :kind==='historical-proof'?'docs/architecture/minimax-video-phase3.json'
      :uiPhase3FixtureFiles(root).find((p:string)=>p.startsWith('src-tauri/runtime_packages/')&&p.endsWith('.json'))!;
    // Appending a literal newline also probes same-length-independent byte identity.
    if(kind==='illegal-addition') writeFileSync(join(fixture,p),'export const secondOwner = true;');
    else writeFileSync(join(fixture,p),readFileSync(join(fixture,p),'utf8')+'\n');
    // Historical proof normalization preserves CRLF only, not this extra newline.
    expect(uiPhase3ParentReader(fixture).violations.length).toBeGreaterThan(0);
    expect(minimaxVideoPhase3ParentReader(fixture).violations.length).toBeGreaterThan(0);
    expect(uiPhase3ParentReader(fixture).addedPaths).toEqual([]);
  } finally {
    if(!fixture.startsWith(root + '/') && !fixture.startsWith(root + '\\')) throw Error('owned fixture outside workspace');
    if(!fixture.includes('.codex-ui-phase3-probe-')) throw Error('unexpected fixture');
    rmSync(fixture,{recursive:true,force:true});
  }
},30000);
