import { cachedBoundary } from './boundary-validation-cache.mjs';
import { uiPhase3CiRepairParentReader, CI_REPAIR_FILES } from './ui-phase3-ci-repair-successor-guard.mjs';
import { UI_PHASE4_FILES } from './minimax-video-v2-ui-phase4-successor-guard.mjs';
import { createHash } from 'node:crypto';
import { readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';
import { READINESS_LIFECYCLE_GROUPS, readinessLifecycleParentFacts, readinessLifecycleHash, readinessLifecycleAggregate } from './readiness-lifecycle-successor-guard.mjs';

export const UI_PHASE3_PARENT = 'de9ca8b9e97b22603aa9488b84de60a03c309b9f';
export const UI_PHASE3_MANIFEST = 'docs/architecture/minimax-video-v2-ui-phase3.json';
export const UI_PHASE3_EXISTING = [
  'scripts/minimax-video-phase3-successor-guard.mjs',
  'scripts/style-boundary-guard.mjs',
  'src/app/App.tsx', 'src/app/NormalProductPages.tsx',
  'src/app/v3/AppShellV3.tsx', 'src/app/v3/AppShellV3.css',
  'src/app/v3/ProjectOverviewPage.tsx', 'src/app/v3/ProjectOverviewPage.test.tsx',
  'src/app/v3/v3.test.tsx', 'src/app/routes/selectors.ts',
  'src/app/routes/legacyAdapter.ts', 'src/app/routes/resumeAdapter.ts',
  'src/app/routes/resume.test.ts',
  'src/app/FrontendConsolidation.test.tsx', 'src/app/Phase7Retirement.test.tsx',
  'src/app/StudioShell.test.tsx', 'src/app/StyleBoundary.test.ts',
  'src/features/library/LibraryPage.test.tsx', 'src/features/workflow-lab/WorkflowLabBoundary.test.tsx',
  'src/styles/studioTokens.css', 'src/features/create/CreatePage.tsx',
  'src/features/library/LibraryPage.css', 'src/features/runs/RunsPage.css',
  'src/features/create/CreatePage.css', 'src/features/create/CreatePage.test.tsx',
];
export const UI_PHASE3_ADDED = [
  'scripts/minimax-video-v2-ui-phase3-successor-guard.mjs',
  'src/app/v3/ShellChrome.tsx', 'src/app/v3/VideoShell.test.tsx',
  'src/app/MiniMaxVideoUiPhase3Boundary.test.ts',
  'docs/architecture/minimax-video-v2-ui-phase3-plan.md',
];
export const UI_PHASE3_FILES = [...UI_PHASE3_EXISTING, ...UI_PHASE3_ADDED, UI_PHASE3_MANIFEST];
export const uiPhase3FixtureFiles = root => [...readinessLifecycleParentFacts(root, UI_PHASE3_PARENT).paths, ...UI_PHASE3_FILES, ...CI_REPAIR_FILES, ...UI_PHASE4_FILES];
export const uiPhase3Groups = () => [...READINESS_LIFECYCLE_GROUPS, ['domainDocs', 'docs/architecture', /\.md$/]];
export const UI_PHASE3_INVARIANTS = {
  schemaChanged: false, backupFormatChanged: false, queueAuthorityChanged: false,
  storeAuthorityChanged: false, uiV2Changed: true, historicalDataRewritten: false,
  immutablePackagesPreserved: true, historicalProofsPreserved: true, singleRouteAuthority: true,
};
export const UI_PHASE3_DATABASE = { migrationBefore:43, migrationAfter:43, newTables:0, backupBefore:21, backupAfter:21 };
const normalize = s => s.replaceAll('\r\n', '\n');
const files = (root, dir, pattern) => readdirSync(join(root, dir), { withFileTypes:true }).flatMap(e => {
  const p = `${dir}/${e.name}`; return e.isDirectory() ? files(root, p, pattern) : pattern.test(p) ? [p] : [];
}).sort();
const frozen = ['CONTEXT.md', 'package.json', 'pnpm-lock.yaml', 'src-tauri/Cargo.toml', 'src-tauri/Cargo.lock',
  'src-tauri/tauri.conf.json', 'src-tauri/build.rs', '.github/workflows/ci.yml', 'README.md',
  'docs/AI_STUDIO_2_1_CLOSEOUT.md', 'docs/RELEASE_NOTES_v2.1.0-personal.md'];
const historical = [1,2,3].map(n => `docs/architecture/minimax-video-phase${n}.json`);
const sha = bytes => createHash('sha256').update(bytes).digest('hex');
function validate(root, override) {
  const successor = uiPhase3CiRepairParentReader(root);
  const violations = [...successor.violations], fail = name => violations.push(`minimax-video-ui-phase3-${name}`);
  const live = new Map(), disk = p => {
    if (!live.has(p)) live.set(p, normalize(successor.read(p)));
    return live.get(p);
  };
  const rejected = () => ({ violations, addedPaths:[], afterHashes:{}, read:disk });
  let proof, facts;
  try { proof = override ?? JSON.parse(disk(UI_PHASE3_MANIFEST)); facts = readinessLifecycleParentFacts(root,UI_PHASE3_PARENT); }
  catch { fail('missing-evidence'); return rejected(); }
  if (proof.schemaVersion !== 1 || proof.checkpoint !== 'MINIMAX_VIDEO_V2_UI_PHASE_3'
    || proof.parentHead !== UI_PHASE3_PARENT || proof.planningParentHead !== UI_PHASE3_PARENT
    || JSON.stringify(proof.phaseCommitsBeforeCheckpoint) !== '[]') fail('header');
  const scope = [...UI_PHASE3_EXISTING,...UI_PHASE3_ADDED].sort();
  if (JSON.stringify(Object.keys(proof.paths ?? {}).sort()) !== JSON.stringify(scope)) fail('scope');
  if (JSON.stringify(proof.invariants) !== JSON.stringify(UI_PHASE3_INVARIANTS)) fail('contract');
  if (JSON.stringify(proof.database) !== JSON.stringify(UI_PHASE3_DATABASE)) fail('database-contract');
  if (violations.length) return rejected();
  try {
    for (const p of historical) if (disk(p) !== facts.read(p)) fail('historical-proof');
    for (const p of UI_PHASE3_EXISTING)
      if (proof.paths[p]?.beforeHash !== readinessLifecycleHash(facts.read(p)) || proof.paths[p]?.afterHash !== readinessLifecycleHash(disk(p))) fail(`path:${p}`);
    for (const p of UI_PHASE3_ADDED)
      if (facts.paths.includes(p) || proof.paths[p]?.beforeHash !== null || proof.paths[p]?.afterHash !== readinessLifecycleHash(disk(p))) fail(`addition:${p}`);
    for (const [name,dir,pattern] of uiPhase3Groups()) {
      const base = facts.paths.filter(p => p.startsWith(dir + '/') && pattern.test(p));
      const untouched = base.filter(p => !UI_PHASE3_EXISTING.includes(p));
      const current = files(root,dir,pattern).filter(p => p !== UI_PHASE3_MANIFEST && !successor.addedPaths.includes(p));
      const expected = [...base,...UI_PHASE3_ADDED.filter(p => p.startsWith(dir + '/') && pattern.test(p))].sort();
      const record = proof[name];
      if (JSON.stringify(current) !== JSON.stringify(expected) || record?.beforeFiles !== base.length || record?.afterFiles !== current.length) fail(`${name}-files`);
      if (untouched.some(p => disk(p) !== facts.read(p))) fail(`${name}-untouched-bytes`);
      if (name === 'packages') {
        // Reuse the immutable Phase2 literal-byte contract, never auto-detect.
        const profiles = JSON.parse(facts.read('docs/architecture/minimax-video-phase2.json')).runtimeByteBaselines;
        const profile = process.env.AI_STUDIO_PHASE2_RUNTIME_BYTE_PROFILE ?? 'git';
        const rawHash = sha(JSON.stringify(base.map(p => [p,sha(readFileSync(join(root,p)))])));
        if (!Object.hasOwn(profiles,profile) || rawHash !== profiles[profile]
          || JSON.stringify(files(root,dir,/./)) !== JSON.stringify(base)) fail('runtime-package-bytes');
      }
      if (record?.beforeAggregateHash !== readinessLifecycleAggregate(base,facts.read)
        || record?.untouchedAggregateHash !== readinessLifecycleAggregate(untouched,facts.read)
        || record?.afterAggregateHash !== readinessLifecycleAggregate(current,disk)) fail(`${name}-aggregate`);
    }
    for (const p of frozen) if (disk(p) !== facts.read(p)) fail(`frozen:${p}`);
  } catch { fail('missing-evidence'); return rejected(); }
  if (violations.length) return rejected();
  return {
    violations, addedPaths:[...UI_PHASE3_ADDED,UI_PHASE3_MANIFEST,...successor.addedPaths],
    afterHashes:{...Object.fromEntries(scope.map(p => [p,proof.paths[p].afterHash])),...successor.afterHashes},
    // Only fully validated live bytes authorize this parent projection.
    read:p => UI_PHASE3_EXISTING.includes(p) ? facts.read(p) : disk(p),
  };
}
export function uiPhase3ParentReader(root,override) {
  return cachedBoundary(root,'uiPhase3ParentReader',override,() => validate(root,override),UI_PHASE3_MANIFEST);
}
