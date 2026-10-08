// @vitest-environment node
import { expect, it } from 'vitest';
// @ts-expect-error Node-only boundary fixture.
import { readFileSync, writeFileSync, copyFileSync, mkdirSync, mkdtempSync, rmSync } from 'node:fs';
// @ts-expect-error Node-only boundary fixture.
import { execFileSync } from 'node:child_process';
// @ts-expect-error Node-only boundary fixture.
import { tmpdir } from 'node:os';
// @ts-expect-error Node-only boundary fixture.
import { join, dirname } from 'node:path';
// @ts-expect-error Node-only byte-profile fixture.
import { env } from 'node:process';
// @ts-expect-error Node-only successor.
import { minimaxVideoPhase2ParentReader, MINIMAX_VIDEO_PHASE2_PARENT, MINIMAX_VIDEO_PHASE2_MANIFEST, MINIMAX_VIDEO_PHASE2_FILES } from '../../scripts/minimax-video-phase2-successor-guard.mjs';
// @ts-expect-error Node-only successor.
import { minimaxVideoPhase3FixtureFiles } from '../../scripts/minimax-video-phase3-successor-guard.mjs';
// @ts-expect-error Node-only successor.
import { readinessLifecycleParentFacts, readinessLifecycleParentReader, MINIMAX_VIDEO_PHASE1_MANIFEST } from '../../scripts/readiness-lifecycle-successor-guard.mjs';

it('validates schema/backup changes honestly and replays immutable Phase1 bytes through a legal successor', () => {
  const proof = JSON.parse(readFileSync(MINIMAX_VIDEO_PHASE2_MANIFEST,'utf8'));
  const phase1 = JSON.parse(readFileSync(MINIMAX_VIDEO_PHASE1_MANIFEST,'utf8'));
  expect(proof.parentHead).toBe(MINIMAX_VIDEO_PHASE2_PARENT); expect(proof.planningParentHead).toBe(MINIMAX_VIDEO_PHASE2_PARENT);
  expect(proof.invariants.schemaChanged).toBe(true); expect(proof.invariants.backupFormatChanged).toBe(true);
  expect(phase1.invariants.schemaChanged).toBe(false); expect(phase1.invariants.backupFormatChanged).toBe(false);
  const accepted = minimaxVideoPhase2ParentReader('.'); expect(accepted.violations).toEqual([]);
  expect(readinessLifecycleParentReader('.').violations).toEqual([]);
  const path = 'src-tauri/src/application/project_backup_service.rs';
  expect(accepted.read(path)).toBe(execFileSync('git',['show',`${MINIMAX_VIDEO_PHASE2_PARENT}:${path}`],{encoding:'utf8'}).replaceAll('\r\n','\n'));
  for (const mutate of [
    (p:typeof proof) => { p.parentHead='0'.repeat(40); },
    (p:typeof proof) => { p.phaseCommitsBeforeCheckpoint=['forged']; },
    (p:typeof proof) => { p.phase1ProofHash='0'.repeat(64); },
    (p:typeof proof) => { p.runtimeByteBaselines.checkout='0'.repeat(64); },
    (p:typeof proof) => { p.paths[path].beforeHash='0'.repeat(64); },
    (p:typeof proof) => { p.paths[path].afterHash='0'.repeat(64); },
    (p:typeof proof) => { p.paths['src/services/ipc.ts']={beforeHash:null,afterHash:'0'.repeat(64)}; },
    ...Object.keys(proof.invariants).map(k => (p:typeof proof) => { p.invariants[k]=!p.invariants[k]; }),
    ...Object.keys(proof.database).map(k => (p:typeof proof) => { p.database[k]++; }),
    ...['backend','tests','frontend','styles','scripts','migrations','architecture','packages','domainDocs'].flatMap(g=>[
      (p:typeof proof) => { p[g].afterFiles++; }, (p:typeof proof) => { p[g].untouchedAggregateHash='0'.repeat(64); },
      (p:typeof proof) => { p[g].afterAggregateHash='0'.repeat(64); }, (p:typeof proof) => { p[g].beforeAggregateHash='0'.repeat(64); },
    ]),
  ]) {
    const forged=structuredClone(proof);mutate(forged);const rejected=minimaxVideoPhase2ParentReader('.',forged);
    expect(rejected.violations.length).toBeGreaterThan(0); expect(rejected.addedPaths).toEqual([]);
    expect(rejected.backendAggregateSha256).toBeUndefined(); expect(rejected.read(path)).toBe(readFileSync(path,'utf8').replaceAll('\r\n','\n'));
  }
},30000);

