// M2-2 successor validates live bytes before historical guards read their parent.
// Historical manifests remain immutable; no authority or transport exemption.
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';
import { m3LibraryParentReader } from './m3-library-findability-boundary-guard.mjs';

export const M2_REUSE_PARENT = '974d59e513961155a1b551b4de3da5658e1359bb';
const existing = ['src/product/client.test.ts', 'scripts/m1-settings-runtime-boundary-guard.mjs', 'scripts/m1-overview-readiness-boundary-guard.mjs', 'scripts/m1-readiness-boundary-guard.mjs', 'scripts/m2-run-recovery-boundary-guard.mjs', 'src-tauri/src/application/product/creation_facade/context.rs', 'src-tauri/src/application/product/creation_facade/submission.rs', 'src-tauri/src/application/product/library_facade/operations.rs', 'src-tauri/src/application/product/library_facade/types.rs', 'src-tauri/src/commands/product.rs', 'src-tauri/tests/support/creation_submission_contract.rs', 'src-tauri/tests/product_library_contract.rs', 'src/app/App.tsx', 'src/features/create/CreateController.ts', 'src/features/create/CreateInputs.tsx', 'src/features/create/CreatePage.test.tsx', 'src/features/create/CreatePage.tsx', 'src/features/library/LibraryController.ts', 'src/features/library/LibraryDetail.tsx', 'src/features/library/LibraryPage.test.tsx', 'src/product/libraryTypes.ts', 'src/product/types.ts', 'src/stores/studioStore.test.ts', 'src/stores/studioStore.ts'].sort();
const added = ['scripts/m2-create-library-reuse-boundary-guard.mjs', 'src/app/M2CreateLibraryNavigation.test.tsx', 'src/app/M2CreateLibraryReuseBoundary.test.ts'].sort();
const text = s => s.replaceAll('\r\n', '\n');
const hash = s => createHash('sha256').update(text(s)).digest('hex');
const disk = (root, p) => text(readFileSync(join(root, p), 'utf8'));
const files = (root, folder, pattern) => readdirSync(join(root, folder), { withFileTypes: true }).flatMap(e => {
  const p = `${folder}/${e.name}`;
  return e.isDirectory() ? files(root, p, pattern) : pattern.test(p) ? [p] : [];
}).sort();
const aggregate = (paths, read) => hash(paths.map(p => `${p}\n${read(p)}`).join('\n'));
const immutable = new Map();
function parentBlobs(root, paths) {
  const cacheKey = JSON.stringify([root, paths]);
  if (immutable.has(cacheKey)) return immutable.get(cacheKey);
  const bytes = execFileSync('git', ['cat-file', '--batch'], { cwd: root,
    input: paths.map(p => `${M2_REUSE_PARENT}:${p}\n`).join(''), maxBuffer: 64 * 1024 * 1024 });
  const result = new Map(); let offset = 0;
  for (const p of paths) {
    const end = bytes.indexOf(10, offset), match = /^[a-f0-9]+ blob (\d+)$/.exec(bytes.subarray(offset, end).toString());
    if (!match) throw Error(`M1 parent unavailable: ${p}`);
    const start = end + 1, length = Number(match[1]);
    if (bytes[start + length] !== 10) throw Error('Invalid M1 parent blob boundary');
    result.set(p, text(bytes.subarray(start, start + length).toString('utf8'))); offset = start + length + 1;
  }
  if (offset !== bytes.length) throw Error('Unexpected M1 parent blob bytes');
  immutable.set(cacheKey, result); return result;
}

