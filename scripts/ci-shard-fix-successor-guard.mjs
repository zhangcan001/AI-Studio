import { createHash } from "node:crypto";
import { readdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { cachedBoundary, immutableGit, immutableGitStoreIdentity, immutableBlobText, sessionLiveRead, sessionGit } from "./boundary-validation-cache.mjs";

export const CI_SHARD_FIX_BASE = "b913a77160fa5d18fe0d239c70f3f04bc46524cb";
export const CI_SHARD_FIX_PROOF = "docs/architecture/ci-shard-fix-successor.json";
export const CI_SHARD_FIX_CHANGED = [
  "scripts/ci-stale-fix-successor-guard.mjs",
  "scripts/testing/ci-stale-fix.test.mjs",
  "scripts/testing/veda-removed-white-flash.test.mjs",
  "src/app/UiPhase5BMixedVideoBoundary.test.ts",
  "src/app/UiPhase5ECiShardBoundary.test.ts"
].sort();
export const CI_SHARD_FIX_ADDED = [
  "scripts/ci-shard-fix-successor-guard.mjs",
  "scripts/testing/ci-shard-fix.test.mjs"
].sort();
export const CI_SHARD_FIX_CONTRACTS = {
  "schemaChanged": false,
  "migrationAdded": false,
  "guardsRelaxed": false,
  "gpuExecuted": false,
  "productionQueueOnlyExecution": true,
  "historicalProofBytesUnchanged": true,
  "timeoutsRaised": false
};
const canonical = v => v.replaceAll("\r\n", "\n");
const sha = v => createHash("sha256").update(v).digest("hex");
const relevant = p => p === "vite.config.ts" || /^(\.github\/workflows\/|src\/|src-tauri\/|scripts\/|docs\/architecture\/)/.test(p);
const git = (root, args) => sessionGit(root, args, { encoding: "utf8", maxBuffer: 64 * 1024 * 1024 });
const factsCache = new Map();
function facts(root, base) {
  const key = JSON.stringify([immutableGitStoreIdentity(root), base]);
  if (!factsCache.has(key)) {
    const paths = immutableGit("git", ["ls-tree", "-r", "--name-only", base], { cwd: root, encoding: "utf8" })
      .trim().split(/\r?\n/).filter(relevant).sort();
    const bytes = immutableGit("git", ["cat-file", "--batch"], {
      cwd: root, input: paths.map(p => base + ":" + p + "\n").join(""), maxBuffer: 64 * 1024 * 1024,
    });
    const blobs = new Map(), blobIds = new Map(); let offset = 0;
    for (const p of paths) {
      const end = bytes.indexOf(10, offset), header = /^([a-f0-9]{40}) blob (\d+)$/.exec(bytes.subarray(offset, end).toString());
      if (!header) throw Error("immutable blob header");
      const start = end + 1, size = Number(header[2]);
      if (bytes[start + size] !== 10) throw Error("immutable blob boundary");
      blobs.set(p, bytes.subarray(start, start + size)); blobIds.set(p, header[1]); offset = start + size + 1;
    }
    if (offset !== bytes.length) throw Error("immutable trailing bytes");
    const text = new Map();
    factsCache.set(key, { paths, read: p => {
      if (!text.has(p)) text.set(p, immutableBlobText(blobIds.get(p), () => blobs.get(p)));
      return text.get(p);
    } });
  }
  return factsCache.get(key);
}
export function ciShardFixScopeSince(root, base) {
  const tip = facts(root, CI_SHARD_FIX_BASE), old = facts(root, base);
  return [...new Set([
    ...tip.paths.filter(p => !old.paths.includes(p) || tip.read(p) !== old.read(p)),
    ...CI_SHARD_FIX_CHANGED, ...CI_SHARD_FIX_ADDED, CI_SHARD_FIX_PROOF,
  ])].sort();
}
export function ciShardFixFixtureFiles(root) {
  return [...facts(root, CI_SHARD_FIX_BASE).paths, ...CI_SHARD_FIX_ADDED, CI_SHARD_FIX_PROOF];
}
function files(root, dir) {
  return readdirSync(join(root, dir), { withFileTypes: true }).flatMap(e => {
    const p = dir + "/" + e.name;
    if (e.isSymbolicLink()) throw Error("symbolic source: " + p);
    return e.isDirectory() ? files(root, p) : [p];
  });
}
function validate(root, override) {
  const workspace = resolve(root), violations = [], parent = facts(workspace, CI_SHARD_FIX_BASE);
  const fail = id => violations.push("ci-shard-fix-" + id);
  const live = p => canonical(override && Object.hasOwn(override, p) ? override[p] : sessionLiveRead(workspace, p));
  let proof;
  try {
    proof = JSON.parse(live(CI_SHARD_FIX_PROOF));
    const scope = [...CI_SHARD_FIX_CHANGED, ...CI_SHARD_FIX_ADDED].sort();
    if (proof.schemaVersion !== 1 || proof.checkpoint !== "CI_SHARD_FIX"
        || proof.parentHead !== CI_SHARD_FIX_BASE || proof.planningParentHead !== CI_SHARD_FIX_BASE
        || JSON.stringify(proof.contracts) !== JSON.stringify(CI_SHARD_FIX_CONTRACTS)
        || JSON.stringify(Object.keys(proof.files ?? {}).sort()) !== JSON.stringify(scope)) fail("proof-contract");
    if (git(workspace, ["rev-parse", CI_SHARD_FIX_BASE]).trim() !== CI_SHARD_FIX_BASE) fail("ancestor");
    const allowed = [...scope, CI_SHARD_FIX_PROOF].sort();
    const expected = [...parent.paths, ...CI_SHARD_FIX_ADDED, CI_SHARD_FIX_PROOF].sort();
    const inventory = git(workspace, ["ls-files", "--cached", "--others", "--exclude-standard"])
      .trim().split(/\r?\n/).filter(relevant);
    if (JSON.stringify([...new Set(inventory)].sort()) !== JSON.stringify(expected)) fail("inventory");
    const actualFiles = [".github/workflows", "src", "scripts", "docs/architecture", "src-tauri/runtime_packages", "src-tauri/src", "src-tauri/migrations", "src-tauri/tests"].flatMap(d => files(workspace, d));
    if (actualFiles.some(p => !expected.includes(p))) fail("illegal-file");
    const changed = parent.paths.filter(p => live(p) !== parent.read(p));
    const added = inventory.filter(p => !parent.paths.includes(p));
    if (JSON.stringify([...new Set([...changed, ...added])].sort()) !== JSON.stringify(allowed)) fail("unapproved-source");
    if (changed.some(p => p.startsWith("src-tauri/runtime_packages/"))) fail("historical-runtime-bytes");
    for (const p of CI_SHARD_FIX_CHANGED) {
      if (proof.files?.[p]?.beforeHash !== sha(parent.read(p)) || proof.files?.[p]?.afterHash !== sha(live(p))) fail("changed:" + p);
    }
    for (const p of CI_SHARD_FIX_ADDED) {
      if (parent.paths.includes(p) || proof.files?.[p]?.beforeHash !== null || proof.files?.[p]?.afterHash !== sha(live(p))) fail("added:" + p);
    }
    for (const p of Object.keys(override ?? {}).filter(p => relevant(p) && !allowed.includes(p))) {
      if (!parent.paths.includes(p) || live(p) !== parent.read(p)) fail("undeclared:" + p);
    }
  } catch (error) { fail("validation:" + String(error).slice(0, 180)); }
  return { violations, changedPaths: CI_SHARD_FIX_CHANGED,
    addedPaths: [...CI_SHARD_FIX_ADDED, CI_SHARD_FIX_PROOF],
    afterHashes: Object.fromEntries([...CI_SHARD_FIX_CHANGED, ...CI_SHARD_FIX_ADDED].map(p => [p, proof?.files?.[p]?.afterHash ?? ""])),
    read: p => CI_SHARD_FIX_CHANGED.includes(p) ? parent.read(p) : live(p) };
}
export function ciShardFixParentReader(root, override = undefined) {
  if (override) return validate(root, override);
  return cachedBoundary(root, "ciShardFixParentReader", undefined,
    () => validate(root, undefined), CI_SHARD_FIX_PROOF);
}
