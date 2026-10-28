import { createHash } from "node:crypto";
import { readdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { cachedBoundary, immutableGit, immutableGitStoreIdentity, immutableBlobText, sessionLiveRead, sessionGit } from "./boundary-validation-cache.mjs";

export const CI_CONSOLIDATE_FIX_BASE = "58265717a4126251ffdba89afa55227ebdbda754";
export const CI_CONSOLIDATE_FIX_PROOF = "docs/architecture/ci-consolidate-fix-successor.json";
export const CI_CONSOLIDATE_FIX_CHANGED = [
  "scripts/ci-shard-fix-successor-guard.mjs",
  "scripts/testing/ci-assert-fix.test.mjs",
  "scripts/testing/ci-boundary-fix.test.mjs",
  "scripts/testing/ci-chain-fix.test.mjs",
  "scripts/testing/ci-closed-tip-fix.test.mjs",
  "scripts/testing/ci-green-fix.test.mjs",
  "scripts/testing/ci-h3-budget-fix.test.mjs",
  "scripts/testing/ci-phase1-fix.test.mjs",
  "scripts/testing/ci-phase5h-fix.test.mjs",
  "scripts/testing/ci-phase6a-fix.test.mjs",
  "scripts/testing/ci-probe-fix.test.mjs",
  "scripts/testing/ci-queue-fix.test.mjs",
  "scripts/testing/ci-scope-fix.test.mjs",
  "scripts/testing/ci-shard-fix.test.mjs",
  "scripts/testing/ci-stable-fix.test.mjs",
  "scripts/testing/ci-stale-fix.test.mjs",
  "src/app/UiPhase5GCiTimeoutBoundary.test.ts"
].sort();
export const CI_CONSOLIDATE_FIX_ADDED = [
  "scripts/ci-consolidate-fix-successor-guard.mjs",
  "scripts/testing/ci-chain-consolidated.test.mjs"
].sort();
export const CI_CONSOLIDATE_FIX_CONTRACTS = {
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
export function ciConsolidateFixScopeSince(root, base) {
  const tip = facts(root, CI_CONSOLIDATE_FIX_BASE), old = facts(root, base);
  return [...new Set([
    ...tip.paths.filter(p => !old.paths.includes(p) || tip.read(p) !== old.read(p)),
    ...CI_CONSOLIDATE_FIX_CHANGED, ...CI_CONSOLIDATE_FIX_ADDED, CI_CONSOLIDATE_FIX_PROOF,
  ])].sort();
}
export function ciConsolidateFixFixtureFiles(root) {
  return [...facts(root, CI_CONSOLIDATE_FIX_BASE).paths, ...CI_CONSOLIDATE_FIX_ADDED, CI_CONSOLIDATE_FIX_PROOF];
}
function files(root, dir) {
  return readdirSync(join(root, dir), { withFileTypes: true }).flatMap(e => {
    const p = dir + "/" + e.name;
    if (e.isSymbolicLink()) throw Error("symbolic source: " + p);
    return e.isDirectory() ? files(root, p) : [p];
  });
}
function validate(root, override) {
  const workspace = resolve(root), violations = [], parent = facts(workspace, CI_CONSOLIDATE_FIX_BASE);
  const fail = id => violations.push("ci-consolidate-fix-" + id);
  const live = p => canonical(override && Object.hasOwn(override, p) ? override[p] : sessionLiveRead(workspace, p));
  let proof;
  try {
    proof = JSON.parse(live(CI_CONSOLIDATE_FIX_PROOF));
    const scope = [...CI_CONSOLIDATE_FIX_CHANGED, ...CI_CONSOLIDATE_FIX_ADDED].sort();
    if (proof.schemaVersion !== 1 || proof.checkpoint !== "CI_CONSOLIDATE_FIX"
        || proof.parentHead !== CI_CONSOLIDATE_FIX_BASE || proof.planningParentHead !== CI_CONSOLIDATE_FIX_BASE
        || JSON.stringify(proof.contracts) !== JSON.stringify(CI_CONSOLIDATE_FIX_CONTRACTS)
        || JSON.stringify(Object.keys(proof.files ?? {}).sort()) !== JSON.stringify(scope)) fail("proof-contract");
    if (git(workspace, ["rev-parse", CI_CONSOLIDATE_FIX_BASE]).trim() !== CI_CONSOLIDATE_FIX_BASE) fail("ancestor");
    const allowed = [...scope, CI_CONSOLIDATE_FIX_PROOF].sort();
    const expected = [...parent.paths, ...CI_CONSOLIDATE_FIX_ADDED, CI_CONSOLIDATE_FIX_PROOF].sort();
    const inventory = git(workspace, ["ls-files", "--cached", "--others", "--exclude-standard"])
      .trim().split(/\r?\n/).filter(relevant);
    if (JSON.stringify([...new Set(inventory)].sort()) !== JSON.stringify(expected)) fail("inventory");
    const actualFiles = [".github/workflows", "src", "scripts", "docs/architecture", "src-tauri/runtime_packages", "src-tauri/src", "src-tauri/migrations", "src-tauri/tests"].flatMap(d => files(workspace, d));
    if (actualFiles.some(p => !expected.includes(p))) fail("illegal-file");
    const changed = parent.paths.filter(p => live(p) !== parent.read(p));
    const added = inventory.filter(p => !parent.paths.includes(p));
    if (JSON.stringify([...new Set([...changed, ...added])].sort()) !== JSON.stringify(allowed)) fail("unapproved-source");
    if (changed.some(p => p.startsWith("src-tauri/runtime_packages/"))) fail("historical-runtime-bytes");
    for (const p of CI_CONSOLIDATE_FIX_CHANGED) {
      if (proof.files?.[p]?.beforeHash !== sha(parent.read(p)) || proof.files?.[p]?.afterHash !== sha(live(p))) fail("changed:" + p);
    }
    for (const p of CI_CONSOLIDATE_FIX_ADDED) {
      if (parent.paths.includes(p) || proof.files?.[p]?.beforeHash !== null || proof.files?.[p]?.afterHash !== sha(live(p))) fail("added:" + p);
    }
    for (const p of Object.keys(override ?? {}).filter(p => relevant(p) && !allowed.includes(p))) {
      if (!parent.paths.includes(p) || live(p) !== parent.read(p)) fail("undeclared:" + p);
    }
  } catch (error) { fail("validation:" + String(error).slice(0, 180)); }
  return { violations, changedPaths: CI_CONSOLIDATE_FIX_CHANGED,
    addedPaths: [...CI_CONSOLIDATE_FIX_ADDED, CI_CONSOLIDATE_FIX_PROOF],
    afterHashes: Object.fromEntries([...CI_CONSOLIDATE_FIX_CHANGED, ...CI_CONSOLIDATE_FIX_ADDED].map(p => [p, proof?.files?.[p]?.afterHash ?? ""])),
    read: p => CI_CONSOLIDATE_FIX_CHANGED.includes(p) ? parent.read(p) : live(p) };
}
export function ciConsolidateFixParentReader(root, override = undefined) {
  if (override) return validate(root, override);
  return cachedBoundary(root, "ciConsolidateFixParentReader", undefined,
    () => validate(root, undefined), CI_CONSOLIDATE_FIX_PROOF);
}
