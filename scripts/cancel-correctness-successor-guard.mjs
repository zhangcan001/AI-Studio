import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { readFileSync, readdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { cachedBoundary, immutableGit, immutableGitStoreIdentity, immutableBlobText, sessionLiveRead, sessionGit } from "./boundary-validation-cache.mjs";

export const CANCEL_CORRECTNESS_BASE = "53743914c1dd0a2158d729f406c977e84cb2f811";
export const CANCEL_CORRECTNESS_PROOF = "docs/architecture/cancel-correctness-successor.json";
export const CANCEL_CORRECTNESS_CHANGED = [
  "scripts/testing/review-hardening.test.mjs",
  "scripts/testing/single-shot-batch-control.test.mjs",
  "scripts/testing/veda-removed-white-flash.test.mjs",
  "src-tauri/src/application/production_orchestrator_service.rs",
  "src-tauri/src/application/production_queue_service.rs",
  "src-tauri/src/infrastructure/database/repositories/production_queue.rs",
  "src-tauri/tests/dev061b_queue_recovery.rs",
  "src-tauri/tests/support/queue_lifecycle_regressions.rs"
].sort();
export const CANCEL_CORRECTNESS_RUNTIME = [];
export const CANCEL_CORRECTNESS_ADDED = [
  "scripts/cancel-correctness-successor-guard.mjs",
  "scripts/testing/cancel-correctness.test.mjs"
].sort();
export const CANCEL_CORRECTNESS_CONTRACTS = {
  "schemaChanged": false,
  "backupFormatChanged": false,
  "runtimeBytesChanged": false,
  "historicalProofBytesUnchanged": true,
  "productionQueueOnlyExecution": true,
  "gpuExecuted": false,
  "productImplementationChanged": true,
  "migrationAdded": false,
  "runCancelCancelsUnfinishedChildBatches": true,
  "dispatchingCancelWaitsForTaskBind": true,
  "cancelFinishRequiresUnboundTask": true,
  "guardsRelaxed": false
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
    factsCache.set(key, { paths, raw: p => blobs.get(p), read: p => {
      if (!text.has(p)) text.set(p, immutableBlobText(blobIds.get(p), () => blobs.get(p)));
      return text.get(p);
    } });
  }
  return factsCache.get(key);
}
export function cancelCorrectnessScopeSince(root, base) {
  const tip = facts(root, CANCEL_CORRECTNESS_BASE), old = facts(root, base);
  return [...new Set([
    ...tip.paths.filter(p => !old.paths.includes(p) || tip.read(p) !== old.read(p)),
    ...CANCEL_CORRECTNESS_CHANGED, ...CANCEL_CORRECTNESS_ADDED, CANCEL_CORRECTNESS_PROOF,
  ])].sort();
}
export function cancelCorrectnessFixtureFiles(root) {
  return [...facts(root, CANCEL_CORRECTNESS_BASE).paths, ...CANCEL_CORRECTNESS_ADDED, CANCEL_CORRECTNESS_PROOF];
}
function files(root, dir) {
  return readdirSync(join(root, dir), { withFileTypes: true }).flatMap(e => {
    const p = dir + "/" + e.name;
    if (e.isSymbolicLink()) throw Error("symbolic source: " + p);
    return e.isDirectory() ? files(root, p) : [p];
  });
}
function validate(root, override) {
  const workspace = resolve(root), violations = [], parent = facts(workspace, CANCEL_CORRECTNESS_BASE);
  const fail = id => violations.push("cancel-correctness-" + id);
  const live = p => canonical(override && Object.hasOwn(override, p) ? override[p] : sessionLiveRead(workspace, p));
  let proof;
  try {
    proof = JSON.parse(live(CANCEL_CORRECTNESS_PROOF));
    const scope = [...CANCEL_CORRECTNESS_CHANGED, ...CANCEL_CORRECTNESS_ADDED].sort();
    if (proof.schemaVersion !== 1 || proof.checkpoint !== "CANCEL_CORRECTNESS"
        || proof.parentHead !== CANCEL_CORRECTNESS_BASE || proof.planningParentHead !== CANCEL_CORRECTNESS_BASE
        || JSON.stringify(proof.contracts) !== JSON.stringify(CANCEL_CORRECTNESS_CONTRACTS)
        || JSON.stringify(Object.keys(proof.files ?? {}).sort()) !== JSON.stringify(scope)) fail("proof-contract");
    if (git(workspace, ["rev-parse", CANCEL_CORRECTNESS_BASE]).trim() !== CANCEL_CORRECTNESS_BASE) fail("ancestor");
    const allowed = [...scope, CANCEL_CORRECTNESS_PROOF].sort();
    const expected = [...parent.paths, ...CANCEL_CORRECTNESS_ADDED, CANCEL_CORRECTNESS_PROOF].sort();
    const inventory = git(workspace, ["ls-files", "--cached", "--others", "--exclude-standard"])
      .trim().split(/\r?\n/).filter(relevant);
    if (JSON.stringify([...new Set(inventory)].sort()) !== JSON.stringify(expected)) fail("inventory");
    const actualFiles = [".github/workflows", "src", "scripts", "docs/architecture", "src-tauri/runtime_packages", "src-tauri/src", "src-tauri/migrations", "src-tauri/tests"].flatMap(d => files(workspace, d));
    if (actualFiles.some(p => !expected.includes(p))) fail("illegal-file");
    const changed = parent.paths.filter(p => live(p) !== parent.read(p));
    const added = inventory.filter(p => !parent.paths.includes(p));
    if (JSON.stringify([...new Set([...changed, ...added])].sort()) !== JSON.stringify(allowed)) fail("unapproved-source");
    if (changed.some(p => p.startsWith("src-tauri/runtime_packages/"))) fail("historical-runtime-bytes");
    for (const p of CANCEL_CORRECTNESS_CHANGED) {
      if (proof.files?.[p]?.beforeHash !== sha(parent.read(p)) || proof.files?.[p]?.afterHash !== sha(live(p))) fail("changed:" + p);
    }
    for (const p of CANCEL_CORRECTNESS_ADDED) {
      if (parent.paths.includes(p) || proof.files?.[p]?.beforeHash !== null || proof.files?.[p]?.afterHash !== sha(live(p))) fail("added:" + p);
    }
    for (const p of CANCEL_CORRECTNESS_RUNTIME) {
      const bytes = override && Object.hasOwn(override, p) ? Buffer.from(override[p]) : readFileSync(join(workspace, p));
      if (proof.files?.[p]?.afterRawHash !== sha(bytes) || sha(bytes) !== sha(canonical(bytes.toString("utf8")))) fail("runtime-package-bytes:" + p);
    }
    for (const p of Object.keys(override ?? {}).filter(p => relevant(p) && !allowed.includes(p))) {
      if (!parent.paths.includes(p) || live(p) !== parent.read(p)) fail("undeclared:" + p);
    }
  } catch (error) { fail("validation:" + String(error).slice(0, 180)); }
  return { violations, changedPaths: CANCEL_CORRECTNESS_CHANGED,
    addedPaths: [...CANCEL_CORRECTNESS_ADDED, CANCEL_CORRECTNESS_PROOF],
    afterHashes: Object.fromEntries([...CANCEL_CORRECTNESS_CHANGED, ...CANCEL_CORRECTNESS_ADDED].map(p => [p, proof?.files?.[p]?.afterHash ?? ""])),
    read: p => CANCEL_CORRECTNESS_CHANGED.includes(p) ? parent.read(p) : live(p) };
}
export function cancelCorrectnessParentReader(root, override = undefined) {
  if (override) return validate(root, override);
  return cachedBoundary(root, "cancelCorrectnessParentReader", undefined,
    () => validate(root, undefined), CANCEL_CORRECTNESS_PROOF);
}
