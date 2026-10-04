// M1-3 successor validates live bytes before historical guards read their parent.
// Historical manifests remain immutable; no authority or transport exemption.
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';

export const M1_SETTINGS_PARENT = '1a66234867692f8b0ec9d6cbb587f9d5f71ddef7';
const existing = ['src/features/settings/SettingsWorkspace.tsx', 'src/features/tools/LocalToolHub.tsx',
  'src/features/tools/LocalToolHub.test.tsx', 'src/app/App.tsx', 'scripts/m1-overview-readiness-boundary-guard.mjs'].sort();
const added = ['src/features/settings/SettingsWorkspace.test.tsx', 'src/app/M1SettingsNavigation.test.tsx',
  'src/app/M1SettingsRuntimeBoundary.test.ts', 'scripts/m1-settings-runtime-boundary-guard.mjs'].sort();
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
    input: paths.map(p => `${M1_SETTINGS_PARENT}:${p}\n`).join(''), maxBuffer: 64 * 1024 * 1024 });
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

export function m1SettingsParentReader(root, override) {
  const review = override ?? JSON.parse(disk(root, 'docs/architecture/m1-3-settings-runtime.json'));
  const violations = [], fail = s => violations.push(`m1-3-${s}`);
  if (review.checkpoint !== 'M1_3_SETTINGS_RUNTIME_SETUP_AND_TOOL_HUB_EXPLANATION' || review.parentHead !== M1_SETTINGS_PARENT || review.schemaVersion !== 1) fail('invalid-header');
  if (JSON.stringify(Object.keys(review.paths ?? {}).sort()) !== JSON.stringify([...existing, ...added].sort())) fail('path-set');
  const listed = execFileSync('git', ['ls-tree', '-r', '--name-only', M1_SETTINGS_PARENT, 'src', 'src-tauri/src', 'scripts', 'src-tauri/tests/support/creation_submission_contract.rs'],
    { cwd: root, encoding: 'utf8' }).trim().split(/\r?\n/).filter(p => /\.(tsx?|rs|css|mjs)$/.test(p)).sort();
  const blobs = parentBlobs(root, listed), before = p => blobs.get(p);
  for (const p of existing) if (review.paths?.[p]?.beforeHash !== hash(before(p)) || review.paths?.[p]?.afterHash !== hash(disk(root, p))) fail(`path-drift:${p}`);
  for (const p of added) if (blobs.has(p) || review.paths?.[p]?.beforeHash !== null || review.paths?.[p]?.afterHash !== hash(disk(root, p))) fail(`addition-drift:${p}`);
  for (const [name, folder, pattern] of [['backend', 'src-tauri/src', /\.rs$/], ['frontend', 'src', /\.tsx?$/], ['styles', 'src', /\.css$/], ['scripts', 'scripts', /\.mjs$/]]) {
    const base = listed.filter(p => p.startsWith(`${folder}/`) && pattern.test(p)), current = files(root, folder, pattern);
    const expected = [...base, ...added.filter(p => p.startsWith(`${folder}/`) && pattern.test(p))].sort();
    const unchanged = base.filter(p => !existing.includes(p));
    const proof = review[name];
    if (JSON.stringify(current) !== JSON.stringify(expected) || proof?.beforeFiles !== base.length || proof?.afterFiles !== current.length) fail(`${name}-file-set`);
    if (proof?.beforeAggregateHash !== aggregate(base, before) || proof?.afterAggregateHash !== aggregate(current, p => disk(root, p)) ||
      proof?.untouchedAggregateHash !== aggregate(unchanged, before) || proof?.untouchedAggregateHash !== aggregate(unchanged, p => disk(root, p))) fail(`${name}-aggregate`);
  }
  for (const flag of ['queueAuthorityChanged', 'taskStateMachineChanged', 'workflowEngineChanged', 'bindingOccChanged', 'draftAuthorityChanged', 'schemaChanged', 'backupFormatChanged', 'remoteTelemetry', 'projectCommandCenterAuthorityChanged', 'newReadinessAuthority', 'newPollingOwner', 'comfySettingsAuthorityChanged', 'comfyRuntimeAuthorityChanged', 'toolHubAuthorityChanged', 'automaticToolDiscovery', 'automaticToolHealthPolling', 'processLaunchAdded']) if (review.invariants?.[flag] !== false) fail(`invariant:${flag}`);
  const tools = disk(root, 'src/features/tools/LocalToolHub.tsx');
  const toolImports = /import\s*\{([^}]+)\}\s*from "\.\.\/\.\.\/services\/tauriClient"/.exec(tools)?.[1].split(',').map(s => s.trim()).filter(Boolean).sort();
  if (JSON.stringify(toolImports) !== JSON.stringify(['listToolCapabilities', 'listToolInstances', 'listToolVersions', 'listTools'].sort()) ||
      /saveComfyEndpoint|applyComfyEnvironmentProfile|setInterval|setTimeout|\binvoke\s*\(|child_process|\b(?:generate|launch|spawn|exec|scan|probe)\s*\(/.test(tools)) fail('tool-viewer-authority');
  if (/\b(?:saveTool|registerTool|updateTool|createTool|setInterval)\w*\s*\(/.test(disk(root, 'src/features/settings/SettingsWorkspace.tsx'))) fail('settings-registry-authority');
  return { violations, addedPaths: added, afterHashes: Object.fromEntries(Object.entries(review.paths ?? {}).map(([p, proof]) => [p, proof.afterHash])),
    backendAggregateSha256: violations.length ? undefined : review.backend.afterAggregateHash,
    read: p => violations.length || !existing.includes(p) ? disk(root, p) : before(p) };
}