it('selects raw-byte baselines explicitly and never accepts the other representation by auto-detection', () => {
  const root=mkdtempSync(join(tmpdir(),'ai-studio-phase2-git-bytes-'));
  const previous=env.AI_STUDIO_PHASE2_RUNTIME_BYTE_PROFILE;
  try {
    const git=execFileSync('git',['rev-parse','--absolute-git-dir'],{encoding:'utf8'}).trim();
    execFileSync('git',['init','--quiet',root]);mkdirSync(join(root,'.git/objects/info'),{recursive:true});writeFileSync(join(root,'.git/objects/info/alternates'),join(git,'objects')+'\n');
    const facts=readinessLifecycleParentFacts('.',MINIMAX_VIDEO_PHASE2_PARENT);
    for (const p of new Set([...facts.paths,...MINIMAX_VIDEO_PHASE2_FILES,...minimaxVideoPhase3FixtureFiles('.')]) as Set<string>) {
      mkdirSync(dirname(join(root,p)),{recursive:true});copyFileSync(p,join(root,p));
      // Only an owned fixture is materialized from literal Git blobs. The real
      // workspace and immutable package files are never normalized or written.
      if(p.startsWith('src-tauri/runtime_packages/'))writeFileSync(join(root,p),facts.raw(p));
    }
    env.AI_STUDIO_PHASE2_RUNTIME_BYTE_PROFILE='git';
    expect(minimaxVideoPhase2ParentReader(root).violations).toEqual([]);
    env.AI_STUDIO_PHASE2_RUNTIME_BYTE_PROFILE='checkout';
    expect(minimaxVideoPhase2ParentReader(root).violations).toContain('minimax-video-phase2-runtime-package-bytes');
    env.AI_STUDIO_PHASE2_RUNTIME_BYTE_PROFILE='git';
    expect(minimaxVideoPhase2ParentReader(root).violations).toEqual([]);
    env.AI_STUDIO_PHASE2_RUNTIME_BYTE_PROFILE='forged';
    expect(minimaxVideoPhase2ParentReader(root).violations).toContain('minimax-video-phase2-runtime-byte-baseline');
  } finally {
    if(previous===undefined)delete env.AI_STUDIO_PHASE2_RUNTIME_BYTE_PROFILE;else env.AI_STUDIO_PHASE2_RUNTIME_BYTE_PROFILE=previous;
    rmSync(root,{recursive:true,force:true});
  }
},30000);

it('rejects undeclared changes, illegal additions, missing evidence and literal Runtime Package byte drift', () => {
  const root=mkdtempSync(join(tmpdir(),'ai-studio-phase2-successor-'));
  try {
    const git=execFileSync('git',['rev-parse','--absolute-git-dir'],{encoding:'utf8'}).trim();
    execFileSync('git',['init','--quiet',root]);mkdirSync(join(root,'.git/objects/info'),{recursive:true});writeFileSync(join(root,'.git/objects/info/alternates'),join(git,'objects')+'\n');
    for (const p of new Set([...readinessLifecycleParentFacts('.',MINIMAX_VIDEO_PHASE2_PARENT).paths,...MINIMAX_VIDEO_PHASE2_FILES,...minimaxVideoPhase3FixtureFiles('.')]) as Set<string>) {
      mkdirSync(dirname(join(root,p)),{recursive:true});copyFileSync(p,join(root,p));
    }
    expect(minimaxVideoPhase2ParentReader(root).violations).toEqual([]);
    for (const p of ['CONTEXT.md','src-tauri/src/application/generation_service.rs','src-tauri/src/domain/asset.rs',MINIMAX_VIDEO_PHASE1_MANIFEST,'src-tauri/migrations/042_project_workflow_binding_revision.sql','src-tauri/tauri.conf.json']) {
      const path=join(root,p),original=readFileSync(path,'utf8');writeFileSync(path,original+'\n# drift\n');
      const rejected=minimaxVideoPhase2ParentReader(root);expect(rejected.violations.length).toBeGreaterThan(0);expect(rejected.addedPaths).toEqual([]);expect(rejected.backendAggregateSha256).toBeUndefined();
      writeFileSync(path,original);expect(minimaxVideoPhase2ParentReader(root).violations).toEqual([]);
    }
    const packagePath=join(root,'src-tauri/runtime_packages/minimax_h3_fl2va_t2v_quality_2_2_0/recipe.yaml'), original=readFileSync(packagePath);
    const text=original.toString('utf8');
    writeFileSync(packagePath,text.includes('\r\n') ? text.replaceAll('\r\n','\n') : text.replaceAll('\n','\r\n'));
    expect(minimaxVideoPhase2ParentReader(root).violations).toContain('minimax-video-phase2-runtime-package-bytes');
    writeFileSync(packagePath,original);
    for (const relative of ['src-tauri/src/application/illegal_phase2.rs','src/illegalPhase2.ts','scripts/illegal-phase2.mjs','src-tauri/migrations/044_illegal_phase2.sql','src-tauri/runtime_packages/illegal.bin']) {
      const path=join(root,relative);writeFileSync(path,'unauthorized');const rejected=minimaxVideoPhase2ParentReader(root);
      expect(rejected.violations.length).toBeGreaterThan(0);expect(rejected.addedPaths).toEqual([]);rmSync(path);
    }
    for (const p of [MINIMAX_VIDEO_PHASE2_MANIFEST,'src-tauri/src/application/shot_video_input_service.rs','src-tauri/src/domain/asset.rs']) {
      const path=join(root,p),original=readFileSync(path);rmSync(path);const rejected=minimaxVideoPhase2ParentReader(root);
      expect(rejected.violations.some((v:string)=>v.includes('missing-evidence')||v.endsWith('-files'))).toBe(true);expect(rejected.addedPaths).toEqual([]);expect(rejected.backendAggregateSha256).toBeUndefined();writeFileSync(path,original);
    }
    expect(minimaxVideoPhase2ParentReader(root).violations).toEqual([]);
  } finally { rmSync(root,{recursive:true,force:true}); }
},30000);
