// Immutable Git objects persist; live acceptance is keyed by freshly read bytes.
import { createHash } from 'node:crypto';
import { execFileSync as exec } from 'node:child_process';
import { existsSync, readFileSync, readdirSync, realpathSync, statSync } from 'node:fs';
import { join, resolve } from 'node:path';
const gitObjects = new Map(), gitBlobs = new Map(), gitTrees = new Map(), results = new Map(), validatedProofs = new Map();
let session;
function immutableObjectStore(root, env) {
  // Only immutable SHA queries share IO. Live acceptance and mutable refs stay
  // root-scoped and freshly hashed. Environment-directed Git layouts do not
  // participate in alternate-store sharing.
  const harmless = ['GIT_PAGER','GIT_TERMINAL_PROMPT','GIT_OPTIONAL_LOCKS','GIT_ASKPASS','GIT_LFS_SKIP_SMUDGE'];
  const gitEnv = Object.entries(env ?? process.env).filter(([key]) => key.startsWith('GIT_') && !harmless.includes(key)).sort();
  if (gitEnv.length) return JSON.stringify([resolve(root), gitEnv]);
  try {
    const dot = join(root, '.git');
    let git = statSync(dot).isDirectory() ? dot : resolve(root, readFileSync(dot,'utf8').trim().replace(/^gitdir: /,''));
    if (existsSync(join(git,'commondir'))) git = resolve(git,readFileSync(join(git,'commondir'),'utf8').trim());
    const objects = join(git,'objects'), alternates = join(objects,'info/alternates');
    const empty = readdirSync(objects).every(p => p === 'info' || p === 'pack')
      && (!existsSync(join(objects,'pack')) || readdirSync(join(objects,'pack')).length === 0);
    if (empty && existsSync(alternates)) {
      const paths = readFileSync(alternates,'utf8').trim().split(/\r?\n/);
      if (paths.length === 1 && paths[0]) return realpathSync(resolve(objects,paths[0]));
    }
    return realpathSync(objects);
  } catch { return resolve(root); } // Missing Git evidence must still reach Git and fail.
}
// Share immutable parent facts across owned fixtures using the same object store.
// Git-layout environment overrides keep their existing root-scoped identity.
export const immutableGitStoreIdentity = root => immutableObjectStore(root, process.env);
export function immutableGit(command, args, options = {}) {
  let tree = 1;
  while (args[tree] === '-r' || args[tree] === '--name-only') tree++;
  const immutable = command === 'git' && (
    args[0] === 'ls-tree' && /^[a-f0-9]{40}$/.test(args[tree] ?? '') ||
    args[0] === 'cat-file' && args[1] === '--batch' &&
    String(options.input ?? '').trim().split('\n').every(a => /^[a-f0-9]{40}:/.test(a)));
  if (!immutable) return exec(command, args, options);
  const store = immutableObjectStore(options.cwd ?? '.',options.env);
  const key = JSON.stringify([store, args, options.input, options.encoding, options.maxBuffer]);
  // Blob batches are reassembled from the verified per-store blob cache into a
  // fresh buffer; retaining each assembled batch as well would hold a second
  // full-snapshot copy per reader without avoiding any Git IO.
  if (args.length === 2 && args[0] === 'cat-file') return immutableBlobBatch(store, args, options);
  // Mutable replace refs must not redefine a supposedly immutable SHA query.
  if (!gitObjects.has(key)) gitObjects.set(key, exec(command, ['--no-replace-objects',...args], options));
  const value = gitObjects.get(key);
  return Buffer.isBuffer(value) ? Buffer.from(value) : value;
}
function immutableBlobBatch(store, args, options) {
  // Different historical commits mostly contain the same immutable blobs. Ask
  // Git for identities first; transfer each verified object only once per store.
  // Commit:path resolution still goes through Git with replace refs disabled.
  let blobs = gitBlobs.get(store);
  if (!blobs) gitBlobs.set(store, blobs = new Map());
  const queries = String(options.input).split('\n');
  if (queries.at(-1) === '') queries.pop();
  if (queries.some(query => !/^[a-f0-9]{40}:/.test(query)))
    return exec('git', ['--no-replace-objects', ...args], options);
  const ids = queries.map(query => {
    const commit = query.slice(0, 40), path = query.slice(41), key = JSON.stringify([store, commit]);
    if (!gitTrees.has(key)) {
      // Resolve immutable commit paths once, rather than traversing the same
      // trees separately for thousands of batch-check queries in each reader.
      const bytes = exec('git', ['--no-replace-objects', 'ls-tree', '-r', '-z', '--full-tree', commit],
        { ...options, input: undefined, encoding: undefined, maxBuffer: 64 * 1024 * 1024 });
      const entries = bytes.toString('utf8').split('\0'), tree = new Map();
      if (entries.pop() !== '') throw Error('Immutable Git tree boundary mismatch');
      for (const entry of entries) {
        const match = /^[0-7]{6} (blob|commit) ([a-f0-9]{40})\t([\s\S]+)$/.exec(entry);
        if (!match || tree.has(match[3])) throw Error('Immutable Git tree entry mismatch');
        tree.set(match[3], match[1] === 'blob' ? match[2] : undefined);
      }
      gitTrees.set(key, tree);
    }
    return gitTrees.get(key).get(path);
  });
  // Preserve Git's missing/directory/path-expression behavior verbatim.
  if (ids.some(id => !id)) return exec('git', ['--no-replace-objects', ...args], options);
  const missing = [...new Set(ids.filter(id => !blobs.has(id)))];
  if (missing.length) {
    const bytes = exec('git', ['--no-replace-objects', 'cat-file', '--batch'], {
      ...options, input: missing.join('\n') + '\n', encoding: undefined,
    });
    const fetched = new Map(); let offset = 0;
    for (const id of missing) {
      const end = bytes.indexOf(10, offset), header = /^([a-f0-9]{40}) blob (\d+)$/.exec(bytes.subarray(offset, end).toString());
      if (!header || header[1] !== id) throw Error('Immutable Git blob header mismatch');
      const start = end + 1, size = Number(header[2]), content = bytes.subarray(start, start + size);
      if (content.length !== size || bytes[start + size] !== 10
          || createHash('sha1').update(`blob ${size}\0`).update(content).digest('hex') !== id) throw Error('Immutable Git blob bytes mismatch');
      fetched.set(id, Buffer.from(content)); offset = start + size + 1;
    }
    if (offset !== bytes.length) throw Error('Immutable Git blob trailing bytes');
    for (const [id, content] of fetched) blobs.set(id, content);
  }
  const chunks = [];
  for (const id of ids) {
    const content = blobs.get(id);
    chunks.push(Buffer.from(`${id} blob ${content.length}\n`), content, Buffer.from('\n'));
  }
  const result = Buffer.concat(chunks);
  // Preserve execFileSync's output limit and encoding contract, including failure.
  if (result.length > (options.maxBuffer ?? 1024 * 1024)) return exec('git', ['--no-replace-objects', ...args], options);
  return options.encoding && options.encoding !== 'buffer' ? result.toString(options.encoding) : result;
}
function identity(root, seen) {
  const hash = createHash('sha256');
  // The explicit raw-byte profile is a validation input, never inferred from
  // live package bytes. Changing it must invalidate every historical reader.
  hash.update(JSON.stringify(['runtime-byte-profile',process.env.AI_STUDIO_PHASE2_RUNTIME_BYTE_PROFILE ?? 'git']));
  const visit = dir => {
    if (!existsSync(join(root, dir))) return;
    for (const entry of readdirSync(join(root, dir), { withFileTypes: true }).sort((a,b) => a.name.localeCompare(b.name))) {
      const path = dir ? `${dir}/${entry.name}` : entry.name;
      if (entry.isDirectory()) visit(path);
      else {
        const bytes = readFileSync(join(root, path));
        seen?.set(path, bytes);
        hash.update(JSON.stringify([path, bytes.length])); hash.update(bytes);
      }
    }
  };
  for (const dir of ['src', 'src-tauri/src', 'src-tauri/tests', 'src-tauri/examples',
    'src-tauri/migrations', 'src-tauri/runtime_packages', 'src-tauri/capabilities',
    'src-tauri/icons', 'scripts', 'docs', '.github']) visit(dir);
  for (const path of ['CONTEXT.md','README.md','package.json','pnpm-lock.yaml','vite.config.ts','src-tauri/Cargo.toml','src-tauri/Cargo.lock',
    'src-tauri/tauri.conf.json','src-tauri/build.rs','src-tauri/.gitignore']) {
    if (existsSync(join(root,path))) {hash.update(path); hash.update(readFileSync(join(root,path)));}
  }
  // Publication acceptance depends on a mutable tag, not just source bytes.
  const dot = join(root,'.git');
  if (existsSync(dot)) {
    // The hydration checkpoint covers Git source inventory too. Empty-index
    // historical fixtures and illegal top-level Native additions must invalidate
    // cached accepts without walking ignored target/ build output.
    try {
      hash.update(exec('git', ['ls-files','--cached','--others','--exclude-standard','--',
        'src','src-tauri','scripts','docs/architecture'], {cwd:root, encoding:'utf8', stdio:'pipe'}));
    } catch {
      // Byte-cache fixtures may contain only a mutable tag, not a repository.
      // Keep their identity distinct from a Git-backed checkout. This is NOT
      // validation success: proof readers still require their immutable Git evidence.
      hash.update('git-source-inventory-unavailable');
    }
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
  // Stage nested successes until the outer live validation commits. A drifted
  // outer identity must not retain nested accepts under a doomed byte profile.
  if (outer) { const files = new Map(); session = {root, identity: identity(root, files), staged: new Map(), files, texts: new Map()}; }
  try {
    if (session.root !== root) throw Error('Mixed boundary roots in one validation');
    const key = JSON.stringify([root, session.identity, name, override ?? null]);
    if (results.has(key)) return results.get(key);
    if (session.staged.has(key)) return session.staged.get(key);
    const result = validate();
    // Never reuse a rejected reader: its live fallback must remain fresh.
    if (!result.violations.length) {
      session.staged.set(key, result);
      if (outer) {
        if (results.size + session.staged.size >= 128) results.clear();
        for (const [stagedKey, stagedResult] of session.staged) results.set(stagedKey, stagedResult);
        session.staged.clear();
        if (!override && proofPath) validatedProofs.set(proofKey,JSON.stringify(JSON.parse(readFileSync(join(root,proofPath),'utf8'))));
      }
    }
    if (outer && !result.violations.length && identity(root) !== session.identity) {
      results.clear(); validatedProofs.clear();
      return {violations: ['boundary-live-drift-during-validation'],addedPaths:[],afterHashes:{},addedCommandSignatures:[],
        read: p => readFileSync(join(root,p),'utf8').replaceAll('\r\n','\n')};
    }
    return result;
  } finally { if (outer) session = undefined; }
}
// Live bytes read during one outer validation session. The session's opening
// identity hashed exactly these bytes and its closing identity re-reads the
// tree, so any drift mid-session is still rejected. Outside a session (or for a
// different root) every read stays fresh from disk.
export function sessionLiveRead(root, path) {
  if (session && session.root === resolve(root) && !path.includes('..')) {
    let text = session.texts.get(path);
    if (text === undefined) {
      const bytes = session.files.get(path) ?? readFileSync(join(root, path));
      session.files.set(path, bytes);
      session.texts.set(path, text = bytes.toString('utf8'));
    }
    return text;
  }
  return readFileSync(join(root, path), 'utf8');
}
// Within one outer validation session, every successor in the guard chain asks
// Git for the same working-tree inventory and the same immutable ancestor ids.
// Memoize only those exact queries, only for the session's own root, and only
// while the session is open. The session's closing identity re-reads the tree
// and Git inventory, so drift during validation is still rejected; outside a
// session (override probes, other roots) every query goes to Git fresh.
const sessionGitQueries = args => args.length === 4 && args[0] === 'ls-files' && args[1] === '--cached'
  && args[2] === '--others' && args[3] === '--exclude-standard'
  || args.length === 2 && args[0] === 'rev-parse' && /^[a-f0-9]{40}$/.test(args[1]);
// `rev-parse <40-hex>` names an immutable object id; like the ls-tree/cat-file
// queries above, a SUCCESSFUL answer is shared per immutable object store (each
// guard asks for its own frozen base once per validation, ~40 Git spawns per
// chain). Failures are never cached, so a missing ancestor still fails every time.
const immutableRevParses = new Map();
export function sessionGit(root, args, options) {
  if (args.length === 2 && args[0] === 'rev-parse' && /^[a-f0-9]{40}$/.test(args[1])
      && options?.encoding === 'utf8' && options?.input === undefined && options?.env === undefined
      && Object.keys(options).every(k => k === 'encoding' || k === 'maxBuffer')) {
    const key = JSON.stringify([immutableObjectStore(root, process.env), args[1]]);
    if (!immutableRevParses.has(key)) immutableRevParses.set(key, exec('git', args, { cwd: root, ...options }));
    return immutableRevParses.get(key);
  }
  if (!session || session.root !== resolve(root) || !sessionGitQueries(args)
      || options?.encoding !== 'utf8' || options?.input !== undefined || options?.env !== undefined)
    return exec('git', args, { cwd: root, ...options });
  session.git ??= new Map();
  const key = JSON.stringify(args);
  if (!session.git.has(key)) session.git.set(key, exec('git', args, { cwd: root, ...options }));
  return session.git.get(key);
}
// Immutable Git blob text keyed by its verified object id: historical readers
// share one canonical decode per blob instead of re-decoding it per checkpoint.
const blobTexts = new Map();
export function immutableBlobText(id, bytes) {
  if (!/^[a-f0-9]{40}$/.test(id ?? '')) return bytes().toString('utf8').replaceAll('\r\n', '\n');
  let text = blobTexts.get(id);
  if (text === undefined) blobTexts.set(id, text = bytes().toString('utf8').replaceAll('\r\n', '\n'));
  return text;
}
// Within one outer validation session, every guard reads its successor's
// projection for the same paths, so an unmemoized read walks the whole chain
// for each path again. Memoize a reader's answers only while the session for
// its root is open; the closing identity still rejects mid-session drift, and
// outside a session (override probes, later reads) every call stays fresh.
const sessionReads = new WeakMap();
export function sessionMemoRead(root, read) {
  root = resolve(root);
  const wrapped = p => {
    if (!session || session.root !== root) return read(p);
    let readers = sessionReads.get(session);
    if (!readers) sessionReads.set(session, readers = new Map());
    let memo = readers.get(wrapped);
    if (!memo) readers.set(wrapped, memo = new Map());
    if (!memo.has(p)) memo.set(p, read(p));
    return memo.get(p);
  };
  return wrapped;
}
// One retained copy per verified immutable blob id. Historical readers parse
// whole-snapshot batches; keeping a subarray would pin each batch's full buffer
// for the process lifetime (one snapshot per checkpoint). Ids are git-verified
// by the batch reader; an id-less blob is never shared.
const blobBytes = new Map();
// Callers pass the subarray itself, not a closure over the batch: a closure
// would capture the batch in the caller's scope context and pin it anyway.
export function immutableBlobBytes(id, bytes) {
  if (!/^[a-f0-9]{40}$/.test(id ?? '')) return Buffer.from(bytes);
  let value = blobBytes.get(id);
  if (value === undefined) blobBytes.set(id, value = Buffer.from(bytes));
  return value;
}
