// Immutable Git objects persist; live acceptance is keyed by freshly read bytes.
import { createHash } from 'node:crypto';
import { execFileSync as exec } from 'node:child_process';
import { existsSync, readFileSync, readdirSync, statSync } from 'node:fs';
import { join, resolve } from 'node:path';
const gitObjects = new Map(), results = new Map(), validatedProofs = new Map();
let session;
export function immutableGit(command, args, options = {}) {
  const immutable = command === 'git' && (
    args[0] === 'ls-tree' && args.some(a => /^[a-f0-9]{40}$/.test(a)) ||
    args[0] === 'cat-file' && args[1] === '--batch' &&
    String(options.input ?? '').trim().split('\n').every(a => /^[a-f0-9]{40}:/.test(a)));
  if (!immutable) return exec(command, args, options);
  const key = JSON.stringify([resolve(options.cwd ?? '.'), args, options.input, options.encoding]);
  if (!gitObjects.has(key)) gitObjects.set(key, exec(command, args, options));
  const value = gitObjects.get(key);
  return Buffer.isBuffer(value) ? Buffer.from(value) : value;
}
function identity(root) {
  const hash = createHash('sha256');
  const visit = dir => {
    if (!existsSync(join(root, dir))) return;
    for (const entry of readdirSync(join(root, dir), { withFileTypes: true }).sort((a,b) => a.name.localeCompare(b.name))) {
      const path = dir ? `${dir}/${entry.name}` : entry.name;
      if (entry.isDirectory()) visit(path);
      else {
        const bytes = readFileSync(join(root, path));
        hash.update(JSON.stringify([path, bytes.length])); hash.update(bytes);
      }
    }
  };
  for (const dir of ['src', 'src-tauri/src', 'src-tauri/tests', 'src-tauri/examples',
    'src-tauri/migrations', 'src-tauri/runtime_packages', 'scripts', 'docs', '.github']) visit(dir);
  for (const path of ['README.md','package.json','pnpm-lock.yaml','src-tauri/Cargo.toml','src-tauri/Cargo.lock',
    'src-tauri/tauri.conf.json','src-tauri/build.rs']) {
    if (existsSync(join(root,path))) {hash.update(path); hash.update(readFileSync(join(root,path)));}
  }
  // Publication acceptance depends on a mutable tag, not just source bytes.
  const dot = join(root,'.git');
  if (existsSync(dot)) {
    let git = statSync(dot).isDirectory() ? dot : resolve(root, readFileSync(dot,'utf8').trim().replace(/^gitdir: /,''));
    if (existsSync(join(git,'commondir'))) git = resolve(git,readFileSync(join(git,'commondir'),'utf8').trim());
    for (const path of ['refs/tags/v2.1.0-personal','packed-refs']) {
      hash.update(path); if (existsSync(join(git,path))) hash.update(readFileSync(join(git,path)));
    }
  }
  return hash.digest('hex');
}
export function cachedBoundary(root, name, override, validate, proofPath) {
  root = resolve(root);
  const proofKey = JSON.stringify([root,name]);
  const known = validatedProofs.get(proofKey);
  if (override && known && JSON.stringify(override) !== known) {
    // Historical review overrides may test rejection, not amend a validated
    // immutable review. No successful projection is returned on this fast path.
    return {violations: [`${name}-immutable-review-metadata-drift`], addedPaths: [],
      afterHashes: {}, addedCommandSignatures: [], backendAggregateSha256: undefined,
      read: p => readFileSync(join(root,p),'utf8').replaceAll('\r\n','\n')};
  }
  const outer = !session;
  if (outer) session = {root, identity: identity(root)};
  try {
    if (session.root !== root) throw Error('Mixed boundary roots in one validation');
    const key = JSON.stringify([root, session.identity, name, override ?? null]);
    if (!results.has(key)) {
      const result = validate();
      // Never reuse a rejected reader: its live fallback must remain fresh.
      if (!result.violations.length) {
        if (results.size >= 128) results.clear();
        results.set(key,result);
        if (!override && proofPath) validatedProofs.set(proofKey,JSON.stringify(JSON.parse(readFileSync(join(root,proofPath),'utf8'))));
      }
      if (outer && !result.violations.length && identity(root) !== session.identity) {
        results.clear(); validatedProofs.clear();
        return {violations: ['boundary-live-drift-during-validation'],addedPaths:[],afterHashes:{},addedCommandSignatures:[],
          read: p => readFileSync(join(root,p),'utf8').replaceAll('\r\n','\n')};
      }
      return result;
    }
    return results.get(key);
  } finally { if (outer) session = undefined; }
}