export function m2ReuseParentReader(root, override) {
  // Memoize reads only within this synchronous validation; no live cache survives
  // a call or replaces a fresh fail-closed read. Parent blobs alone persist.
  const successor = m3LibraryParentReader(root);
  const projectedFiles = (root,folder,pattern) => files(root,folder,pattern).filter(p=>!successor.addedPaths.includes(p));
  const live = new Map();
  const disk = (_root,p) => { if (!live.has(p)) live.set(p,successor.read(p)); return live.get(p); };
  const review = override ?? JSON.parse(disk(root, 'docs/architecture/m2-2-create-library-reuse.json'));
  const violations = [...successor.violations], fail = s => violations.push(`m2-2-${s}`);
  if (review.checkpoint !== 'M2_2_CREATE_LIBRARY_PRECISE_REUSE' || review.parentHead !== M2_REUSE_PARENT || review.schemaVersion !== 1) fail('invalid-header');
  if (JSON.stringify(Object.keys(review.paths ?? {}).sort()) !== JSON.stringify([...existing, ...added].sort())) fail('path-set');
  const listed = execFileSync('git', ['ls-tree', '-r', '--name-only', M2_REUSE_PARENT, 'src', 'src-tauri/src', 'scripts', 'src-tauri/tests/support/creation_submission_contract.rs', 'src-tauri/tests/product_library_contract.rs'],
    { cwd: root, encoding: 'utf8' }).trim().split(/\r?\n/).filter(p => /\.(tsx?|rs|css|mjs)$/.test(p)).sort();
  const blobs = parentBlobs(root, listed), before = p => blobs.get(p);
  for (const p of existing) if (review.paths?.[p]?.beforeHash !== hash(before(p)) || review.paths?.[p]?.afterHash !== hash(disk(root, p))) fail(`path-drift:${p}`);
  for (const p of added) if (blobs.has(p) || review.paths?.[p]?.beforeHash !== null || review.paths?.[p]?.afterHash !== hash(disk(root, p))) fail(`addition-drift:${p}`);
  for (const [name, folder, pattern] of [['backend', 'src-tauri/src', /\.rs$/], ['frontend', 'src', /\.tsx?$/], ['styles', 'src', /\.css$/], ['scripts', 'scripts', /\.mjs$/]]) {
    const base = listed.filter(p => p.startsWith(`${folder}/`) && pattern.test(p)), current = projectedFiles(root, folder, pattern);
    const expected = [...base, ...added.filter(p => p.startsWith(`${folder}/`) && pattern.test(p))].sort();
    const unchanged = base.filter(p => !existing.includes(p));
    const proof = review[name];
    if (JSON.stringify(current) !== JSON.stringify(expected) || proof?.beforeFiles !== base.length || proof?.afterFiles !== current.length) fail(`${name}-file-set`);
    if (proof?.beforeAggregateHash !== aggregate(base, before) || proof?.afterAggregateHash !== aggregate(current, p => disk(root, p)) ||
      proof?.untouchedAggregateHash !== aggregate(unchanged, before) || proof?.untouchedAggregateHash !== aggregate(unchanged, p => disk(root, p))) fail(`${name}-aggregate`);
  }
  for (const flag of ['queueAuthorityChanged','taskStateMachineChanged','workflowEngineChanged','bindingOccChanged','reviewAuthorityChanged','shotSelectionAuthorityChanged','libraryAuthorityChanged','draftAuthorityChanged','promptLibraryAuthorityChanged','newSearchAuthority','newReuseStore','newRouter','newPollingOwner','schemaChanged','backupFormatChanged','remoteTelemetry']) if (review.invariants?.[flag] !== false) fail(`invariant:${flag}`);
  const controller = disk(root,'src/features/create/CreateController.ts');
  const store = disk(root,'src/stores/studioStore.ts');
  const submission = disk(root,'src-tauri/src/application/product/creation_facade/submission.rs');
  if (!controller.includes('applyCreationPrompt(choice)') || !store.includes('key === "prompt" ? {creationPromptProvenance: undefined}') || /(?:text|name)\s*===.*promptVersion|\.find\([^\n]*(?:\.text|\.name)\s*===/.test(controller+store)) fail('typed-prompt-selection');
  const apply = controller.slice(controller.indexOf('function applyLibraryAsset'),controller.indexOf('function setValue'));
  if (/generate|referencesSet|setReferences|queue|chooseGenerator/.test(apply) || !apply.includes('setValue(field.key,mediaValue(field,[libraryIntent.assetId]))')) fail('asset-draft-only');
  if (!/prompts\s*\.get\(&request\.project_id, prompt_id\)/.test(submission) || !submission.includes('text == &version.text') || !submission.includes('version.prompt_id != *prompt_id') || !submission.includes('_ => return Err(prompt_provenance_error())') || !submission.includes('            prompt_version_id,') || !submission.includes('model_version_id: None')) fail('backend-provenance');
  for (const p of ['src/features/create/CreateController.ts','src/features/create/CreateInputs.tsx','src/features/library/LibraryController.ts','src/features/library/LibraryDetail.tsx']) if (/services\/(?:tauriClient|ipc)|@tauri-apps|\binvoke\s*\(/.test(disk(root,p))) fail(`transport-owner:${p}`);
  return { violations, addedPaths: [...added,...successor.addedPaths], afterHashes: { ...Object.fromEntries(Object.entries(review.paths ?? {}).map(([p, proof]) => [p, proof.afterHash])), ...successor.afterHashes },
    backendAggregateSha256: violations.length ? undefined : successor.backendAggregateSha256 ?? review.backend.afterAggregateHash,
    read: p => violations.length || !existing.includes(p) ? successor.read(p) : before(p) };
}
