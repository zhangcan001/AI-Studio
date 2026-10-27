// @vitest-environment node
import { expect, it } from 'vitest';
import { readFileSync, writeFileSync, mkdirSync, rmSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import { dirname } from 'node:path';
import { env } from 'node:process';
import { minimaxVideoPhase2ParentReader, MINIMAX_VIDEO_PHASE2_PARENT, MINIMAX_VIDEO_PHASE2_MANIFEST } from '../../scripts/minimax-video-phase2-successor-guard.mjs';
import { readinessLifecycleParentReader, MINIMAX_VIDEO_PHASE1_MANIFEST } from '../../scripts/readiness-lifecycle-successor-guard.mjs';

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
}, 90000);

it('selects raw-byte baselines explicitly and never accepts the other representation by auto-detection', () => {
  // Live working-tree mutate — tip-inflated FixtureFiles blow the 90s Boundary budget.
  const previous=env.AI_STUDIO_PHASE2_RUNTIME_BYTE_PROFILE;
  try {
    env.AI_STUDIO_PHASE2_RUNTIME_BYTE_PROFILE='git';
    expect(minimaxVideoPhase2ParentReader('.').violations).toEqual([]);
    env.AI_STUDIO_PHASE2_RUNTIME_BYTE_PROFILE='checkout';
    expect(minimaxVideoPhase2ParentReader('.').violations).toContain('minimax-video-phase2-runtime-package-bytes');
    env.AI_STUDIO_PHASE2_RUNTIME_BYTE_PROFILE='git';
    expect(minimaxVideoPhase2ParentReader('.').violations).toEqual([]);
    env.AI_STUDIO_PHASE2_RUNTIME_BYTE_PROFILE='forged';
    expect(minimaxVideoPhase2ParentReader('.').violations).toContain('minimax-video-phase2-runtime-byte-baseline');
  } finally {
    if(previous===undefined)delete env.AI_STUDIO_PHASE2_RUNTIME_BYTE_PROFILE;else env.AI_STUDIO_PHASE2_RUNTIME_BYTE_PROFILE=previous;
  }
}, 90000);

it('rejects individual undeclared source, design and migration drift', () => {
  // Live working-tree mutate — tip-inflated FixtureFiles blow the 90s Boundary budget.
  for (const p of ['CONTEXT.md','src-tauri/src/application/generation_service.rs','src-tauri/src/domain/asset.rs',
    MINIMAX_VIDEO_PHASE1_MANIFEST,'src-tauri/migrations/042_project_workflow_binding_revision.sql','src-tauri/tauri.conf.json']) {
    const original=readFileSync(p,'utf8');
    try {
      writeFileSync(p,original+'\n# drift\n');
      const rejected=minimaxVideoPhase2ParentReader('.');
      expect(rejected.violations.length,p).toBeGreaterThan(0);
      expect(rejected.addedPaths,p).toEqual([]);
      expect(rejected.backendAggregateSha256,p).toBeUndefined();
    } finally {writeFileSync(p,original);}
    expect(minimaxVideoPhase2ParentReader('.').violations,p).toEqual([]);
  }
}, 90000);

it('rejects literal Runtime Package line endings and new source files', () => {
  const packagePath='src-tauri/runtime_packages/minimax_h3_fl2va_t2v_quality_2_2_0/recipe.yaml';
  const original=readFileSync(packagePath);
  try {
    const text=original.toString('utf8');
    writeFileSync(packagePath,text.includes('\r\n')?text.replaceAll('\r\n','\n'):text.replaceAll('\n','\r\n'));
    expect(minimaxVideoPhase2ParentReader('.').violations).toContain('minimax-video-phase2-runtime-package-bytes');
  } finally {writeFileSync(packagePath,original);}
  for (const relative of ['src-tauri/src/application/illegal_phase2.rs','src/illegalPhase2.ts']) {
    mkdirSync(dirname(relative),{recursive:true});
    try {
      writeFileSync(relative,'unauthorized');
      const rejected=minimaxVideoPhase2ParentReader('.');
      expect(rejected.violations.length,relative).toBeGreaterThan(0);expect(rejected.addedPaths,relative).toEqual([]);
    } finally {rmSync(relative,{force:true});}
  }
  expect(minimaxVideoPhase2ParentReader('.').violations).toEqual([]);
}, 90000);

it('rejects illegal scripts, migrations and Runtime Package additions', () => {
  for (const relative of ['scripts/illegal-phase2.mjs','src-tauri/migrations/044_illegal_phase2.sql','src-tauri/runtime_packages/illegal.bin']) {
    mkdirSync(dirname(relative),{recursive:true});
    try {
      writeFileSync(relative,'unauthorized');
      const rejected=minimaxVideoPhase2ParentReader('.');
      expect(rejected.violations.length,relative).toBeGreaterThan(0);expect(rejected.addedPaths,relative).toEqual([]);
    } finally {rmSync(relative,{force:true});}
  }
  expect(minimaxVideoPhase2ParentReader('.').violations).toEqual([]);
},90000);

it('rejects missing proof, repository source and domain evidence', () => {
  for (const p of [MINIMAX_VIDEO_PHASE2_MANIFEST,'src-tauri/src/application/shot_video_input_service.rs','src-tauri/src/domain/asset.rs']) {
    const original=readFileSync(p);
    try {
      rmSync(p);
      const rejected=minimaxVideoPhase2ParentReader('.');
      expect(rejected.violations.some((v:string)=>v.includes('missing-evidence')||v.endsWith('-files')),p).toBe(true);
      expect(rejected.addedPaths,p).toEqual([]);
      expect(rejected.backendAggregateSha256,p).toBeUndefined();
    } finally {writeFileSync(p,original);}
  }
  expect(minimaxVideoPhase2ParentReader('.').violations).toEqual([]);
}, 90000);
