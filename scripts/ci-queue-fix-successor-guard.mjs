import { ciPhase1FixParentReader, ciPhase1FixScopeSince, ciPhase1FixFixtureFiles, CI_PHASE1_FIX_BASE } from "./ci-phase1-fix-successor-guard.mjs";
import { createHash } from "node:crypto";
import { readdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { cachedBoundary, immutableGit, immutableGitStoreIdentity, immutableBlobText, sessionLiveRead, sessionGit, sessionMemoRead, immutableBlobBytes } from "./boundary-validation-cache.mjs";

export const CI_QUEUE_FIX_BASE = "5d06ce2681e4376d56e33be1eeb06fc3f1481ff1";
export const CI_QUEUE_FIX_PROOF = "docs/architecture/ci-queue-fix-successor.json";
export const CI_QUEUE_FIX_CHANGED = [
  "scripts/ci-chain-fix-successor-guard.mjs",
  "scripts/testing/ci-chain-fix.test.mjs",
  "src/app/H3GenerationRepairBoundary.test.ts",
  "src/app/QueueLifecycleRepairBoundary.test.ts",
  "src/app/UiPhase5CReviewBoundary.test.ts"
].sort();
export const CI_QUEUE_FIX_ADDED = [
  "scripts/ci-queue-fix-successor-guard.mjs",
  "scripts/testing/ci-queue-fix.test.mjs"
].sort();
export const CI_QUEUE_FIX_CONTRACTS = {
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
      blobs.set(p, immutableBlobBytes(header[1], () => bytes.subarray(start, start + size))); blobIds.set(p, header[1]); offset = start + size + 1;
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
export function ciQueueFixScopeSince(root, base) {
  const tip = facts(root, CI_QUEUE_FIX_BASE), old = facts(root, base);
  return [...new Set([
    ...tip.paths.filter(p => !old.paths.includes(p) || tip.read(p) !== old.read(p)),
    ...CI_QUEUE_FIX_CHANGED, ...CI_QUEUE_FIX_ADDED, CI_QUEUE_FIX_PROOF,
    ...ciPhase1FixScopeSince(root, CI_PHASE1_FIX_BASE),
  ])].sort();
}
export function ciQueueFixFixtureFiles(root) {
  return [...new Set([...facts(root, CI_QUEUE_FIX_BASE).paths, ...CI_QUEUE_FIX_ADDED, CI_QUEUE_FIX_PROOF, ...ciPhase1FixFixtureFiles(root)])];
}
function files(root, dir) {
  return readdirSync(join(root, dir), { withFileTypes: true }).flatMap(e => {
    const p = dir + "/" + e.name;
    if (e.isSymbolicLink()) throw Error("symbolic source: " + p);
    return e.isDirectory() ? files(root, p) : [p];
  });
}
function validate(root, override, successor) {
  const workspace = resolve(root), violations = [], parent = facts(workspace, CI_QUEUE_FIX_BASE);
  const fail = id => violations.push("ci-queue-fix-" + id);
  const live = p => {
    const value = override && Object.hasOwn(override, p) ? canonical(override[p]) : undefined;
    return value !== undefined && !(successor.violations.length === 0 && sha(value) === successor.afterHashes[p]) ? value : canonical(successor.read(p));
  };
  let proof;
  try {
    proof = JSON.parse(live(CI_QUEUE_FIX_PROOF));
    const scope = [...CI_QUEUE_FIX_CHANGED, ...CI_QUEUE_FIX_ADDED].sort();
    if (proof.schemaVersion !== 1 || proof.checkpoint !== "CI_QUEUE_FIX"
        || proof.parentHead !== CI_QUEUE_FIX_BASE || proof.planningParentHead !== CI_QUEUE_FIX_BASE
        || JSON.stringify(proof.contracts) !== JSON.stringify(CI_QUEUE_FIX_CONTRACTS)
        || JSON.stringify(Object.keys(proof.files ?? {}).sort()) !== JSON.stringify(scope)) fail("proof-contract");
    if (git(workspace, ["rev-parse", CI_QUEUE_FIX_BASE]).trim() !== CI_QUEUE_FIX_BASE) fail("ancestor");
    const allowed = [...scope, CI_QUEUE_FIX_PROOF].sort();
    const expected = [...parent.paths, ...CI_QUEUE_FIX_ADDED, CI_QUEUE_FIX_PROOF].sort();
    const inventory = git(workspace, ["ls-files", "--cached", "--others", "--exclude-standard"])
      .trim().split(/\r?\n/).filter(p => relevant(p) && !successor.addedPaths.includes(p));
    if (JSON.stringify([...new Set(inventory)].sort()) !== JSON.stringify(expected)) fail("inventory");
    const actualFiles = [".github/workflows", "src", "scripts", "docs/architecture", "src-tauri/runtime_packages", "src-tauri/src", "src-tauri/migrations", "src-tauri/tests"].flatMap(d => files(workspace, d));
    if (actualFiles.some(p => !expected.includes(p) && !successor.addedPaths.includes(p))) fail("illegal-file");
    const changed = parent.paths.filter(p => live(p) !== parent.read(p));
    const added = inventory.filter(p => !parent.paths.includes(p));
    if (JSON.stringify([...new Set([...changed, ...added])].sort()) !== JSON.stringify(allowed)) fail("unapproved-source");
    if (changed.some(p => p.startsWith("src-tauri/runtime_packages/"))) fail("historical-runtime-bytes");
    for (const p of CI_QUEUE_FIX_CHANGED) {
      if (proof.files?.[p]?.beforeHash !== sha(parent.read(p)) || proof.files?.[p]?.afterHash !== sha(live(p))) fail("changed:" + p);
    }
    for (const p of CI_QUEUE_FIX_ADDED) {
      if (parent.paths.includes(p) || proof.files?.[p]?.beforeHash !== null || proof.files?.[p]?.afterHash !== sha(live(p))) fail("added:" + p);
    }
    for (const p of Object.keys(override ?? {}).filter(p => relevant(p) && !allowed.includes(p))) {
      if (!parent.paths.includes(p) || live(p) !== parent.read(p)) fail("undeclared:" + p);
    }
  } catch (error) { fail("validation:" + String(error).slice(0, 180)); }
  return { violations, changedPaths: [...new Set([...CI_QUEUE_FIX_CHANGED, ...successor.changedPaths])].sort(),
    addedPaths: [...new Set([...CI_QUEUE_FIX_ADDED, CI_QUEUE_FIX_PROOF, ...successor.addedPaths])].sort(),
    afterHashes: { ...Object.fromEntries([...CI_QUEUE_FIX_CHANGED, ...CI_QUEUE_FIX_ADDED].map(p => [p, proof?.files?.[p]?.afterHash ?? ""])), ...successor.afterHashes },
    read: sessionMemoRead(workspace, p => CI_QUEUE_FIX_CHANGED.includes(p) ? parent.read(p) : live(p)) };
}
export function ciQueueFixParentReader(root, override = undefined) {
  const check = () => { const successor = ciPhase1FixParentReader(root); const historical = validate(root, override, successor); return { ...historical, violations: [...successor.violations, ...historical.violations] }; };
  if (override) return check();
  return cachedBoundary(root, "ciQueueFixParentReader", undefined, check, CI_QUEUE_FIX_PROOF);
}
