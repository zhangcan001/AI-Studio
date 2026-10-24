// @vitest-environment node
import { expect, it } from 'vitest';
import { readFileSync, writeFileSync, copyFileSync, mkdirSync, mkdtempSync, rmSync } from 'node:fs';
import { join, dirname, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { execFileSync } from 'node:child_process';
import { frontendCiFourShardsParentReader, FRONTEND_CI_FOUR_SHARDS_BASE, FRONTEND_CI_FOUR_SHARDS_PROOF } from '../frontend-ci-four-shards-successor-guard.mjs';
import { compositionSceneAnchorParentReader as parentReader, COMPOSITION_SCENE_ANCHOR_PROOF as PARENT_PROOF } from '../composition-scene-anchor-successor-guard.mjs';
import { uiPhase7BR1MetadataParentReader } from '../ui-phase7b-r1-metadata-successor-guard.mjs';

const ci = '.github/workflows/ci.yml';

it('runs every non-Boundary frontend test across four shards while Boundary probes keep three shards', () => {
  for (const path of [ci, '.github/workflows/frontend-only-ci.yml']) {
    const workflow = readFileSync(path, 'utf8');
    const frontend = workflow.split('  frontend_tests:\n')[1].split('  frontend_boundary_tests:\n')[0];
    expect(frontend).toContain('name: Frontend tests (${{ matrix.shard }}/4)');
    expect(frontend).toContain('matrix:\n        shard: [1, 2, 3, 4]');
    expect(frontend).toContain('pnpm test --maxWorkers=1 --shard=${{ matrix.shard }}/4');
    expect(frontend).toContain('fail-fast: false');
    expect(frontend).toContain('timeout-minutes: 35');
    expect(frontend).not.toContain('/3');
    const boundary = workflow.split('  frontend_boundary_tests:\n')[1];
    expect(boundary).toContain('shard: [1, 2, 3]');
    expect(boundary).toContain('pnpm test:boundary --shard=${{ matrix.shard }}/3');
    expect(workflow).toContain('pnpm test:architecture');
    expect(workflow).toContain('pnpm exec tsc --noEmit');
    expect(workflow).toContain('pnpm exec vite build');
    expect(workflow).not.toContain('--passWithNoTests');
  }
});

it('preserves the frozen composition-scene-anchor checkpoint through parent projection', () => {
  const checked = frontendCiFourShardsParentReader('.');
  expect(checked.violations).toEqual([]);
  expect(checked.read(ci)).toBe(execFileSync('git', ['show', `${FRONTEND_CI_FOUR_SHARDS_BASE}:${ci}`], {encoding:'utf8'}).replaceAll('\r\n','\n'));
  expect(readFileSync(PARENT_PROOF)).toEqual(execFileSync('git', ['show', `${FRONTEND_CI_FOUR_SHARDS_BASE}:${PARENT_PROOF}`]));
  expect(parentReader('.').violations).toEqual([]);
}, 120000);

it('rejects forged proof identity/hash, workflow drift, migrations and undeclared additions', () => {
  const proof = JSON.parse(readFileSync(FRONTEND_CI_FOUR_SHARDS_PROOF, 'utf8'));
  for (const override of [
    {[FRONTEND_CI_FOUR_SHARDS_PROOF]: JSON.stringify({...proof, parentHead: 'forged'})},
    {[FRONTEND_CI_FOUR_SHARDS_PROOF]: JSON.stringify({...proof, contracts: {...proof.contracts, guardsRelaxed: true}})},
    {[FRONTEND_CI_FOUR_SHARDS_PROOF]: JSON.stringify({...proof, files: {...proof.files, [ci]: {...proof.files[ci], afterHash: '0'.repeat(64)}}})},
    {[ci]: readFileSync(ci, 'utf8') + '\n# drift'},
    {'src-tauri/src/application/shot_video_input_service.rs': 'changed compiler'},
    {'src/unapproved-default.ts': 'unapproved'},
    {'src-tauri/migrations/051_unapproved.sql': 'SELECT 1;'},
  ]) expect(frontendCiFourShardsParentReader('.', override).violations.length).toBeGreaterThan(0);
  expect(frontendCiFourShardsParentReader('.').violations).toEqual([]);
}, 120000);

it('invalidates warm-cache fixtures on live mutations, deletion and illegal files; restoration passes', () => {
  const root = mkdtempSync(join(tmpdir(), 'frontend-ci-four-shards-proof-'));
  try {
    const paths = execFileSync('git', ['ls-files','--cached','--others','--exclude-standard'], {encoding:'utf8'}).trim().split(/\r?\n/).filter(p => p === 'vite.config.ts' || /^(\.github\/workflows\/|src\/|src-tauri\/|scripts\/|docs\/architecture\/)/.test(p));
    for (const p of paths) { mkdirSync(dirname(join(root,p)), {recursive:true}); copyFileSync(p, join(root,p)); }
    execFileSync('git', ['init','-q',root]);
    const objects = resolve(execFileSync('git', ['rev-parse','--git-path','objects'], {encoding:'utf8'}).trim());
    writeFileSync(join(root,'.git/objects/info/alternates'), objects.replaceAll('\\','/') + '\n');
    const validate = () => uiPhase7BR1MetadataParentReader(root).violations;
    expect(validate()).toEqual([]);
    for (const p of [FRONTEND_CI_FOUR_SHARDS_PROOF, PARENT_PROOF, ci]) {
      const file = join(root,p), original = readFileSync(file);
      try {
        writeFileSync(file, Buffer.concat([original, Buffer.from('\nmutation')])); expect(validate().length).toBeGreaterThan(0);
        rmSync(file); expect(validate().length).toBeGreaterThan(0);
      } finally { writeFileSync(file, original); }
      expect(validate()).toEqual([]);
    }
    const illegal = join(root, 'src/unapproved-default.ts');
    try { writeFileSync(illegal, 'unapproved'); expect(validate().length).toBeGreaterThan(0); }
    finally { rmSync(illegal); }
    expect(validate()).toEqual([]);
  } finally { rmSync(root, {recursive:true, force:true}); }
}, 120000);

it('memoizes Git inventory only inside one validation session and still rejects mid-session drift', async () => {
  const { cachedBoundary, sessionGit } = await import('../boundary-validation-cache.mjs');
  const root = mkdtempSync(join(tmpdir(), 'frontend-ci-four-shards-session-git-'));
  try {
    execFileSync('git', ['init', '-q', root]);
    mkdirSync(join(root, 'src'));
    writeFileSync(join(root, 'src/a.ts'), 'one');
    const inventory = () => sessionGit(root, ['ls-files', '--cached', '--others', '--exclude-standard'], { encoding: 'utf8' });
    expect(inventory()).toBe('src/a.ts\n');
    writeFileSync(join(root, 'src/b.ts'), 'two');
    expect(inventory()).toBe('src/a.ts\nsrc/b.ts\n');
    let seen = [];
    const drift = cachedBoundary(root, 'session-git-drift', undefined, () => {
      seen.push(inventory());
      writeFileSync(join(root, 'src/c.ts'), 'three');
      seen.push(inventory());
      return { violations: [], read: () => '' };
    });
    expect(seen[0]).toBe(seen[1]);
    expect(drift.violations).toEqual(['boundary-live-drift-during-validation']);
    seen = [];
    const fresh = cachedBoundary(root, 'session-git-fresh', undefined, () => { seen.push(inventory()); return { violations: [], read: () => '' }; });
    expect(fresh.violations).toEqual([]);
    expect(seen[0]).toBe('src/a.ts\nsrc/b.ts\nsrc/c.ts\n');
  } finally { rmSync(root, { recursive: true, force: true }); }
});
