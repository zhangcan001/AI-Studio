import {MINIMAX_VIDEO_PHASE1_FILES} from '../../scripts/readiness-lifecycle-successor-guard.mjs';
import {minimaxVideoPhase2FixtureFiles,} from '../../scripts/minimax-video-phase2-successor-guard.mjs';
// @vitest-environment node
import { expect, it } from 'vitest';
import { copyFileSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { publicationParentReader } from '../../scripts/2-1-publication-successor-guard.mjs';
import { rcRepairParentReader } from '../../scripts/2-1-rc-backup-asset-version-repair-guard.mjs';
import { closeoutParentReader } from '../../scripts/ai-studio-2-1-closeout-boundary-guard.mjs';
import { m1ParentReader } from '../../scripts/m1-readiness-boundary-guard.mjs';

const manifest = 'docs/architecture/2-1-publication-successor.json';
const docs = ['README.md', 'docs/AI_STUDIO_2_1_CLOSEOUT.md', 'docs/RELEASE_NOTES_v2.1.0-personal.md'];
const integration = 'scripts/2-1-rc-backup-asset-version-repair-guard.mjs';
it('validates live publication and preserves the exact pre-publication historical chain', () => {
  const publication = publicationParentReader('.');
  expect(publication.violations).toEqual([]);
  expect(readFileSync(docs[0], 'utf8')).toContain('**released**');
  expect(publication.read(docs[0])).toContain('NOT YET PUBLISHED');
  expect(readFileSync(docs[2], 'utf8')).toContain('GitHub Release: [published]');
  expect(publication.read(docs[2])).toContain('This document does not announce a Git tag or GitHub Release.');
  expect(readFileSync(docs[1], 'utf8')).toContain('## Final Publication');
  for (const path of docs) {
    const historical = execFileSync('git', ['show', `5af3f20273e722466c82b91ede4970cd83e0bcb8:${path}`], {encoding:'utf8'});
    expect(publication.read(path)).toBe(historical.replaceAll('\r\n', '\n'));
  }
  expect(rcRepairParentReader('.').violations).toEqual([]);
  expect(closeoutParentReader('.').violations).toEqual([]);
  expect(m1ParentReader('.').violations).toEqual([]);
}, 90000);

it('rejects altered provenance, reviewed hashes and invariants without exposing historical projection', () => {
  const proof = JSON.parse(readFileSync(manifest, 'utf8'));
  const mutations = [
    (p: typeof proof) => { p.parentHead = '0'.repeat(40); },
    (p: typeof proof) => { p.releaseTagHead = '0'.repeat(40); },
    (p: typeof proof) => { p.installerSha256 = '0'.repeat(64); },
    (p: typeof proof) => { p.releaseEvidence.downloadedEqualsAccepted = false; },
    ...docs.map(path => (p: typeof proof) => { p.paths[path].afterHash = '0'.repeat(64); }),
    (p: typeof proof) => { p.paths[integration].afterHash = '0'.repeat(64); },
    (p: typeof proof) => { p.backend.untouchedAggregateHash = '0'.repeat(64); },
    (p: typeof proof) => { p.frontend.afterFiles++; },
    ...Object.keys(proof.invariants).map(flag => (p: typeof proof) => { p.invariants[flag] = true; }),
  ];
  for (const mutate of mutations) {
    const changed = structuredClone(proof); mutate(changed);
    const rejected = publicationParentReader('.', changed);
    expect(rejected.violations.length).toBeGreaterThan(0);
    expect(rejected.addedPaths).toEqual([]);
    expect(rejected.afterHashes).toEqual({});
    expect(rejected.read(docs[0])).toBe(readFileSync(docs[0], 'utf8').replaceAll('\r\n', '\n'));
  }
}, 90000);

async function withPublicationFixture(run: (root: string, tagRef: string) => void) {
  const root = mkdtempSync(join(tmpdir(), 'ai-studio-publication-'));
  try {
    const gitDir = execFileSync('git', ['rev-parse','--absolute-git-dir'], {encoding:'utf8'}).trim();
    execFileSync('git', ['init','--quiet',root]);
    mkdirSync(join(root, '.git/objects/info'), {recursive:true});
    writeFileSync(join(root, '.git/objects/info/alternates'), join(gitDir,'objects') + '\n');
    const tagRef = join(root, '.git/refs/tags/v2.1.0-personal');
    mkdirSync(dirname(tagRef), {recursive:true});
    writeFileSync(tagRef, '5af3f20273e722466c82b91ede4970cd83e0bcb8\n');
    const tracked = execFileSync('git', ['ls-files'], {encoding:'utf8'}).trim().split(/\r?\n/);
    const closeout = JSON.parse(readFileSync('docs/architecture/h3-release-closeout-phase1.json','utf8'));
    const paths = [...Object.keys(closeout.paths), 'docs/architecture/h3-release-closeout-phase1.json', ...tracked.filter((p: string) =>
      /^(src|src-tauri\/(src|tests)|scripts)\/.*\.(rs|tsx?|mjs|css)$/.test(p) ||
      /^src-tauri\/runtime_packages\/.*\.(yaml|json)$/.test(p) || /^src-tauri\/migrations\/.*\.sql$/.test(p) || /^docs\/architecture\/.*\.json$/.test(p) ||
      docs.includes(p) || ['package.json','pnpm-lock.yaml','src-tauri/Cargo.toml','src-tauri/Cargo.lock',
        'src-tauri/tauri.conf.json','src-tauri/build.rs','.github/workflows/ci.yml'].includes(p)),
      manifest, 'scripts/2-1-publication-successor-guard.mjs', 'src/app/AIStudio21PublicationBoundary.test.ts'];
    const queue = JSON.parse(readFileSync('docs/architecture/queue-lifecycle-repair.json','utf8'));
    paths.push(...Object.keys(queue.paths),'docs/architecture/queue-lifecycle-repair.json');
    const readiness = JSON.parse(readFileSync('docs/architecture/readiness-lifecycle-fix.json','utf8'));
    paths.push(...Object.keys(readiness.paths),'docs/architecture/readiness-lifecycle-fix.json','docs/architecture/readiness-post-run-fix.json');
    for (const p of new Set([...paths,...MINIMAX_VIDEO_PHASE1_FILES,...minimaxVideoPhase2FixtureFiles('.')])) {
      mkdirSync(dirname(join(root,p)), {recursive:true}); copyFileSync(p,join(root,p));
    }
    expect(publicationParentReader(root).violations).toEqual([]);
    run(root,tagRef);
  } finally { rmSync(root, {recursive:true, force:true}); await new Promise<void>(resolve=>setTimeout(resolve,0)); }
}
it('rejects stale publication docs and tampered integration guard', () => withPublicationFixture(root => {
  const doc = join(root, docs[0]), original = readFileSync(doc,'utf8');
  writeFileSync(doc,original.replace('**released**','NOT YET PUBLISHED'));
  const rejected = publicationParentReader(root);
  expect(rejected.violations.length).toBeGreaterThan(0);
  expect(rejected.addedPaths).toEqual([]);
  expect(rejected.read(docs[0])).toContain('NOT YET PUBLISHED');
  writeFileSync(doc,original);
  const guard = join(root,integration), before = readFileSync(guard,'utf8');
  writeFileSync(guard,before+'\n// owned invalid integration drift\n');
  expect(publicationParentReader(root).violations).toContain(`minimax-video-phase2-path:${integration}`);
  writeFileSync(guard,before);
  expect(publicationParentReader(root).violations).toEqual([]);
}), 90000);
it('rejects modified publication config and backend runtime bytes', () => withPublicationFixture(root => {
  const config=join(root,'src-tauri/tauri.conf.json'), before=readFileSync(config,'utf8');
  writeFileSync(config,before+' ');
  expect(publicationParentReader(root).violations).toContain('minimax-video-phase2-frozen:src-tauri/tauri.conf.json');
  writeFileSync(config,before);
  const runtime=join(root,'src-tauri/src/domain/asset.rs'), original=readFileSync(runtime,'utf8');
  writeFileSync(runtime,original+'\n// owned invalid runtime drift\n');
  expect(publicationParentReader(root).violations).toContain('minimax-video-phase2-backend-untouched-bytes');
  writeFileSync(runtime,original);
  expect(publicationParentReader(root).violations).toEqual([]);
}), 360000);
it('rejects wrong and missing immutable publication tag without hidden success', () => withPublicationFixture((root,tagRef) => {
  writeFileSync(tagRef,'130cbbfaf67e1627fca7e447de322cf0f5f5ae85\n');
  const wrong=publicationParentReader(root);
  expect(wrong.violations).toContain('publication-tag-target');
  expect(wrong.addedPaths).toEqual([]);
  rmSync(tagRef);
  const missing=publicationParentReader(root);
  expect(missing.violations).toContain('publication-missing-git-or-proof-evidence');
  expect(missing.addedPaths).toEqual([]);
}), 90000);
