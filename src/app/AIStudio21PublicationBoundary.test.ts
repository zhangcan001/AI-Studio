// @ts-expect-error Node-only successor fixture.
import {MINIMAX_VIDEO_PHASE1_FILES} from '../../scripts/readiness-lifecycle-successor-guard.mjs';
// @vitest-environment node
import { expect, it } from 'vitest';
// @ts-expect-error Node-only acceptance helper.
import { copyFileSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
// @ts-expect-error Node-only acceptance helper.
import { execFileSync } from 'node:child_process';
// @ts-expect-error Node-only acceptance helper.
import { tmpdir } from 'node:os';
// @ts-expect-error Node-only acceptance helper.
import { dirname, join } from 'node:path';
// @ts-expect-error Build-time publication proof, not browser runtime.
import { publicationParentReader } from '../../scripts/2-1-publication-successor-guard.mjs';
// @ts-expect-error Existing build-time successor.
import { rcRepairParentReader } from '../../scripts/2-1-rc-backup-asset-version-repair-guard.mjs';
// @ts-expect-error Existing build-time successor.
import { closeoutParentReader } from '../../scripts/ai-studio-2-1-closeout-boundary-guard.mjs';
// @ts-expect-error Existing build-time successor.
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
}, 30000);

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
}, 30000);

it('fails closed on stale live docs and a missing tag in an owned fixture', () => {
  const root = mkdtempSync(join(tmpdir(), 'ai-studio-publication-'));
  try {
    // Own all refs; borrow immutable Git objects read-only, never user tags.
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
    const queue=JSON.parse(readFileSync('docs/architecture/queue-lifecycle-repair.json','utf8'));
    paths.push(...Object.keys(queue.paths),'docs/architecture/queue-lifecycle-repair.json');
  const readiness=JSON.parse(readFileSync('docs/architecture/readiness-lifecycle-fix.json','utf8'));
  paths.push(...Object.keys(readiness.paths),'docs/architecture/readiness-lifecycle-fix.json','docs/architecture/readiness-post-run-fix.json');
    for (const p of new Set([...paths,...MINIMAX_VIDEO_PHASE1_FILES])) { mkdirSync(dirname(join(root,p)), {recursive:true}); copyFileSync(p,join(root,p)); }
    expect(publicationParentReader(root).violations).toEqual([]);
    const original = readFileSync(join(root, docs[0]), 'utf8');
    writeFileSync(join(root, docs[0]), original.replace('**released**','NOT YET PUBLISHED'));
    const rejected = publicationParentReader(root);
    expect(rejected.violations.length).toBeGreaterThan(0);
    expect(rejected.addedPaths).toEqual([]);
    expect(rejected.read(docs[0])).toContain('NOT YET PUBLISHED');
    // Cached immutable facts must not hide changed live evidence on later calls.
    writeFileSync(join(root, docs[0]), original);
    expect(publicationParentReader(root).violations).toEqual([]);
    const guard = join(root, integration);
    const guardBytes = readFileSync(guard, 'utf8');
    writeFileSync(guard, guardBytes + '\n// owned invalid integration drift\n');
    expect(publicationParentReader(root).violations).toContain('minimax-video-phase1-scripts-untouched-bytes');
    writeFileSync(guard, guardBytes);
    expect(publicationParentReader(root).violations).toEqual([]);
    const config = join(root, 'src-tauri/tauri.conf.json');
    const configBytes = readFileSync(config, 'utf8');
    writeFileSync(config, configBytes + ' ');
    expect(publicationParentReader(root).violations).toContain('minimax-video-phase1-frozen:src-tauri/tauri.conf.json');
    writeFileSync(config, configBytes);
    const runtime = join(root, 'src-tauri/src/domain/asset.rs');
    const runtimeBytes = readFileSync(runtime, 'utf8');
    writeFileSync(runtime, runtimeBytes + '\n// owned invalid runtime drift\n');
    expect(publicationParentReader(root).violations).toContain('minimax-video-phase1-backend-untouched-bytes');
    writeFileSync(runtime, runtimeBytes);
    expect(publicationParentReader(root).violations).toEqual([]);
    writeFileSync(tagRef, '130cbbfaf67e1627fca7e447de322cf0f5f5ae85\n');
    const wrongTag = publicationParentReader(root);
    expect(wrongTag.violations).toContain('publication-tag-target');
    expect(wrongTag.addedPaths).toEqual([]);
    rmSync(tagRef);
    const missing = publicationParentReader(root);
    expect(missing.violations).toContain('publication-missing-git-or-proof-evidence');
    expect(missing.addedPaths).toEqual([]);
  } finally { rmSync(root, {recursive:true, force:true}); }
}, 30000);
